//! Deterministic, source-period based retroactive entitlement calculation.
//!
//! This module never mutates an existing payroll record. It replays the
//! historical attendance and compensation inputs, builds a target entitlement,
//! reads the recognized ledger from authoritative payroll events and earlier
//! retro allocations, and emits an auditable delta allocation.

#![allow(non_snake_case)]

use crate::calculations::{
    calculate_gunluk_gelirler_from_puantaj,
    calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state, canonical_sgk_earning_class,
    round_sgk_amount, CanonicalSgkEarningClass, PekCalculationOptions,
};
use crate::models::*;
use crate::payroll_engine::{
    accrual_order_for_payroll, calculate_paid_sick_dates_from_records,
    ensure_authoritative_payment_event, incoming_devreden_pek_for_replay,
    resolve_statutory_snapshot_for_period_with_paid_sick_dates, validate_tax_month_overlap,
    PayrollDatasetSnapshot,
};
use crate::{DomainError, Result};
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

type RetroAmountsByPeriodAndCode = HashMap<(String, RetroEarningCode), Decimal>;
type RetroSgkLedgerTotals = BTreeMap<String, (Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal)>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetroCalculationRequest {
    pub batchId: String,
    pub revision: CompensationRevision,
    pub overrides: Vec<CompensationRevisionOverride>,
    pub personnelId: String,
    pub paymentDate: String,
    pub calculatedAt: String,
    #[serde(default)]
    pub description: Option<String>,
    pub dataset: PayrollDatasetSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetroPeriodPreview {
    pub sourcePeriodId: String,
    pub originalRecognizedAmount: Decimal,
    pub previousAuthoritativeRetroAmount: Decimal,
    pub targetAmount: Decimal,
    pub deltaAmount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetroCalculationResult {
    pub batch: RetroAdjustmentBatch,
    pub allocations: Vec<RetroAllocation>,
    pub periods: Vec<RetroPeriodPreview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetroEarningPolicy {
    pub incomeTaxTreatment: RetroTaxTreatment,
    pub stampTaxTreatment: RetroTaxTreatment,
    pub sgkTreatment: RetroSgkTreatment,
}

/// The registry is the only place where an earning code gets statutory
/// treatment. Formula code does not branch on arbitrary UI labels.
pub fn retro_earning_policy(code: RetroEarningCode) -> RetroEarningPolicy {
    let wage_source = RetroEarningPolicy {
        incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
        stampTaxTreatment: RetroTaxTreatment::TAXABLE,
        sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
    };
    match canonical_sgk_earning_class(code) {
        CanonicalSgkEarningClass::Wage => wage_source,
        CanonicalSgkEarningClass::NonWage => RetroEarningPolicy {
            incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
            stampTaxTreatment: RetroTaxTreatment::TAXABLE,
            sgkTreatment: RetroSgkTreatment::NON_WAGE_PAYMENT_MONTH,
        },
    }
}

fn round2(value: Decimal) -> Decimal {
    value.round_dp(2)
}

fn validate_retro_carry_snapshot(records: &[DevredenPekKaydi], field: &str) -> Result<()> {
    if records
        .iter()
        .any(|record| record.tutar < Decimal::ZERO || record.kalanAySayisi < 0)
    {
        return Err(DomainError::InvalidData(format!(
            "{} negatif tutar veya negatif kalan ay içeriyor.",
            field
        )));
    }
    Ok(())
}

/// A revision value is an absolute target for the affected compensation
/// parameter.  When several revisions are effective on the same service day,
/// the later revision in signed/created/id order wins.  Keeping this rule in
/// the core prevents an add-on protocol from being accidentally composed as a
/// second percentage on top of the first agreement.
#[derive(Debug, Clone)]
struct RevisionApplication {
    revision: CompensationRevision,
    overrides: Vec<CompensationRevisionOverride>,
    effective_from: NaiveDate,
    effective_to: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
struct ReplaySegment {
    key: String,
    settings: DonemselKurumDegerleri,
    summary: PuantajOzeti,
    paid_sick_days: i32,
    days: i32,
}

fn parse_date(value: &str, field: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        DomainError::ValidationError(format!("{field} tarihi geçersiz: {value}: {error}"))
    })
}

fn period_date(period: &BordroDonemi, start: bool) -> Result<NaiveDate> {
    parse_date(
        if start {
            &period.baslangicTarihi
        } else {
            &period.bitisTarihi
        },
        &format!("{} dönemi", period.id),
    )
}

fn add_summary(summary: &mut PuantajOzeti, code: &str, period_id: &str) -> Result<()> {
    match code {
        "Ç" => summary.c += 1,
        "T" => summary.t += 1,
        "G" => summary.g += 1,
        "İ" => summary.i += 1,
        "GÇ" => summary.gc += 1,
        "GÇT" => summary.gct += 1,
        "R" => summary.r += 1,
        _ => {
            return Err(DomainError::InvalidData(format!(
                "{} döneminde desteklenmeyen puantaj kodu: {}",
                period_id, code
            )))
        }
    }
    Ok(())
}

fn add_income(target: &mut GelirKalemleri, source: &GelirKalemleri) {
    fn add(target: &mut Option<Decimal>, source: Option<Decimal>) {
        if let Some(value) = source {
            *target = Some(round2(target.unwrap_or_default() + value));
        }
    }
    add(&mut target.tabanBrutAylik, source.tabanBrutAylik);
    add(&mut target.tediye, source.tediye);
    add(&mut target.tisIkramiyesi, source.tisIkramiyesi);
    add(&mut target.ekOdeme, source.ekOdeme);
    add(&mut target.yemek, source.yemek);
    add(
        &mut target.birlestirilmisSosyalYardim,
        source.birlestirilmisSosyalYardim,
    );
    add(&mut target.vasitaYol, source.vasitaYol);
    add(&mut target.giyimYardimi, source.giyimYardimi);
    add(&mut target.isPrimi, source.isPrimi);
    add(&mut target.geceCalismasiUcreti, source.geceCalismasiUcreti);
    add(
        &mut target.geceCalismasiTatiliUcreti,
        source.geceCalismasiTatiliUcreti,
    );
    add(&mut target.hizmetZammi, source.hizmetZammi);
    add(&mut target.digerGelir, source.digerGelir);
}

fn code_value(income: &GelirKalemleri, code: RetroEarningCode) -> Decimal {
    match code {
        RetroEarningCode::BASE_WAGE => income.tabanBrutAylik.unwrap_or_default(),
        RetroEarningCode::NIGHT_WORK => income.geceCalismasiUcreti.unwrap_or_default(),
        RetroEarningCode::NIGHT_HOLIDAY => income.geceCalismasiTatiliUcreti.unwrap_or_default(),
        RetroEarningCode::WORK_PREMIUM => income.isPrimi.unwrap_or_default(),
        RetroEarningCode::SOCIAL_AID => income.birlestirilmisSosyalYardim.unwrap_or_default(),
        RetroEarningCode::MEAL => income.yemek.unwrap_or_default(),
        RetroEarningCode::TRANSPORT => income.vasitaYol.unwrap_or_default(),
        RetroEarningCode::CLOTHING => income.giyimYardimi.unwrap_or_default(),
        RetroEarningCode::SERVICE_INCREMENT => income.hizmetZammi.unwrap_or_default(),
        RetroEarningCode::TIS_BONUS => income.tisIkramiyesi.unwrap_or_default(),
        RetroEarningCode::TEDIYE => income.tediye.unwrap_or_default(),
        RetroEarningCode::SUPPLEMENTAL => income.ekOdeme.unwrap_or_default(),
        RetroEarningCode::OTHER => income.digerGelir.unwrap_or_default(),
    }
}

fn add_code(map: &mut HashMap<RetroEarningCode, Decimal>, code: RetroEarningCode, value: Decimal) {
    if value == Decimal::ZERO {
        return;
    }
    let entry = map.entry(code).or_default();
    *entry = round2(*entry + value);
}

fn income_by_code(income: &GelirKalemleri) -> HashMap<RetroEarningCode, Decimal> {
    let mut result = HashMap::new();
    for code in [
        RetroEarningCode::BASE_WAGE,
        RetroEarningCode::NIGHT_WORK,
        RetroEarningCode::NIGHT_HOLIDAY,
        RetroEarningCode::WORK_PREMIUM,
        RetroEarningCode::SOCIAL_AID,
        RetroEarningCode::MEAL,
        RetroEarningCode::TRANSPORT,
        RetroEarningCode::CLOTHING,
        RetroEarningCode::SERVICE_INCREMENT,
        RetroEarningCode::TIS_BONUS,
        RetroEarningCode::TEDIYE,
        RetroEarningCode::SUPPLEMENTAL,
        RetroEarningCode::OTHER,
    ] {
        add_code(&mut result, code, code_value(income, code));
    }
    result
}

fn set_code(income: &mut GelirKalemleri, code: RetroEarningCode, value: Decimal) {
    let value = round2(value);
    let target = match code {
        RetroEarningCode::BASE_WAGE => &mut income.tabanBrutAylik,
        RetroEarningCode::NIGHT_WORK => &mut income.geceCalismasiUcreti,
        RetroEarningCode::NIGHT_HOLIDAY => &mut income.geceCalismasiTatiliUcreti,
        RetroEarningCode::WORK_PREMIUM => &mut income.isPrimi,
        RetroEarningCode::SOCIAL_AID => &mut income.birlestirilmisSosyalYardim,
        RetroEarningCode::MEAL => &mut income.yemek,
        RetroEarningCode::TRANSPORT => &mut income.vasitaYol,
        RetroEarningCode::CLOTHING => &mut income.giyimYardimi,
        RetroEarningCode::SERVICE_INCREMENT => &mut income.hizmetZammi,
        RetroEarningCode::TIS_BONUS => &mut income.tisIkramiyesi,
        RetroEarningCode::TEDIYE => &mut income.tediye,
        RetroEarningCode::SUPPLEMENTAL => &mut income.ekOdeme,
        RetroEarningCode::OTHER => &mut income.digerGelir,
    };
    *target = Some(value);
}

fn selected_override<'a>(
    overrides: &'a [CompensationRevisionOverride],
    revision_id: &str,
    personnel_id: &str,
    parameter: RetroParameterKey,
) -> Result<Option<&'a CompensationRevisionOverride>> {
    let mut specific = overrides.iter().filter(|item| {
        item.revisionId == revision_id
            && item.parameter == parameter
            && item.personnelId.as_deref() == Some(personnel_id)
    });
    let found_specific = specific.next();
    if specific.next().is_some() {
        return Err(DomainError::ValidationError(format!(
            "{parameter:?} parametresi için personel bazında duplicate revision override var."
        )));
    }
    if found_specific.is_some() {
        return Ok(found_specific);
    }
    let mut global = overrides.iter().filter(|item| {
        item.revisionId == revision_id && item.parameter == parameter && item.personnelId.is_none()
    });
    let found_global = global.next();
    if global.next().is_some() {
        return Err(DomainError::ValidationError(format!(
            "{parameter:?} parametresi için duplicate revision override var."
        )));
    }
    Ok(found_global)
}

