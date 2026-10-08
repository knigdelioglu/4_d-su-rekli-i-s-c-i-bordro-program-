//! Deterministic, source-period based retroactive entitlement calculation.
//!
//! This module never mutates an existing payroll record. It replays the
//! historical attendance and compensation inputs, builds a target entitlement,
//! reads the recognized ledger from authoritative payroll events and earlier
//! retro allocations, and emits an auditable delta allocation.

#![allow(non_snake_case)]

use crate::calculations::{
    calculate_gelir_toplam, calculate_kesinti_toplam,
    calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state, canonical_sgk_earning_class,
    round_sgk_amount, CanonicalSgkEarningClass, MealExemptionTotals, PekCalculationOptions,
};
use crate::index::PayrollDatasetIndex;
use crate::models::*;
use crate::payroll_engine::{
    accrual_order_for_payroll_with_index, ensure_authoritative_payment_event,
    incoming_devreden_pek_for_replay, replay_saved_normal_compensation_income,
    validate_tax_month_overlap, PayrollDatasetSnapshot,
};
use crate::{DomainError, Result};
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

type RetroAmountsByPeriodAndCode = HashMap<(String, RetroEarningCode), Decimal>;
type RetroSgkLedgerTotals = BTreeMap<
    String,
    (
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
    ),
>;

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
    retro_earning_policy_with_meal_tax_policy(code, false)
}

/// Resolve the registry policy with the statutory meal income-tax treatment
/// captured for the source period. Other earning codes ignore this setting.
pub fn retro_earning_policy_with_meal_tax_policy(
    code: RetroEarningCode,
    meal_income_tax_exempt: bool,
) -> RetroEarningPolicy {
    let wage_source = RetroEarningPolicy {
        incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
        stampTaxTreatment: RetroTaxTreatment::TAXABLE,
        sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
    };
    let mut policy = match canonical_sgk_earning_class(code) {
        CanonicalSgkEarningClass::Wage => wage_source,
        CanonicalSgkEarningClass::NonWage => RetroEarningPolicy {
            incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
            stampTaxTreatment: RetroTaxTreatment::TAXABLE,
            sgkTreatment: RetroSgkTreatment::NON_WAGE_PAYMENT_MONTH,
        },
    };
    if code == RetroEarningCode::MEAL && meal_income_tax_exempt {
        policy.incomeTaxTreatment = RetroTaxTreatment::EXEMPT;
    }
    policy
}

fn retro_period_earning_policy(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
    code: RetroEarningCode,
) -> Result<RetroEarningPolicy> {
    let meal_income_tax_exempt = if code == RetroEarningCode::MEAL {
        let normal = authoritative_normal_payroll(index, dataset, personnel_id, period_id)?;
        if let Some(snapshot) = normal
            .calculationMetadata
            .as_ref()
            .and_then(|metadata| metadata.snapshot.as_ref())
        {
            crate::validate_payroll_calculation_metadata_with_context(
                normal,
                index.period(dataset, period_id).ok_or_else(|| {
                    DomainError::NotFound(format!("{period_id} source period bulunamadı."))
                })?,
                &dataset.payrolls,
                &dataset.periods,
            )
            .map_err(|error| DomainError::InvalidData(error.to_string()))?;
            snapshot
                .usedInputs
                .institutionDeductionSettings
                .yemekTamGvIstisnaDamgaTabi
        } else {
            return Err(DomainError::InvalidData(format!(
                "LEGACY_REPLAY_UNAVAILABLE: {period_id} source has no saved meal-tax policy snapshot."
            )));
        }
    } else {
        false
    };
    Ok(retro_earning_policy_with_meal_tax_policy(
        code,
        meal_income_tax_exempt,
    ))
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
struct TargetIncomeForPeriod {
    income: GelirKalemleri,
    meal_exemption: MealExemptionTotals,
    meal_income_tax_exempt: bool,
}

/// Boundary rule for retro source-period selection (BUG-RETRO-001).
///
/// A service period `[start, end]` can only be affected by a revision whose
/// validity ends on `effective_to` when `start <= effective_to` (closed
/// interval, both ends inclusive). The retro payment date never participates
/// in this decision.
pub(crate) fn source_period_intersects_revision_end(
    period_start: NaiveDate,
    effective_to: Option<NaiveDate>,
) -> bool {
    effective_to.is_none_or(|effective_to| period_start <= effective_to)
}

/// BUG-RETRO-001 second defence: a source period may only be replayed when it
/// has an authoritative NORMAL payroll baseline. Without it the recognized
/// ledger would silently be zero and the full monthly entitlement would be
/// reported as a retro difference.
fn ensure_authoritative_normal_baseline(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<()> {
    let normals = index
        .payrolls_for_person_period(dataset, personnel_id, &period.id)
        .filter(|payroll| payroll.accrualType == AccrualType::NORMAL)
        .collect::<Vec<_>>();
    let name = if period.donemAdi.trim().is_empty() {
        period.id.as_str()
    } else {
        period.donemAdi.as_str()
    };
    let authoritative = normals
        .iter()
        .filter(|payroll| {
            matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
        })
        .collect::<Vec<_>>();
    if authoritative.len() == 1 {
        return Ok(());
    }
    if authoritative.len() > 1 {
        return Err(DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {name} has multiple authoritative NORMAL source records."
        )));
    }
    if let Some(stale) = normals.first() {
        return Err(DomainError::ValidationError(format!(
            "{name} dönemindeki NORMAL bordro ({}) {:?} durumda; retro fark hesaplanmadan önce Bordro Hesaplama ekranında \
             {name} normal bordrosunu yeniden hesaplayın.",
            stale.accrualId, stale.status
        )));
    }
    Err(DomainError::ValidationError(format!(
        "{name} dönemi revizyonun geçerlilik aralığında ancak bu dönem için hesaplanmış (CALCULATED/FINALIZED) NORMAL bordro yok; \
         önceden tanınmış hak belirlenemediği için retro fark hesaplanamaz. Önce Bordro Hesaplama ekranında {name} normal bordrosunu \
         hesaplayın veya revizyonun geçerlilik tarihlerini kontrol edin."
    )))
}

fn parse_date(value: &str, field: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        DomainError::ValidationError(format!("{field} tarihi geçersiz: {value}: {error}"))
    })
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

fn scope_matches_person_id(revision: &CompensationRevision, personnel_id: &str) -> bool {
    match revision.scope {
        CompensationRevisionScope::ALL_PERSONNEL | CompensationRevisionScope::PERSONNEL_GROUP => {
            true
        }
        CompensationRevisionScope::SELECTED_PERSONNEL => {
            revision.personnelIds.iter().any(|id| id == personnel_id)
        }
    }
}

fn scope_matches_source_group(
    revision: &CompensationRevision,
    personnel_id: &str,
    personnel_group: &str,
) -> bool {
    match revision.scope {
        CompensationRevisionScope::ALL_PERSONNEL => true,
        CompensationRevisionScope::SELECTED_PERSONNEL => {
            revision.personnelIds.iter().any(|id| id == personnel_id)
        }
        CompensationRevisionScope::PERSONNEL_GROUP => revision
            .personnelGroup
            .as_deref()
            .is_some_and(|group| group == personnel_group),
    }
}

fn revision_has_authoritative_batch(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    revision_id: &str,
) -> bool {
    index
        .retro_batches_for_revision(dataset, revision_id)
        .any(|batch| {
            batch.revisionId == revision_id
                && matches!(
                    batch.status,
                    CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
                )
        })
}

