use crate::domain::models::*;
use crate::domain::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use chrono::Utc;

pub struct AttendanceRepository;

impl AttendanceRepository {
    pub fn get_all(conn: &Connection) -> Result<Vec<PersonelPuantaj>> {
        let mut stmt = conn.prepare(
            "SELECT id, personnel_id, period_id, attendance_json FROM attendance_records",
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let personnel_id: String = row.get(1)?;
            let period_id: String = row.get(2)?;
            let attendance_json: String = row.get(3)?;
            let gunler: HashMap<String, String> = serde_json::from_str(&attendance_json).unwrap_or_default();

            Ok(PersonelPuantaj {
                id,
                personelId: personnel_id,
                donemId: period_id,
                gunler,
            })
        }).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn get_by_personnel_and_period(conn: &Connection, personnel_id: &str, period_id: &str) -> Result<Option<PersonelPuantaj>> {
        let all = Self::get_all(conn)?;
        Ok(all.into_iter().find(|p| p.personelId == personnel_id && p.donemId == period_id))
    }

    pub fn save(conn: &Connection, p: &PersonelPuantaj) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let json_str = serde_json::to_string(&p.gunler)
            .map_err(|e| crate::domain::DomainError::InvalidData(e.to_string()))?;

        conn.execute(
            "INSERT INTO attendance_records (id, personnel_id, period_id, attendance_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(personnel_id, period_id) DO UPDATE SET
                attendance_json=?4, updated_at=?5",
            params![p.id, p.personelId, p.donemId, json_str, now],
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
