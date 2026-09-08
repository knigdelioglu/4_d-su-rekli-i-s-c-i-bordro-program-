use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::transaction::with_transaction;
use chrono::Utc;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::{BTreeSet, HashMap};

pub const ZAM_AYLARI_SETTING_KEY: &str = "zam_aylari";

pub struct SettingsRepository;

impl SettingsRepository {
    pub fn normalize_zam_aylari(months: &[i32]) -> Result<Vec<i32>> {
        let mut unique = BTreeSet::new();
        for month in months {
            if !(1..=12).contains(month) {
                return Err(crate::domain::DomainError::ValidationError(
                    "Zam ayları 1-12 arasında olmalıdır.".into(),
                ));
            }
            unique.insert(*month);
        }
        Ok(unique.into_iter().collect())
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
        payroll_core::validate_statutory_segments_for_period(period, k)
    }

    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {
        with_transaction(conn, |tx| {
            Self::save_institution_settings_in_transaction(tx, k)
        })
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
        let impact = PayrollInvalidationRepository::assert_mutation_allowed(
            conn,
            &payroll_core::PayrollMutation::Period {
                periodId: k.donemId.clone(),
            },
        )?;

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
        // A statutory snapshot is period history, not a mutable copy of the
        // current form values. Once captured, later settings edits may change
        // ordinary payroll inputs but must never rewrite the legal reference
        // used by historical asgari-GV calculations.
        if let Some(existing_json) = existing_json.as_deref() {
            let existing = Self::decode_settings(&k.donemId, existing_json)?;
            if existing.statutoryParameterSnapshot.is_some() {
                normalized.statutoryParameterSnapshot = existing.statutoryParameterSnapshot;
            }
        }
        crate::domain::calculations::validate_kurum_degerleri_for_payroll(&normalized)?;
        Self::validate_statutory_segments_for_period(&period, &normalized)?;

        let json_str = serde_json::to_string(&normalized)
            .map_err(|e| crate::domain::DomainError::InvalidData(e.to_string()))?;
        let changed = existing_json.as_deref() != Some(json_str.as_str());
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO institution_settings (period_id, settings_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(period_id) DO UPDATE SET
                settings_json=?2, updated_at=?3",
            params![k.donemId, json_str, now],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        if changed {
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
        let mut settings = Self::decode_settings(period_id, &current_json)?;
        if settings.statutoryParameterSnapshot.is_some() {
            return Ok(());
        }

        settings.statutoryParameterSnapshot = Some(
            payroll_core::payroll_engine::snapshot_statutory_parameters(&settings)?,
        );
        let json_str = serde_json::to_string(&settings)
            .map_err(|e| DomainError::InvalidData(e.to_string()))?;
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
        let previous = Self::get_app_setting(conn, key)?;
        let impact = if key == ZAM_AYLARI_SETTING_KEY && previous.as_deref() != Some(value) {
            Some(PayrollInvalidationRepository::assert_mutation_allowed(
                conn,
                &payroll_core::PayrollMutation::All,
            )?)
        } else {
            None
        };

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value=?2, updated_at=?3",
            params![key, value, now],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        if let Some(impact) = impact {
            PayrollInvalidationRepository::apply_impact(conn, &impact)?;
        }

        Ok(())
    }
}