fn revision_applications(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    current_revision: &CompensationRevision,
    current_overrides: &[CompensationRevisionOverride],
    personnel: &Personel,
) -> Result<Vec<RevisionApplication>> {
    let mut applications = Vec::new();

    for revision in &dataset.compensationRevisions {
        if revision.id == current_revision.id
            || revision.status == CompensationRevisionStatus::STALE
            || !scope_matches_person_id(revision, &personnel.id)
            || (!matches!(
                revision.status,
                CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
            ) && !revision_has_authoritative_batch(index, dataset, &revision.id))
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
        let overrides = index
            .overrides_for_revision(dataset, &revision.id)
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

fn override_value(
    overrides: &[CompensationRevisionOverride],
    revision_id: &str,
    personnel_id: &str,
    key: RetroParameterKey,
) -> Result<Option<Decimal>> {
    Ok(selected_override(overrides, revision_id, personnel_id, key)?.map(|item| item.value))
}

fn historical_attendance<'a>(
    index: &'a PayrollDatasetIndex,
    dataset: &'a PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
) -> Result<&'a PersonelPuantaj> {
    index
        .attendances(dataset, personnel_id, period_id)
        .next()
        .ok_or_else(|| {
            DomainError::NotFound(format!(
                "{} / {} için tarihsel puantaj bulunamadı.",
                personnel_id, period_id
            ))
        })
}

fn authoritative_normal_payroll<'a>(
    index: &'a PayrollDatasetIndex,
    dataset: &'a PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
) -> Result<&'a BordroKaydi> {
    let normals = index
        .payrolls_for_person_period(dataset, personnel_id, period_id)
        .filter(|payroll| payroll.accrualType == AccrualType::NORMAL)
        .collect::<Vec<_>>();
    let authoritative = normals
        .iter()
        .copied()
        .filter(|payroll| {
            matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
        })
        .collect::<Vec<_>>();
    match authoritative.as_slice() {
        [payroll] => Ok(*payroll),
        [] => Err(DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {period_id} has no CALCULATED/FINALIZED NORMAL source record; bu dönem için hesaplanmış (CALCULATED/FINALIZED) NORMAL bordro yok, STALE/DRAFT rows are not authority."
        ))),
        _ => Err(DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {period_id} has multiple authoritative NORMAL source records."
        ))),
    }
}

fn source_personnel_group(payroll: &BordroKaydi, fallback: &Personel) -> Result<String> {
    if let Some(snapshot) = payroll
        .calculationMetadata
        .as_ref()
        .and_then(|metadata| metadata.snapshot.as_ref())
    {
        if snapshot.usedInputs.personnelGroup.trim().is_empty() {
            return Err(DomainError::InvalidData(format!(
                "{} snapshot personnelGroup is empty.",
                payroll.accrualId
            )));
        }
        return Ok(snapshot.usedInputs.personnelGroup.clone());
    }
    let _ = fallback;
    Err(DomainError::InvalidData(format!(
        "LEGACY_REPLAY_UNAVAILABLE: {} source has no saved personnel-group snapshot.",
        payroll.accrualId
    )))
}

fn same_income_values(left: &GelirKalemleri, right: &GelirKalemleri) -> bool {
    left.tabanBrutAylik == right.tabanBrutAylik
        && left.tediye == right.tediye
        && left.tisIkramiyesi == right.tisIkramiyesi
        && left.ekOdeme == right.ekOdeme
        && left.yemek == right.yemek
        && left.birlestirilmisSosyalYardim == right.birlestirilmisSosyalYardim
        && left.vasitaYol == right.vasitaYol
        && left.giyimYardimi == right.giyimYardimi
        && left.isPrimi == right.isPrimi
        && left.geceCalismasiUcreti == right.geceCalismasiUcreti
        && left.geceCalismasiTatiliUcreti == right.geceCalismasiTatiliUcreti
        && left.hizmetZammi == right.hizmetZammi
        && left.digerGelir == right.digerGelir
}

fn apply_snapshot_revision_overrides(
    state: &mut CompensationPolicyState,
    revision_id: &str,
    personnel_id: &str,
    personnel_group: &str,
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
        let Some(item) = selected_override(overrides, revision_id, personnel_id, key)? else {
            continue;
        };
        validate_override_value(key, item.value)?;
        match key {
            RetroParameterKey::GUNLUK_TABAN_UCRET => state.gunlukTabanUcret = item.value,
            RetroParameterKey::GUNLUK_YEMEK => state.gunlukYemek = item.value,
            RetroParameterKey::BIRLESTIRILMIS_SOSYAL_YARDIM => {
                state.birlestirilmisSosyalYardim = item.value
            }
            RetroParameterKey::GUNLUK_VASITA_YOL => state.gunlukVasitaYol = item.value,
            RetroParameterKey::GIYIM_YARDIMI => state.giyimYardimi = item.value,
            RetroParameterKey::HIZMET_ZAMMI_BIRIMI => state.hizmetZammiBirimi = item.value,
            RetroParameterKey::IS_PRIMI_YUZDE => {
                let matches = state
                    .isPrimiGruplari
                    .iter_mut()
                    .filter(|group| {
                        group.aktif && (group.id == personnel_group || group.ad == personnel_group)
                    })
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err(DomainError::ValidationError(format!(
                        "{} personeli için snapshot iş primi grubunda tek aktif eşleşme bekleniyordu.",
                        personnel_id
                    )));
                }
                matches.into_iter().next().expect("length checked").oran = item.value;
            }
            RetroParameterKey::GECE_CALISMA_PRIMI_YUZDE => state.geceCalismaPrimiYuzde = item.value,
            RetroParameterKey::GECE_CALISMA_TATILI_PRIMI_YUZDE => {
                state.geceCalismaTatiliPrimiYuzde = item.value
            }
            RetroParameterKey::EK_ODEME => state.ekOdeme = Some(item.value),
            RetroParameterKey::DIGER_GELIR => state.digerGelirVarsayilan = Some(item.value),
            RetroParameterKey::TEDIYE | RetroParameterKey::TIS_BONUS => unreachable!(),
        }
    }
    crate::validate_compensation_policy_state(state)?;
    Ok(())
}

