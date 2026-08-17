use crate::domain::{DomainError, Result};
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayrollNoticeSeverity {
    Info,
    Warning,
    Critical,
    Success,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayrollNoticeScope {
    Period,
    Personnel,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PayrollNotice {
    pub code: String,
    pub severity: PayrollNoticeSeverity,
    pub scope: PayrollNoticeScope,
    pub personnel_id: Option<String>,
    pub title: String,
    pub message: String,
    pub details: Vec<String>,
    pub action: Option<String>,
}

pub struct PayrollNoticeService;

impl PayrollNoticeService {
    pub fn get_period_notices(conn: &Connection, period_id: &str) -> Result<Vec<PayrollNotice>> {
        let period = PeriodRepository::get_by_id(conn, period_id)?.ok_or_else(|| {
            DomainError::ValidationError(format!("Bordro dönemi bulunamadı: {period_id}"))
        })?;
        PeriodRepository::validate_period(&period)?;

        let start = Self::parse_date(&period.baslangicTarihi, "dönem başlangıç")?;
        let end = Self::parse_date(&period.bitisTarihi, "dönem bitiş")?;
        let period_dates = Self::date_range(start, end);
        let personnel = PersonnelRepository::get_all(conn)?;

        let mut notices = Vec::new();

        for person in personnel {
            let full_name = format!("{} {}", person.ad, person.soyad);

            let payroll_status = conn
                .query_row(
                    "SELECT status FROM payroll_records WHERE personnel_id = ?1 AND period_id = ?2",
                    params![person.id, period_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

            if payroll_status.as_deref() == Some("STALE") {
                notices.push(PayrollNotice {
                    code: "STALE_PAYROLL".into(),
                    severity: PayrollNoticeSeverity::Critical,
                    scope: PayrollNoticeScope::Personnel,
                    personnel_id: Some(person.id.clone()),
                    title: "Bordro yeniden hesaplanmalı".into(),
                    message: format!(
                        "{full_name} bordrosunun dayandığı kaynak veriler değişti. Güncel tutarlar kullanılmadan önce bordroyu yeniden hesaplayın."
                    ),
                    details: Vec::new(),
                    action: Some("RECALCULATE_PAYROLL".into()),
                });
            }

            let attendance =
                AttendanceRepository::get_by_personnel_and_period(conn, &person.id, period_id)?;

            let Some(attendance) = attendance else {
                notices.push(PayrollNotice {
                    code: "MISSING_ATTENDANCE".into(),
                    severity: PayrollNoticeSeverity::Critical,
                    scope: PayrollNoticeScope::Personnel,
                    personnel_id: Some(person.id.clone()),
                    title: "Kayıtlı puantaj yok".into(),
                    message: format!(
                        "{full_name} için {} döneminde veritabanına kaydedilmiş puantaj bulunmuyor. Ekrandaki varsayılan Ç/T görünümü gerçek kayıt sayılmaz.",
                        period.donemAdi
                    ),
                    details: Vec::new(),
                    action: Some("GO_TO_PUANTAJ".into()),
                });
                continue;
            };

            let missing_dates: Vec<String> = period_dates
                .iter()
                .filter(|date| !attendance.gunler.contains_key(&date.format("%Y-%m-%d").to_string()))
                .map(|date| date.format("%Y-%m-%d").to_string())
                .collect();

            if !missing_dates.is_empty() {
                notices.push(PayrollNotice {
                    code: "INCOMPLETE_ATTENDANCE".into(),
                    severity: PayrollNoticeSeverity::Critical,
                    scope: PayrollNoticeScope::Personnel,
                    personnel_id: Some(person.id.clone()),
                    title: "Puantaj günleri eksik".into(),
                    message: format!(
                        "{full_name} için {} takvim gününde puantaj kodu kayıtlı değil.",
                        missing_dates.len()
                    ),
                    details: Self::preview_dates(&missing_dates),
                    action: Some("GO_TO_PUANTAJ".into()),
                });
            }

            let sick_records = SickLeaveRepository::get_by_personnel(conn, &person.id)?;
            let mut report_ranges = Vec::new();
            for record in sick_records {
                let record_start = Self::parse_date(&record.startDate, "rapor başlangıç")?;
                let record_end = Self::parse_date(&record.endDate, "rapor bitiş")?;
                if record_start <= end && record_end >= start {
                    report_ranges.push((record_start, record_end));
                }
            }

            let r_without_report: Vec<String> = attendance
                .gunler
                .iter()
                .filter(|(_, code)| code.as_str() == "R")
                .filter_map(|(date_text, _)| {
                    let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").ok()?;
                    let covered = report_ranges
                        .iter()
                        .any(|(report_start, report_end)| date >= *report_start && date <= *report_end);
                    (!covered).then(|| date_text.clone())
                })
                .collect();

            if !r_without_report.is_empty() {
                notices.push(PayrollNotice {
                    code: "ATTENDANCE_R_WITHOUT_SICK_LEAVE".into(),
                    severity: PayrollNoticeSeverity::Warning,
                    scope: PayrollNoticeScope::Personnel,
                    personnel_id: Some(person.id.clone()),
                    title: "Rapor kodu ile rapor kaydı uyuşmuyor".into(),
                    message: format!(
                        "{full_name} puantajında R kodu bulunan {} gün için bu tarihi kapsayan rapor kaydı yok.",
                        r_without_report.len()
                    ),
                    details: Self::preview_dates(&r_without_report),
                    action: Some("GO_TO_PUANTAJ".into()),
                });
            }

            let report_without_r: Vec<String> = period_dates
                .iter()
                .filter(|date| {
                    report_ranges
                        .iter()
                        .any(|(report_start, report_end)| **date >= *report_start && **date <= *report_end)
                })
                .filter_map(|date| {
                    let date_text = date.format("%Y-%m-%d").to_string();
                    (attendance.gunler.get(&date_text).map(String::as_str) != Some("R"))
                        .then_some(date_text)
                })
                .collect();

            if !report_without_r.is_empty() {
                notices.push(PayrollNotice {
                    code: "SICK_LEAVE_WITHOUT_ATTENDANCE_R".into(),
                    severity: PayrollNoticeSeverity::Warning,
                    scope: PayrollNoticeScope::Personnel,
                    personnel_id: Some(person.id.clone()),
                    title: "Rapor kaydı puantaja yansımamış".into(),
                    message: format!(
                        "{full_name} için rapor kaydıyla örtüşen {} gün puantajda R olarak işaretlenmemiş.",
                        report_without_r.len()
                    ),
                    details: Self::preview_dates(&report_without_r),
                    action: Some("GO_TO_PUANTAJ".into()),
                });
            }
        }

        Ok(notices)
    }

    fn parse_date(value: &str, label: &str) -> Result<NaiveDate> {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
            DomainError::InvalidData(format!("Geçersiz {label} tarihi: {value}"))
        })
    }

    fn date_range(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
        let mut dates = Vec::new();
        let mut current = start;
        while current <= end {
            dates.push(current);
            current += Duration::days(1);
        }
        dates
    }

    fn preview_dates(dates: &[String]) -> Vec<String> {
        const PREVIEW_LIMIT: usize = 8;
        let mut details: Vec<String> = dates.iter().take(PREVIEW_LIMIT).cloned().collect();
        if dates.len() > PREVIEW_LIMIT {
            details.push(format!("+{} gün daha", dates.len() - PREVIEW_LIMIT));
        }
        details
    }
}
