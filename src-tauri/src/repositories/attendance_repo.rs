use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::period_repo::PeriodRepository;
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

pub struct AttendanceRepository;

impl AttendanceRepository {
    pub const VALID_CODES: [&'static str; 7] = ["Ç", "T", "G", "İ", "GÇ", "GÇT", "R"];

    fn parse_codes(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        attendance_json: &str,
    ) -> Result<HashMap<String, String>> {
        let gunler: HashMap<String, String> =
            serde_json::from_str(attendance_json).map_err(|e| {
                DomainError::InvalidData(format!(
                    "{} personelinin {} dönemi puantaj JSON'u bozuk: {}",
                    personnel_id, period_id, e
                ))
            })?;
        Self::validate_codes(&gunler, period_id)?;
        Self::validate_dates(conn, &gunler, period_id)?;
        Ok(gunler)
    }

    fn validate_codes(gunler: &HashMap<String, String>, period_id: &str) -> Result<()> {
        for (date, code) in gunler {
            if !Self::VALID_CODES.contains(&code.as_str()) {
                return Err(DomainError::ValidationError(format!(
                    "{} dönemi {} tarihinde bilinmeyen puantaj kodu: {}",
                    period_id, date, code
                )));
            }
        }
        Ok(())
    }

    fn validate_dates(
        conn: &Connection,
        gunler: &HashMap<String, String>,
        period_id: &str,
    ) -> Result<()> {
        let period = PeriodRepository::get_by_id(conn, period_id)?.ok_or_else(|| {
            DomainError::NotFound(format!(
                "Puantajın bağlı olduğu dönem bulunamadı: {}",
                period_id
            ))
        })?;
        let start =
            NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").map_err(|e| {
                DomainError::InvalidData(format!(
                    "{} dönemi başlangıç tarihi bozuk: {}",
                    period_id, e
                ))
            })?;
        let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").map_err(|e| {
            DomainError::InvalidData(format!("{} dönemi bitiş tarihi bozuk: {}", period_id, e))
        })?;

        for date_text in gunler.keys() {
            let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|e| {
                DomainError::ValidationError(format!(
                    "{} dönemi puantaj tarihi geçersiz: {} ({})",
                    period_id, date_text, e
                ))
            })?;
            if date < start || date > end {
                return Err(DomainError::ValidationError(format!(
                    "{} puantaj tarihi {} dönemi aralığı dışında: {} - {}.",
                    date_text, period_id, period.baslangicTarihi, period.bitisTarihi
                )));
            }
        }
        Ok(())
    }

    pub fn get_all(conn: &Connection) -> Result<Vec<PersonelPuantaj>> {
        let mut stmt = conn
            .prepare("SELECT id, personnel_id, period_id, attendance_json FROM attendance_records")
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut raw_records = Vec::new();
        for row in rows {
            raw_records.push(row.map_err(|e| DomainError::DatabaseError(e.to_string()))?);
        }
        drop(stmt);

        let mut result = Vec::with_capacity(raw_records.len());
        for (id, personnel_id, period_id, attendance_json) in raw_records {
            let gunler = Self::parse_codes(conn, &personnel_id, &period_id, &attendance_json)?;
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
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        row.map(|(id, personnel_id, period_id, attendance_json)| {
            let gunler = Self::parse_codes(conn, &personnel_id, &period_id, &attendance_json)?;
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
        Self::validate_dates(conn, &p.gunler, &p.donemId)?;
        let now = Utc::now().to_rfc3339();
        let json_str = serde_json::to_string(&p.gunler)
            .map_err(|e| DomainError::InvalidData(e.to_string()))?;

        conn.execute(
            "INSERT INTO attendance_records (id, personnel_id, period_id, attendance_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(personnel_id, period_id) DO UPDATE SET
                attendance_json=?4, updated_at=?5",
            params![p.id, p.personelId, p.donemId, json_str, now],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