fn target_snapshot_segments(
    payroll: &BordroKaydi,
    period: &BordroDonemi,
    applications: &[RevisionApplication],
) -> Result<Vec<ResolvedCompensationSegment>> {
    let snapshot = payroll
        .calculationMetadata
        .as_ref()
        .and_then(|metadata| metadata.snapshot.as_ref())
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} has no persisted compensation segments.",
                payroll.accrualId
            ))
        })?;
    let group = &snapshot.usedInputs.personnelGroup;
    let mut output = Vec::<ResolvedCompensationSegment>::new();
    let mut previous_signature = String::new();
    let mut previous_source_id = String::new();
    for source in &snapshot.compensation.segments {
        let start = parse_date(&source.startDate, "snapshot compensation start")?;
        let end = parse_date(&source.endDate, "snapshot compensation end")?;
        let mut current = start;
        while current <= end {
            let active = applications
                .iter()
                .filter(|application| {
                    current >= application.effective_from
                        && application
                            .effective_to
                            .is_none_or(|effective_to| current <= effective_to)
                        && scope_matches_source_group(
                            &application.revision,
                            &payroll.personelId,
                            group,
                        )
                })
                .collect::<Vec<_>>();
            let signature = active
                .iter()
                .map(|application| application.revision.id.as_str())
                .collect::<Vec<_>>()
                .join("|");
            let mut state = source.state.clone();
            for application in &active {
                apply_snapshot_revision_overrides(
                    &mut state,
                    &application.revision.id,
                    &payroll.personelId,
                    group,
                    &application.overrides,
                )?;
            }
            let date = current.format("%Y-%m-%d").to_string();
            let merge = output.last().is_some_and(|last| {
                !last.endDate.is_empty()
                    && last.endDate == current.pred_opt().unwrap().format("%Y-%m-%d").to_string()
                    && previous_signature == signature
                    && previous_source_id == source.versionId
                    && last.state == state
            });
            if merge {
                output.last_mut().expect("merge checked").endDate = date;
            } else {
                let revision_boundary = !previous_signature.is_empty() || !signature.is_empty();
                let source_boundary = current == start;
                let boundary_scope =
                    if revision_boundary && (!source_boundary || previous_signature != signature) {
                        CompensationBoundaryScope::FullEarning
                    } else {
                        source.boundaryScope
                    };
                let version_id = if active.is_empty()
                    && current == start
                    && current == parse_date(&source.startDate, "snapshot compensation start")?
                {
                    source.versionId.clone()
                } else {
                    format!("{}@retro-replay:{}", source.versionId, date)
                };
                output.push(ResolvedCompensationSegment {
                    versionId: version_id,
                    startDate: date.clone(),
                    endDate: date,
                    state,
                    boundaryScope: boundary_scope,
                    roadRoundingMode: source.roadRoundingMode,
                });
            }
            previous_signature = signature;
            previous_source_id = source.versionId.clone();
            current = current
                .checked_add_signed(Duration::days(1))
                .ok_or_else(|| DomainError::InvalidData("Retro snapshot date overflow.".into()))?;
        }
    }
    if output.is_empty() {
        return Err(DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {} snapshot has no compensation segments.",
            payroll.accrualId
        )));
    }
    // The calculation seam reuses current NORMAL formulas. Its private
    // timeline resolver expects distinct IDs, which the derived replay IDs
    // above provide; no derived ID is persisted as a policy version.
    let _ = period;
    Ok(output)
}

fn target_income_for_period(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel: &Personel,
    period: &BordroDonemi,
    payment_date: NaiveDate,
    applications: &[RevisionApplication],
) -> Result<TargetIncomeForPeriod> {
    let source = authoritative_normal_payroll(index, dataset, &personnel.id, &period.id)?;
    if source.calculationMetadata.is_some() {
        let snapshot = source
            .calculationMetadata
            .as_ref()
            .and_then(|metadata| metadata.snapshot.as_ref())
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} has current-engine metadata without a snapshot.",
                    source.accrualId
                ))
            })?;
        let original_segments = &snapshot.compensation.segments;
        let (replayed_original, _, _) = replay_saved_normal_compensation_income(
            source,
            period,
            original_segments,
            &dataset.payrolls,
            &dataset.periods,
        )?;
        if !same_income_values(&replayed_original, &source.gelirler) {
            return Err(DomainError::InvalidData(format!(
                "{} persisted NORMAL income does not match replay from its schema-2 snapshot.",
                period.id
            )));
        }
        let target_segments = target_snapshot_segments(source, period, applications)?;
        let (income, meal_exemption, meal_income_tax_exempt) =
            replay_saved_normal_compensation_income(
                source,
                period,
                &target_segments,
                &dataset.payrolls,
                &dataset.periods,
            )?;
        return Ok(TargetIncomeForPeriod {
            income,
            meal_exemption,
            meal_income_tax_exempt,
        });
    }
    let _ = (payment_date, applications);
    Err(DomainError::InvalidData(format!(
        "LEGACY_REPLAY_UNAVAILABLE: {} snapshotless NORMAL source cannot be replayed from mutable settings or attendance.",
        period.id
    )))
}

fn ensure_revision_scope(revision: &CompensationRevision, personnel: &Personel) -> Result<()> {
    match revision.scope {
        CompensationRevisionScope::ALL_PERSONNEL | CompensationRevisionScope::PERSONNEL_GROUP => {
            Ok(()) // Group scope is resolved from each persisted source snapshot.
        }
        CompensationRevisionScope::SELECTED_PERSONNEL
            if revision.personnelIds.iter().any(|id| id == &personnel.id) =>
        {
            Ok(())
        }
        CompensationRevisionScope::SELECTED_PERSONNEL => Err(DomainError::ValidationError(
            format!("{} personeli revision kapsamı dışında.", personnel.id),
        )),
    }
}

