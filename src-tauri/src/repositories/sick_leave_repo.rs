use crate::domain::models::SickLeaveRecord;
use crate::domain::{DomainError, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension, Row};

pub struct SickLeaveRepository;

impl SickLeaveRepository {
    pub fn validate_record(record: &SickLeaveRecord) -> Result<()> {
        let start = NaiveDate::parse_from_str(&record.startDate, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError(format!(
                "Rapor başlangıç tarihi geçersiz: {}.",
                record.startDate
            ))
        })?;
        let end = NaiveDate::parse_from_str(&record.endDate, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError(format!(
                "Rapor bitiş tarihi geçersiz: {}.",
                record.endDate
            ))
        })?;
        if start > end {
            return Err(DomainError::ValidationError(
                "Rapor başlangıç tarihi bitiş tarihinden sonra olamaz.".into(),
            ));
        }
        if record.personnelId.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "Rapor kaydında personel zorunludur.".into(),
            ));
        }
        Ok(())
    }

    fn validate_no_overlap(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        let overlapping_id = conn
            .query_row(
                "SELECT id
                 FROM sick_leave_records
                 WHERE personnel_id = ?1
                   AND id <> ?2
                   AND start_date <= ?3
                   AND end_date >= ?4
                 LIMIT 1",
                params![
                    record.personnelId,
                    record.id,
                    record.endDate,
                    record.startDate
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some(existing_id) = overlapping_id {
            return Err(DomainError::ValidationError(format!(
                "Rapor tarihleri aynı personele ait {} kaydıyla çakışıyor; aynı günler ikinci bir rapor olayı olarak sayılamaz.",
                existing_id
            )));
        }
        Ok(())
    }

    fn from_row(row: &Row) -> rusqlite::Result<SickLeaveRecord> {
        Ok(SickLeaveRecord {
            id: row.get(0)?,
            personnelId: row.get(1)?,
            startDate: row.get(2)?,
            endDate: row.get(3)?,
            createdAt: row.get(4)?,
            updatedAt: row.get(5)?,
        })
    }

    pub fn save(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        Self::validate_record(record)?;
        Self::validate_no_overlap(conn, record)?;
        let now = chrono::Utc::now().to_rfc3339();
        let created_at = record.createdAt.as_ref().unwrap_or(&now);
        let updated_at = &now;

        conn.execute(
            r#"
            INSERT INTO sick_leave_records (
                id, personnel_id, start_date, end_date, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                personnel_id = excluded.personnel_id,
                start_date = excluded.start_date,
                end_date = excluded.end_date,
                updated_at = excluded.updated_at
            "#,
            params![
                record.id,
                record.personnelId,
                record.startDate,
                record.endDate,
                created_at,
                updated_at,
            ],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        conn.execute("DELETE FROM sick_leave_records WHERE id = ?1", params![id])
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<SickLeaveRecord>> {
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, personnel_id, start_date, end_date, created_at, updated_at
                FROM sick_leave_records
                WHERE id = ?1
                "#,
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut rows = stmt
            .query(params![id])
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
        {
            let record = Self::from_row(row).map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            Self::validate_record(&record)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    pub fn get_by_personnel(conn: &Connection, personnel_id: &str) -> Result<Vec<SickLeaveRecord>> {
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, personnel_id, start_date, end_date, created_at, updated_at
                FROM sick_leave_records
                WHERE personnel_id = ?1
                ORDER BY start_date ASC
                "#,
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(params![personnel_id], Self::from_row)
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            let record = r.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            Self::validate_record(&record)?;
            result.push(record);
        }
        Ok(result)
    }

    pub fn get_all(conn: &Connection) -> Result<Vec<SickLeaveRecord>> {
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, personnel_id, start_date, end_date, created_at, updated_at
                FROM sick_leave_records
                ORDER BY start_date ASC
                "#,
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], Self::from_row)
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            let record = r.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            Self::validate_record(&record)?;
            result.push(record);
        }
        Ok(result)
    }
}