fn validate_override_value(key: RetroParameterKey, value: Decimal) -> Result<()> {
    if matches!(
        key,
        RetroParameterKey::TEDIYE | RetroParameterKey::TIS_BONUS
    ) {
        return Err(DomainError::ValidationError(format!(
            "{:?} revision override'ı event tarihini ve kısmi dönem geometrisini belirtmeden güvenli şekilde replay edilemez; ayrı ödeme event'i olarak tanımlanmalıdır.",
            key
        )));
    }
    if value < Decimal::ZERO {
        return Err(DomainError::ValidationError(format!(
            "{:?} revision değeri negatif olamaz.",
            key
        )));
    }
    if matches!(
        key,
        RetroParameterKey::IS_PRIMI_YUZDE
            | RetroParameterKey::GECE_CALISMA_PRIMI_YUZDE
            | RetroParameterKey::GECE_CALISMA_TATILI_PRIMI_YUZDE
    ) && value > dec!(100)
    {
        return Err(DomainError::ValidationError(format!(
            "{:?} revision oranı %100'ü aşamaz.",
            key
        )));
    }
    Ok(())
}

fn apply_compensation_overrides(
    settings: &mut DonemselKurumDegerleri,
    revision_id: &str,
    personnel: &Personel,
    overrides: &[CompensationRevisionOverride],
) -> Result<()> {
    let scalar_keys = [
        RetroParameterKey::GUNLUK_TABAN_UCRET,
        RetroParameterKey::GUNLUK_YEMEK,
        RetroParameterKey::BIRLESTIRILMIS_SOSYAL_YARDIM,
        RetroParameterKey::GUNLUK_VASITA_YOL,
        RetroParameterKey::GIYIM_YARDIMI,
        RetroParameterKey::HIZMET_ZAMMI_BIRIMI,
        RetroParameterKey::IS_PRIMI_YUZDE,
        RetroParameterKey::GECE_CALISMA_PRIMI_YUZDE,
        RetroParameterKey::GECE_CALISMA_TATILI_PRIMI_YUZDE,
        RetroParameterKey::EK_ODEME,
        RetroParameterKey::DIGER_GELIR,
    ];
    for key in scalar_keys {
        let Some(item) = selected_override(overrides, revision_id, &personnel.id, key)? else {
            continue;
        };
        validate_override_value(key, item.value)?;
        match key {
            RetroParameterKey::GUNLUK_TABAN_UCRET => settings.gunlukTabanUcret = item.value,
            RetroParameterKey::GUNLUK_YEMEK => settings.gunlukYemek = item.value,
            RetroParameterKey::BIRLESTIRILMIS_SOSYAL_YARDIM => {
                settings.birlestirilmisSosyalYardim = item.value
            }
            RetroParameterKey::GUNLUK_VASITA_YOL => settings.gunlukVasitaYol = item.value,
            RetroParameterKey::GIYIM_YARDIMI => settings.giyimYardimi = item.value,
            RetroParameterKey::HIZMET_ZAMMI_BIRIMI => settings.hizmetZammiBirimi = item.value,
            RetroParameterKey::IS_PRIMI_YUZDE => {
                let groups = settings.isPrimiGruplari.as_mut().ok_or_else(|| {
                    DomainError::ValidationError(
                        "İş primi revision'ı için tarihsel iş primi grupları eksik.".into(),
                    )
                })?;
                let group = groups
                    .iter_mut()
                    .find(|group| {
                        (group.id == personnel.grup || group.ad == personnel.grup) && group.aktif
                    })
                    .ok_or_else(|| {
                        DomainError::ValidationError(format!(
                            "{} personeli için tarihsel iş primi grubu bulunamadı.",
                            personnel.id
                        ))
                    })?;
                group.oran = item.value;
            }
            RetroParameterKey::GECE_CALISMA_PRIMI_YUZDE => {
                settings.geceCalismaPrimiYuzde = Some(item.value)
            }
            RetroParameterKey::GECE_CALISMA_TATILI_PRIMI_YUZDE => {
                settings.geceCalismaTatiliPrimiYuzde = Some(item.value)
            }
            RetroParameterKey::EK_ODEME => settings.ekOdeme = Some(item.value),
            RetroParameterKey::DIGER_GELIR => settings.digerGelirVarsayilan = Some(item.value),
            RetroParameterKey::TEDIYE | RetroParameterKey::TIS_BONUS => unreachable!(),
        }
    }
    Ok(())
}

fn scope_matches_person(revision: &CompensationRevision, personnel: &Personel) -> bool {
    match revision.scope {
        CompensationRevisionScope::ALL_PERSONNEL => true,
        CompensationRevisionScope::SELECTED_PERSONNEL => {
            revision.personnelIds.iter().any(|id| id == &personnel.id)
        }
        CompensationRevisionScope::PERSONNEL_GROUP => revision
            .personnelGroup
            .as_deref()
            .is_some_and(|group| group == personnel.grup),
    }
}

fn revision_has_authoritative_batch(dataset: &PayrollDatasetSnapshot, revision_id: &str) -> bool {
    dataset.retroBatches.iter().any(|batch| {
        batch.revisionId == revision_id
            && matches!(
                batch.status,
                CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
            )
    })
}

fn revision_applications(
    dataset: &PayrollDatasetSnapshot,
    current_revision: &CompensationRevision,
    current_overrides: &[CompensationRevisionOverride],
    personnel: &Personel,
) -> Result<Vec<RevisionApplication>> {
    let mut applications = Vec::new();

    for revision in &dataset.compensationRevisions {
        if revision.id == current_revision.id
            || revision.status == CompensationRevisionStatus::STALE
            || !scope_matches_person(revision, personnel)
            || (!matches!(
                revision.status,
                CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
            ) && !revision_has_authoritative_batch(dataset, &revision.id))
        {
            continue;
        }
        let effective_from = parse_date(&revision.effectiveFrom, "revision yürürlük")?;
        let effective_to = revision
            .effectiveTo
            .as_deref()
            .map(|value| parse_date(value, "revision bitiş"))
            .transpose()?;
        if effective_to.is_some_and(|end| end < effective_from) {
            return Err(DomainError::ValidationError(format!(
                "{} revision bitiş tarihi yürürlük tarihinden önce olamaz.",
                revision.id
            )));
        }
        let overrides = dataset
            .compensationRevisionOverrides
            .iter()
            .filter(|item| item.revisionId == revision.id)
            .cloned()
            .collect::<Vec<_>>();
        for item in &overrides {
            validate_override_value(item.parameter, item.value)?;
        }
        applications.push(RevisionApplication {
            revision: revision.clone(),
            overrides,
            effective_from,
            effective_to,
        });
    }

    let effective_from = parse_date(&current_revision.effectiveFrom, "revision yürürlük")?;
    let effective_to = current_revision
        .effectiveTo
        .as_deref()
        .map(|value| parse_date(value, "revision bitiş"))
        .transpose()?;
    if effective_to.is_some_and(|end| end < effective_from) {
        return Err(DomainError::ValidationError(
            "Revision bitiş tarihi yürürlük tarihinden önce olamaz.".into(),
        ));
    }
    for item in current_overrides {
        validate_override_value(item.parameter, item.value)?;
    }
    applications.push(RevisionApplication {
        revision: current_revision.clone(),
        overrides: current_overrides.to_vec(),
        effective_from,
        effective_to,
    });

    applications.sort_by(|left, right| {
        left.effective_from
            .cmp(&right.effective_from)
            .then_with(|| {
                left.revision
                    .signedAt
                    .as_deref()
                    .unwrap_or("")
                    .cmp(right.revision.signedAt.as_deref().unwrap_or(""))
            })
            .then_with(|| {
                left.revision
                    .updatedAt
                    .as_deref()
                    .unwrap_or("")
                    .cmp(right.revision.updatedAt.as_deref().unwrap_or(""))
            })
            .then_with(|| left.revision.id.cmp(&right.revision.id))
    });
    Ok(applications)
}

fn settings_for_replay_date(
    historical_settings: &DonemselKurumDegerleri,
    personnel: &Personel,
    date: NaiveDate,
    applications: &[RevisionApplication],
) -> Result<DonemselKurumDegerleri> {
    let mut settings = historical_settings.clone();
    for application in applications.iter().filter(|application| {
        date >= application.effective_from
            && application
                .effective_to
                .is_none_or(|effective_to| date <= effective_to)
    }) {
        apply_compensation_overrides(
            &mut settings,
            &application.revision.id,
            personnel,
            &application.overrides,
        )?;
    }
    crate::calculations::validate_kurum_degerleri_for_payroll(&settings)?;
    Ok(settings)
}

fn override_value(
    overrides: &[CompensationRevisionOverride],
    revision_id: &str,
    personnel_id: &str,
    key: RetroParameterKey,
) -> Result<Option<Decimal>> {
    Ok(selected_override(overrides, revision_id, personnel_id, key)?.map(|item| item.value))
}

fn incrementally_add_paid_sick_wage(
    income: &mut GelirKalemleri,
    paid_days: i32,
    daily_wage: Decimal,
) {
    if paid_days <= 0 {
        return;
    }
    let current = income.tabanBrutAylik.unwrap_or_default();
    income.tabanBrutAylik = Some(round2(
        current + round2(daily_wage * Decimal::from(paid_days)),
    ));
}

fn calculate_segment_income(
    summary: &PuantajOzeti,
    paid_sick_days: i32,
    settings: &DonemselKurumDegerleri,
    personnel: &Personel,
    fixed_weight: Decimal,
) -> Result<GelirKalemleri> {
    let (mut income, _) =
        calculate_gunluk_gelirler_from_puantaj(summary, settings, Some(&personnel.grup))?;
    incrementally_add_paid_sick_wage(&mut income, paid_sick_days, settings.gunlukTabanUcret);
    income.birlestirilmisSosyalYardim =
        Some(round2(settings.birlestirilmisSosyalYardim * fixed_weight));
    income.giyimYardimi = Some(round2(settings.giyimYardimi * fixed_weight));
    income.hizmetZammi = Some(round2(
        Decimal::from(personnel.hizmetYili) * settings.hizmetZammiBirimi * fixed_weight,
    ));
    income.ekOdeme = settings.ekOdeme.map(|value| round2(value * fixed_weight));
    income.digerGelir = settings
        .digerGelirVarsayilan
        .map(|value| round2(value * fixed_weight));
    Ok(income)
}

fn historical_attendance<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
) -> Result<&'a PersonelPuantaj> {
    dataset
        .attendances
        .iter()
        .find(|attendance| attendance.personelId == personnel_id && attendance.donemId == period_id)
        .ok_or_else(|| {
            DomainError::NotFound(format!(
                "{} / {} için tarihsel puantaj bulunamadı.",
                personnel_id, period_id
            ))
        })
}