fn original_recognized_by_period_and_code(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
) -> Result<HashMap<RetroEarningCode, Decimal>> {
    let mut result = HashMap::new();
    for payroll in index
        .payrolls_for_person_period(dataset, personnel_id, period_id)
        .filter(|payroll| payroll.accrualType != AccrualType::RETRO_ADJUSTMENT)
    {
        match payroll.status {
            BordroStatus::CALCULATED | BordroStatus::FINALIZED => {
                if payroll.accrualType == AccrualType::NORMAL {
                    let period = index.period(dataset, period_id).ok_or_else(|| {
                        DomainError::NotFound(format!("{} source period bulunamadı.", period_id))
                    })?;
                    let disposition = crate::validate_payroll_calculation_metadata_with_context(
                        payroll,
                        period,
                        &dataset.payrolls,
                        &dataset.periods,
                    )
                    .map_err(|error| {
                        DomainError::InvalidData(format!(
                            "{} source snapshot doğrulanamadı: {error}",
                            payroll.accrualId
                        ))
                    })?;
                    if disposition
                        == crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
                    {
                        let snapshot = payroll
                            .calculationMetadata
                            .as_ref()
                            .and_then(|metadata| metadata.snapshot.as_ref())
                            .ok_or_else(|| {
                                DomainError::InvalidData(format!(
                                    "UNSUPPORTED_HISTORICAL_REPLAY: {} snapshot missing.",
                                    payroll.accrualId
                                ))
                            })?;
                        let (replayed, _, _) = replay_saved_normal_compensation_income(
                            payroll,
                            period,
                            &snapshot.compensation.segments,
                            &dataset.payrolls,
                            &dataset.periods,
                        )?;
                        if !same_income_values(&replayed, &payroll.gelirler) {
                            return Err(DomainError::InvalidData(format!(
                                "{} persisted NORMAL income does not match replay from its schema-2 snapshot.",
                                period_id
                            )));
                        }
                    }
                }
                for (code, value) in income_by_code(&payroll.gelirler) {
                    add_code(&mut result, code, value);
                }
            }
            BordroStatus::DRAFT => {
                return Err(DomainError::ValidationError(format!(
                "{} dönemindeki {} tahakkuku authoritative değil ({:?}); retro hesap durduruldu.",
                period_id, payroll.accrualId, payroll.status
            )))
            }
            // STALE records are superseded and never contribute to an
            // authoritative original entitlement.
            BordroStatus::STALE => continue,
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
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    batch: &RetroAdjustmentBatch,
) -> Result<Vec<RetroAllocation>> {
    let allocations: Vec<RetroAllocation> = index
        .retro_allocations_for_batch(dataset, &batch.id)
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
        let policy = retro_period_earning_policy(
            index,
            dataset,
            &batch.personnelId,
            &allocation.sourcePeriodId,
            allocation.earningCode,
        )?;
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
            if round2(allocation.payableSettlementAmount + allocation.offsetSettlementAmount)
                > round2(positive_delta)
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
        if batch.totalGrossDelta >= Decimal::ZERO && batch.recoverableAmount != Decimal::ZERO {
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
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
) -> Result<Decimal> {
    let batches = index
        .retro_batches_for_person(dataset, personnel_id)
        .filter(|batch| {
            batch.id != current_batch_id
                && matches!(
                    batch.status,
                    CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    let batches = sort_retro_batches_chronologically(batches)?;

    let mut applicable_batches = Vec::new();
    for batch in batches {
        let batch_payment_date = parse_date(&batch.paymentDate, "önceki retro ödeme")?;
        if batch_payment_date > payment_date {
            continue;
        }
        validate_batch_ledger(index, dataset, &batch)?;
        applicable_batches.push(batch);
    }
    Ok(replay_retro_outstanding_receivables(&applicable_batches)?
        .get(personnel_id)
        .copied()
        .unwrap_or(Decimal::ZERO))
}

/// Orders authoritative retro events by their payment date and then their
/// recorded creation instant. IDs provide a stable tie-break for legacy or
/// same-instant events; imported timestamps are read as-is and never rewritten.
fn sort_retro_batches_chronologically(
    batches: Vec<RetroAdjustmentBatch>,
) -> Result<Vec<RetroAdjustmentBatch>> {
    let mut keyed = batches
        .into_iter()
        .map(|batch| {
            let payment_date = parse_date(&batch.paymentDate, "önceki retro ödeme")?;
            let created_at = batch
                .createdAt
                .as_deref()
                .map(DateTime::<FixedOffset>::parse_from_rfc3339)
                .transpose()
                .map_err(|_| {
                    DomainError::InvalidData(format!(
                        "{} retro batch createdAt geçerli RFC3339 timestamp değil.",
                        batch.id
                    ))
                })?;
            Ok((payment_date, created_at, batch))
        })
        .collect::<Result<Vec<_>>>()?;
    keyed.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.id.cmp(&right.2.id))
    });
    Ok(keyed.into_iter().map(|(_, _, batch)| batch).collect())
}

/// Replays persisted receivable snapshots in the same chronological order
/// used by runtime retro calculations. A zero-difference batch carries the
/// prior balance forward, while an OVERPAYMENT batch adds its recoverable
/// amount even when it has no payment event.
pub fn replay_retro_outstanding_receivables(
    batches: &[RetroAdjustmentBatch],
) -> Result<HashMap<String, Decimal>> {
    replay_retro_outstanding_receivables_with_legacy_flow(batches, true)
}

/// Validates a current V5 receivable snapshot. Unlike runtime reads of legacy
/// rows, V5 explicitly persists every settlement-flow field, so an all-zero
/// flow is a real zero-difference batch and its outstanding snapshot is still
/// checked.
pub fn validate_v5_retro_outstanding_receivables(batches: &[RetroAdjustmentBatch]) -> Result<()> {
    replay_retro_outstanding_receivables_with_legacy_flow(batches, false).map(|_| ())
}

fn replay_retro_outstanding_receivables_with_legacy_flow(
    batches: &[RetroAdjustmentBatch],
    allow_legacy_flow: bool,
) -> Result<HashMap<String, Decimal>> {
    let batches = sort_retro_batches_chronologically(batches.to_vec())?;
    let mut outstanding_by_personnel = HashMap::<String, Decimal>::new();

    for batch in batches.into_iter().filter(|batch| {
        matches!(
            batch.status,
            CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
        )
    }) {
        let personnel_id = batch.personnelId.clone();
        let outstanding = outstanding_by_personnel
            .get(&personnel_id)
            .copied()
            .unwrap_or(Decimal::ZERO);
        let legacy = allow_legacy_flow && has_legacy_settlement_flow(&batch);
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
        let replayed = round2(available - offset - recovered);
        if !legacy && round2(batch.outstandingReceivable) != replayed {
            return Err(DomainError::InvalidData(format!(
                "{} outstanding receivable snapshot'ı kronolojik settlement replay ile eşleşmiyor.",
                batch.id
            )));
        }
        outstanding_by_personnel.insert(personnel_id, replayed);
    }

    Ok(outstanding_by_personnel)
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
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
) -> Result<(RetroAmountsByPeriodAndCode, RetroAmountsByPeriodAndCode)> {
    let mut previous = HashMap::new();
    let mut previous_pek = HashMap::new();
    let batches = index
        .retro_batches_for_person(dataset, personnel_id)
        .filter(|batch| {
            batch.id != current_batch_id
                && !matches!(
                    batch.status,
                    CompensationRevisionStatus::DRAFT | CompensationRevisionStatus::STALE
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    let batches = sort_retro_batches_chronologically(batches)?;
    for batch in batches {
        let batch_payment_date = parse_date(&batch.paymentDate, "önceki retro ödeme")?;
        if batch_payment_date > payment_date {
            continue;
        }
        for allocation in validate_batch_ledger(index, dataset, &batch)? {
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
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
    source_period_ids: &HashSet<String>,
) -> Result<()> {
    for batch in index
        .retro_batches_for_person(dataset, personnel_id)
        .filter(|batch| {
            batch.id != current_batch_id
                && matches!(
                    batch.status,
                    CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
                )
        })
    {
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
        let allocations = validate_batch_ledger(index, dataset, batch)?;
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
    meal_exemption: Decimal,
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
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<ResolvedStatutorySnapshot> {
    let normal = authoritative_normal_payroll(index, dataset, personnel_id, &period.id)?;
    let disposition = crate::validate_payroll_calculation_metadata_with_context(
        normal,
        period,
        &dataset.payrolls,
        &dataset.periods,
    )
    .map_err(|error| DomainError::InvalidData(error.to_string()))?;
    if disposition == crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated {
        return normal.statutorySnapshot.clone().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} validated NORMAL source has no persisted statutory output.",
                period.id
            ))
        });
    }
    if let Some(snapshot) = normal.statutorySnapshot.clone() {
        return Ok(snapshot);
    }
    Err(DomainError::InvalidData(format!(
        "LEGACY_REPLAY_UNAVAILABLE: {} NORMAL source has no persisted statutory snapshot.",
        normal.accrualId
    )))
}

/// Monetary/statutory context used by the PEK formula. For schema-2 source
/// payrolls every consumed field is populated from the persisted snapshot;
/// the default constructor is used only as a struct initializer and no
/// calculation field is left at its financial default.
fn source_replay_settings(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<PayrollPeriodInstitutionSettings> {
    let normal = authoritative_normal_payroll(index, dataset, personnel_id, &period.id)?;
    let disposition = crate::validate_payroll_calculation_metadata_with_context(
        normal,
        period,
        &dataset.payrolls,
        &dataset.periods,
    )
    .map_err(|error| DomainError::InvalidData(error.to_string()))?;
    if disposition == crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated {
        let snapshot = normal
            .calculationMetadata
            .as_ref()
            .and_then(|metadata| metadata.snapshot.as_ref())
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} snapshot is missing.",
                    normal.accrualId
                ))
            })?;
        let used = &snapshot.usedInputs;
        let statutory = &used.statutoryParameters;
        let deduction = &used.institutionDeductionSettings;
        return Ok(PayrollPeriodInstitutionSettings {
            donemId: period.id.clone(),
            yemekTamGvIstisnaDamgaTabi: deduction.yemekTamGvIstisnaDamgaTabi,
            tediyeListesi: None,
            tisIkramiyeListesi: None,
            tediyeTisNotu: None,
            sgkIsciOraniYuzde: Some(statutory.sgkIsciOraniYuzde),
            issizlikIsciOraniYuzde: Some(statutory.issizlikIsciOraniYuzde),
            gelirVergisiOraniYuzde: None,
            damgaVergisiOraniBinde: deduction.damgaVergisiOraniBinde,
            sendikaAidatiYuzde: deduction.sendikaAidatiYuzde,
            sabitSendikaAidati: deduction.sabitSendikaAidati,
            besOraniYuzde: deduction.besOraniYuzde,
            sabitBesTutar: deduction.sabitBesTutar,
            gunlukYemekIstisnasiSGK: Some(statutory.gunlukYemekIstisnasiSGK),
            gunlukYemekIstisnasiGV: Some(statutory.gunlukYemekIstisnasiGV),
            statutoryParameterSegments: None,
            statutoryParameterSnapshot: None,
            pekTavanKatsayisi: Some(statutory.pekTavanKatsayisi),
            gunlukAsgariUcret: Some(statutory.gunlukAsgariUcret),
            sgkIsverenOraniYuzde: statutory.sgkIsverenOraniYuzde,
            issizlikIsverenOraniYuzde: statutory.issizlikIsverenOraniYuzde,
        });
    }
    Err(DomainError::InvalidData(format!(
        "LEGACY_REPLAY_UNAVAILABLE: {} source has no exact persisted replay settings.",
        normal.accrualId
    )))
}

fn source_period_events(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<Vec<BordroKaydi>> {
    let events = index
        .payrolls_for_person_period(dataset, personnel_id, &period.id)
        .filter(|payroll| payroll.accrualType != AccrualType::RETRO_ADJUSTMENT)
        .filter(|payroll| payroll.status != BordroStatus::STALE)
        .cloned()
        .collect::<Vec<_>>();
    for event in &events {
        ensure_authoritative_payment_event(event)?;
    }
    let mut ordered = events
        .into_iter()
        .map(|event| {
            let order = accrual_order_for_payroll_with_index(dataset, index, &event)?;
            Ok((order, event))
        })
        .collect::<Result<Vec<_>>>()?;
    ordered.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(ordered.into_iter().map(|(_, event)| event).collect())
}

fn schema2_source_normal<'a>(
    index: &'a PayrollDatasetIndex,
    dataset: &'a PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<Option<&'a BordroKaydi>> {
    let normal = authoritative_normal_payroll(index, dataset, personnel_id, &period.id)?;
    let disposition = crate::validate_payroll_calculation_metadata_with_context(
        normal,
        period,
        &dataset.payrolls,
        &dataset.periods,
    )
    .map_err(|error| DomainError::InvalidData(error.to_string()))?;
    Ok(
        (disposition == crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated)
            .then_some(normal),
    )
}

/// Schema-2 NORMAL owns the compensation snapshot; other authoritative
/// payment events own their persisted payment, statutory and PEK outputs.
/// Do not require NORMAL-only calculation metadata on those events and do not
/// fill missing event inputs from today's settings.
fn validate_schema2_source_events(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    events: &[BordroKaydi],
    authoritative_normal: &BordroKaydi,
) -> Result<()> {
    let mut normal_count = 0;
    for event in events {
        ensure_authoritative_payment_event(event)?;
        if event.personelId != personnel_id || event.donemId != period.id {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} payment identity does not match its source period.",
                event.accrualId
            )));
        }
        if event.accrualId.trim().is_empty() || event.paymentDate.trim().is_empty() {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} payment identity/date is incomplete.",
                period.id
            )));
        }
        let event_period = index.period(dataset, &event.donemId).ok_or_else(|| {
            DomainError::NotFound(format!("{} payment period not found.", event.donemId))
        })?;
        accrual_order_for_payroll_with_index(dataset, index, event)?;

        if event.accrualType == AccrualType::NORMAL {
            normal_count += 1;
            if event.accrualId != authoritative_normal.accrualId {
                return Err(DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} is not the authoritative NORMAL event.",
                    event.accrualId
                )));
            }
            let disposition = crate::validate_payroll_calculation_metadata_with_context(
                event,
                event_period,
                &dataset.payrolls,
                &dataset.periods,
            )
            .map_err(|error| {
                DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} NORMAL snapshot invalid: {error}",
                    event.accrualId
                ))
            })?;
            if disposition != crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
            {
                return Err(DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} authoritative NORMAL lacks a validated schema-2 snapshot.",
                    event.accrualId
                )));
            }
        } else if event
            .calculationMetadata
            .as_ref()
            .is_some_and(|metadata| metadata.snapshot.is_some())
        {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} non-NORMAL event carries an unsupported compensation snapshot.",
                event.accrualId
            )));
        }

        crate::validate_payroll_snapshot_authority(event).map_err(|error| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} persisted payment snapshot invalid: {error}",
                event.accrualId
            ))
        })?;
        if event.gelirToplam != calculate_gelir_toplam(&event.gelirler)
            || event.kesintiToplam != calculate_kesinti_toplam(&event.kesintiler)
            || event.netOdeme != round2(event.gelirToplam - event.kesintiToplam)
        {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} persisted payment totals do not reconcile.",
                event.accrualId
            )));
        }
        let detail = event.pekDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} lacks persisted PEK outputs.",
                event.accrualId
            ))
        })?;
        let statutory = event.statutorySnapshot.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} lacks its persisted statutory inputs.",
                event.accrualId
            ))
        })?;
        if event.devredenPekGelen.is_none()
            || event.sonrakiDevredenPek.is_none()
            || detail.aylikOncekiPekTuketimi.is_none()
            || detail.aylikSonrasiPekTuketimi.is_none()
            || detail.sgkIsverenOraniYuzde.is_none()
            || detail.isverenIssizlikOraniYuzde.is_none()
            || statutory.sgkIsciOraniYuzde.is_none()
            || statutory.issizlikIsciOraniYuzde.is_none()
            || statutory.segments.is_empty()
            || event.gvDetay.is_none()
            || event.persistedGvBase.is_none()
            || event.damgaDetay.is_none()
        {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} lacks immutable payment/statutory/PEK/GV inputs required for replay.",
                event.accrualId
            )));
        }
        if detail.pekUstSinir != statutory.pekUstSinir
            || detail.pekAltSinir != statutory.pekAltSinir
        {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} persisted PEK limits disagree with its statutory snapshot.",
                event.accrualId
            )));
        }
    }
    if normal_count != 1 {
        return Err(DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {} schema-2 source must contain exactly one authoritative NORMAL event.",
            period.id
        )));
    }
    Ok(())
}

