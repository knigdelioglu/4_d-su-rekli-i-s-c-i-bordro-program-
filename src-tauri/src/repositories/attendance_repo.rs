use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::transaction::with_transaction;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

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
                DomainError::InvalidData(format!(
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
                return Err(DomainError::ValidationError(format!(
                    "{} dönemi {} tarihinde bilinmeyen puantaj kodu: {}",
                    period_id, date, code
                )));
            }
        }
        Ok(())
    }

    /// Puantajın bordro dönemiyle birebir uyumlu olmasını sağlayan authoritative
    /// backend doğrulaması. UI doğrulamasına güvenmez; repository save/read ve
    /// PayrollService aynı invariantı kullanır.
    pub fn validate_attendance_for_period(
        attendance: &PersonelPuantaj,
        period: &BordroDonemi,
    ) -> Result<()> {
        PeriodRepository::validate_period(period)?;

        if attendance.donemId != period.id {
            return Err(DomainError::ValidationError(format!(
                "Puantaj dönem kimliği '{}' ile bordro dönemi '{}' eşleşmiyor.",
                attendance.donemId, period.id
            )));
        }

        Self::validate_codes(&attendance.gunler, &period.id)?;

        let start =
            NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").map_err(|_| {
                DomainError::ValidationError(format!(
                    "{} dönemi başlangıç tarihi geçersiz: {}",
                    period.id, period.baslangicTarihi
                ))
            })?;
        let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError(format!(
                "{} dönemi bitiş tarihi geçersiz: {}",
                period.id, period.bitisTarihi
            ))
        })?;

        let calendar_day_count = (end - start).num_days() + 1;
        if attendance.gunler.len() as i64 > calendar_day_count {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi {} takvim günü içeriyor ancak puantajda {} kayıt var.",
                period.id,
                calendar_day_count,
                attendance.gunler.len()
            )));
        }

        for date_text in attendance.gunler.keys() {
            let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| {
                DomainError::ValidationError(format!(
                    "{} döneminde puantaj tarihi YYYY-MM-DD biçiminde geçerli bir tarih olmalıdır: {}",
                    period.id, date_text
                ))
            })?;

            if date < start || date > end {
                return Err(DomainError::ValidationError(format!(
                    "{} puantaj tarihi {} döneminin {}–{} aralığı dışında.",
                    date_text, period.id, period.baslangicTarihi, period.bitisTarihi
                )));
            }

            if date.format("%Y-%m-%d").to_string() != date_text.as_str() {
                return Err(DomainError::ValidationError(format!(
                    "{} döneminde puantaj tarihi YYYY-MM-DD biçiminde kanonik olmalıdır: {}",
                    period.id, date_text
                )));
            }
        }

        // `validate_period` above establishes the 15–14 service-period
        // geometry, so this finite offset range ends at `end` and cannot
        // advance beyond Chrono's supported calendar.
        for day_offset in 0..calendar_day_count {
            let date = start + Duration::days(day_offset);
            let date_text = date.format("%Y-%m-%d").to_string();
            if !attendance.gunler.contains_key(&date_text) {
                return Err(DomainError::ValidationError(format!(
                    "{} döneminde puantaj günü eksik: {}. Her hizmet dönemi günü bir puantaj koduyla kaydedilmelidir.",
                    period.id, date_text
                )));
            }
        }

        Ok(())
    }

    fn period_for_attendance(conn: &Connection, period_id: &str) -> Result<BordroDonemi> {
        PeriodRepository::get_by_id(conn, period_id)?.ok_or_else(|| {
            DomainError::ValidationError(format!(
                "Puantaj için tanımlı bordro dönemi bulunamadı: {}",
                period_id
            ))
        })
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

        let mut result = Vec::new();
        for r in rows {
            let (id, personnel_id, period_id, attendance_json) =
                r.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            let gunler = Self::parse_codes(&personnel_id, &period_id, &attendance_json)?;
            let attendance = PersonelPuantaj {
                id,
                personelId: personnel_id,
                donemId: period_id.clone(),
                gunler,
            };
            let period = Self::period_for_attendance(conn, &period_id)?;
            Self::validate_attendance_for_period(&attendance, &period)?;
            result.push(attendance);
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

        row.map(|(id, personnel_id, stored_period_id, attendance_json)| {
            let gunler = Self::parse_codes(&personnel_id, &stored_period_id, &attendance_json)?;
            let attendance = PersonelPuantaj {
                id,
                personelId: personnel_id,
                donemId: stored_period_id.clone(),
                gunler,
            };
            let period = Self::period_for_attendance(conn, &stored_period_id)?;
            Self::validate_attendance_for_period(&attendance, &period)?;
            Ok(attendance)
        })
        .transpose()
    }

    pub fn save(conn: &Connection, p: &PersonelPuantaj) -> Result<()> {
        with_transaction(conn, |tx| Self::save_in_transaction(tx, p))
    }

    /// Replay only selected dates against current data inside the same transaction as policy authorization.
    pub fn apply_bulk_intent(
        conn: &Connection,
        period_id: &str,
        personnel_ids: &[String],
        dates: &[String],
        code: &str,
    ) -> Result<()> {
        if personnel_ids.is_empty() || dates.is_empty() || !Self::VALID_CODES.contains(&code) {
            return Err(DomainError::ValidationError(
                "Personel, tarih ve geçerli puantaj kodu seçin.".into(),
            ));
        }
        with_transaction(conn, |tx| {
            let period = Self::period_for_attendance(tx, period_id)?;
            let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
                .map_err(|e| DomainError::ValidationError(e.to_string()))?;
            let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")
                .map_err(|e| DomainError::ValidationError(e.to_string()))?;
            if dates
                .iter()
                .any(|date| date < &period.baslangicTarihi || date > &period.bitisTarihi)
            {
                return Err(DomainError::ValidationError(
                    "Seçili tarih hizmet döneminin dışında.".into(),
                ));
            }
            let mut seen = HashSet::new();
            for personnel_id in personnel_ids {
                if !seen.insert(personnel_id) {
                    return Err(DomainError::ValidationError(
                        "Personel seçimi mükerrer kayıt içeriyor.".into(),
                    ));
                }
                let existing = Self::get_by_personnel_and_period(tx, personnel_id, period_id)?;
                let mut attendance = existing.clone().unwrap_or_else(|| PersonelPuantaj {
                    id: format!("{personnel_id}_{period_id}"),
                    personelId: personnel_id.clone(),
                    donemId: period_id.into(),
                    gunler: (0..=(end - start).num_days())
                        .map(|offset| {
                            let date = start + Duration::days(offset);
                            (
                                date.format("%Y-%m-%d").to_string(),
                                if date.weekday().number_from_monday() >= 6 {
                                    "T"
                                } else {
                                    "Ç"
                                }
                                .into(),
                            )
                        })
                        .collect(),
                });
                let reports =
                    crate::repositories::sick_leave_repo::SickLeaveRepository::get_by_personnel(
                        tx,
                        personnel_id,
                    )?;
                for date in dates {
                    let current = attendance.gunler.get(date).ok_or_else(|| {
                        DomainError::ValidationError("Seçili gün puantajda bulunamadı.".into())
                    })?;
                    let covered = reports
                        .iter()
                        .any(|report| report.startDate <= *date && report.endDate >= *date);
                    if covered && current != "R" {
                        return Err(DomainError::ValidationError("Hastalık raporu günü R koduyla eşleşmiyor. Önce raporu ve puantajı düzeltin.".into()));
                    }
                    if current != "R" && !covered {
                        attendance.gunler.insert(date.clone(), code.into());
                    }
                }
                if existing
                    .as_ref()
                    .is_none_or(|previous| previous.gunler != attendance.gunler)
                {
                    Self::save_in_transaction(tx, &attendance)?;
                }
            }
            Ok(())
        })
    }

    pub fn save_many(conn: &Connection, attendances: &[PersonelPuantaj]) -> Result<()> {
        if attendances.is_empty() {
            return Err(DomainError::ValidationError(
                "Toplu puantaj kaydı en az bir personel içermelidir.".into(),
            ));
        }

        let mut seen = HashSet::with_capacity(attendances.len());
        for attendance in attendances {
            if !seen.insert((attendance.personelId.as_str(), attendance.donemId.as_str())) {
                return Err(DomainError::ValidationError(
                    "Toplu puantaj kaydında aynı personel ve dönem birden fazla kez bulunuyor."
                        .into(),
                ));
            }
        }

        with_transaction(conn, |tx| {
            for attendance in attendances {
                Self::save_in_transaction(tx, attendance)?;
            }
            Ok(())
        })
    }

    /// Caller-owned transaction variant used by backup restore and composite
    /// use cases.  The source write and the invalidation must remain in the
    /// same transaction as the caller's other mutations.
    pub fn save_in_transaction(conn: &Connection, p: &PersonelPuantaj) -> Result<()> {
        let period = Self::period_for_attendance(conn, &p.donemId)?;
        Self::validate_attendance_for_period(p, &period)?;

        let impact = PayrollInvalidationRepository::assert_mutation_allowed(
            conn,
            &payroll_core::PayrollMutation::PersonPeriod {
                personnelId: p.personelId.clone(),
                periodId: p.donemId.clone(),
            },
        )?;

        let changed = Self::get_by_personnel_and_period(conn, &p.personelId, &p.donemId)?
            .map(|existing| existing.gunler != p.gunler)
            .unwrap_or(true);

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

        if changed {
            PayrollInvalidationRepository::apply_impact(conn, &impact)?;
        }

        Ok(())
    }
}