fn target_income_for_period(
    dataset: &PayrollDatasetSnapshot,
    personnel: &Personel,
    period: &BordroDonemi,
    payment_date: NaiveDate,
    applications: &[RevisionApplication],
) -> Result<GelirKalemleri> {
    let attendance = historical_attendance(dataset, &personnel.id, &period.id)?;
    let start = period_date(period, true)?;
    let period_end = period_date(period, false)?;
    let covered_end = period_end.min(payment_date);
    if covered_end < start {
        return Ok(GelirKalemleri::default());
    }
    let sick_records: Vec<SickLeaveRecord> = dataset
        .sickLeaveRecords
        .iter()
        .filter(|record| record.personnelId == personnel.id)
        .cloned()
        .collect();
    let paid_sick_dates = calculate_paid_sick_dates_from_records(&sick_records, period);

    let historical_settings = dataset
        .institutionSettings
        .get(&period.id)
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi tarihsel kurum ayarları bulunamadı.",
                period.id
            ))
        })?
        .clone();
    crate::calculations::validate_kurum_degerleri_for_payroll(&historical_settings)?;
    let mut segments: Vec<ReplaySegment> = Vec::new();
    let mut current = start;
    while current <= covered_end {
        let date_text = current.format("%Y-%m-%d").to_string();
        let code = attendance.gunler.get(&date_text).ok_or_else(|| {
            DomainError::ValidationError(format!(
                "{} dönemi tarihsel puantajı eksik: {} günü bulunamadı.",
                period.id, date_text
            ))
        })?;
        let active_revision_ids = applications
            .iter()
            .filter(|application| {
                current >= application.effective_from
                    && application
                        .effective_to
                        .is_none_or(|effective_to| current <= effective_to)
            })
            .map(|application| application.revision.id.as_str())
            .collect::<Vec<_>>()
            .join("|");
        let segment_index = if let Some(index) = segments
            .iter()
            .position(|segment| segment.key == active_revision_ids)
        {
            index
        } else {
            let settings =
                settings_for_replay_date(&historical_settings, personnel, current, applications)?;
            segments.push(ReplaySegment {
                key: active_revision_ids,
                settings,
                summary: PuantajOzeti::default(),
                paid_sick_days: 0,
                days: 0,
            });
            segments.len() - 1
        };
        let segment = &mut segments[segment_index];
        add_summary(&mut segment.summary, code, &period.id)?;
        segment.days += 1;
        if code == "R" && paid_sick_dates.contains(&current) {
            segment.paid_sick_days += 1;
        }
        current = current
            .checked_add_signed(Duration::days(1))
            .ok_or_else(|| DomainError::InvalidData("Retro puantaj tarihi taşması.".into()))?;
    }

    let total_weight_days = segments.iter().map(|segment| segment.days).sum::<i32>();
    if total_weight_days <= 0 {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi için retro hesaplanabilir hizmet günü bulunamadı.",
            period.id
        )));
    }
    let mut target = GelirKalemleri::default();
    for segment in segments {
        if segment.days > 0 {
            let weight = Decimal::from(segment.days) / Decimal::from(total_weight_days);
            let segment_income = calculate_segment_income(
                &segment.summary,
                segment.paid_sick_days,
                &segment.settings,
                personnel,
                weight,
            )?;
            add_income(&mut target, &segment_income);
        }
    }
    // Existing Tediye/TİS values are event-specific and are not generated by
    // the normal attendance formula. Their target is carried forward unless
    // this revision explicitly supplies a replacement value.
    Ok(target)
}

fn ensure_revision_scope(revision: &CompensationRevision, personnel: &Personel) -> Result<()> {
    match revision.scope {
        CompensationRevisionScope::ALL_PERSONNEL => Ok(()),
        CompensationRevisionScope::SELECTED_PERSONNEL => {
            if revision.personnelIds.iter().any(|id| id == &personnel.id) {
                Ok(())
            } else {
                Err(DomainError::ValidationError(format!(
                    "{} personeli revision kapsamı dışında.",
                    personnel.id
                )))
            }
        }
        CompensationRevisionScope::PERSONNEL_GROUP => {
            if revision.personnelGroup.as_deref() == Some(personnel.grup.as_str()) {
                Ok(())
            } else {
                Err(DomainError::ValidationError(format!(
                    "{} personeli revision personel grubu kapsamı dışında.",
                    personnel.id
                )))
            }
        }
    }
}

fn original_recognized_by_period_and_code(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
) -> Result<HashMap<RetroEarningCode, Decimal>> {
    let mut result = HashMap::new();
    for payroll in dataset.payrolls.iter().filter(|payroll| {
        payroll.personelId == personnel_id
            && payroll.donemId == period_id
            && payroll.accrualType != AccrualType::RETRO_ADJUSTMENT
    }) {
        match payroll.status {
            BordroStatus::CALCULATED | BordroStatus::FINALIZED => {
                for (code, value) in income_by_code(&payroll.gelirler) {
                    add_code(&mut result, code, value);
                }
            }
            BordroStatus::DRAFT | BordroStatus::STALE => {
                return Err(DomainError::ValidationError(format!(
                    "{} dönemindeki {} tahakkuku authoritative değil ({:?}); retro hesap durduruldu.",
                    period_id,
                    payroll.accrualId,
                    payroll.status
                )))
            }
        }
    }
    Ok(result)
}

/// V4 rows predate the explicit settlement-flow columns.  A zeroed flow is
/// therefore interpreted as legacy only for compatibility; newly calculated
/// batches always write all five batch flow fields explicitly.
fn has_legacy_settlement_flow(batch: &RetroAdjustmentBatch) -> bool {
    batch.payableSettlementAmount == Decimal::ZERO
        && batch.offsetSettlementAmount == Decimal::ZERO
        && batch.recoveredAmount == Decimal::ZERO
        && batch.recoverableAmount == Decimal::ZERO
        && batch.outstandingReceivable == Decimal::ZERO
        && batch.settlementStatus != RetroSettlementStatus::SETTLED_BY_OFFSET
}

/// Returns the gross amount that is actually eligible for a cash payment
/// event.  The legacy fallback is intentionally isolated here so an old V4
/// positive PAID/UNSETTLED batch remains importable without making a new
/// offset-only batch payable by accident.
pub fn retro_payable_settlement_amount(batch: &RetroAdjustmentBatch) -> Decimal {
    if batch.payableSettlementAmount > Decimal::ZERO {
        batch.payableSettlementAmount
    } else if has_legacy_settlement_flow(batch)
        && batch.totalGrossDelta > Decimal::ZERO
        && batch.settlementStatus != RetroSettlementStatus::OVERPAYMENT
    {
        batch.totalGrossDelta
    } else {
        Decimal::ZERO
    }
}

fn negative_entitlement_amount(batch: &RetroAdjustmentBatch) -> Decimal {
    // Legacy V4 rows have no settlement snapshot.  Their receivable is the
    // batch's signed net entitlement, not the sum of negative allocations:
    // mixed-sign allocations are one net correction and must not invent a
    // second receivable beside the payable side.
    (-batch.totalGrossDelta).max(Decimal::ZERO)
}

fn effective_allocation_payable(
    batch: &RetroAdjustmentBatch,
    allocation: &RetroAllocation,
) -> Decimal {
    if !has_legacy_settlement_flow(batch) {
        allocation.payableSettlementAmount
    } else {
        allocation.deltaAmount.max(Decimal::ZERO)
    }
}

pub fn retro_payable_allocation_amount(
    batch: &RetroAdjustmentBatch,
    allocation: &RetroAllocation,
) -> Decimal {
    if has_legacy_settlement_flow(batch) {
        // `retro_payment_income` materializes the deterministic legacy flow on
        // its local allocation snapshot before this helper is consumed by GV
        // and DV calculations.  An un-normalized V4 allocation deliberately
        // returns zero here rather than risking a cash overstatement; callers
        // must enter through `retro_payment_income`.
        allocation.payableSettlementAmount
    } else {
        effective_allocation_payable(batch, allocation)
    }
}