fn source_event_replay_settings(
    base: &PayrollPeriodInstitutionSettings,
    event: &BordroKaydi,
) -> Result<PayrollPeriodInstitutionSettings> {
    let statutory = event.statutorySnapshot.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {} statutory snapshot is missing.",
            event.accrualId
        ))
    })?;
    let detail = event.pekDetay.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {} PEK snapshot is missing.",
            event.accrualId
        ))
    })?;
    let mut settings = base.clone();
    settings.sgkIsciOraniYuzde = statutory.sgkIsciOraniYuzde;
    settings.issizlikIsciOraniYuzde = statutory.issizlikIsciOraniYuzde;
    settings.sgkIsverenOraniYuzde = detail.sgkIsverenOraniYuzde;
    settings.issizlikIsverenOraniYuzde = detail.isverenIssizlikOraniYuzde;
    if settings.sgkIsciOraniYuzde.is_none()
        || settings.issizlikIsciOraniYuzde.is_none()
        || settings.sgkIsverenOraniYuzde.is_none()
        || settings.issizlikIsverenOraniYuzde.is_none()
    {
        return Err(DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {} lacks persisted statutory contribution rates.",
            event.accrualId
        )));
    }
    Ok(settings)
}

fn calendar_tax_month_ordinal(event: &BordroKaydi) -> Result<i64> {
    let date = NaiveDate::parse_from_str(&event.paymentDate, "%Y-%m-%d").map_err(|_| {
        DomainError::InvalidData(format!(
            "UNSUPPORTED_HISTORICAL_REPLAY: {} has an invalid payment date.",
            event.accrualId
        ))
    })?;
    Ok(i64::from(date.year()) * 12 + i64::from(date.month0()))
}

