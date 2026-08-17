use crate::domain::models::SickLeaveRecord;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
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

    fn latest_finalized_end_date(
        conn: &Connection,
        personnel_id: &str,
    ) -> Result<Option<NaiveDate>> {
        let end_date = conn
            .query_row(
                "SELECT MAX(pp.bitis_tarihi)
                 FROM payroll_records AS pr
                 JOIN payroll_periods AS pp ON pp.id = pr.period_id
                 WHERE pr.personnel_id = ?1
                   AND pr.status = 'FINALIZED'",
                params![personnel_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        end_date
            .map(|value| {
                NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|_| {
                    DomainError::InvalidData(format!(
                        "Kesinleştirilmiş bordro dönem bitiş tarihi geçersiz: {}",
                        value
                    ))
                })
            })
            .transpose()
    }

    fn ensure_after_finalized_boundary(
        conn: &Connection,
        personnel_id: &str,
        start_date: &str,
    ) -> Result<()> {
        let Some(boundary) = Self::latest_finalized_end_date(conn, personnel_id)? else {
            return Ok(());
        };
        let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError(format!(
                "Rapor başlangıç tarihi geçersiz: {}.",
                start_date
            ))
        })?;

        if start <= boundary {
            return Err(DomainError::PayrollFinalized(format!(
                "{} tarihine kadar kesinleştirilmiş bordro bulunduğundan kapanmış rapor geçmişi değiştirilemez.",
                boundary.format("%Y-%m-%d")
            )));
        }
        Ok(())
    }

    fn validate_no_overlap(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        let overlap = conn
            .query_row(
                r#"
                SELECT id, start_date, end_date
                FROM sick_leave_records
                WHERE personnel_id = ?1
                  AND id <> ?2
                  AND start_date <= ?4
                  AND end_date >= ?3
                ORDER BY start_date ASC, end_date ASC, id ASC
                LIMIT 1
                "#,
                params![
                    record.personnelId,
                    record.id,
                    record.startDate,
                    record.endDate,
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some((id, start, end)) = overlap {
            return Err(DomainError::ValidationError(format!(
                "Rapor tarihleri çakışıyor: {}–{} aralığı, {} kaydındaki {}–{} aralığıyla örtüşüyor. Örtüşen raporlar ayrı episode olarak kaydedilemez.",
                record.startDate, record.endDate, id, start, end
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

        let existing = Self::get_by_id(conn, &record.id)?;
        if let Some(existing) = existing.as_ref() {
            Self::ensure_after_finalized_boundary(
                conn,
                &existing.personnelId,
                &existing.startDate,
            )?;
        }
        Self::ensure_after_finalized_boundary(conn, &record.personnelId, &record.startDate)?;
        Self::validate_no_overlap(conn, record)?;

        let changed = existing
            .as_ref()
            .map(|old| {
                old.personnelId != record.personnelId
                    || old.startDate != record.startDate
                    || old.endDate != record.endDate
            })
            .unwrap_or(true);
        if changed {
            if let Some(old) = existing.as_ref() {
                PayrollInvalidationRepository::mark_personnel_stale(conn, &old.personnelId)?;
            }
            if existing
                .as_ref()
                .is_none_or(|old| old.personnelId != record.personnelId)
            {
                PayrollInvalidationRepository::mark_personnel_stale(conn, &record.personnelId)?;
            }
        }

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
        if let Some(existing) = Self::get_by_id(conn, id)? {
            Self::ensure_after_finalized_boundary(
                conn,
                &existing.personnelId,
                &existing.startDate,
            )?;
            PayrollInvalidationRepository::mark_personnel_stale(conn, &existing.personnelId)?;
        }

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
            let record =
                Self::from_row(row).map_err(|e| DomainError::DatabaseError(e.to_string()))?;
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