fn validate_batch_ledger(
    dataset: &PayrollDatasetSnapshot,
    batch: &RetroAdjustmentBatch,
) -> Result<Vec<RetroAllocation>> {
    let allocations: Vec<RetroAllocation> = dataset
        .retroAllocations
        .iter()
        .filter(|allocation| allocation.batchId == batch.id)
        .cloned()
        .collect();
    let total = allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
        sum + allocation.deltaAmount
    });
    if round2(total) != round2(batch.totalGrossDelta) {
        return Err(DomainError::InvalidData(format!(
            "{} retro batch'inde allocation toplamı batch toplamıyla eşleşmiyor.",
            batch.id
        )));
    }
    if allocations.is_empty() && batch.totalGrossDelta != Decimal::ZERO {
        return Err(DomainError::InvalidData(format!(
            "{} retro batch'inde toplam delta var ancak allocation bulunmuyor.",
            batch.id
        )));
    }
    let has_negative_delta = batch.totalGrossDelta < Decimal::ZERO;
    if has_negative_delta && batch.status == CompensationRevisionStatus::FINALIZED {
        return Err(DomainError::InvalidData(format!(
            "{} negatif retro batch'i FINALIZED olamaz; OVERPAYMENT settlement'ı açık kalmalıdır.",
            batch.id
        )));
    }
    let expected_settlement = if has_negative_delta {
        RetroSettlementStatus::OVERPAYMENT
    } else if batch.status == CompensationRevisionStatus::FINALIZED {
        RetroSettlementStatus::PAID
    } else if batch.settlementStatus == RetroSettlementStatus::SETTLED_BY_OFFSET
        && batch.offsetSettlementAmount > Decimal::ZERO
        && retro_payable_settlement_amount(batch) == Decimal::ZERO
    {
        RetroSettlementStatus::SETTLED_BY_OFFSET
    } else {
        RetroSettlementStatus::UNSETTLED
    };
    if batch.settlementStatus != expected_settlement {
        return Err(DomainError::InvalidData(format!(
            "{} retro batch settlement statusı tutarsız; beklenen {:?}.",
            batch.id, expected_settlement
        )));
    }
    let mut allocation_ids = HashSet::new();
    let mut allocation_keys = HashSet::new();
    for allocation in &allocations {
        if allocation.personnelId != batch.personnelId {
            return Err(DomainError::InvalidData(format!(
                "{} allocation personel/batch ilişkisi bozuk.",
                allocation.id
            )));
        }
        if !allocation_ids.insert(allocation.id.clone()) {
            return Err(DomainError::InvalidData(format!(
                "{} retro batch'inde duplicate allocation kimliği bulundu.",
                batch.id
            )));
        }
        if !allocation_keys.insert((allocation.sourcePeriodId.clone(), allocation.earningCode)) {
            return Err(DomainError::InvalidData(format!(
                "{} retro batch'inde aynı source period/earning code birden fazla kez bulundu.",
                batch.id
            )));
        }
        if !dataset
            .periods
            .iter()
            .any(|period| period.id == allocation.sourcePeriodId)
        {
            return Err(DomainError::InvalidData(format!(
                "{} allocation'ı bilinmeyen source period'a bağlı.",
                allocation.id
            )));
        }
        let policy = retro_earning_policy(allocation.earningCode);
        if allocation.sgkTreatment != policy.sgkTreatment
            || allocation.incomeTaxTreatment != policy.incomeTaxTreatment
            || allocation.stampTaxTreatment != policy.stampTaxTreatment
        {
            return Err(DomainError::InvalidData(format!(
                "{} allocation'ının earning policy snapshot'ı registry ile eşleşmiyor.",
                allocation.id
            )));
        }
        if round2(
            allocation.targetAmount
                - allocation.originalRecognizedAmount
                - allocation.previousAuthoritativeRetroAmount,
        ) != round2(allocation.deltaAmount)
        {
            return Err(DomainError::InvalidData(format!(
                "{} allocation'ında target - recognized ledger hesabı delta ile eşleşmiyor.",
                allocation.id
            )));
        }
        // A correction/overpayment ledger is signed. Only the immutable
        // entitlement and balance bases must remain non-negative; previous
        // recognized deltas and all incremental SGK fields may legitimately
        // be negative when an overpayment is being reversed.
        if allocation.originalRecognizedAmount < Decimal::ZERO
            || allocation.targetAmount < Decimal::ZERO
            || allocation.originalPek < Decimal::ZERO
            || allocation.adjustedPek < Decimal::ZERO
            || allocation.originalEmployerLowerBound < Decimal::ZERO
            || allocation.targetEmployerLowerBound < Decimal::ZERO
            || allocation.payableSettlementAmount < Decimal::ZERO
            || allocation.offsetSettlementAmount < Decimal::ZERO
            || allocation.recoverableAmount < Decimal::ZERO
        {
            return Err(DomainError::InvalidData(format!(
                "{} allocation'ında negatif authoritative ledger alanı bulundu.",
                allocation.id
            )));
        }
        if let Some(carry) = &allocation.originalSourceCarry {
            validate_retro_carry_snapshot(
                carry,
                &format!("{} original source carry", allocation.id),
            )?;
        }
        if let Some(carry) = &allocation.targetSourceCarry {
            validate_retro_carry_snapshot(
                carry,
                &format!("{} target source carry", allocation.id),
            )?;
        }
        if !has_legacy_settlement_flow(batch) {
            let positive_delta = allocation.deltaAmount.max(Decimal::ZERO);
            let negative_delta = (-allocation.deltaAmount).max(Decimal::ZERO);
            if round2(
                allocation.payableSettlementAmount + allocation.offsetSettlementAmount,
            ) > round2(positive_delta)
                || round2(allocation.recoverableAmount) > round2(negative_delta)
            {
                return Err(DomainError::InvalidData(format!(
                    "{} allocation settlement akışı signed entitlement delta sınırını aşamaz.",
                    allocation.id
                )));
            }
        }
    }

    if !has_legacy_settlement_flow(batch) {
        let allocation_payable = allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
            sum + allocation.payableSettlementAmount
        });
        let allocation_offset = allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
            sum + allocation.offsetSettlementAmount
        });
        let allocation_recoverable = allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
            sum + allocation.recoverableAmount
        });
        if round2(allocation_payable) != round2(batch.payableSettlementAmount)
            || round2(allocation_offset) != round2(batch.offsetSettlementAmount)
            || round2(allocation_recoverable) != round2(batch.recoverableAmount)
        {
            return Err(DomainError::InvalidData(format!(
                "{} settlement allocation toplamı batch settlement akışlarıyla eşleşmiyor.",
                batch.id
            )));
        }
        if batch.totalGrossDelta >= Decimal::ZERO
            && round2(batch.payableSettlementAmount + batch.offsetSettlementAmount)
                != round2(batch.totalGrossDelta)
        {
            return Err(DomainError::InvalidData(format!(
                "{} pozitif entitlement delta'sı payable/offset settlement toplamıyla eşleşmiyor.",
                batch.id
            )));
        }
        if batch.totalGrossDelta >= Decimal::ZERO
            && batch.recoverableAmount != Decimal::ZERO
        {
            return Err(DomainError::InvalidData(format!(
                "{} pozitif entitlement delta'sı recoverable settlement üretemez.",
                batch.id
            )));
        }
        if batch.totalGrossDelta < Decimal::ZERO
            && round2(batch.recoverableAmount) != round2(-batch.totalGrossDelta)
        {
            return Err(DomainError::InvalidData(format!(
                "{} negatif entitlement delta'sı recoverable settlement ile eşleşmiyor.",
                batch.id
            )));
        }
        if batch.totalGrossDelta < Decimal::ZERO
            && (batch.payableSettlementAmount != Decimal::ZERO
                || batch.offsetSettlementAmount != Decimal::ZERO)
        {
            return Err(DomainError::InvalidData(format!(
                "{} negatif entitlement delta'sı payable veya offset settlement üretemez.",
                batch.id
            )));
        }
        if batch.outstandingReceivable < Decimal::ZERO || batch.recoveredAmount < Decimal::ZERO {
            return Err(DomainError::InvalidData(format!(
                "{} settlement bakiyesi negatif olamaz.",
                batch.id
            )));
        }
    }
    if batch.status == CompensationRevisionStatus::FINALIZED
        && retro_payable_settlement_amount(batch) <= Decimal::ZERO
    {
        return Err(DomainError::InvalidData(format!(
            "{} payable settlement olmadan FINALIZED olamaz.",
            batch.id
        )));
    }
    Ok(allocations)
}

