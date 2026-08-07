use crate::domain::models::*;
use crate::domain::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use chrono::Utc;

pub struct SettingsRepository;

impl SettingsRepository {
    pub fn get_all_institution_settings(conn: &Connection) -> Result<HashMap<String, DonemselKurumDegerleri>> {
        let mut stmt = conn.prepare(
            "SELECT period_id, settings_json FROM institution_settings",
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            let period_id: String = row.get(0)?;
            let settings_json: String = row.get(1)?;
            let mut val: DonemselKurumDegerleri = serde_json::from_str(&settings_json).unwrap_or_default();
            val.donemId = period_id.clone();
            Ok((period_id, val))
        }).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut map = HashMap::new();
        for r in rows {
            let (k, v) = r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            map.insert(k, v);
        }
        Ok(map)
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
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub fn get_app_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
        let mut stmt = conn.prepare(
            "SELECT value FROM app_settings WHERE key = ?1",
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut rows = stmt.query_map(params![key], |row| {
            row.get::<_, String>(0)
        }).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        if let Some(r) = rows.next() {
            Ok(Some(r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?))
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
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
