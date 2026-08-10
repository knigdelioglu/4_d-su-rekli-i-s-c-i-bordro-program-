use crate::domain::models::*;
use crate::domain::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

pub struct AttendanceRepository;

impl AttendanceRepository {
    pub const VALID_CODES: [&'static str; 7] = ["Ç", "T", "G", "İ", "GÇ", "GÇT", "R"];

    fn parse_codes(
        personnel_id: &str,
        period_id: &str,
        attendance_json: &str,
    ) -> Result<HashMap<String, String>> {
        let gunler: HashMap<String, String> =
            serde_json::from_str(attendance_json).map_err(|e| {
                crate::domain::DomainError::InvalidData(format!(
                    "{} personelinin {} dönemi puantaj JSON'u bozuk: {}",
                    personnel_id, period_id, e
                ))
            })?;
        Self::validate_codes(&gunler, period_id)?;
        Ok(gunler)
    }

    fn validate_codes(gunler: &HashMap<String, String>, period_id: &str) -> Result<()> {
        for (date, code) in gunler {
            if !Self::VALID_CODES.contains(&code.as_str()) {
                return Err(crate::domain::DomainError::ValidationError(format!(
                    "{} dönemi {} tarihinde bilinmeyen puantaj kodu: {}",
                    period_id, date, code
                )));
            }
        }
        Ok(())
    }

    pub fn get_all(conn: &Connection) -> Result<Vec<PersonelPuantaj>> {
        let mut stmt = conn
            .prepare("SELECT id, personnel_id, period_id, attendance_json FROM attendance_records")
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            let (id, personnel_id, period_id, attendance_json) =
                r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            let gunler = Self::parse_codes(&personnel_id, &period_id, &attendance_json)?;
            result.push(PersonelPuantaj {
                id,
                personelId: personnel_id,
                donemId: period_id,
                gunler,
            });
        }
        Ok(result)
    }

    pub fn get_by_personnel_and_period(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<Option<PersonelPuantaj>> {
        let row = conn
            .query_row(
                "SELECT id, personnel_id, period_id, attendance_json
                 FROM attendance_records WHERE personnel_id = ?1 AND period_id = ?2",
                params![personnel_id, period_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        row.map(|(id, personnel_id, period_id, attendance_json)| {
            let gunler = Self::parse_codes(&personnel_id, &period_id, &attendance_json)?;
            Ok(PersonelPuantaj {
                id,
                personelId: personnel_id,
                donemId: period_id,
                gunler,
            })
        })
        .transpose()
    }

    pub fn save(conn: &Connection, p: &PersonelPuantaj) -> Result<()> {
        Self::validate_codes(&p.gunler, &p.donemId)?;
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
