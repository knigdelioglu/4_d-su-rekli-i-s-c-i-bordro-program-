use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::transaction::with_transaction;
use chrono::Utc;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::HashMap;

pub struct SettingsRepository;

impl SettingsRepository {
    fn decode_active_settings(
        period_id: &str,
        settings_json: &str,
    ) -> Result<payroll_core::PayrollPeriodInstitutionSettings> {
        let mut value: payroll_core::PayrollPeriodInstitutionSettings =
            serde_json::from_str(settings_json).map_err(|e| {
                DomainError::InvalidData(format!(
                    "{} dönemi aktif kurum ayarları bozuk JSON içeriyor: {}",
                    period_id, e
                ))
            })?;
        value.donemId = period_id.to_string();
        if value.gunlukYemekIstisnasiGV.is_none() {
            value.gunlukYemekIstisnasiGV = value.gunlukYemekIstisnasiSGK;
        }
        Ok(value)
    }

    fn decode_settings(period_id: &str, settings_json: &str) -> Result<DonemselKurumDegerleri> {
        let mut value: DonemselKurumDegerleri =
            serde_json::from_str(settings_json).map_err(|e| {
                crate::domain::DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bozuk JSON içeriyor: {}",
                    period_id, e
                ))
            })?;
        value.donemId = period_id.to_string();
        // Legacy settings stored a single meal exemption value. Preserve old data by
        // copying it once into the new, independent GV field; new saves persist both.
        if value.gunlukYemekIstisnasiGV.is_none() {
            value.gunlukYemekIstisnasiGV = value.gunlukYemekIstisnasiSGK;
        }
        Ok(value)
    }

    pub fn get_all_active_institution_settings(
        conn: &Connection,
    ) -> Result<HashMap<String, payroll_core::PayrollPeriodInstitutionSettings>> {
        let mut stmt = conn
            .prepare("SELECT period_id, settings_json FROM institution_settings")
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        rows.map(|row| {
            let (period_id, settings_json) =
                row.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            Ok((
                period_id.clone(),
                Self::decode_active_settings(&period_id, &settings_json)?,
            ))
        })
        .collect()
    }

    pub fn get_all_institution_settings(
        conn: &Connection,
    ) -> Result<HashMap<String, DonemselKurumDegerleri>> {
        let mut stmt = conn
            .prepare("SELECT period_id, settings_json FROM institution_settings")
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut map = HashMap::new();
        for r in rows {
            let (period_id, settings_json) =
                r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            let value = Self::decode_settings(&period_id, &settings_json)?;
            map.insert(period_id, value);
        }
        Ok(map)
    }

    pub fn get_institution_settings(
        conn: &Connection,
        period_id: &str,
    ) -> Result<Option<DonemselKurumDegerleri>> {
        let row = conn
            .query_row(
                "SELECT settings_json FROM institution_settings WHERE period_id = ?1",
                params![period_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        row.map(|settings_json| Self::decode_settings(period_id, &settings_json))
            .transpose()
    }

    /// Returns the stored JSON without deserializing through legacy defaults.
    /// PREFILL_ONLY adapters use this so missing historical money stays missing.
    pub fn get_raw_institution_settings(
        conn: &Connection,
        period_id: &str,
    ) -> Result<Option<serde_json::Value>> {
        conn.query_row(
            "SELECT settings_json FROM institution_settings WHERE period_id = ?1",
            params![period_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| {
                DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bozuk JSON içeriyor: {}",
                    period_id, error
                ))
            })
        })
        .transpose()
    }

    /// Retains exact imported legacy compensation values beside active inputs.
    /// Missing keys stay missing and active settings remain the calculation source.
    pub fn merge_legacy_compensation_settings_in_transaction(
        conn: &Connection,
        period_id: &str,
        legacy_settings: &serde_json::Value,
    ) -> Result<()> {
        const LEGACY_COMPENSATION_FIELDS: &[&str] = &[
            "gunlukTabanUcret",
            "gunlukYemek",
            "birlestirilmisSosyalYardim",
            "gunlukVasitaYol",
            "transportRateChanges",
            "giyimYardimi",
            "hizmetZammiBirimi",
            "isPrimiYuzde",
            "isPrimiGruplari",
            "geceCalismaPrimiYuzde",
            "geceCalismaTatiliPrimiYuzde",
            "ekOdeme",
            "digerGelirVarsayilan",
        ];
        let Some(legacy_object) = legacy_settings.as_object() else {
            return Ok(());
        };
        let current = Self::get_raw_institution_settings(conn, period_id)?;
        let mut merged = current.unwrap_or_else(|| serde_json::json!({"donemId": period_id}));
        let before = merged.clone();
        let Some(current_object) = merged.as_object_mut() else {
            return Err(DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları JSON nesnesi olmalıdır.",
                period_id
            )));
        };
        for field in LEGACY_COMPENSATION_FIELDS {
            if let Some(value) = legacy_object.get(*field) {
                current_object.insert((*field).to_owned(), value.clone());
            }
        }
        if merged == before {
            return Ok(());
        }
        let json = serde_json::to_string(&merged)
            .map_err(|error| DomainError::InvalidData(error.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO institution_settings (period_id, settings_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(period_id) DO UPDATE SET settings_json=?2, updated_at=?3",
            params![period_id, json, now],
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        Ok(())
    }

    pub fn get_for_periods(
        conn: &Connection,
        period_ids: &[String],
    ) -> Result<HashMap<String, DonemselKurumDegerleri>> {
        if period_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = (1..=period_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT period_id, settings_json FROM institution_settings WHERE period_id IN ({placeholders})"
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map(params_from_iter(period_ids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = HashMap::new();
        for row in rows {
            let (period_id, settings_json) =
                row.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            result.insert(
                period_id.clone(),
                Self::decode_settings(&period_id, &settings_json)?,
            );
        }
        Ok(result)
    }

    pub fn validate_statutory_segments_for_period(
        period: &BordroDonemi,
        k: &DonemselKurumDegerleri,
    ) -> Result<()> {
        let active_settings = payroll_core::PayrollPeriodInstitutionSettings::from(k);
        payroll_core::validate_statutory_segments_for_period(period, &active_settings)
    }

    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {
        with_transaction(conn, |tx| {
            Self::save_institution_settings_in_transaction(tx, k)
        })
    }

    /// Saves active period inputs while retaining every legacy money field in
    /// the existing raw JSON object. New periods persist only active inputs.
    pub fn save_active_institution_settings(
        conn: &Connection,
        settings: &payroll_core::PayrollPeriodInstitutionSettings,
    ) -> Result<()> {
        with_transaction(conn, |tx| {
            Self::save_active_institution_settings_in_transaction(tx, settings)
        })
    }

    pub fn save_active_institution_settings_in_transaction(
        conn: &Connection,
        settings: &payroll_core::PayrollPeriodInstitutionSettings,
    ) -> Result<()> {
        let period = PeriodRepository::get_by_id(conn, &settings.donemId)?.ok_or_else(|| {
            DomainError::ValidationError(format!(
                "Kurum ayarı için bordro dönemi bulunamadı: {}.",
                settings.donemId
            ))
        })?;
        let existing_json = conn
            .query_row(
                "SELECT settings_json FROM institution_settings WHERE period_id = ?1",
                params![settings.donemId],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut normalized = settings.clone();
        if normalized.gunlukYemekIstisnasiGV.is_none() {
            normalized.gunlukYemekIstisnasiGV = normalized.gunlukYemekIstisnasiSGK;
        }
        payroll_core::calculations::validate_period_institution_settings_for_payroll(&normalized)?;
        payroll_core::validate_statutory_segments_for_period(&period, &normalized)?;

        if let Some(existing_json) = existing_json.as_deref() {
            let existing = Self::decode_active_settings(&settings.donemId, existing_json)?;
            let statutory_changed = match (
                payroll_core::payroll_engine::current_statutory_parameter_definition(&existing),
                payroll_core::payroll_engine::current_statutory_parameter_definition(&normalized),
            ) {
                (Ok(old_definition), Ok(new_definition)) => {
                    old_definition != new_definition
                        || existing
                            .statutoryParameterSnapshot
                            .as_ref()
                            .is_some_and(|snapshot| snapshot != &old_definition)
                }
                (Err(_), Ok(new_definition)) => existing
                    .statutoryParameterSnapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot != &new_definition),
                (Ok(_), Err(_)) | (Err(_), Err(_)) => true,
            };
            normalized.statutoryParameterSnapshot = if statutory_changed {
                None
            } else {
                existing
                    .statutoryParameterSnapshot
                    .or(normalized.statutoryParameterSnapshot)
            };
        }

        let mut merged = if let Some(existing_json) = existing_json.as_deref() {
            serde_json::from_str::<serde_json::Value>(existing_json)
                .map_err(|e| DomainError::InvalidData(e.to_string()))?
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        let existing_value = merged.clone();
        let object = merged.as_object_mut().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları JSON nesnesi olmalıdır.",
                settings.donemId
            ))
        })?;
        let active_value = serde_json::to_value(&normalized)
            .map_err(|e| DomainError::InvalidData(e.to_string()))?;
        let active_fields = active_value.as_object().ok_or_else(|| {
            DomainError::InvalidData("Aktif kurum ayarları nesneye çevrilemedi.".into())
        })?;
        for (key, value) in active_fields {
            object.insert(key.clone(), value.clone());
        }
        if merged == existing_value {
            return Ok(());
        }

        let impact = PayrollInvalidationRepository::assert_mutation_allowed(
            conn,
            &payroll_core::PayrollMutation::Period {
                periodId: settings.donemId.clone(),
            },
        )?;
        let json_str =
            serde_json::to_string(&merged).map_err(|e| DomainError::InvalidData(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO institution_settings (period_id, settings_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(period_id) DO UPDATE SET
                settings_json=?2, updated_at=?3",
            params![settings.donemId, json_str, now],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        PayrollInvalidationRepository::apply_impact(conn, &impact)?;
        Ok(())
    }

    /// Caller-owned transaction variant used by period save and backup restore.
    pub fn save_institution_settings_in_transaction(
        conn: &Connection,
        k: &DonemselKurumDegerleri,
    ) -> Result<()> {
        let period = PeriodRepository::get_by_id(conn, &k.donemId)?.ok_or_else(|| {
            crate::domain::DomainError::ValidationError(format!(
                "Kurum ayarı için bordro dönemi bulunamadı: {}.",
                k.donemId
            ))
        })?;
        let existing_json = conn
            .query_row(
                "SELECT settings_json FROM institution_settings WHERE period_id = ?1",
                params![k.donemId],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut normalized = k.clone();
        if normalized.gunlukYemekIstisnasiGV.is_none() {
            normalized.gunlukYemekIstisnasiGV = normalized.gunlukYemekIstisnasiSGK;
        }
        crate::domain::calculations::validate_kurum_degerleri_for_payroll(&normalized)?;
        Self::validate_statutory_segments_for_period(&period, &normalized)?;

        // A period snapshot is immutable only while it protects FINALIZED
        // history. CALCULATED/STALE records are invalidated by the same period
        // mutation above, so a statutory correction must clear the old
        // snapshot and let the next authoritative calculation capture the new
        // definition. Non-statutory edits keep the existing legal snapshot.
        if let Some(existing_json) = existing_json.as_deref() {
            let existing = Self::decode_settings(&k.donemId, existing_json)?;
            let statutory_changed = match (
                payroll_core::payroll_engine::current_statutory_parameter_definition(
                    &payroll_core::PayrollPeriodInstitutionSettings::from(&existing),
                ),
                payroll_core::payroll_engine::current_statutory_parameter_definition(
                    &payroll_core::PayrollPeriodInstitutionSettings::from(&normalized),
                ),
            ) {
                (Ok(old_definition), Ok(new_definition)) => {
                    old_definition != new_definition
                        || existing
                            .statutoryParameterSnapshot
                            .as_ref()
                            .is_some_and(|snapshot| snapshot != &old_definition)
                }
                (Err(_), Ok(new_definition)) => existing
                    .statutoryParameterSnapshot
                    .as_ref()
                    .is_some_and(|old_definition| old_definition != &new_definition),
                (Ok(_), Err(_)) | (Err(_), Err(_)) => true,
            };
            normalized.statutoryParameterSnapshot = if statutory_changed {
                None
            } else {
                existing
                    .statutoryParameterSnapshot
                    .or(normalized.statutoryParameterSnapshot)
            };
        }

        let json_str = serde_json::to_string(&normalized)
            .map_err(|e| crate::domain::DomainError::InvalidData(e.to_string()))?;
        let changed = existing_json.as_deref() != Some(json_str.as_str());
        if !changed {
            // A byte-identical save is not a payroll mutation. Asserting the
            // PERIOD policy here would reject harmless re-saves whenever any
            // later history is FINALIZED, and must not invalidate anything.
            return Ok(());
        }
        let legacy_transport_only_change = if let Some(existing_json) = existing_json.as_deref() {
            let mut existing = Self::decode_settings(&k.donemId, existing_json)?;
            existing.transportRateChanges = None;
            let mut updated = normalized.clone();
            updated.transportRateChanges = None;
            let existing_without_legacy = serde_json::to_string(&existing)
                .map_err(|error| DomainError::InvalidData(error.to_string()))?;
            let updated_without_legacy = serde_json::to_string(&updated)
                .map_err(|error| DomainError::InvalidData(error.to_string()))?;
            existing_without_legacy == updated_without_legacy
        } else {
            false
        };
        // Dated transport values are retained only for legacy file roundtrips;
        // the production calculator reads road amounts from immutable policy
        // versions, so changing this obsolete field cannot invalidate payroll.
        let impact = if legacy_transport_only_change {
            None
        } else {
            Some(PayrollInvalidationRepository::assert_mutation_allowed(
                conn,
                &payroll_core::PayrollMutation::Period {
                    periodId: k.donemId.clone(),
                },
            )?)
        };
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO institution_settings (period_id, settings_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(period_id) DO UPDATE SET
                settings_json=?2, updated_at=?3",
            params![k.donemId, json_str, now],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        if let Some(impact) = impact {
            PayrollInvalidationRepository::apply_impact(conn, &impact)?;
        }

        Ok(())
    }

    /// Freezes the period-level legal inputs on the first authoritative
    /// payroll calculation. This is deliberately not a normal settings save:
    /// it does not invalidate payrolls and it never replaces an existing
    /// snapshot.
    pub fn persist_statutory_snapshot_if_missing(conn: &Connection, period_id: &str) -> Result<()> {
        let current_json = conn
            .query_row(
                "SELECT settings_json FROM institution_settings WHERE period_id = ?1",
                params![period_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bulunamadı; statutory snapshot oluşturulamaz.",
                    period_id
                ))
            })?;
        let mut settings = Self::decode_active_settings(period_id, &current_json)?;
        if settings.statutoryParameterSnapshot.is_some() {
            return Ok(());
        }

        settings.statutoryParameterSnapshot = Some(
            payroll_core::payroll_engine::snapshot_statutory_parameters(&settings)?,
        );
        let mut raw = serde_json::from_str::<serde_json::Value>(&current_json)
            .map_err(|e| DomainError::InvalidData(e.to_string()))?;
        let raw_object = raw.as_object_mut().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları JSON nesnesi olmalıdır.",
                period_id
            ))
        })?;
        raw_object.insert(
            "statutoryParameterSnapshot".into(),
            serde_json::to_value(settings.statutoryParameterSnapshot)
                .map_err(|e| DomainError::InvalidData(e.to_string()))?,
        );
        let json_str =
            serde_json::to_string(&raw).map_err(|e| DomainError::InvalidData(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE institution_settings
             SET settings_json = ?1, updated_at = ?2
             WHERE period_id = ?3 AND settings_json = ?4",
            params![json_str, now, period_id, current_json],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn get_app_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
        let mut stmt = conn
            .prepare("SELECT value FROM app_settings WHERE key = ?1")
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut rows = stmt
            .query_map(params![key], |row| row.get::<_, String>(0))
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        if let Some(r) = rows.next() {
            Ok(Some(r.map_err(|e| {
                crate::domain::DomainError::DatabaseError(e.to_string())
            })?))
        } else {
            Ok(None)
        }
    }

    pub fn set_app_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
        with_transaction(conn, |tx| {
            Self::set_app_setting_in_transaction(tx, key, value)
        })
    }

    /// Caller-owned transaction variant used by backup restore.
    pub fn set_app_setting_in_transaction(conn: &Connection, key: &str, value: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value=?2, updated_at=?3",
            params![key, value, now],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
