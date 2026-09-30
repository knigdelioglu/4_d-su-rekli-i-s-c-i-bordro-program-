use crate::domain::models::SickLeaveRecord;
use crate::domain::{DomainError, Result};
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::transaction::with_transaction;
use chrono::{Datelike, NaiveDate};
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
        with_transaction(conn, |tx| Self::save_in_transaction(tx, record))
    }

    /// Caller-owned transaction variant used by backup restore and composite
    /// use cases.
    pub fn save_in_transaction(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        Self::validate_record(record)?;

        let existing = Self::get_by_id(conn, &record.id)?;
        let mut impacts = Vec::new();
        if let Some(existing) = existing.as_ref() {
            impacts.push(PayrollInvalidationRepository::assert_mutation_allowed(
                conn,
                &payroll_core::PayrollMutation::PersonFromDate {
                    personnelId: existing.personnelId.clone(),
                    effectiveFrom: existing.startDate.clone(),
                },
            )?);
        }
        if existing.as_ref().is_none_or(|old| {
            old.personnelId != record.personnelId || old.startDate != record.startDate
        }) {
            impacts.push(PayrollInvalidationRepository::assert_mutation_allowed(
                conn,
                &payroll_core::PayrollMutation::PersonFromDate {
                    personnelId: record.personnelId.clone(),
                    effectiveFrom: record.startDate.clone(),
                },
            )?);
        }
        Self::validate_no_overlap(conn, record)?;

        let changed = existing
            .as_ref()
            .map(|old| {
                old.personnelId != record.personnelId
                    || old.startDate != record.startDate
                    || old.endDate != record.endDate
            })
            .unwrap_or(true);
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

        if changed {
            for impact in impacts {
                PayrollInvalidationRepository::apply_impact(conn, &impact)?;
            }
        }

        Ok(())
    }

    pub fn save_and_sync_attendance(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        with_transaction(conn, |tx| {
            let existing = Self::get_by_id(tx, &record.id)?;
            Self::save_in_transaction(tx, record)?;
            Self::sync_attendance_on_save(tx, record, existing.as_ref())
        })
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        with_transaction(conn, |tx| Self::delete_in_transaction(tx, id))
    }

    /// Caller-owned transaction variant used by composite use cases.
    pub fn delete_in_transaction(conn: &Connection, id: &str) -> Result<()> {
        if let Some(existing) = Self::get_by_id(conn, id)? {
            let impact = PayrollInvalidationRepository::assert_mutation_allowed(
                conn,
                &payroll_core::PayrollMutation::PersonFromDate {
                    personnelId: existing.personnelId,
                    effectiveFrom: existing.startDate,
                },
            )?;

            conn.execute("DELETE FROM sick_leave_records WHERE id = ?1", params![id])
                .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            PayrollInvalidationRepository::apply_impact(conn, &impact)?;
            return Ok(());
        }

        conn.execute("DELETE FROM sick_leave_records WHERE id = ?1", params![id])
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn delete_and_sync_attendance(conn: &Connection, id: &str) -> Result<()> {
        with_transaction(conn, |tx| {
            if let Some(existing) = Self::get_by_id(tx, id)? {
                Self::sync_attendance_on_delete(tx, &existing)?;
            }
            Self::delete_in_transaction(tx, id)
        })
    }

    fn sync_attendance_on_save(
        conn: &Connection,
        record: &SickLeaveRecord,
        existing: Option<&SickLeaveRecord>,
    ) -> Result<()> {
        let all_periods = PeriodRepository::get_all(conn)?;
        if all_periods.is_empty() {
            return Ok(());
        }

        let record_start = NaiveDate::parse_from_str(&record.startDate, "%Y-%m-%d").map_err(|e| {
            DomainError::ValidationError(format!("Rapor başlangıç tarihi geçersiz: {e}"))
        })?;
        let record_end = NaiveDate::parse_from_str(&record.endDate, "%Y-%m-%d").map_err(|e| {
            DomainError::ValidationError(format!("Rapor bitiş tarihi geçersiz: {e}"))
        })?;

        for period in &all_periods {
            let p_start = match NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };
            let p_end = match NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };

            let overlaps_new = record_start <= p_end && record_end >= p_start;
            let overlaps_old = if let Some(old) = existing {
                if let (Ok(old_s), Ok(old_e)) = (
                    NaiveDate::parse_from_str(&old.startDate, "%Y-%m-%d"),
                    NaiveDate::parse_from_str(&old.endDate, "%Y-%m-%d"),
                ) {
                    old_s <= p_end && old_e >= p_start
                } else {
                    false
                }
            } else {
                false
            };

            if !overlaps_new && !overlaps_old {
                continue;
            }

            let existing_att = AttendanceRepository::get_by_personnel_and_period(
                conn,
                &record.personnelId,
                &period.id,
            )?;

            let mut gunler = if let Some(att) = existing_att.as_ref() {
                att.gunler.clone()
            } else {
                let mut default_gunler = std::collections::HashMap::new();
                let mut curr = p_start;
                while curr <= p_end {
                    let code = if curr.weekday() == chrono::Weekday::Sat
                        || curr.weekday() == chrono::Weekday::Sun
                    {
                        "T"
                    } else {
                        "Ç"
                    };
                    default_gunler.insert(curr.format("%Y-%m-%d").to_string(), code.to_string());
                    curr += chrono::Duration::days(1);
                }
                default_gunler
            };

            let mut changed = false;

            // If editing: restore old dates not in new record, ONLY if still "R"
            if let Some(old) = existing {
                if let (Ok(old_s), Ok(old_e)) = (
                    NaiveDate::parse_from_str(&old.startDate, "%Y-%m-%d"),
                    NaiveDate::parse_from_str(&old.endDate, "%Y-%m-%d"),
                ) {
                    let mut curr = old_s;
                    while curr <= old_e {
                        if curr >= p_start && curr <= p_end {
                            if curr < record_start || curr > record_end {
                                let date_str = curr.format("%Y-%m-%d").to_string();
                                if gunler.get(&date_str).map(|s| s.as_str()) == Some("R") {
                                    let default_code = if curr.weekday() == chrono::Weekday::Sat
                                        || curr.weekday() == chrono::Weekday::Sun
                                    {
                                        "T"
                                    } else {
                                        "Ç"
                                    };
                                    gunler.insert(date_str, default_code.to_string());
                                    changed = true;
                                }
                            }
                        }
                        curr += chrono::Duration::days(1);
                    }
                }
            }

            // Apply new record dates:
            if overlaps_new {
                let mut curr = record_start.max(p_start);
                let end_overlap = record_end.min(p_end);
                while curr <= end_overlap {
                    let date_str = curr.format("%Y-%m-%d").to_string();
                    if gunler.get(&date_str).map(|s| s.as_str()) != Some("R") {
                        gunler.insert(date_str, "R".to_string());
                        changed = true;
                    }
                    curr += chrono::Duration::days(1);
                }
            }

            if changed || existing_att.is_none() {
                let attendance = crate::domain::models::PersonelPuantaj {
                    id: existing_att
                        .map(|a| a.id)
                        .unwrap_or_else(|| format!("{}_{}", record.personnelId, period.id)),
                    personelId: record.personnelId.clone(),
                    donemId: period.id.clone(),
                    gunler,
                };
                AttendanceRepository::save_in_transaction(conn, &attendance)?;
            }
        }

        Ok(())
    }

    fn sync_attendance_on_delete(conn: &Connection, existing: &SickLeaveRecord) -> Result<()> {
        let all_periods = PeriodRepository::get_all(conn)?;
        if all_periods.is_empty() {
            return Ok(());
        }

        let start = match NaiveDate::parse_from_str(&existing.startDate, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let end = match NaiveDate::parse_from_str(&existing.endDate, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };

        for period in &all_periods {
            let p_start = match NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };
            let p_end = match NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };

            if start <= p_end && end >= p_start {
                if let Some(mut att) = AttendanceRepository::get_by_personnel_and_period(
                    conn,
                    &existing.personnelId,
                    &period.id,
                )? {
                    let mut changed = false;
                    let mut curr = start.max(p_start);
                    let end_overlap = end.min(p_end);
                    while curr <= end_overlap {
                        let date_str = curr.format("%Y-%m-%d").to_string();
                        // ONLY restore if currently "R" (never overwrite user's manual change)
                        if att.gunler.get(&date_str).map(|s| s.as_str()) == Some("R") {
                            let default_code = if curr.weekday() == chrono::Weekday::Sat
                                || curr.weekday() == chrono::Weekday::Sun
                            {
                                "T"
                            } else {
                                "Ç"
                            };
                            att.gunler.insert(date_str, default_code.to_string());
                            changed = true;
                        }
                        curr += chrono::Duration::days(1);
                    }

                    if changed {
                        AttendanceRepository::save_in_transaction(conn, &att)?;
                    }
                }
            }
        }

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
