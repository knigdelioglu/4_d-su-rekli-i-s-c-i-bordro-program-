use crate::domain::models::*;
use crate::domain::Result;
use crate::repositories::period_repo::PeriodRepository;
use chrono::{NaiveDate, Utc};
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
        PeriodRepository::validate_period(period)?;
        if k.donemId != period.id {
            return Err(crate::domain::DomainError::ValidationError(format!(
                "Kurum ayarı dönem kimliği eşleşmiyor: {} / {}.",
                k.donemId, period.id
            )));
        }

        let start =
            NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").map_err(|_| {
                crate::domain::DomainError::ValidationError(format!(
                    "Dönem başlangıç tarihi geçersiz: {}.",
                    period.baslangicTarihi
                ))
            })?;
        let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").map_err(|_| {
            crate::domain::DomainError::ValidationError(format!(
                "Dönem bitiş tarihi geçersiz: {}.",
                period.bitisTarihi
            ))
        })?;

        let mut previous_date: Option<NaiveDate> = None;
        for segment in k.statutoryParameterSegments.as_deref().unwrap_or(&[]) {
            let effective =
                NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d").map_err(|_| {
                    crate::domain::DomainError::ValidationError(format!(
                        "Yasal parametre segment tarihi geçersiz: {}.",
                        segment.effectiveFrom
                    ))
                })?;
            if effective < start || effective > end {
                return Err(crate::domain::DomainError::ValidationError(format!(
                    "Yasal parametre segment tarihi {} dönemin dışında ({}–{}).",
                    segment.effectiveFrom, period.baslangicTarihi, period.bitisTarihi
                )));
            }
            if previous_date.is_some_and(|previous| effective <= previous) {
                return Err(crate::domain::DomainError::ValidationError(
                    "Yasal parametre segmentleri strictly artan tarihte ve tekrarsız olmalıdır."
                        .into(),
                ));
            }
            previous_date = Some(effective);

            if segment.gunlukAsgariUcret.is_none()
                && segment.pekTavanKatsayisi.is_none()
                && segment.gunlukYemekIstisnasiSGK.is_none()
                && segment.gunlukYemekIstisnasiGV.is_none()
            {
                return Err(crate::domain::DomainError::ValidationError(format!(
                    "{} tarihli yasal parametre segmentinde en az bir override bulunmalıdır.",
                    segment.effectiveFrom
                )));
            }
            if segment
                .gunlukAsgariUcret
                .is_some_and(|value| value <= rust_decimal::Decimal::ZERO)
            {
                return Err(crate::domain::DomainError::ValidationError(
                    "Segment günlük asgari ücret değeri sıfırdan büyük olmalıdır.".into(),
                ));
            }
            if segment
                .pekTavanKatsayisi
                .is_some_and(|value| value < rust_decimal::Decimal::ONE)
            {
                return Err(crate::domain::DomainError::ValidationError(
                    "Segment PEK tavan katsayısı en az 1 olmalıdır.".into(),
                ));
            }
            if segment
                .gunlukYemekIstisnasiSGK
                .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
                || segment
                    .gunlukYemekIstisnasiGV
                    .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
            {
                return Err(crate::domain::DomainError::ValidationError(
                    "Segment yemek istisnası negatif olamaz.".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {
        let period = PeriodRepository::get_by_id(conn, &k.donemId)?.ok_or_else(|| {
            crate::domain::DomainError::ValidationError(format!(
                "Kurum ayarı için bordro dönemi bulunamadı: {}.",
                k.donemId
            ))
        })?;
        let mut normalized = k.clone();
        if normalized.gunlukYemekIstisnasiGV.is_none() {
            normalized.gunlukYemekIstisnasiGV = normalized.gunlukYemekIstisnasiSGK;
        }
        crate::domain::calculations::validate_kurum_degerleri_for_payroll(&normalized)?;
        Self::validate_statutory_segments_for_period(&period, &normalized)?;

        let now = Utc::now().to_rfc3339();
        let json_str = serde_json::to_string(&normalized)
            .map_err(|e| crate::domain::DomainError::InvalidData(e.to_string()))?;

        conn.execute(
            "INSERT INTO institution_settings (period_id, settings_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(period_id) DO UPDATE SET
                settings_json=?2, updated_at=?3",
            params![k.donemId, json_str, now],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

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
