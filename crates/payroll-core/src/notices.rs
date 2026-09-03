use crate::models::{BordroStatus, SickLeaveRecord};
use crate::payroll_engine::PayrollDatasetSnapshot;
use crate::{DomainError, Result};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayrollNoticeSeverity {
    Info,
    Warning,
    Critical,
    Success,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayrollNoticeScope {
    Period,
    Personnel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn parse_date(value: &str, field: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|error| DomainError::InvalidData(format!("{} tarihi geçersiz: {}", field, error)))
}

fn preview_dates(dates: &[String]) -> Vec<String> {
    dates.iter().take(5).cloned().collect()
}

fn date_range(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut result = Vec::new();
    let mut current = start;
    while current <= end {
        result.push(current);
        let Some(next) = current.checked_add_signed(Duration::days(1)) else {
            break;
        };
        current = next;
    }
    result
}

fn report_ranges(
    records: &[SickLeaveRecord],
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<(NaiveDate, NaiveDate)>> {
    let mut ranges = Vec::new();
    for record in records {
        let record_start = parse_date(&record.startDate, "rapor başlangıç")?;
        let record_end = parse_date(&record.endDate, "rapor bitiş")?;
        if record_start <= end && record_end >= start {
            ranges.push((record_start, record_end));
        }
    }
    Ok(ranges)
}

/// Returns the browser-visible period/person notices from the same snapshot
/// used by the calculation engine. Native has a richer repository-backed
/// presentation service, but the blocking data-quality rules are shared here.
pub fn get_period_notices(
    dataset: &PayrollDatasetSnapshot,
    period_id: &str,
) -> Result<Vec<PayrollNotice>> {
    let period = dataset
        .periods
        .iter()
        .find(|period| period.id == period_id)
        .ok_or_else(|| {
            DomainError::ValidationError(format!("Bordro dönemi bulunamadı: {}", period_id))
        })?;
    let start = parse_date(&period.baslangicTarihi, "dönem başlangıç")?;
    let end = parse_date(&period.bitisTarihi, "dönem bitiş")?;
    let dates = date_range(start, end);
    let mut notices = Vec::new();

    if !dataset.institutionSettings.contains_key(period_id) {
        notices.push(PayrollNotice {
            code: "MISSING_PERIOD_PARAMETERS".into(),
            severity: PayrollNoticeSeverity::Critical,
            scope: PayrollNoticeScope::Period,
            personnel_id: None,
            title: "Dönem kurum ayarları yok".into(),
            message: format!(
                "{} dönemi kurum ayarları olmadan bordro hesaplanamaz.",
                period.donemAdi
            ),
            details: Vec::new(),
            action: Some("CHECK_PERIOD_PARAMETERS".into()),
        });
    }
    if !dataset
        .annualPayrollParameters
        .iter()
        .any(|parameters| parameters.year == period.taxYear)
    {
        notices.push(PayrollNotice {
            code: "MISSING_ANNUAL_PARAMETERS".into(),
            severity: PayrollNoticeSeverity::Critical,
            scope: PayrollNoticeScope::Period,
            personnel_id: None,
            title: "Yıllık vergi parametreleri yok".into(),
            message: format!(
                "{} vergi yılı yıllık bordro parametreleri olmadan bordro hesaplanamaz.",
                period.taxYear
            ),
            details: Vec::new(),
            action: Some("CHECK_ANNUAL_PARAMETERS".into()),
        });
    }

    for person in &dataset.personnel {
        let full_name = format!("{} {}", person.ad, person.soyad);
        let payroll = dataset
            .payrolls
            .iter()
            .find(|payroll| payroll.personelId == person.id && payroll.donemId == period_id);
        if payroll.is_some_and(|payroll| payroll.status == BordroStatus::STALE) {
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

        let attendance = dataset.attendances.iter().find(|attendance| {
            attendance.personelId == person.id && attendance.donemId == period_id
        });
        let Some(attendance) = attendance else {
            notices.push(PayrollNotice {
                code: "MISSING_ATTENDANCE".into(),
                severity: PayrollNoticeSeverity::Critical,
                scope: PayrollNoticeScope::Personnel,
                personnel_id: Some(person.id.clone()),
                title: "Kayıtlı puantaj yok".into(),
                message: format!(
                    "{full_name} için {} döneminde kaydedilmiş puantaj bulunmuyor.",
                    period.donemAdi
                ),
                details: Vec::new(),
                action: Some("GO_TO_PUANTAJ".into()),
            });
            continue;
        };

        let missing_dates: Vec<String> = dates
            .iter()
            .filter(|date| {
                !attendance
                    .gunler
                    .contains_key(&date.format("%Y-%m-%d").to_string())
            })
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
                details: preview_dates(&missing_dates),
                action: Some("GO_TO_PUANTAJ".into()),
            });
        }

        let records: Vec<SickLeaveRecord> = dataset
            .sickLeaveRecords
            .iter()
            .filter(|record| record.personnelId == person.id)
            .cloned()
            .collect();
        let ranges = report_ranges(&records, start, end)?;
        let r_without_report: Vec<String> = attendance
            .gunler
            .iter()
            .filter(|(_, code)| code.as_str() == "R")
            .filter_map(|(date_text, _)| {
                let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").ok()?;
                (!ranges
                    .iter()
                    .any(|(range_start, range_end)| date >= *range_start && date <= *range_end))
                .then_some(date_text.clone())
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
                    "{full_name} puantajında {} R günü için rapor kaydı yok.",
                    r_without_report.len()
                ),
                details: preview_dates(&r_without_report),
                action: Some("GO_TO_PUANTAJ".into()),
            });
        }

        let report_without_r: Vec<String> = dates
            .iter()
            .filter(|date| {
                ranges
                    .iter()
                    .any(|(range_start, range_end)| **date >= *range_start && **date <= *range_end)
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
                    "{full_name} için rapor kaydıyla örtüşen {} gün puantajda R değil.",
                    report_without_r.len()
                ),
                details: preview_dates(&report_without_r),
                action: Some("GO_TO_PUANTAJ".into()),
            });
        }
    }

    Ok(notices)
}