fn attendance_summary_for_source(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<PuantajOzeti> {
    let attendance = historical_attendance(index, dataset, personnel_id, &period.id)?;
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
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    events: &[BordroKaydi],
) -> Result<SourcePekReplay> {
    let schema2_normal = schema2_source_normal(index, dataset, personnel_id, period)?;
    if let Some(normal) = schema2_normal {
        validate_schema2_source_events(index, dataset, personnel_id, period, events, normal)?;
    }
    let mut state = SourcePekState::default();
    for payroll in events {
        let detail = payroll.pekDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} tahakkukunda historical PEK snapshot'ı eksik.",
                payroll.accrualId
            ))
        })?;
        state.worker_pek = round2(state.worker_pek + detail.primMatrahi);
        state.meal_exemption = round2(state.meal_exemption + detail.yemekIstisnasiTutar);
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
        if schema2_normal.is_none()
            && state.upper != Decimal::ZERO
            && round2(state.upper) != round2(detail.pekUstSinir)
        {
            return Err(DomainError::InvalidData(format!(
                "{} source period PEK tavan snapshot'ları çelişkili.",
                period.id
            )));
        }
        state.upper = detail.pekUstSinir;
    }
    if schema2_normal.is_none() {
        let settings = source_replay_settings(index, dataset, personnel_id, period)?;
        let (employer_sgk, employer_unemployment, employer_lower_bound_premium) =
            canonical_source_employer_premiums(
                state.worker_pek,
                state.employer_lower_bound,
                &settings,
            )?;
        state.employer_sgk = employer_sgk;
        state.employer_unemployment = employer_unemployment;
        state.employer_lower_bound_premium = employer_lower_bound_premium;
        if state.upper == Decimal::ZERO {
            state.upper =
                source_statutory_snapshot(index, dataset, personnel_id, period)?.pekUstSinir;
        }
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
    settings: &PayrollPeriodInstitutionSettings,
) -> Result<(Decimal, Decimal, Decimal)> {
    let employer_sgk_rate = settings
        .sgkIsverenOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("Historical SGK işveren oranı eksik.".into()))?
        / dec!(100);
    let employer_unemployment_rate = settings.issizlikIsverenOraniYuzde.ok_or_else(|| {
        DomainError::InvalidData("Historical işveren işsizlik oranı eksik.".into())
    })? / dec!(100);
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
    let lower_bound_sgk =
        round_sgk_amount(employer_lower_bound.max(Decimal::ZERO) * worker_sgk_rate);
    let lower_bound_unemployment =
        round_sgk_amount(employer_lower_bound.max(Decimal::ZERO) * worker_unemployment_rate);
    Ok((
        employer_sgk,
        employer_unemployment,
        lower_bound_sgk + lower_bound_unemployment,
    ))
}