fn outstanding_receivable_before_current(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
) -> Result<Decimal> {
    let mut batches = dataset
        .retroBatches
        .iter()
        .filter(|batch| {
            batch.personnelId == personnel_id
                && batch.id != current_batch_id
                && matches!(
                    batch.status,
                    CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    batches.sort_by(|left, right| {
        left.paymentDate
            .cmp(&right.paymentDate)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut outstanding = Decimal::ZERO;
    for batch in batches {
        let batch_payment_date = parse_date(&batch.paymentDate, "önceki retro ödeme")?;
        if batch_payment_date > payment_date {
            continue;
        }
        validate_batch_ledger(dataset, &batch)?;
        let legacy = has_legacy_settlement_flow(&batch);
        let recoverable = if legacy {
            negative_entitlement_amount(&batch)
        } else {
            batch.recoverableAmount
        };
        let offset = if legacy {
            Decimal::ZERO
        } else {
            batch.offsetSettlementAmount
        };
        let recovered = if legacy {
            Decimal::ZERO
        } else {
            batch.recoveredAmount
        };
        let available = outstanding + recoverable;
        if offset + recovered > available {
            return Err(DomainError::InvalidData(format!(
                "{} settlement mahsup/tahsil akışı açık receivable bakiyesini aşıyor.",
                batch.id
            )));
        }
        outstanding = round2(available - offset - recovered);
        if !legacy && round2(batch.outstandingReceivable) != round2(outstanding) {
            return Err(DomainError::InvalidData(format!(
                "{} outstanding receivable snapshot'ı kronolojik settlement replay ile eşleşmiyor.",
                batch.id
            )));
        }
    }
    Ok(outstanding)
}

fn assign_current_settlement_allocations(
    allocations: &mut [RetroAllocation],
    payable: Decimal,
    offset: Decimal,
    recoverable: Decimal,
) {
    let mut indices = (0..allocations.len()).collect::<Vec<_>>();
    indices.sort_by_key(|index| {
        (
            allocations[*index].sourcePeriodId.clone(),
            allocations[*index].earningCode,
            allocations[*index].id.clone(),
        )
    });

    let mut remaining_payable = payable;
    let mut remaining_offset = offset;
    for index in &indices {
        let amount = allocations[*index].deltaAmount.max(Decimal::ZERO);
        let applied_offset = amount.min(remaining_offset);
        remaining_offset = round2(remaining_offset - applied_offset);
        let applied_payable = (amount - applied_offset).min(remaining_payable);
        remaining_payable = round2(remaining_payable - applied_payable);
        allocations[*index].offsetSettlementAmount = round2(applied_offset);
        allocations[*index].payableSettlementAmount = round2(applied_payable);
    }

    let mut remaining_recoverable = recoverable;
    for index in &indices {
        let amount = (-allocations[*index].deltaAmount).max(Decimal::ZERO);
        let applied = amount.min(remaining_recoverable);
        remaining_recoverable = round2(remaining_recoverable - applied);
        allocations[*index].recoverableAmount = round2(applied);
    }
}

fn previous_authoritative_retro_by_period_and_code(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
) -> Result<(RetroAmountsByPeriodAndCode, RetroAmountsByPeriodAndCode)> {
    let mut previous = HashMap::new();
    let mut previous_pek = HashMap::new();
    let mut batches = dataset
        .retroBatches
        .iter()
        .filter(|batch| batch.personnelId == personnel_id && batch.id != current_batch_id)
        .cloned()
        .collect::<Vec<_>>();
    batches.sort_by(|left, right| {
        left.paymentDate
            .cmp(&right.paymentDate)
            .then_with(|| left.id.cmp(&right.id))
    });
    for batch in batches {
        if matches!(
            batch.status,
            CompensationRevisionStatus::DRAFT | CompensationRevisionStatus::STALE
        ) {
            // Draft and stale calculations are retained for audit/reproducibility
            // but are not authoritative recognized entitlement.
            continue;
        }
        let batch_payment_date = parse_date(&batch.paymentDate, "önceki retro ödeme")?;
        if batch_payment_date > payment_date {
            continue;
        }
        for allocation in validate_batch_ledger(dataset, &batch)? {
            let key = (allocation.sourcePeriodId.clone(), allocation.earningCode);
            let entry = previous.entry(key.clone()).or_default();
            *entry = round2(*entry + allocation.deltaAmount);
            let pek_entry = previous_pek.entry(key).or_default();
            *pek_entry = round2(*pek_entry + allocation.retroPekDelta);
        }
    }
    Ok((previous, previous_pek))
}

fn reject_later_authoritative_retro_for_same_source_period(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
    source_period_ids: &HashSet<String>,
) -> Result<()> {
    for batch in dataset.retroBatches.iter().filter(|batch| {
        batch.personnelId == personnel_id
            && batch.id != current_batch_id
            && matches!(
                batch.status,
                CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
            )
    }) {
        let batch_payment_date = parse_date(&batch.paymentDate, "önceki retro ödeme")?;
        if batch_payment_date <= payment_date {
            // A new retro payment is assigned the next canonical payment-event
            // sequence by the persistence service. The batch predates its
            // payment event and has no independent sequence, so a same-day
            // authoritative batch is recognized as an earlier appended event.
            // A later calendar-day event remains a hard blocker below because
            // date-only ordering cannot safely reconstruct an earlier payment.
            continue;
        }
        let allocations = validate_batch_ledger(dataset, batch)?;
        if allocations
            .iter()
            .any(|allocation| source_period_ids.contains(&allocation.sourcePeriodId))
        {
            return Err(DomainError::ValidationError(format!(
                "{} source period'i için {} tarihli authoritative retro batch varken {} tarihli daha erken retro ödeme hesaplanamaz; ödeme olaylarını kronolojik sırada düzeltin.",
                current_batch_id,
                batch.paymentDate,
                payment_date
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct SourcePekState {
    worker_pek: Decimal,
    worker_sgk: Decimal,
    worker_unemployment: Decimal,
    employer_sgk: Decimal,
    employer_unemployment: Decimal,
    employer_lower_bound: Decimal,
    employer_lower_bound_premium: Decimal,
    upper: Decimal,
}

#[derive(Debug, Clone, Default)]
struct SourcePekReplay {
    state: SourcePekState,
    /// Carry after the final authoritative source-period payment event.
    outgoing_carry: Vec<DevredenPekKaydi>,
}

fn source_statutory_snapshot(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<ResolvedStatutorySnapshot> {
    let attendance = historical_attendance(dataset, personnel_id, &period.id)?;
    let settings = dataset.institutionSettings.get(&period.id).ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} source period için historical kurum ayarları çözümlenemedi.",
            period.id
        ))
    })?;
    let sick_records = dataset
        .sickLeaveRecords
        .iter()
        .filter(|record| record.personnelId == personnel_id)
        .cloned()
        .collect::<Vec<_>>();
    let paid_sick_dates = calculate_paid_sick_dates_from_records(&sick_records, period);
    // A missing original payroll must use the same R-day rule as a normal
    // payroll. In particular, an attendance-backed R day is not automatically
    // a prim-bearing day; only the resolved paid sick dates are.
    resolve_statutory_snapshot_for_period_with_paid_sick_dates(
        attendance,
        period,
        settings,
        &paid_sick_dates,
    )
}

fn source_period_events(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<Vec<BordroKaydi>> {
    let mut events = dataset
        .payrolls
        .iter()
        .filter(|payroll| {
            payroll.personelId == personnel_id
                && payroll.donemId == period.id
                && payroll.accrualType != AccrualType::RETRO_ADJUSTMENT
        })
        .cloned()
        .collect::<Vec<_>>();
    for event in &events {
        ensure_authoritative_payment_event(event)?;
    }
    events.sort_by(|left, right| {
        accrual_order_for_payroll(dataset, left)
            .and_then(|left_order| {
                accrual_order_for_payroll(dataset, right)
                    .map(|right_order| left_order.cmp(&right_order))
            })
            .unwrap_or_else(|_| left.accrualId.cmp(&right.accrualId))
    });
    Ok(events)
}

fn attendance_summary_for_source(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<PuantajOzeti> {
    let attendance = historical_attendance(dataset, personnel_id, &period.id)?;
    let mut summary = PuantajOzeti::default();
    for code in attendance.gunler.values() {
        add_summary(&mut summary, code, &period.id)?;
    }
    Ok(summary)
}

fn source_income_target(
    original_normal_income: &GelirKalemleri,
    target_normal_income: &GelirKalemleri,
) -> GelirKalemleri {
    let mut result = original_normal_income.clone();
    for code in [
        RetroEarningCode::BASE_WAGE,
        RetroEarningCode::NIGHT_WORK,
        RetroEarningCode::NIGHT_HOLIDAY,
        RetroEarningCode::WORK_PREMIUM,
        RetroEarningCode::SOCIAL_AID,
        RetroEarningCode::MEAL,
        RetroEarningCode::TRANSPORT,
        RetroEarningCode::CLOTHING,
        RetroEarningCode::SERVICE_INCREMENT,
        RetroEarningCode::TIS_BONUS,
        RetroEarningCode::TEDIYE,
        RetroEarningCode::SUPPLEMENTAL,
        RetroEarningCode::OTHER,
    ] {
        if canonical_sgk_earning_class(code) == CanonicalSgkEarningClass::Wage {
            set_code(&mut result, code, code_value(target_normal_income, code));
        }
    }
    result
}

fn source_original_state(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    events: &[BordroKaydi],
) -> Result<SourcePekReplay> {
    let mut state = SourcePekState::default();
    for payroll in events {
        let detail = payroll.pekDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} tahakkukunda historical PEK snapshot'ı eksik.",
                payroll.accrualId
            ))
        })?;
        state.worker_pek = round2(state.worker_pek + detail.primMatrahi);
        state.employer_sgk =
            round_sgk_amount(state.employer_sgk + detail.isverenSgkPrimi.unwrap_or_default());
        state.employer_unemployment = round_sgk_amount(
            state.employer_unemployment + detail.isverenIssizlikPrimi.unwrap_or_default(),
        );
        state.employer_lower_bound =
            round2(state.employer_lower_bound + detail.altSinirTamamlamaFarki);
        state.employer_lower_bound_premium = round_sgk_amount(
            state.employer_lower_bound_premium
                + detail.pekAltSinirTamamlamaIsverenPrimi.unwrap_or_default(),
        );
        if state.upper != Decimal::ZERO && round2(state.upper) != round2(detail.pekUstSinir) {
            return Err(DomainError::InvalidData(format!(
                "{} source period PEK tavan snapshot'ları çelişkili.",
                period.id
            )));
        }
        state.upper = detail.pekUstSinir;
    }
    let settings = dataset.institutionSettings.get(&period.id).ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} source settings eksik.",
            period.id
        ))
    })?;
    let (employer_sgk, employer_unemployment, employer_lower_bound_premium) =
        canonical_source_employer_premiums(
            state.worker_pek,
            state.employer_lower_bound,
            settings,
        )?;
    state.employer_sgk = employer_sgk;
    state.employer_unemployment = employer_unemployment;
    state.employer_lower_bound_premium = employer_lower_bound_premium;
    if state.upper == Decimal::ZERO {
        state.upper = source_statutory_snapshot(dataset, personnel_id, period)?.pekUstSinir;
    }
    Ok(SourcePekReplay {
        state,
        outgoing_carry: events
            .last()
            .and_then(|event| event.sonrakiDevredenPek.clone())
            .unwrap_or_default(),
    })
}

fn canonical_source_employer_premiums(
    worker_pek: Decimal,
    employer_lower_bound: Decimal,
    settings: &DonemselKurumDegerleri,
) -> Result<(Decimal, Decimal, Decimal)> {
    let employer_sgk_rate = settings
        .sgkIsverenOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical SGK işveren oranı eksik.".into()))?
        / dec!(100);
    let employer_unemployment_rate = settings
        .issizlikIsverenOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical işveren işsizlik oranı eksik.".into()))?
        / dec!(100);
    let worker_sgk_rate = settings
        .sgkIsciOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical SGK işçi oranı eksik.".into()))?
        / dec!(100);
    let worker_unemployment_rate = settings
        .issizlikIsciOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical işsizlik işçi oranı eksik.".into()))?
        / dec!(100);
    let final_pek = round2((worker_pek + employer_lower_bound).max(Decimal::ZERO));
    let employer_sgk = round_sgk_amount(final_pek * employer_sgk_rate);
    let employer_unemployment = round_sgk_amount(final_pek * employer_unemployment_rate);
    let lower_bound_sgk = round_sgk_amount(
        employer_lower_bound.max(Decimal::ZERO) * worker_sgk_rate,
    );
    let lower_bound_unemployment = round_sgk_amount(
        employer_lower_bound.max(Decimal::ZERO) * worker_unemployment_rate,
    );
    Ok((
        employer_sgk,
        employer_unemployment,
        lower_bound_sgk + lower_bound_unemployment,
    ))
}

fn source_target_state(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    events: &[BordroKaydi],
    target_normal_income: &GelirKalemleri,
) -> Result<SourcePekReplay> {
    let settings = dataset
        .institutionSettings
        .get(&period.id)
        .ok_or_else(|| DomainError::InvalidData(format!("{} source settings eksik.", period.id)))?;
    let statutory_fallback = source_statutory_snapshot(dataset, personnel_id, period)?;
    let mut ordered_events = events.to_vec();
    let original_normal_income = events
        .iter()
        .find(|event| event.accrualType == AccrualType::NORMAL)
        .map(|event| event.gelirler.clone())
        .unwrap_or_default();
    if ordered_events
        .iter()
        .all(|event| event.accrualType != AccrualType::NORMAL)
    {
        // Missing original accrual: replay a synthetic NORMAL event using the
        // source attendance/statutory snapshot. This is also where paid sick
        // R-days enter the source PEK capacity.
        let synthetic_summary = attendance_summary_for_source(dataset, personnel_id, period)?;
        ordered_events.push(BordroKaydi {
            id: format!("{}_retro_source_normal", period.id),
            personelId: personnel_id.to_string(),
            donemId: period.id.clone(),
            accrualId: format!("{}_retro_source_normal", period.id),
            accrualType: AccrualType::NORMAL,
            paymentDate: period.bitisTarihi.clone(),
            sequence: -1,
            accrualDescription: None,
            puantajOzeti: synthetic_summary,
            gelirler: GelirKalemleri::default(),
            gelirToplam: Decimal::ZERO,
            kesintiler: KesintiKalemleri::default(),
            kesintiToplam: Decimal::ZERO,
            netOdeme: Decimal::ZERO,
            status: BordroStatus::CALCULATED,
            olusturulmaTarihi: String::new(),
            sonGuncellemeTarihi: String::new(),
            notlar: None,
            oncekiKumulatifGvMatrahi: None,
            oncekiKumulatifAsgariGvMatrahi: None,
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: None,
            sonrakiDevredenPek: None,
            pekDetay: None,
            isPrimiDetay: None,
            gvDetay: None,
            damgaDetay: None,
            statutorySnapshot: Some(statutory_fallback.clone()),
            odenenRaporluGun: None,
            raporluGun: None,
        });
        ordered_events.sort_by_key(|event| (event.sequence, event.accrualId.clone()));
    }

    let first_event = ordered_events.first().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} source period replay'i için payment event bulunamadı.",
            period.id
        ))
    })?;
    let (canonical_incoming, canonical_incoming_available) =
        incoming_devreden_pek_for_replay(dataset, personnel_id, period, first_event)?;
    let first_carry = if canonical_incoming_available {
        canonical_incoming.records
    } else {
        first_event.devredenPekGelen.clone().unwrap_or_default()
    };
    let mut carry = first_carry;
    let mut month_to_date = Decimal::ZERO;
    let mut tax_months_elapsed = if canonical_incoming_available {
        canonical_incoming.tax_months_elapsed
    } else if carry.is_empty() {
        0
    } else {
        1
    };
    let mut state = SourcePekState {
        upper: statutory_fallback.pekUstSinir,
        ..SourcePekState::default()
    };
    let target_normal_income = source_income_target(&original_normal_income, target_normal_income);

    for event in ordered_events {
        let is_normal = event.accrualType == AccrualType::NORMAL;
        let income = if is_normal {
            &target_normal_income
        } else {
            &event.gelirler
        };
        let statutory = event
            .statutorySnapshot
            .clone()
            .unwrap_or_else(|| statutory_fallback.clone());
        let (detail, next_carry) =
            calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state(
                income,
                Some(&event.puantajOzeti),
                Some(settings),
                &carry,
                Some(&statutory),
                month_to_date,
                PekCalculationOptions {
                    tax_months_elapsed,
                    apply_lower_bound: is_normal,
                },
            )?;
        state.worker_pek = round2(state.worker_pek + detail.primMatrahi);
        state.employer_sgk =
            round_sgk_amount(state.employer_sgk + detail.isverenSgkPrimi.unwrap_or_default());
        state.employer_unemployment = round_sgk_amount(
            state.employer_unemployment + detail.isverenIssizlikPrimi.unwrap_or_default(),
        );
        state.employer_lower_bound =
            round2(state.employer_lower_bound + detail.altSinirTamamlamaFarki);
        state.employer_lower_bound_premium = round_sgk_amount(
            state.employer_lower_bound_premium
                + detail.pekAltSinirTamamlamaIsverenPrimi.unwrap_or_default(),
        );
        state.upper = detail.pekUstSinir;
        month_to_date = round2(month_to_date + detail.primMatrahi);
        carry = next_carry;
        tax_months_elapsed = 0;
    }
    let (employer_sgk, employer_unemployment, employer_lower_bound_premium) =
        canonical_source_employer_premiums(
            state.worker_pek,
            state.employer_lower_bound,
            settings,
        )?;
    state.employer_sgk = employer_sgk;
    state.employer_unemployment = employer_unemployment;
    state.employer_lower_bound_premium = employer_lower_bound_premium;
    Ok(SourcePekReplay {
        state,
        outgoing_carry: carry,
    })
}

