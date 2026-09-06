use crate::domain::models::{
    AnnualPayrollParameters, BordroKaydi, BordroStatus, SickLeaveRecord, TaxBracket,
};
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::{SettingsRepository, ZAM_AYLARI_SETTING_KEY};
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

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
    fn shared_snapshot(conn: &Connection) -> Result<payroll_core::PayrollDatasetSnapshot> {
        Ok(payroll_core::PayrollDatasetSnapshot {
            personnel: PersonnelRepository::get_all(conn)?,
            periods: PeriodRepository::get_all(conn)?,
            institutionSettings: SettingsRepository::get_all_institution_settings(conn)?,
            attendances: AttendanceRepository::get_all(conn)?,
            payrolls: PayrollRepository::get_all(conn)?,
            taxOpenings: crate::repositories::tax_opening_repo::TaxOpeningRepository::get_all(
                conn,
            )?,
            sickLeaveRecords: SickLeaveRepository::get_all(conn)?,
            annualPayrollParameters: AnnualPayrollParametersRepository::get_all(conn)?,
            zamAylari: Self::get_zam_aylari(conn)?,
            compensationRevisions: crate::repositories::retro_repo::get_revisions(conn)?,
            compensationRevisionOverrides: crate::repositories::retro_repo::get_overrides(conn)?,
            retroBatches: crate::repositories::retro_repo::get_batches(conn)?,
            retroAllocations: crate::repositories::retro_repo::get_allocations(conn)?,
        })
    }

    fn from_shared_notice(notice: payroll_core::PayrollNotice) -> PayrollNotice {
        PayrollNotice {
            code: notice.code,
            severity: match notice.severity {
                payroll_core::PayrollNoticeSeverity::Info => PayrollNoticeSeverity::Info,
                payroll_core::PayrollNoticeSeverity::Warning => PayrollNoticeSeverity::Warning,
                payroll_core::PayrollNoticeSeverity::Critical => PayrollNoticeSeverity::Critical,
                payroll_core::PayrollNoticeSeverity::Success => PayrollNoticeSeverity::Success,
            },
            scope: match notice.scope {
                payroll_core::PayrollNoticeScope::Period => PayrollNoticeScope::Period,
                payroll_core::PayrollNoticeScope::Personnel => PayrollNoticeScope::Personnel,
            },
            personnel_id: notice.personnel_id,
            title: notice.title,
            message: notice.message,
            details: notice.details,
            action: notice.action,
        }
    }

    pub fn get_period_notices(conn: &Connection, period_id: &str) -> Result<Vec<PayrollNotice>> {
        let period = PeriodRepository::get_by_id(conn, period_id)?.ok_or_else(|| {
            DomainError::ValidationError(format!("Bordro dönemi bulunamadı: {period_id}"))
        })?;
        PeriodRepository::validate_period(&period)?;

        let start = Self::parse_date(&period.baslangicTarihi, "dönem başlangıç")?;
        let end = Self::parse_date(&period.bitisTarihi, "dönem bitiş")?;
        let personnel = PersonnelRepository::get_all(conn)?;
        let annual_parameters =
            AnnualPayrollParametersRepository::get_by_year(conn, period.taxYear)?;
        let payrolls_by_person: HashMap<String, BordroKaydi> = PayrollRepository::get_all(conn)?
            .into_iter()
            .filter(|payroll| payroll.donemId == period_id)
            .map(|payroll| (payroll.personelId.clone(), payroll))
            .collect();

        let mut notices =
            payroll_core::get_period_notices(&Self::shared_snapshot(conn)?, period_id)?
                .into_iter()
                .map(Self::from_shared_notice)
                .collect::<Vec<_>>();
        Self::append_period_parameter_notices(
            conn,
            &period,
            start,
            end,
            annual_parameters.as_ref(),
            &mut notices,
        )?;

        for person in personnel {
            let full_name = format!("{} {}", person.ad, person.soyad);
            let payroll = payrolls_by_person.get(&person.id);

            if let Some(record) = payroll {
                Self::append_payroll_result_notices(
                    &person.id,
                    &full_name,
                    record,
                    annual_parameters.as_ref(),
                    &mut notices,
                );
            }

            let sick_records = SickLeaveRepository::get_by_personnel(conn, &person.id)?;
            Self::append_sick_leave_quota_notices(
                &person.id,
                &full_name,
                &sick_records,
                start,
                end,
                &mut notices,
            );
        }

        Ok(notices)
    }

    fn append_payroll_result_notices(
        personnel_id: &str,
        full_name: &str,
        payroll: &BordroKaydi,
        annual_parameters: Option<&AnnualPayrollParameters>,
        notices: &mut Vec<PayrollNotice>,
    ) {
        if !matches!(
            payroll.status,
            BordroStatus::CALCULATED | BordroStatus::FINALIZED
        ) {
            return;
        }

        let incoming = payroll.devredenPekGelen.as_deref().unwrap_or(&[]);
        let outgoing = payroll.sonrakiDevredenPek.as_deref().unwrap_or(&[]);
        let incoming_total = incoming
            .iter()
            .filter(|item| item.tutar > Decimal::ZERO && item.kalanAySayisi > 0)
            .fold(Decimal::ZERO, |total, item| total + item.tutar)
            .round_dp(2);
        let outgoing_total = outgoing
            .iter()
            .filter(|item| item.tutar > Decimal::ZERO && item.kalanAySayisi > 0)
            .fold(Decimal::ZERO, |total, item| total + item.tutar)
            .round_dp(2);
        let used = payroll
            .pekDetay
            .as_ref()
            .map_or(Decimal::ZERO, |detail| detail.devredenPekKullanilan)
            .round_dp(2);

        if incoming_total > Decimal::ZERO {
            let mut details = vec![format!("Bu ay kullanılan: {}", Self::format_money(used))];
            if outgoing_total > Decimal::ZERO {
                details.push(format!(
                    "Sonraki döneme devreden: {}",
                    Self::format_money(outgoing_total)
                ));
            }
            notices.push(PayrollNotice {
                code: "INCOMING_PEK_CARRY".into(),
                severity: PayrollNoticeSeverity::Info,
                scope: PayrollNoticeScope::Personnel,
                personnel_id: Some(personnel_id.to_string()),
                title: "Devreden PEK uygulanıyor".into(),
                message: format!(
                    "{full_name} için önceki dönemlerden {} PEK geldi. Cari ay tavanına sığan kısım prim matrahına dahil edildi.",
                    Self::format_money(incoming_total)
                ),
                details,
                action: Some("REVIEW_PEK".into()),
            });
        } else if outgoing_total > Decimal::ZERO {
            notices.push(PayrollNotice {
                code: "OUTGOING_PEK_CARRY".into(),
                severity: PayrollNoticeSeverity::Info,
                scope: PayrollNoticeScope::Personnel,
                personnel_id: Some(personnel_id.to_string()),
                title: "PEK sonraki döneme devredilecek".into(),
                message: format!(
                    "{full_name} bordrosunda PEK tavanını aşan/kullanılamayan {} sonraki döneme taşınacak.",
                    Self::format_money(outgoing_total)
                ),
                details: Vec::new(),
                action: Some("REVIEW_PEK".into()),
            });
        }

        let last_month_total = incoming
            .iter()
            .filter(|item| item.tutar > Decimal::ZERO && item.kalanAySayisi == 1)
            .fold(Decimal::ZERO, |total, item| total + item.tutar)
            .round_dp(2);
        if last_month_total > Decimal::ZERO {
            notices.push(PayrollNotice {
                code: "PEK_CARRY_LAST_MONTH".into(),
                severity: PayrollNoticeSeverity::Warning,
                scope: PayrollNoticeScope::Personnel,
                personnel_id: Some(personnel_id.to_string()),
                title: "Devreden PEK için son kullanım ayı".into(),
                message: format!(
                    "{full_name} için {} tutarındaki devreden PEK kaydının kullanım hakkı bu ay sona eriyor. Cari ay tavanına sığmayan kısmı sonraki döneme taşınmayacak.",
                    Self::format_money(last_month_total)
                ),
                details: Vec::new(),
                action: Some("REVIEW_PEK".into()),
            });
        }

        let Some(parameters) = annual_parameters else {
            return;
        };
        let Some(gv_detail) = payroll.gvDetay.as_ref() else {
            return;
        };
        let previous_cumulative =
            (gv_detail.yeniKumulatifGvMatrahi - gv_detail.cariGvMatrahi).max(Decimal::ZERO);
        let slices = Self::tax_slices(
            previous_cumulative,
            gv_detail.yeniKumulatifGvMatrahi,
            &parameters.gelirVergisiDilimleri,
        );
        let Some((_, first_rate)) = slices.first() else {
            return;
        };
        let Some((_, last_rate)) = slices.last() else {
            return;
        };
        if slices.len() < 2 || first_rate == last_rate {
            return;
        }

        notices.push(PayrollNotice {
            code: "INCOME_TAX_BRACKET_TRANSITION".into(),
            severity: PayrollNoticeSeverity::Warning,
            scope: PayrollNoticeScope::Personnel,
            personnel_id: Some(personnel_id.to_string()),
            title: "Gelir vergisi dilimi değişti".into(),
            message: format!(
                "{full_name} bordrosunda kümülatif GV matrahı bu ay %{} diliminden %{} dilimine geçti.",
                Self::format_rate(*first_rate),
                Self::format_rate(*last_rate)
            ),
            details: slices
                .iter()
                .map(|(amount, rate)| {
                    format!(
                        "{} matrah %{}",
                        Self::format_money(*amount),
                        Self::format_rate(*rate)
                    )
                })
                .collect(),
            action: Some("REVIEW_TAX_DETAIL".into()),
        });
    }

    fn append_sick_leave_quota_notices(
        personnel_id: &str,
        full_name: &str,
        records: &[SickLeaveRecord],
        period_start: NaiveDate,
        period_end: NaiveDate,
        notices: &mut Vec<PayrollNotice>,
    ) {
        let mut year_groups: BTreeMap<i32, Vec<(NaiveDate, NaiveDate)>> = BTreeMap::new();
        for record in records {
            let (Ok(start), Ok(end)) = (
                NaiveDate::parse_from_str(&record.startDate, "%Y-%m-%d"),
                NaiveDate::parse_from_str(&record.endDate, "%Y-%m-%d"),
            ) else {
                continue;
            };
            if end < start {
                continue;
            }
            year_groups
                .entry(start.year())
                .or_default()
                .push((start, end));
        }

        for (year, mut episodes) in year_groups {
            episodes.sort_by_key(|episode| (episode.0, episode.1));
            episodes.dedup();

            for (index, (start, end)) in episodes.iter().enumerate() {
                if *start < period_start || *start > period_end {
                    continue;
                }
                let episode_number = index + 1;
                let details = vec![format!(
                    "Rapor: {} – {}",
                    start.format("%Y-%m-%d"),
                    end.format("%Y-%m-%d")
                )];

                if episode_number < 5 {
                    notices.push(PayrollNotice {
                        code: "SICK_LEAVE_QUOTA_INFO".into(),
                        severity: PayrollNoticeSeverity::Info,
                        scope: PayrollNoticeScope::Personnel,
                        personnel_id: Some(personnel_id.to_string()),
                        title: format!("Rapor kotası: {episode_number}. vaka"),
                        message: format!(
                            "{full_name} için bu kayıt {year} yılındaki {episode_number}. rapor vakasıdır. Raporun ilk en fazla 2 günü kurumca ödenir."
                        ),
                        details,
                        action: Some("CHECK_SICK_LEAVE".into()),
                    });
                } else if episode_number == 5 {
                    notices.push(PayrollNotice {
                        code: "SICK_LEAVE_QUOTA_LAST_PAID".into(),
                        severity: PayrollNoticeSeverity::Warning,
                        scope: PayrollNoticeScope::Personnel,
                        personnel_id: Some(personnel_id.to_string()),
                        title: "Kurum ödemeli son rapor vakası".into(),
                        message: format!(
                            "{full_name} için bu kayıt {year} yılındaki 5. rapor vakasıdır. İlk en fazla 2 gün kurumca ödenir; bu yıldaki sonraki rapor vakalarında ilk iki gün kurumca ödenmeyecektir."
                        ),
                        details,
                        action: Some("CHECK_SICK_LEAVE".into()),
                    });
                } else {
                    notices.push(PayrollNotice {
                        code: "SICK_LEAVE_QUOTA_EXHAUSTED".into(),
                        severity: PayrollNoticeSeverity::Warning,
                        scope: PayrollNoticeScope::Personnel,
                        personnel_id: Some(personnel_id.to_string()),
                        title: "Yıllık rapor kotası dolu".into(),
                        message: format!(
                            "{full_name} için bu kayıt {year} yılındaki {episode_number}. rapor vakasıdır. Yıllık ilk 5 vaka kotası dolduğu için bu raporun ilk iki günü kurumca ödenmeyecektir."
                        ),
                        details,
                        action: Some("CHECK_SICK_LEAVE".into()),
                    });
                }
            }
        }
    }

    fn append_period_parameter_notices(
        conn: &Connection,
        period: &crate::domain::models::BordroDonemi,
        start: NaiveDate,
        end: NaiveDate,
        annual_parameters: Option<&AnnualPayrollParameters>,
        notices: &mut Vec<PayrollNotice>,
    ) -> Result<()> {
        if let Some(parameters) = annual_parameters {
            if period.taxMonth == 1 {
                notices.push(PayrollNotice {
                    code: "NEW_TAX_YEAR_PARAMETERS_ACTIVE".into(),
                    severity: PayrollNoticeSeverity::Info,
                    scope: PayrollNoticeScope::Period,
                    personnel_id: None,
                    title: "Yeni vergi yılı parametreleri devrede".into(),
                    message: format!(
                        "{} vergi yılının ilk ayında {} gelir vergisi dilimli yıllık parametre paketi kullanılacak.",
                        period.taxYear,
                        parameters.gelirVergisiDilimleri.len()
                    ),
                    details: vec![format!("Vergi dönemi: {}-01", period.taxYear)],
                    action: Some("CHECK_ANNUAL_PARAMETERS".into()),
                });
            }
        }

        let current_settings = SettingsRepository::get_institution_settings(conn, &period.id)?;

        let zam_aylari = Self::get_zam_aylari(conn)?;
        if zam_aylari.is_empty() {
            notices.push(PayrollNotice {
                code: "RAISE_MONTHS_NOT_CONFIGURED".into(),
                severity: PayrollNoticeSeverity::Warning,
                scope: PayrollNoticeScope::Period,
                personnel_id: None,
                title: "Zam yürürlük ayları tanımlı değil".into(),
                message: "Kurum genelinde zam yürürlük ayı seçilmedi. Bu durumda dönem bordrosu tek ücret setiyle hesaplanır; kurumunuzda dönem içi zam uygulanıyorsa ayları tanımlayın.".into(),
                details: Vec::new(),
                action: Some("CHECK_RAISE_PARAMETERS".into()),
            });
            return Ok(());
        }

        let Some(raise_date) = Self::find_raise_date(start, end, &zam_aylari) else {
            return Ok(());
        };

        let previous_period = PeriodRepository::get_previous_by_work_period(conn, period)?;
        let mut details = vec![format!(
            "Zam yürürlük tarihi: {}",
            raise_date.format("%Y-%m-%d")
        )];

        if let Some(current) = current_settings.as_ref() {
            details.push(format!(
                "Yeni günlük taban: {} TL",
                current.gunlukTabanUcret
            ));
        }

        match previous_period {
            None => notices.push(PayrollNotice {
                code: "MISSING_PRE_RAISE_PERIOD".into(),
                severity: PayrollNoticeSeverity::Critical,
                scope: PayrollNoticeScope::Period,
                personnel_id: None,
                title: "Zam öncesi dönem bulunamadı".into(),
                message: format!(
                    "{} dönemi {} tarihinde zam geçişi içeriyor ancak zam öncesi ücret setini sağlayacak önceki çalışma dönemi yok.",
                    period.donemAdi,
                    raise_date.format("%Y-%m-%d")
                ),
                details,
                action: Some("CHECK_RAISE_PARAMETERS".into()),
            }),
            Some(previous) => {
                let previous_settings =
                    SettingsRepository::get_institution_settings(conn, &previous.id)?;
                if let Some(previous_settings) = previous_settings {
                    details.push(format!(
                        "Eski günlük taban: {} TL ({})",
                        previous_settings.gunlukTabanUcret, previous.donemAdi
                    ));
                    notices.push(PayrollNotice {
                        code: "RAISE_TRANSITION_PERIOD".into(),
                        severity: PayrollNoticeSeverity::Warning,
                        scope: PayrollNoticeScope::Period,
                        personnel_id: None,
                        title: "Bu bordro zam geçişi içeriyor".into(),
                        message: format!(
                            "{} tarihinde ücret seti değişiyor. Motor zam öncesi günlerde {} döneminin, zam sonrası günlerde {} döneminin kurum değerlerini kullanacak; eski ve yeni tutarları kontrol edin.",
                            raise_date.format("%Y-%m-%d"),
                            previous.donemAdi,
                            period.donemAdi
                        ),
                        details,
                        action: Some("CHECK_RAISE_PARAMETERS".into()),
                    });
                } else {
                    details.push(format!("Zam öncesi dönem: {}", previous.donemAdi));
                    notices.push(PayrollNotice {
                        code: "MISSING_PRE_RAISE_SETTINGS".into(),
                        severity: PayrollNoticeSeverity::Critical,
                        scope: PayrollNoticeScope::Period,
                        personnel_id: None,
                        title: "Zam öncesi ücret parametreleri eksik".into(),
                        message: format!(
                            "{} tarihinde zam geçişi var ancak {} döneminin kurum değerleri bulunamadı. Zam öncesi günlerin doğru hesaplanması için bu kayıt zorunludur.",
                            raise_date.format("%Y-%m-%d"),
                            previous.donemAdi
                        ),
                        details,
                        action: Some("CHECK_RAISE_PARAMETERS".into()),
                    });
                }
            }
        }

        Ok(())
    }

    fn tax_slices(
        previous_cumulative: Decimal,
        new_cumulative: Decimal,
        brackets: &[TaxBracket],
    ) -> Vec<(Decimal, Decimal)> {
        let mut result = Vec::new();
        let mut lower = previous_cumulative.max(Decimal::ZERO);
        let upper_total = new_cumulative.max(lower);

        for bracket in brackets {
            if lower >= upper_total {
                break;
            }
            if lower >= bracket.limit {
                continue;
            }
            let upper = upper_total.min(bracket.limit);
            if upper > lower {
                result.push(((upper - lower).round_dp(2), bracket.oran));
                lower = upper;
            }
        }

        if lower < upper_total {
            if let Some(last) = brackets.last() {
                result.push(((upper_total - lower).round_dp(2), last.oran));
            }
        }

        result
    }

    fn get_zam_aylari(conn: &Connection) -> Result<Vec<i32>> {
        let Some(raw) = SettingsRepository::get_app_setting(conn, ZAM_AYLARI_SETTING_KEY)? else {
            return Ok(Vec::new());
        };
        let months: Vec<i32> = serde_json::from_str(&raw).map_err(|e| {
            DomainError::InvalidData(format!("Kurum zam ayarı bozuk JSON içeriyor: {e}"))
        })?;
        SettingsRepository::normalize_zam_aylari(&months)
    }

    fn find_raise_date(start: NaiveDate, end: NaiveDate, zam_aylari: &[i32]) -> Option<NaiveDate> {
        let mut result = None;
        for year in (start.year() - 1)..=(end.year() + 1) {
            for month in zam_aylari {
                let Some(candidate) = NaiveDate::from_ymd_opt(year, *month as u32, 1) else {
                    continue;
                };
                if candidate >= start && candidate <= end {
                    result =
                        Some(result.map_or(candidate, |current: NaiveDate| current.min(candidate)));
                }
            }
        }
        result
    }

    fn parse_date(value: &str, label: &str) -> Result<NaiveDate> {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| DomainError::InvalidData(format!("Geçersiz {label} tarihi: {value}")))
    }

    fn format_money(value: Decimal) -> String {
        format!("{:.2} TL", value.round_dp(2))
    }

    fn format_rate(rate: Decimal) -> String {
        (rate * Decimal::from(100))
            .round_dp(2)
            .normalize()
            .to_string()
    }
}