fn source_target_state(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    events: &[BordroKaydi],
    target_normal_income: &GelirKalemleri,
    target_meal_exemption: MealExemptionTotals,
) -> Result<SourcePekReplay> {
    let settings = source_replay_settings(index, dataset, personnel_id, period)?;
    let statutory_fallback = source_statutory_snapshot(index, dataset, personnel_id, period)?;
    let mut ordered_events = events.to_vec();
    let authoritative_normal =
        authoritative_normal_payroll(index, dataset, personnel_id, &period.id)?;
    let normal_disposition = crate::validate_payroll_calculation_metadata_with_context(
        authoritative_normal,
        period,
        &dataset.payrolls,
        &dataset.periods,
    )
    .map_err(|error| DomainError::InvalidData(error.to_string()))?;
    let snapshot_replay = normal_disposition
        == crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated;
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
        let synthetic_summary =
            attendance_summary_for_source(index, dataset, personnel_id, period)?;
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
            persistedGvBase: None,
            damgaDetay: None,
            statutorySnapshot: Some(statutory_fallback.clone()),
            odenenRaporluGun: None,
            raporluGun: None,
            calculationMetadata: None,
        });
        ordered_events.sort_by_key(|event| (event.sequence, event.accrualId.clone()));
    }

    if snapshot_replay {
        // NORMAL owns the schema-2 compensation snapshot. Supplementary
        // payment events remain authoritative through their own saved payment,
        // statutory and PEK fields and need not carry a NORMAL-only snapshot.
        if ordered_events.is_empty() {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} schema-2 source has no authoritative payment events.",
                period.id
            )));
        }
        validate_schema2_source_events(
            index,
            dataset,
            personnel_id,
            period,
            &ordered_events,
            authoritative_normal,
        )?;
        let first_event = &ordered_events[0];
        let first_carry = first_event.devredenPekGelen.clone().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} first payment event lacks incoming PEK carry.",
                first_event.accrualId
            ))
        })?;
        validate_retro_carry_snapshot(&first_carry, "schema-2 incoming PEK carry")?;
        let (first_incoming, _) =
            incoming_devreden_pek_for_replay(dataset, index, personnel_id, period, first_event)?;
        if first_incoming.records != first_carry {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} first payment event incoming carry disagrees with the saved payment history.",
                first_event.accrualId
            )));
        }
        let first_detail = first_event.pekDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} first payment event PEK snapshot is missing.",
                first_event.accrualId
            ))
        })?;
        first_detail.aylikOncekiPekTuketimi.ok_or_else(|| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} first payment event same-month PEK input is missing.",
                first_event.accrualId
            ))
        })?;
        let target_normal_income =
            source_income_target(&authoritative_normal.gelirler, target_normal_income);
        let normal_snapshot = authoritative_normal
            .calculationMetadata
            .as_ref()
            .and_then(|metadata| metadata.snapshot.as_ref())
            .expect("schema2_normal was validated above");
        let normal_incoming = normal_snapshot
            .usedInputs
            .incomingPekCarry
            .records
            .iter()
            .map(|item| DevredenPekKaydi {
                tutar: item.tutar,
                kalanAySayisi: item.kalanAySayisi,
                kaynakDonemId: item.kaynakDonemId.clone(),
            })
            .collect::<Vec<_>>();
        let normal_detail = authoritative_normal.pekDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} NORMAL PEK snapshot is missing.",
                authoritative_normal.accrualId
            ))
        })?;
        if authoritative_normal.devredenPekGelen.as_ref() != Some(&normal_incoming)
            || normal_detail.aylikOncekiPekTuketimi
                != Some(
                    normal_snapshot
                        .usedInputs
                        .taxHistory
                        .samePaymentMonthPekUsed,
                )
        {
            return Err(DomainError::InvalidData(format!(
                "UNSUPPORTED_HISTORICAL_REPLAY: {} NORMAL payment inputs disagree with its schema-2 snapshot.",
                authoritative_normal.accrualId
            )));
        }
        let mut carry = first_carry;
        let mut previous_tax_month = calendar_tax_month_ordinal(first_event)?;
        let mut accumulated_source_delta_by_month = HashMap::<i64, Decimal>::new();
        let mut state = SourcePekState {
            upper: first_detail.pekUstSinir,
            ..SourcePekState::default()
        };
        for event in &ordered_events {
            let tax_month = calendar_tax_month_ordinal(event)?;
            let event_month_to_date = event
                .pekDetay
                .as_ref()
                .and_then(|detail| detail.aylikOncekiPekTuketimi)
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "UNSUPPORTED_HISTORICAL_REPLAY: {} same-month PEK input is missing.",
                        event.accrualId
                    ))
                })?;
            let month_to_date = round2(
                event_month_to_date
                    + accumulated_source_delta_by_month
                        .get(&tax_month)
                        .copied()
                        .unwrap_or_default(),
            );
            let actual_incoming = event.devredenPekGelen.as_ref().ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} incoming PEK carry is missing.",
                    event.accrualId
                ))
            })?;
            let (persisted_incoming, _) =
                incoming_devreden_pek_for_replay(dataset, index, personnel_id, period, event)?;
            if &persisted_incoming.records != actual_incoming {
                return Err(DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} incoming carry disagrees with saved payment history.",
                    event.accrualId
                )));
            }
            let elapsed_since_previous = if event.accrualId == first_event.accrualId {
                first_incoming.tax_months_elapsed
            } else {
                let elapsed = tax_month - previous_tax_month;
                if elapsed < 0 || elapsed > i64::from(i32::MAX) {
                    return Err(DomainError::InvalidData(format!(
                        "UNSUPPORTED_HISTORICAL_REPLAY: {} payment order has an invalid tax-month gap.",
                        event.accrualId
                    )));
                }
                elapsed as i32
            };
            let is_normal = event.accrualType == AccrualType::NORMAL;
            let income = if is_normal {
                &target_normal_income
            } else {
                &event.gelirler
            };
            let statutory = event.statutorySnapshot.clone().ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "UNSUPPORTED_HISTORICAL_REPLAY: {} validated event has no statutory snapshot.",
                    event.accrualId
                ))
            })?;
            let event_settings = source_event_replay_settings(&settings, event)?;
            let (detail, next_carry) =
                calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state(
                    income,
                    Some(&event.puantajOzeti),
                    Some(&event_settings),
                    &carry,
                    Some(&statutory),
                    month_to_date,
                    PekCalculationOptions {
                        tax_months_elapsed: elapsed_since_previous,
                        apply_lower_bound: is_normal,
                        meal_exemption: is_normal.then_some(target_meal_exemption),
                    },
                )?;
            let original_prim = event
                .pekDetay
                .as_ref()
                .expect("schema2 source validation requires PEK details")
                .primMatrahi;
            *accumulated_source_delta_by_month
                .entry(tax_month)
                .or_default() += detail.primMatrahi - original_prim;
            state.worker_pek = round2(state.worker_pek + detail.primMatrahi);
            state.meal_exemption = round2(state.meal_exemption + detail.yemekIstisnasiTutar);
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
            carry = next_carry;
            previous_tax_month = tax_month;
        }
        return Ok(SourcePekReplay {
            state,
            outgoing_carry: carry,
        });
    }

    let first_event = ordered_events.first().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} source period replay'i için payment event bulunamadı.",
            period.id
        ))
    })?;
    let (canonical_incoming, canonical_incoming_available) =
        incoming_devreden_pek_for_replay(dataset, index, personnel_id, period, first_event)?;
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
                Some(&settings),
                &carry,
                Some(&statutory),
                month_to_date,
                PekCalculationOptions {
                    tax_months_elapsed,
                    apply_lower_bound: is_normal,
                    meal_exemption: is_normal.then_some(target_meal_exemption),
                },
            )?;
        state.worker_pek = round2(state.worker_pek + detail.primMatrahi);
        state.meal_exemption = round2(state.meal_exemption + detail.yemekIstisnasiTutar);
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
            &settings,
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
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    payment_date: NaiveDate,
    current_batch_id: &str,
    source_period_id: &str,
) -> Result<SourcePekState> {
    let batches = index
        .retro_batches_for_person(dataset, personnel_id)
        .filter(|batch| {
            batch.id != current_batch_id
                && matches!(
                    batch.status,
                    CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    let batches = sort_retro_batches_chronologically(batches)?;

    let mut state = SourcePekState::default();
    for batch in batches {
        if parse_date(&batch.paymentDate, "önceki retro ödeme")? > payment_date {
            continue;
        }
        for allocation in validate_batch_ledger(index, dataset, &batch)? {
            if allocation.sourcePeriodId != source_period_id
                || allocation.sgkTreatment != RetroSgkTreatment::WAGE_SOURCE_MONTH
            {
                continue;
            }
            state.worker_pek = round2(state.worker_pek + allocation.retroPekDelta);
            state.worker_sgk = round_sgk_amount(state.worker_sgk + allocation.workerSgkDelta);
            state.worker_unemployment =
                round_sgk_amount(state.worker_unemployment + allocation.workerUnemploymentDelta);
            state.employer_sgk = round_sgk_amount(state.employer_sgk + allocation.employerSgkDelta);
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
    allocation: &RetroAllocation,
    original_meal_exemption: Decimal,
    target_meal_exemption: Decimal,
) -> Result<Decimal> {
    if allocation.earningCode != RetroEarningCode::MEAL {
        return Ok(allocation.deltaAmount);
    }
    let recognized_before =
        allocation.originalRecognizedAmount + allocation.previousAuthoritativeRetroAmount;
    // The source and target exemptions are produced by the corresponding
    // canonical segment replay. Never collapse the two segment capacities into
    // one aggregate min(total meal, total capacity) here.
    let subject_before = (recognized_before - original_meal_exemption).max(Decimal::ZERO);
    let subject_after = (allocation.targetAmount - target_meal_exemption).max(Decimal::ZERO);
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

struct SourceMonthSgkPayment<'a> {
    payment_date: NaiveDate,
    current_batch_id: &'a str,
}

struct SourceMonthSgkTarget<'a> {
    normal_income: &'a GelirKalemleri,
    meal_exemption: MealExemptionTotals,
}

fn apply_source_month_sgk(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    target: SourceMonthSgkTarget<'_>,
    allocations: &mut [RetroAllocation],
    payment: SourceMonthSgkPayment<'_>,
) -> Result<()> {
    let events = source_period_events(index, dataset, personnel_id, period)?;
    let original_replay = source_original_state(index, dataset, personnel_id, period, &events)?;
    let target_replay = source_target_state(
        index,
        dataset,
        personnel_id,
        period,
        &events,
        target.normal_income,
        target.meal_exemption,
    )?;
    let original = original_replay.state;
    let target = target_replay.state;
    let original_meal_exemption = original.meal_exemption;
    let target_meal_exemption = target.meal_exemption;
    let previous = previous_source_retro_state(
        index,
        dataset,
        personnel_id,
        payment.payment_date,
        payment.current_batch_id,
        &period.id,
    )?;
    let settings = source_replay_settings(index, dataset, personnel_id, period)?;
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
        .map(|allocation_index| {
            source_eligible_delta(
                &allocations[*allocation_index],
                original_meal_exemption,
                target_meal_exemption,
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
    let index = PayrollDatasetIndex::build(dataset);
    retro_payment_income_with_index(dataset, &index, batch_id)
}

pub(crate) fn retro_payment_income_with_index(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    batch_id: &str,
) -> Result<(
    RetroAdjustmentBatch,
    Vec<RetroAllocation>,
    GelirKalemleri,
    GelirKalemleri,
)> {
    let batch = index
        .retro_batch(dataset, batch_id)
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
    let mut allocations = validate_batch_ledger(index, dataset, &batch)?;
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
        let index = PayrollDatasetIndex::build(&request.dataset);
        let payment_date = parse_date(&request.paymentDate, "retro ödeme")?;
        let effective_from = parse_date(&request.revision.effectiveFrom, "revision yürürlük")?;
        if effective_from > payment_date {
            return Err(DomainError::ValidationError(
                "Revision yürürlük tarihi retro ödeme tarihinden sonra olamaz.".into(),
            ));
        }
        let current_effective_to = request
            .revision
            .effectiveTo
            .as_deref()
            .map(|value| parse_date(value, "revision bitiş"))
            .transpose()?;
        if current_effective_to.is_some_and(|effective_to| effective_to < effective_from) {
            return Err(DomainError::ValidationError(
                "Revision bitiş tarihi yürürlük tarihinden önce olamaz.".into(),
            ));
        }
        let personnel = index
            .personnel(&request.dataset, &request.personnelId)
            .ok_or_else(|| DomainError::NotFound("Retro personeli bulunamadı.".into()))?;
        ensure_revision_scope(&request.revision, personnel)?;
        if request.batchId.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "Retro batch kimliği boş olamaz.".into(),
            ));
        }
        if let Some(existing_batch) = index.retro_batch(&request.dataset, &request.batchId) {
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
            &index,
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
                    // A retro calculation may only use a closed service period.
                    // The payment month is an event month, not permission to
                    // replay an open 15-14 period through the payment day.
                    end >= earliest_effective_from
                        && end <= payment_date
                        && start <= payment_date
                        // BUG-RETRO-001: the payment date is only the event
                        // (tax/payment) date. It must never widen the set of
                        // service periods affected by this revision. A source
                        // period is a candidate only while it intersects the
                        // revision's closed [effectiveFrom, effectiveTo]
                        // interval, i.e. it starts on or before effectiveTo.
                        // A period that partially overlaps the end date stays
                        // in scope; its day-level replay only changes the
                        // covered days. Open-ended revisions keep the
                        // closed-period-through-payment rule above.
                        && source_period_intersects_revision_end(start, current_effective_to)
                        // Historical revisions can predate this person's
                        // payroll history. Only replay a period when this
                        // person has source attendance or a source payroll;
                        // if a payroll exists but its required attendance is
                        // missing, downstream source validation still fails
                        // closed.
                        && (index.attendances(&request.dataset, &request.personnelId, &period.id).next().is_some()
                            || index
                                .payrolls_for_person_period(
                                    &request.dataset,
                                    &request.personnelId,
                                    &period.id,
                                )
                                .any(|payroll| payroll.accrualType != AccrualType::RETRO_ADJUSTMENT))
                })
            })
            .cloned()
            .collect();
        periods.sort_by_key(|period| (period.baslangicTarihi.clone(), period.id.clone()));
        let candidate_period_count = periods.len();
        let mut scoped_periods = Vec::new();
        for period in periods {
            let source = authoritative_normal_payroll(
                &index,
                &request.dataset,
                &request.personnelId,
                &period.id,
            )?;
            let source_group = source_personnel_group(source, personnel)?;
            let period_start = parse_date(&period.baslangicTarihi, "dönem başlangıç")?;
            let period_end = parse_date(&period.bitisTarihi, "dönem bitiş")?;
            let has_applicable_revision = applications.iter().any(|application| {
                period_start <= application.effective_to.unwrap_or(period_end)
                    && period_end >= application.effective_from
                    && scope_matches_source_group(
                        &application.revision,
                        &personnel.id,
                        &source_group,
                    )
            });
            if scope_matches_source_group(&request.revision, &personnel.id, &source_group)
                && has_applicable_revision
            {
                scoped_periods.push(period);
            }
        }
        let periods = scoped_periods;
        if periods.is_empty() {
            if candidate_period_count > 0
                && request.revision.scope == CompensationRevisionScope::PERSONNEL_GROUP
            {
                return Err(DomainError::ValidationError(format!(
                    "{} personeli {} grubu kapsamı dışında.",
                    personnel.id,
                    request
                        .revision
                        .personnelGroup
                        .as_deref()
                        .unwrap_or_default()
                )));
            }
            return Err(DomainError::NotFound(
                "Revision için etkilenen authoritative hizmet dönemi bulunamadı.".into(),
            ));
        }

        // A service period's tax metadata is an input to the eventual payment
        // event. Do not let a malformed period produce a seemingly valid
        // retro ledger that will later use a different tax month.
        for period in &periods {
            validate_tax_month_overlap(period)?;
            ensure_authoritative_normal_baseline(
                &index,
                &request.dataset,
                &request.personnelId,
                period,
            )?;
        }

        let source_period_ids = periods
            .iter()
            .map(|period| period.id.clone())
            .collect::<HashSet<_>>();
        reject_later_authoritative_retro_for_same_source_period(
            &index,
            &request.dataset,
            &request.personnelId,
            payment_date,
            &request.batchId,
            &source_period_ids,
        )?;

        let (previous_retro, _previous_pek) = previous_authoritative_retro_by_period_and_code(
            &index,
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
                &index,
                &request.dataset,
                personnel,
                period,
                payment_date,
                &applications,
            )?;
            target_income_by_period.insert(period.id.clone(), target_income.clone());
            let original = original_recognized_by_period_and_code(
                &index,
                &request.dataset,
                &request.personnelId,
                &period.id,
            )?;

            let mut target_by_code = income_by_code(&target_income.income);
            // Preserve independently paid legacy events. They are already part
            // of recognized entitlement and are not regenerated by shadow
            // NORMAL replay.
            for payroll in index
                .payrolls_for_person_period(&request.dataset, &request.personnelId, &period.id)
                .filter(|payroll| {
                    payroll.accrualType != AccrualType::NORMAL
                        && payroll.accrualType != AccrualType::RETRO_ADJUSTMENT
                        && matches!(
                            payroll.status,
                            BordroStatus::CALCULATED | BordroStatus::FINALIZED
                        )
                })
            {
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
                let policy = retro_earning_policy_with_meal_tax_policy(
                    code,
                    target_income.meal_income_tax_exempt,
                );
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
            let target_income = target_income_by_period.get(period_id).ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} retro target income snapshot'ı eksik.",
                    period_id
                ))
            })?;
            apply_source_month_sgk(
                &index,
                &request.dataset,
                &request.personnelId,
                period,
                SourceMonthSgkTarget {
                    normal_income: &target_income.income,
                    meal_exemption: target_income.meal_exemption,
                },
                &mut allocations,
                SourceMonthSgkPayment {
                    payment_date,
                    current_batch_id: &request.batchId,
                },
            )?;
        }

        let total = round2(allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
            sum + allocation.deltaAmount
        }));
        let prior_outstanding = outstanding_receivable_before_current(
            &index,
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
pub fn retro_sgk_ledger_totals(allocations: &[RetroAllocation]) -> RetroSgkLedgerTotals {
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