fn previous_source_retro_state(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
    source_period_id: &str,
) -> Result<SourcePekState> {
    let mut batches = dataset
        .retroBatches
        .iter()
        .filter(|batch| {
            batch.personnelId == personnel_id
                && batch.id != current_batch_id
                && matches!(
                    batch.status,
                    CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    batches.sort_by(|left, right| {
        left.paymentDate
            .cmp(&right.paymentDate)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut state = SourcePekState::default();
    for batch in batches {
        if parse_date(&batch.paymentDate, "önceki retro ödeme")? > payment_date {
            continue;
        }
        for allocation in validate_batch_ledger(dataset, &batch)? {
            if allocation.sourcePeriodId != source_period_id
                || allocation.sgkTreatment != RetroSgkTreatment::WAGE_SOURCE_MONTH
            {
                continue;
            }
            state.worker_pek = round2(state.worker_pek + allocation.retroPekDelta);
            state.worker_sgk =
                round_sgk_amount(state.worker_sgk + allocation.workerSgkDelta);
            state.worker_unemployment = round_sgk_amount(
                state.worker_unemployment + allocation.workerUnemploymentDelta,
            );
            state.employer_sgk =
                round_sgk_amount(state.employer_sgk + allocation.employerSgkDelta);
            state.employer_unemployment = round_sgk_amount(
                state.employer_unemployment + allocation.employerUnemploymentDelta,
            );
            state.employer_lower_bound =
                round2(state.employer_lower_bound + allocation.employerLowerBoundDelta);
            state.employer_lower_bound_premium = round_sgk_amount(
                state.employer_lower_bound_premium + allocation.employerLowerBoundPremiumDelta,
            );
        }
    }
    Ok(state)
}

fn source_eligible_delta(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
    allocation: &RetroAllocation,
) -> Result<Decimal> {
    if allocation.earningCode != RetroEarningCode::MEAL {
        return Ok(allocation.deltaAmount);
    }
    let statutory = source_statutory_snapshot(dataset, personnel_id, period)?;
    let exempt = statutory.sgkYemekIstisnasiToplam;
    let recognized_before =
        allocation.originalRecognizedAmount + allocation.previousAuthoritativeRetroAmount;
    let subject_before = (recognized_before - exempt).max(Decimal::ZERO);
    let subject_after = (allocation.targetAmount - exempt).max(Decimal::ZERO);
    let _ = settings;
    Ok(round2(subject_after - subject_before))
}

fn distribute_signed_delta(target: Decimal, desired: &[Decimal]) -> Vec<Decimal> {
    let positive_weight = desired
        .iter()
        .map(|value| (*value).max(Decimal::ZERO))
        .fold(Decimal::ZERO, |sum, value| sum + value);
    let negative_weight = desired
        .iter()
        .map(|value| (-*value).max(Decimal::ZERO))
        .fold(Decimal::ZERO, |sum, value| sum + value);
    let use_positive = target >= Decimal::ZERO;
    let weight = if use_positive {
        positive_weight
    } else {
        negative_weight
    };
    let target_abs = target.abs();
    let mut result = vec![Decimal::ZERO; desired.len()];
    let mut assigned = Decimal::ZERO;
    let candidates = desired
        .iter()
        .enumerate()
        .filter(|(_, value)| {
            if use_positive {
                **value > Decimal::ZERO
            } else {
                **value < Decimal::ZERO
            }
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    for (position, index) in candidates.iter().enumerate() {
        let is_last = position + 1 == candidates.len();
        let amount = if is_last {
            target_abs - assigned
        } else if weight > Decimal::ZERO {
            round2(
                target_abs
                    * (if use_positive {
                        desired[*index]
                    } else {
                        -desired[*index]
                    })
                    / weight,
            )
        } else {
            Decimal::ZERO
        };
        result[*index] = if use_positive { amount } else { -amount };
        assigned = round2(assigned + amount);
    }
    if candidates.is_empty() && target != Decimal::ZERO && !result.is_empty() {
        result[0] = round2(target);
    }
    result
}

fn rebalance_sgk_component_to_total(
    allocations: &mut [RetroAllocation],
    indices: &[usize],
    total: Decimal,
    read: fn(&RetroAllocation) -> Decimal,
    write: fn(&mut RetroAllocation, Decimal),
) {
    let Some(last_index) = indices.last().copied() else {
        return;
    };
    let assigned = indices
        .iter()
        .fold(Decimal::ZERO, |sum, index| sum + read(&allocations[*index]));
    let adjustment = round_sgk_amount(total - assigned);
    if adjustment != Decimal::ZERO {
        let corrected = round_sgk_amount(read(&allocations[last_index]) + adjustment);
        write(&mut allocations[last_index], corrected);
    }
}

fn apply_source_month_sgk(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    target_normal_income: &GelirKalemleri,
    allocations: &mut [RetroAllocation],
    payment_date: NaiveDate,
    current_batch_id: &str,
) -> Result<()> {
    let events = source_period_events(dataset, personnel_id, period)?;
    let original_replay = source_original_state(dataset, personnel_id, period, &events)?;
    let target_replay = source_target_state(dataset, personnel_id, period, &events, target_normal_income)?;
    let original = original_replay.state;
    let target = target_replay.state;
    let previous = previous_source_retro_state(
        dataset,
        personnel_id,
        payment_date,
        current_batch_id,
        &period.id,
    )?;
    let settings = dataset
        .institutionSettings
        .get(&period.id)
        .ok_or_else(|| DomainError::InvalidData(format!("{} source settings eksik.", period.id)))?;
    let worker_sgk_rate = settings
        .sgkIsciOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical SGK işçi oranı eksik.".into()))?
        / dec!(100);
    let worker_unemployment_rate = settings
        .issizlikIsciOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical işsizlik işçi oranı eksik.".into()))?
        / dec!(100);

    let recognized_worker_pek = round2(original.worker_pek + previous.worker_pek);
    let target_worker_sgk = round_sgk_amount(target.worker_pek * worker_sgk_rate);
    let original_worker_sgk = round_sgk_amount(original.worker_pek * worker_sgk_rate);
    let target_worker_unemployment = round_sgk_amount(target.worker_pek * worker_unemployment_rate);
    let original_worker_unemployment =
        round_sgk_amount(original.worker_pek * worker_unemployment_rate);

    let source_allocations = allocations
        .iter()
        .enumerate()
        .filter(|(_, allocation)| {
            allocation.sourcePeriodId == period.id
                && allocation.sgkTreatment == RetroSgkTreatment::WAGE_SOURCE_MONTH
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if source_allocations.is_empty() {
        if target.worker_pek != recognized_worker_pek {
            return Err(DomainError::InvalidData(format!(
                "{} source PEK replay delta'sı allocation olmadan kaldı.",
                period.id
            )));
        }
        return Ok(());
    }
    let mut source_allocations = source_allocations;
    source_allocations.sort_by_key(|index| {
        (
            allocations[*index].earningCode,
            allocations[*index].id.clone(),
        )
    });

    let desired = source_allocations
        .iter()
        .map(|index| {
            source_eligible_delta(
                dataset,
                personnel_id,
                period,
                settings,
                &allocations[*index],
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let worker_pek_total = round2(target.worker_pek - recognized_worker_pek);
    let pek_parts = distribute_signed_delta(worker_pek_total, &desired);
    let target_employer_sgk = target.employer_sgk;
    let original_employer_sgk = original.employer_sgk;

    for (position, index) in source_allocations.iter().enumerate() {
        let cumulative_pek = pek_parts
            .iter()
            .take(position + 1)
            .fold(Decimal::ZERO, |sum, value| sum + *value);
        let allocation = &mut allocations[*index];
        allocation.originalPek = round2(original.worker_pek);
        allocation.retroPekDelta = pek_parts[position];
        allocation.adjustedPek = round2(recognized_worker_pek + cumulative_pek);
        allocation.workerSgkDelta = round_sgk_amount(pek_parts[position] * worker_sgk_rate);
        allocation.workerUnemploymentDelta =
            round_sgk_amount(pek_parts[position] * worker_unemployment_rate);
        allocation.employerSgkDelta = Decimal::ZERO;
        allocation.employerUnemploymentDelta = Decimal::ZERO;
        allocation.employerLowerBoundDelta = Decimal::ZERO;
        allocation.employerLowerBoundPremiumDelta = Decimal::ZERO;
    }

    let employer_sgk_rate = settings
        .sgkIsverenOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical SGK işveren oranı eksik.".into()))?
        / dec!(100);
    let employer_unemployment_rate = settings.issizlikIsverenOraniYuzde.ok_or_else(|| {
        DomainError::InvalidData("Historical işveren işsizlik oranı eksik.".into())
    })? / dec!(100);
    // The target/original states above are canonical aggregate snapshots. The
    // following residual allocations intentionally use those state deltas,
    // rather than independently rounded earning-code shortcuts.
    let canonical_employer_sgk_total =
        round_sgk_amount(target_employer_sgk - original_employer_sgk - previous.employer_sgk);
    let canonical_employer_unemployment_total = round_sgk_amount(
        target.employer_unemployment
            - original.employer_unemployment
            - previous.employer_unemployment,
    );
    let canonical_lower_bound_total = round2(
        target.employer_lower_bound - original.employer_lower_bound - previous.employer_lower_bound,
    );
    let canonical_lower_bound_premium_total = round_sgk_amount(
        target.employer_lower_bound_premium
            - original.employer_lower_bound_premium
            - previous.employer_lower_bound_premium,
    );
    let canonical_worker_sgk_total =
        round_sgk_amount(target_worker_sgk - original_worker_sgk - previous.worker_sgk);
    let canonical_worker_unemployment_total = round_sgk_amount(
        target_worker_unemployment - original_worker_unemployment - previous.worker_unemployment,
    );
    rebalance_sgk_component_to_total(
        allocations,
        &source_allocations,
        canonical_worker_sgk_total,
        |allocation| allocation.workerSgkDelta,
        |allocation, value| allocation.workerSgkDelta = value,
    );
    rebalance_sgk_component_to_total(
        allocations,
        &source_allocations,
        canonical_worker_unemployment_total,
        |allocation| allocation.workerUnemploymentDelta,
        |allocation, value| allocation.workerUnemploymentDelta = value,
    );
    for (position, index) in source_allocations.iter().enumerate() {
        allocations[*index].employerSgkDelta =
            round_sgk_amount(pek_parts[position] * employer_sgk_rate);
        allocations[*index].employerUnemploymentDelta =
            round_sgk_amount(pek_parts[position] * employer_unemployment_rate);
    }
    rebalance_sgk_component_to_total(
        allocations,
        &source_allocations,
        canonical_employer_sgk_total,
        |allocation| allocation.employerSgkDelta,
        |allocation, value| allocation.employerSgkDelta = value,
    );
    rebalance_sgk_component_to_total(
        allocations,
        &source_allocations,
        canonical_employer_unemployment_total,
        |allocation| allocation.employerUnemploymentDelta,
        |allocation, value| allocation.employerUnemploymentDelta = value,
    );

    if let Some(last_index) = source_allocations.last().copied() {
        allocations[last_index].originalEmployerLowerBound = round2(original.employer_lower_bound);
        allocations[last_index].targetEmployerLowerBound = round2(target.employer_lower_bound);
        allocations[last_index].employerLowerBoundDelta = canonical_lower_bound_total;
        allocations[last_index].employerLowerBoundPremiumDelta =
            canonical_lower_bound_premium_total;
        allocations[last_index].originalSourceCarry = Some(original_replay.outgoing_carry);
        allocations[last_index].targetSourceCarry = Some(target_replay.outgoing_carry);
    }
    Ok(())
}

fn policy_map_for_income(
    batch: &RetroAdjustmentBatch,
    allocations: &[RetroAllocation],
    include_payment_month_sgk: bool,
) -> GelirKalemleri {
    let mut income = GelirKalemleri::default();
    for allocation in allocations {
        let should_include = match allocation.sgkTreatment {
            RetroSgkTreatment::NON_WAGE_PAYMENT_MONTH if include_payment_month_sgk => true,
            RetroSgkTreatment::NON_WAGE_CARRY if include_payment_month_sgk => true,
            _ if !include_payment_month_sgk => false,
            _ => false,
        };
        if should_include {
            let current = code_value(&income, allocation.earningCode);
            let payable_amount = retro_payable_allocation_amount(batch, allocation);
            set_code(
                &mut income,
                allocation.earningCode,
                current + payable_amount,
            );
        }
    }
    income
}

/// Resolves a batch into the two payment views needed by the canonical payroll
/// engine: all taxable gross income and only the portion that belongs to the
/// payment-month PEK state.
pub fn retro_payment_income(
    dataset: &PayrollDatasetSnapshot,
    batch_id: &str,
) -> Result<(
    RetroAdjustmentBatch,
    Vec<RetroAllocation>,
    GelirKalemleri,
    GelirKalemleri,
)> {
    let batch = dataset
        .retroBatches
        .iter()
        .find(|batch| batch.id == batch_id)
        .cloned()
        .ok_or_else(|| DomainError::NotFound(format!("Retro batch bulunamadı: {}", batch_id)))?;
    if matches!(
        batch.status,
        CompensationRevisionStatus::DRAFT | CompensationRevisionStatus::STALE
    ) {
        return Err(DomainError::ValidationError(format!(
            "{} retro batch'i authoritative değil; ödeme oluşturulamaz.",
            batch.id
        )));
    }
    if retro_payable_settlement_amount(&batch) <= Decimal::ZERO {
        return Err(DomainError::ValidationError(
            "Payable settlement sıfır olan retro batch için ödeme event'i oluşturulamaz; entitlement/receivable ledger'ı ayrı tutulmalıdır."
                .into(),
        ));
    }
    let mut allocations = validate_batch_ledger(dataset, &batch)?;
    if has_legacy_settlement_flow(&batch) {
        // V4 had no per-allocation settlement flow.  Materialize the batch-net
        // payable allocation locally so a mixed-sign legacy batch cannot turn
        // every positive allocation into a separate cash payment.
        assign_current_settlement_allocations(
            &mut allocations,
            retro_payable_settlement_amount(&batch),
            Decimal::ZERO,
            negative_entitlement_amount(&batch),
        );
    }
    let mut income = GelirKalemleri::default();
    for allocation in &allocations {
        let next = code_value(&income, allocation.earningCode)
            + retro_payable_allocation_amount(&batch, allocation);
        set_code(&mut income, allocation.earningCode, next);
    }
    let payment_month_pek_income = policy_map_for_income(&batch, &allocations, true);
    Ok((batch, allocations, income, payment_month_pek_income))
}

/// Pure replay/ledger engine. It is safe to call for preview repeatedly: no
/// database, clock, global balance, or mutable payroll record is touched.
pub struct RetroEntitlementEngine;

impl RetroEntitlementEngine {
    pub fn calculate(request: &RetroCalculationRequest) -> Result<RetroCalculationResult> {
        let payment_date = parse_date(&request.paymentDate, "retro ödeme")?;
        let effective_from = parse_date(&request.revision.effectiveFrom, "revision yürürlük")?;
        if effective_from > payment_date {
            return Err(DomainError::ValidationError(
                "Revision yürürlük tarihi retro ödeme tarihinden sonra olamaz.".into(),
            ));
        }
        if let Some(effective_to) = request.revision.effectiveTo.as_deref() {
            if parse_date(effective_to, "revision bitiş")? < effective_from {
                return Err(DomainError::ValidationError(
                    "Revision bitiş tarihi yürürlük tarihinden önce olamaz.".into(),
                ));
            }
        }
        let personnel = request
            .dataset
            .personnel
            .iter()
            .find(|personnel| personnel.id == request.personnelId)
            .ok_or_else(|| DomainError::NotFound("Retro personeli bulunamadı.".into()))?;
        ensure_revision_scope(&request.revision, personnel)?;
        if request.batchId.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "Retro batch kimliği boş olamaz.".into(),
            ));
        }
        if let Some(existing_batch) = request
            .dataset
            .retroBatches
            .iter()
            .find(|batch| batch.id == request.batchId)
        {
            if existing_batch.revisionId != request.revision.id
                || existing_batch.personnelId != request.personnelId
                || existing_batch.paymentDate != request.paymentDate
            {
                return Err(DomainError::InvalidData(
                    "Retro batch primary id'si farklı revision/personel/ödeme olayına ait; yeniden bağlanamaz."
                        .into(),
                ));
            }
            if existing_batch.status == CompensationRevisionStatus::FINALIZED {
                return Err(DomainError::PayrollFinalized(
                    "FINALIZED retro batch yeniden hesaplanamaz; yeni bir correction batch'i oluşturulmalıdır."
                        .into(),
                ));
            }
        }
        let selected_overrides: Vec<CompensationRevisionOverride> = request
            .overrides
            .iter()
            .filter(|item| item.revisionId == request.revision.id)
            .cloned()
            .collect();
        if request
            .overrides
            .iter()
            .any(|item| item.revisionId != request.revision.id)
        {
            return Err(DomainError::ValidationError(
                "Retro hesap isteği başka bir revision'a ait override içeremez.".into(),
            ));
        }
        for item in &selected_overrides {
            validate_override_value(item.parameter, item.value)?;
        }

        let applications = revision_applications(
            &request.dataset,
            &request.revision,
            &selected_overrides,
            personnel,
        )?;
        let earliest_effective_from = applications
            .iter()
            .map(|application| application.effective_from)
            .min()
            .unwrap_or(effective_from);

        let mut periods: Vec<BordroDonemi> = request
            .dataset
            .periods
            .iter()
            .filter(|period| {
                let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").ok();
                let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").ok();
                start.zip(end).is_some_and(|(start, end)| {
                    end >= earliest_effective_from
                            // A retro calculation may only use a closed
                            // service period. The payment month is an event
                            // month, not permission to replay an open 15-14
                            // period through the payment day.
                            && end <= payment_date
                            && start <= payment_date
                })
            })
            .cloned()
            .collect();
        periods.sort_by_key(|period| (period.baslangicTarihi.clone(), period.id.clone()));
        if periods.is_empty() {
            return Err(DomainError::NotFound(
                "Revision için etkilenen authoritative hizmet dönemi bulunamadı.".into(),
            ));
        }

        // A service period's tax metadata is an input to the eventual payment
        // event. Do not let a malformed period produce a seemingly valid
        // retro ledger that will later use a different tax month.
        for period in &periods {
            validate_tax_month_overlap(period)?;
        }

        let source_period_ids = periods
            .iter()
            .map(|period| period.id.clone())
            .collect::<HashSet<_>>();
        reject_later_authoritative_retro_for_same_source_period(
            &request.dataset,
            &request.personnelId,
            payment_date,
            &request.batchId,
            &source_period_ids,
        )?;

        let (previous_retro, _previous_pek) = previous_authoritative_retro_by_period_and_code(
            &request.dataset,
            &request.personnelId,
            payment_date,
            &request.batchId,
        )?;

        let mut allocations = Vec::new();
        let mut previews = Vec::new();
        let mut target_income_by_period = HashMap::new();
        for period in &periods {
            let target_income = target_income_for_period(
                &request.dataset,
                personnel,
                period,
                payment_date,
                &applications,
            )?;
            target_income_by_period.insert(period.id.clone(), target_income.clone());
            let original = original_recognized_by_period_and_code(
                &request.dataset,
                &request.personnelId,
                &period.id,
            )?;

            let mut target_by_code = income_by_code(&target_income);
            // Preserve independently paid legacy events. They are already part
            // of recognized entitlement and are not regenerated by shadow
            // NORMAL replay.
            for payroll in request.dataset.payrolls.iter().filter(|payroll| {
                payroll.personelId == request.personnelId
                    && payroll.donemId == period.id
                    && payroll.accrualType != AccrualType::NORMAL
                    && payroll.accrualType != AccrualType::RETRO_ADJUSTMENT
                    && matches!(
                        payroll.status,
                        BordroStatus::CALCULATED | BordroStatus::FINALIZED
                    )
            }) {
                for (code, value) in income_by_code(&payroll.gelirler) {
                    add_code(&mut target_by_code, code, value);
                }
            }

            for (parameter, code) in [
                (RetroParameterKey::TEDIYE, RetroEarningCode::TEDIYE),
                (RetroParameterKey::TIS_BONUS, RetroEarningCode::TIS_BONUS),
            ] {
                if let Some(value) = override_value(
                    &selected_overrides,
                    &request.revision.id,
                    &personnel.id,
                    parameter,
                )? {
                    target_by_code.insert(code, value);
                } else if let Some(value) = original.get(&code) {
                    target_by_code.insert(code, *value);
                }
            }

            let mut all_codes: Vec<RetroEarningCode> = target_by_code
                .keys()
                .chain(original.keys())
                .copied()
                .collect();
            all_codes.extend(
                previous_retro
                    .keys()
                    .filter(|(period_id, _)| period_id == &period.id)
                    .map(|(_, code)| *code),
            );
            all_codes.sort();
            all_codes.dedup();
            let period_original_total = original
                .values()
                .fold(Decimal::ZERO, |sum, value| sum + *value);
            let period_previous_total = previous_retro
                .iter()
                .filter(|((period_id, _), _)| period_id == &period.id)
                .fold(Decimal::ZERO, |sum, (_, value)| sum + *value);
            let period_target_total = all_codes.iter().fold(Decimal::ZERO, |sum, code| {
                sum + target_by_code.get(code).copied().unwrap_or_default()
            });
            previews.push(RetroPeriodPreview {
                sourcePeriodId: period.id.clone(),
                originalRecognizedAmount: round2(period_original_total),
                previousAuthoritativeRetroAmount: round2(period_previous_total),
                targetAmount: round2(period_target_total),
                deltaAmount: round2(
                    period_target_total - period_original_total - period_previous_total,
                ),
            });

            for code in all_codes {
                let original_value = original.get(&code).copied().unwrap_or_default();
                let previous_value = previous_retro
                    .get(&(period.id.clone(), code))
                    .copied()
                    .unwrap_or_default();
                let target_value = target_by_code.get(&code).copied().unwrap_or_default();
                let delta = round2(target_value - original_value - previous_value);
                if delta == Decimal::ZERO {
                    continue;
                }
                let policy = retro_earning_policy(code);
                allocations.push(RetroAllocation {
                    id: format!("{}_{}_{code:?}", request.batchId, period.id),
                    batchId: request.batchId.clone(),
                    personnelId: request.personnelId.clone(),
                    sourcePeriodId: period.id.clone(),
                    earningCode: code,
                    originalRecognizedAmount: round2(original_value),
                    previousAuthoritativeRetroAmount: round2(previous_value),
                    targetAmount: round2(target_value),
                    deltaAmount: delta,
                    sgkTreatment: policy.sgkTreatment,
                    incomeTaxTreatment: policy.incomeTaxTreatment,
                    stampTaxTreatment: policy.stampTaxTreatment,
                    originalPek: Decimal::ZERO,
                    retroPekDelta: Decimal::ZERO,
                    adjustedPek: Decimal::ZERO,
                    workerSgkDelta: Decimal::ZERO,
                    workerUnemploymentDelta: Decimal::ZERO,
                    employerSgkDelta: Decimal::ZERO,
                    employerUnemploymentDelta: Decimal::ZERO,
                    originalEmployerLowerBound: Decimal::ZERO,
                    targetEmployerLowerBound: Decimal::ZERO,
                    employerLowerBoundDelta: Decimal::ZERO,
                    employerLowerBoundPremiumDelta: Decimal::ZERO,
                    originalSourceCarry: None,
                    targetSourceCarry: None,
                    payableSettlementAmount: Decimal::ZERO,
                    offsetSettlementAmount: Decimal::ZERO,
                    recoverableAmount: Decimal::ZERO,
                    metadata: Some(
                        serde_json::json!({
                            "revisionId": request.revision.id,
                            "effectiveFrom": request.revision.effectiveFrom,
                            "sourcePeriodId": period.id,
                        })
                        .to_string(),
                    ),
                });
            }
        }

        // Source-month PEK is an incremental ledger. Group allocations by
        // service period and apply the historical ceiling once.
        let mut source_period_ids: Vec<String> = allocations
            .iter()
            .filter(|allocation| allocation.sgkTreatment == RetroSgkTreatment::WAGE_SOURCE_MONTH)
            .map(|allocation| allocation.sourcePeriodId.clone())
            .collect();
        source_period_ids.sort();
        source_period_ids.dedup();
        for period_id in &source_period_ids {
            let period = periods
                .iter()
                .find(|period| &period.id == period_id)
                .ok_or_else(|| {
                    DomainError::NotFound(format!("{} source period bulunamadı.", period_id))
                })?;
            let target_normal_income = target_income_by_period.get(period_id).ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} retro target income snapshot'ı eksik.",
                    period_id
                ))
            })?;
            apply_source_month_sgk(
                &request.dataset,
                &request.personnelId,
                period,
                target_normal_income,
                &mut allocations,
                payment_date,
                &request.batchId,
            )?;
        }

        let total = round2(allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
            sum + allocation.deltaAmount
        }));
        let prior_outstanding = outstanding_receivable_before_current(
            &request.dataset,
            &request.personnelId,
            payment_date,
            &request.batchId,
        )?;
        let positive_entitlement = total.max(Decimal::ZERO);
        let recoverable = (-total).max(Decimal::ZERO);
        let offset = positive_entitlement.min(prior_outstanding);
        let payable = round2(positive_entitlement - offset);
        let outstanding = round2(prior_outstanding - offset + recoverable);
        assign_current_settlement_allocations(&mut allocations, payable, offset, recoverable);
        let settlement_status = if recoverable > Decimal::ZERO {
            RetroSettlementStatus::OVERPAYMENT
        } else if positive_entitlement > Decimal::ZERO && payable == Decimal::ZERO {
            RetroSettlementStatus::SETTLED_BY_OFFSET
        } else {
            RetroSettlementStatus::UNSETTLED
        };
        let batch = RetroAdjustmentBatch {
            id: request.batchId.clone(),
            revisionId: request.revision.id.clone(),
            personnelId: request.personnelId.clone(),
            paymentDate: request.paymentDate.clone(),
            status: CompensationRevisionStatus::CALCULATED,
            settlementStatus: settlement_status,
            totalGrossDelta: total,
            payableSettlementAmount: payable,
            offsetSettlementAmount: offset,
            recoveredAmount: Decimal::ZERO,
            recoverableAmount: recoverable,
            outstandingReceivable: outstanding,
            description: request
                .description
                .clone()
                .or_else(|| Some(request.revision.title.clone())),
            createdAt: Some(request.calculatedAt.clone()),
            calculatedAt: Some(request.calculatedAt.clone()),
            finalizedAt: None,
        };
        Ok(RetroCalculationResult {
            batch,
            allocations,
            periods: previews,
        })
    }
}

/// Computes source-period PEK deltas without exposing a mutable balance table.
pub fn retro_sgk_ledger_totals(
    allocations: &[RetroAllocation],
) -> RetroSgkLedgerTotals {
    let mut result = BTreeMap::new();
    for allocation in allocations {
        let entry = result.entry(allocation.sourcePeriodId.clone()).or_insert((
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        ));
        entry.0 = entry.0.max(round2(allocation.originalPek));
        entry.1 = round2(entry.1 + allocation.retroPekDelta);
        entry.2 = entry.2.max(round2(allocation.adjustedPek));
        entry.3 = round_sgk_amount(entry.3 + allocation.workerSgkDelta);
        entry.4 = round_sgk_amount(entry.4 + allocation.workerUnemploymentDelta);
        entry.5 = round_sgk_amount(entry.5 + allocation.employerSgkDelta);
        entry.6 = round_sgk_amount(entry.6 + allocation.employerUnemploymentDelta);
        entry.7 = round2(entry.7 + allocation.deltaAmount);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payroll_engine::PayrollDatasetSnapshot;

    #[test]
    fn policy_keeps_statutory_treatment_out_of_revision_parameters() {
        let policy = retro_earning_policy(RetroEarningCode::BASE_WAGE);
        assert_eq!(policy.sgkTreatment, RetroSgkTreatment::WAGE_SOURCE_MONTH);
        assert!(matches!(
            RetroParameterKey::GUNLUK_TABAN_UCRET,
            RetroParameterKey::GUNLUK_TABAN_UCRET
        ));
    }

    #[test]
    fn zero_delta_batch_has_a_deterministic_empty_payment_total() {
        let batch = RetroAdjustmentBatch {
            id: "b".into(),
            revisionId: "r".into(),
            personnelId: "p1".into(),
            paymentDate: "2026-06-20".into(),
            status: CompensationRevisionStatus::CALCULATED,
            settlementStatus: RetroSettlementStatus::UNSETTLED,
            totalGrossDelta: Decimal::ZERO,
            payableSettlementAmount: Decimal::ZERO,
            offsetSettlementAmount: Decimal::ZERO,
            recoveredAmount: Decimal::ZERO,
            recoverableAmount: Decimal::ZERO,
            outstandingReceivable: Decimal::ZERO,
            description: None,
            createdAt: None,
            calculatedAt: None,
            finalizedAt: None,
        };
        let allocation = RetroAllocation {
            id: "a".into(),
            batchId: "b".into(),
            personnelId: "p1".into(),
            sourcePeriodId: "p".into(),
            earningCode: RetroEarningCode::BASE_WAGE,
            originalRecognizedAmount: dec!(10),
            previousAuthoritativeRetroAmount: Decimal::ZERO,
            targetAmount: dec!(10),
            deltaAmount: Decimal::ZERO,
            sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
            incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
            stampTaxTreatment: RetroTaxTreatment::TAXABLE,
            originalPek: Decimal::ZERO,
            retroPekDelta: Decimal::ZERO,
            adjustedPek: Decimal::ZERO,
            workerSgkDelta: Decimal::ZERO,
            workerUnemploymentDelta: Decimal::ZERO,
            employerSgkDelta: Decimal::ZERO,
            employerUnemploymentDelta: Decimal::ZERO,
            originalEmployerLowerBound: Decimal::ZERO,
            targetEmployerLowerBound: Decimal::ZERO,
            employerLowerBoundDelta: Decimal::ZERO,
            employerLowerBoundPremiumDelta: Decimal::ZERO,
            originalSourceCarry: None,
            targetSourceCarry: None,
            payableSettlementAmount: Decimal::ZERO,
            offsetSettlementAmount: Decimal::ZERO,
            recoverableAmount: Decimal::ZERO,
            metadata: None,
        };
        let dataset = PayrollDatasetSnapshot {
            retroBatches: vec![batch],
            retroAllocations: vec![allocation],
            ..PayrollDatasetSnapshot::default()
        };
        assert!(retro_payment_income(&dataset, "b").is_err());
    }
}
