use crate::domain::models::*;
use crate::domain::Result;
use chrono::Utc;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::HashMap;

pub struct SettingsRepository;

impl SettingsRepository {
    fn decode_settings(period_id: &str, settings_json: &str) -> Result<DonemselKurumDegerleri> {
        let mut value: DonemselKurumDegerleri =
            serde_json::from_str(settings_json).map_err(|e| {
                crate::domain::DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bozuk JSON içeriyor: {}",
                    period_id, e
                ))
            })?;
        value.donemId = period_id.to_string();
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

    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let json_str = serde_json::to_string(k)
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
