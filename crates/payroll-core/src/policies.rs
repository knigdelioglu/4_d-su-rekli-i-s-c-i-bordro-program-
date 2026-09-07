//! Pure payroll mutation/invalidation policy shared by native and browser.
//!
//! This module deliberately returns keys and blockers only. Adapters decide
//! how to persist the mutation and how to mark the returned mutable records.

#![allow(non_snake_case)]

use crate::index::PayrollDatasetIndex;
use crate::models::{
    BordroDonemi, BordroStatus, CompensationRevisionStatus, RetroAdjustmentBatch,
    StatutorySnapshotSource,
};
use crate::payroll_engine::{
    accrual_order_for_payroll_with_index as payroll_order, PayrollDatasetSnapshot,
};
use crate::{DomainError, Result};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct PayrollKey {
    pub personnelId: String,
    pub periodId: String,
    #[serde(default)]
    pub accrualId: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum PayrollMutation {
    #[serde(rename = "PERSON")]
    Person { personnelId: String },
    #[serde(rename = "PERSON_PERIOD")]
    PersonPeriod {
        personnelId: String,
        periodId: String,
    },
    #[serde(rename = "PERSON_TAX_YEAR")]
    PersonTaxYear { personnelId: String, taxYear: i32 },
    #[serde(rename = "TAX_YEAR")]
    TaxYear { taxYear: i32 },
    #[serde(rename = "PERIOD")]
    Period { periodId: String },
    #[serde(rename = "PERIOD_FROM_POSITION")]
    PeriodFromPosition {
        startDate: String,
        taxYear: i32,
        taxMonth: i32,
    },
    #[serde(rename = "PERSON_FROM_DATE")]
    PersonFromDate {
        personnelId: String,
        effectiveFrom: String,
    },
    /// A recalculation changes the current node's authoritative output and
    /// can therefore invalidate only later nodes for the same person.
    #[serde(rename = "PAYROLL_CALCULATION")]
    PayrollCalculation {
        personnelId: String,
        periodId: String,
    },
    #[serde(rename = "ACCRUAL_CALCULATION")]
    AccrualCalculation {
        personnelId: String,
        periodId: String,
        accrualId: String,
    },
    #[serde(rename = "ACCRUAL_DELETE")]
    AccrualDelete {
        personnelId: String,
        periodId: String,
        accrualId: String,
    },
    #[serde(rename = "ACCRUAL_INSERT")]
    AccrualInsert {
        personnelId: String,
        periodId: String,
        accrualId: String,
        paymentDate: String,
        sequence: i32,
    },
    /// Saving a source-period retro ledger can revise the outgoing PEK carry
    /// seen by later payment events. The adapter persists the batch first in
    /// its transaction, then asks this policy which downstream events must be
    /// invalidated or whether a FINALIZED event makes the save impossible.
    #[serde(rename = "RETRO_BATCH_SAVE")]
    RetroBatchSave {
        personnelId: String,
        batchId: String,
        paymentDate: String,
    },
    #[serde(rename = "ALL")]
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MutationImpact {
    pub affectedPayrolls: Vec<PayrollKey>,
    pub blockedByFinalized: Vec<PayrollKey>,
    /// Retro entitlement ledgers whose source inputs or settlement event were
    /// invalidated by the mutation. Batch ids are globally unique.
    #[serde(default)]
    pub affectedRetroBatches: Vec<String>,
    #[serde(default)]
    pub blockedByFinalizedRetroBatches: Vec<String>,
}

fn period_for<'a>(
    index: &'a PayrollDatasetIndex,
    dataset: &'a PayrollDatasetSnapshot,
    period_id: &str,
) -> Option<&'a BordroDonemi> {
    index.period(dataset, period_id)
}

fn require_period<'a>(
    index: &'a PayrollDatasetIndex,
    dataset: &'a PayrollDatasetSnapshot,
    period_id: &str,
) -> Result<&'a BordroDonemi> {
    period_for(index, dataset, period_id).ok_or_else(|| {
        DomainError::ValidationError(format!("Bordro dönemi bulunamadı: {}", period_id))
    })
}

fn is_period_dependent(candidate: &BordroDonemi, source: &BordroDonemi) -> bool {
    candidate.id == source.id
        || candidate.baslangicTarihi > source.baslangicTarihi
        || (candidate.taxYear == source.taxYear && candidate.taxMonth > source.taxMonth)
}

fn is_from_position(
    candidate: &BordroDonemi,
    start_date: &str,
    tax_year: i32,
    tax_month: i32,
) -> bool {
    candidate.baslangicTarihi.as_str() >= start_date
        || (candidate.taxYear == tax_year && candidate.taxMonth >= tax_month)
}

fn is_person_from_date(candidate: &BordroDonemi, effective_from: &str) -> Result<bool> {
    let effective = NaiveDate::parse_from_str(effective_from, "%Y-%m-%d").map_err(|error| {
        DomainError::ValidationError(format!(
            "Mutation etkinleşme tarihi geçersiz: {} ({})",
            effective_from, error
        ))
    })?;
    let end = NaiveDate::parse_from_str(&candidate.bitisTarihi, "%Y-%m-%d").map_err(|error| {
        DomainError::InvalidData(format!(
            "{} dönemi bitiş tarihi geçersiz: {}",
            candidate.id, error
        ))
    })?;
    Ok(end >= effective)
}

fn statutory_snapshot_source(payroll: &crate::models::BordroKaydi) -> StatutorySnapshotSource {
    payroll
        .statutorySnapshot
        .as_ref()
        .map(|snapshot| snapshot.source)
        .unwrap_or_default()
}

fn is_attendance_dependency_root(payroll: &crate::models::BordroKaydi) -> bool {
    if payroll.accrualType == crate::models::AccrualType::NORMAL {
        return true;
    }

    // A supplementary record with missing provenance is legacy data. Treat it
    // as attendance-dependent for mutation safety rather than guessing that it
    // was independent of attendance.
    matches!(
        statutory_snapshot_source(payroll),
        StatutorySnapshotSource::AttendanceBacked
            | StatutorySnapshotSource::ProvisionalPaymentMonth
            | StatutorySnapshotSource::LegacyUnknown
    )
}

fn is_sick_leave_dependency_root(payroll: &crate::models::BordroKaydi) -> bool {
    payroll.accrualType == crate::models::AccrualType::NORMAL
        || (payroll.accrualType != crate::models::AccrualType::NORMAL
            && statutory_snapshot_source(payroll) == StatutorySnapshotSource::AttendanceBacked)
}

fn affected_after_dependency_roots(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    payroll: &crate::models::BordroKaydi,
    dependency_roots: &[&crate::models::BordroKaydi],
) -> Result<bool> {
    if dependency_roots.is_empty() {
        return Ok(false);
    }
    let candidate_order = payroll_order(dataset, index, payroll)?;
    dependency_roots.iter().try_fold(false, |affected, root| {
        if affected {
            Ok(true)
        } else {
            Ok(candidate_order >= payroll_order(dataset, index, root)?)
        }
    })
}

fn effective_accrual_id(payroll: &crate::models::BordroKaydi) -> String {
    if payroll.accrualId.trim().is_empty() {
        payroll.id.clone()
    } else {
        payroll.accrualId.clone()
    }
}

fn input_order(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    period_id: &str,
    payment_date: &str,
    sequence: i32,
    accrual_id: &str,
) -> Result<crate::payroll_engine::AccrualOrder> {
    crate::payroll_engine::payment_event_order(
        require_period(index, dataset, period_id)?,
        payment_date,
        sequence,
        accrual_id,
    )
}

fn payroll_effective_payment_date(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    payroll: &crate::models::BordroKaydi,
) -> Result<NaiveDate> {
    let period = require_period(index, dataset, &payroll.donemId)?;
    let payment_date = if payroll.paymentDate.trim().is_empty() {
        crate::payroll_engine::default_payment_date(period)
    } else {
        payroll.paymentDate.clone()
    };
    NaiveDate::parse_from_str(&payment_date, "%Y-%m-%d").map_err(|error| {
        DomainError::InvalidData(format!(
            "{} bordro ödeme tarihi geçersiz: {} ({})",
            payroll.id, payment_date, error
        ))
    })
}

fn retro_batch_requires_source_carry_replay(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    batch_id: &str,
) -> bool {
    index
        .retro_allocations_for_batch(dataset, batch_id)
        .filter(|allocation| {
            allocation.batchId == batch_id
                && allocation.sgkTreatment == crate::models::RetroSgkTreatment::WAGE_SOURCE_MONTH
        })
        .any(|allocation| {
            let Some(target) = allocation.targetSourceCarry.as_ref() else {
                // A newly calculated source-month allocation should always
                // carry a canonical target snapshot. Missing provenance is
                // unsafe: force downstream revalidation instead of silently
                // retaining a stale carry chain.
                return true;
            };
            allocation.originalSourceCarry.as_deref() != Some(target.as_slice())
        })
}

fn retro_batch_payment_date(
    batch: &RetroAdjustmentBatch,
    mutation_payment_date: &str,
) -> Result<NaiveDate> {
    if batch.paymentDate != mutation_payment_date {
        return Err(DomainError::InvalidData(format!(
            "{} retro batch ödeme tarihi mutation snapshot'ı ile eşleşmiyor.",
            batch.id
        )));
    }
    NaiveDate::parse_from_str(mutation_payment_date, "%Y-%m-%d").map_err(|error| {
        DomainError::InvalidData(format!(
            "{} retro batch ödeme tarihi geçersiz: {} ({})",
            batch.id, mutation_payment_date, error
        ))
    })
}

fn affected_by_mutation(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    payroll_personnel_id: &str,
    payroll_period_id: &str,
    payroll: &crate::models::BordroKaydi,
    mutation: &PayrollMutation,
) -> Result<bool> {
    let candidate = period_for(index, dataset, payroll_period_id);
    match mutation {
        PayrollMutation::Person { personnelId } => Ok(payroll_personnel_id == personnelId),
        PayrollMutation::PersonPeriod {
            personnelId,
            periodId,
        } => {
            let source = require_period(index, dataset, periodId)?;
            if payroll_personnel_id != personnelId || candidate.is_none() {
                return Ok(false);
            }
            let dependency_roots: Vec<&crate::models::BordroKaydi> = index
                .payrolls_for_person_period(dataset, personnelId, &source.id)
                .filter(|root| {
                    root.personelId == *personnelId
                        && root.donemId == source.id
                        && is_attendance_dependency_root(root)
                })
                .collect();
            affected_after_dependency_roots(index, dataset, payroll, &dependency_roots)
        }
        PayrollMutation::PersonTaxYear {
            personnelId,
            taxYear,
        } => Ok(payroll_personnel_id == personnelId
            && candidate.is_some_and(|candidate| candidate.taxYear == *taxYear)),
        PayrollMutation::TaxYear { taxYear } => {
            Ok(candidate.is_some_and(|candidate| candidate.taxYear == *taxYear))
        }
        PayrollMutation::Period { periodId } => {
            let source = require_period(index, dataset, periodId)?;
            Ok(candidate.is_some_and(|candidate| is_period_dependent(candidate, source)))
        }
        PayrollMutation::PeriodFromPosition {
            startDate,
            taxYear,
            taxMonth,
        } => Ok(candidate
            .is_some_and(|candidate| is_from_position(candidate, startDate, *taxYear, *taxMonth))),
        PayrollMutation::PersonFromDate {
            personnelId,
            effectiveFrom,
        } => {
            if payroll_personnel_id != personnelId {
                return Ok(false);
            }
            if candidate.is_none() {
                return Ok(false);
            }
            let mut dependency_roots = Vec::new();
            for root in index
                .payrolls_for_person(dataset, personnelId)
                .filter(|root| is_sick_leave_dependency_root(root))
            {
                let Some(root_period) = period_for(index, dataset, &root.donemId) else {
                    continue;
                };
                if is_person_from_date(root_period, effectiveFrom)? {
                    dependency_roots.push(root);
                }
            }
            affected_after_dependency_roots(index, dataset, payroll, &dependency_roots)
        }
        PayrollMutation::PayrollCalculation {
            personnelId,
            periodId,
        } => {
            let source = require_period(index, dataset, periodId)?;
            Ok(payroll_personnel_id == personnelId
                && candidate.is_some_and(|candidate| {
                    candidate.id != source.id && is_period_dependent(candidate, source)
                }))
        }
        PayrollMutation::AccrualCalculation {
            personnelId,
            periodId,
            accrualId,
        }
        | PayrollMutation::AccrualDelete {
            personnelId,
            periodId,
            accrualId,
        } => {
            let source = index
                .payrolls_for_person_period(dataset, personnelId, periodId)
                .find(|candidate| effective_accrual_id(candidate) == *accrualId)
                .ok_or_else(|| {
                    DomainError::ValidationError(format!(
                        "Invalidation kaynağı tahakkuk bulunamadı: {}.",
                        accrualId
                    ))
                })?;
            Ok(payroll_personnel_id == personnelId
                && (payroll_order(dataset, index, payroll)?
                    > payroll_order(dataset, index, source)?
                    || (matches!(mutation, PayrollMutation::AccrualDelete { .. })
                        && effective_accrual_id(payroll) == *accrualId)))
        }
        PayrollMutation::AccrualInsert {
            personnelId,
            periodId,
            accrualId,
            paymentDate,
            sequence,
        } => Ok(payroll_personnel_id == personnelId
            && payroll_order(dataset, index, payroll)?
                > input_order(index, dataset, periodId, paymentDate, *sequence, accrualId)?),
        PayrollMutation::RetroBatchSave {
            personnelId,
            batchId,
            paymentDate,
        } => {
            if payroll_personnel_id != personnelId
                || !retro_batch_requires_source_carry_replay(index, dataset, batchId)
            {
                return Ok(false);
            }
            let batch = index.retro_batch(dataset, batchId).ok_or_else(|| {
                DomainError::ValidationError(format!(
                    "Retro carry mutation batch'i bulunamadı: {}.",
                    batchId
                ))
            })?;
            if batch.personnelId != *personnelId {
                return Err(DomainError::InvalidData(format!(
                    "{} retro batch personel kimliği mutation ile eşleşmiyor.",
                    batchId
                )));
            }
            let retro_date = retro_batch_payment_date(batch, paymentDate)?;
            Ok(payroll_effective_payment_date(index, dataset, payroll)? >= retro_date)
        }
        PayrollMutation::All => Ok(true),
    }
}

fn retro_batch_source_period_matches<F>(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    batch: &RetroAdjustmentBatch,
    mut predicate: F,
) -> Result<bool>
where
    F: FnMut(&BordroDonemi) -> Result<bool>,
{
    for allocation in index.retro_allocations_for_batch(dataset, &batch.id) {
        if let Some(period) = index.period(dataset, &allocation.sourcePeriodId) {
            if predicate(period)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn retro_batch_payment_tax_year_matches(
    batch: &RetroAdjustmentBatch,
    tax_year: i32,
) -> Result<bool> {
    let payment_date =
        NaiveDate::parse_from_str(&batch.paymentDate, "%Y-%m-%d").map_err(|error| {
            DomainError::InvalidData(format!(
                "{} retro batch ödeme tarihi geçersiz: {} ({})",
                batch.id, batch.paymentDate, error
            ))
        })?;
    Ok(payment_date.year() == tax_year)
}

fn retro_batch_affected_by_mutation(
    index: &PayrollDatasetIndex,
    dataset: &PayrollDatasetSnapshot,
    batch: &RetroAdjustmentBatch,
    mutation: &PayrollMutation,
) -> Result<bool> {
    if batch.status == CompensationRevisionStatus::STALE {
        return Ok(false);
    }

    let same_source_period = |period_id: &str| {
        index
            .retro_allocations_for_batch(dataset, &batch.id)
            .any(|allocation| allocation.sourcePeriodId == period_id)
    };

    match mutation {
        PayrollMutation::Person { personnelId } => Ok(batch.personnelId == *personnelId),
        PayrollMutation::PersonPeriod {
            personnelId,
            periodId,
        } => Ok(batch.personnelId == *personnelId && same_source_period(periodId)),
        PayrollMutation::PersonTaxYear {
            personnelId,
            taxYear,
        } => {
            if batch.personnelId != *personnelId {
                return Ok(false);
            }
            retro_batch_payment_tax_year_matches(batch, *taxYear)
        }
        PayrollMutation::TaxYear { taxYear } => {
            retro_batch_payment_tax_year_matches(batch, *taxYear)
        }
        PayrollMutation::Period { periodId } => Ok(same_source_period(periodId)),
        PayrollMutation::PeriodFromPosition {
            startDate,
            taxYear,
            taxMonth,
        } => retro_batch_source_period_matches(index, dataset, batch, |period| {
            Ok(is_from_position(period, startDate, *taxYear, *taxMonth))
        }),
        PayrollMutation::PersonFromDate {
            personnelId,
            effectiveFrom,
        } => {
            if batch.personnelId != *personnelId {
                return Ok(false);
            }
            retro_batch_source_period_matches(index, dataset, batch, |period| {
                is_person_from_date(period, effectiveFrom)
            })
        }
        PayrollMutation::PayrollCalculation {
            personnelId,
            periodId,
        } => Ok(batch.personnelId == *personnelId && same_source_period(periodId)),
        PayrollMutation::AccrualCalculation {
            personnelId,
            periodId,
            accrualId,
        } => {
            // Recalculating the settlement node itself must not stale the
            // ledger that is being used to rebuild that same node. A source
            // payroll recalculation still invalidates allocations for its
            // service period.
            Ok(batch.id != *accrualId
                && batch.personnelId == *personnelId
                && same_source_period(periodId))
        }
        PayrollMutation::AccrualDelete {
            personnelId,
            periodId,
            accrualId,
        } => Ok(
            (batch.personnelId == *personnelId && batch.id == *accrualId)
                || (batch.personnelId == *personnelId && same_source_period(periodId)),
        ),
        PayrollMutation::AccrualInsert {
            personnelId,
            periodId,
            accrualId,
            ..
        } => Ok(batch.id != *accrualId
            && batch.personnelId == *personnelId
            && same_source_period(periodId)),
        // The saved batch is already the mutation's new authoritative node.
        // Only downstream payroll events are invalidated by its carry replay;
        // another retro ledger is not implicitly rewritten.
        PayrollMutation::RetroBatchSave { .. } => Ok(false),
        PayrollMutation::All => Ok(true),
    }
}

/// Computes the deterministic invalidation impact and FINALIZED blockers for
/// one source mutation. It performs no persistence and reads no clock.
pub fn evaluate_payroll_invalidation(
    dataset: &PayrollDatasetSnapshot,
    mutation: &PayrollMutation,
) -> Result<MutationImpact> {
    let index = PayrollDatasetIndex::build(dataset);
    evaluate_payroll_invalidation_with_index(dataset, mutation, &index)
}

pub(crate) fn evaluate_payroll_invalidation_with_index(
    dataset: &PayrollDatasetSnapshot,
    mutation: &PayrollMutation,
    index: &PayrollDatasetIndex,
) -> Result<MutationImpact> {
    let mut affected = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut affected_retro_batches = BTreeSet::new();
    let mut blocked_retro_batches = BTreeSet::new();

    for payroll in &dataset.payrolls {
        if !affected_by_mutation(
            index,
            dataset,
            &payroll.personelId,
            &payroll.donemId,
            payroll,
            mutation,
        )? {
            continue;
        }
        let key = PayrollKey {
            personnelId: payroll.personelId.clone(),
            periodId: payroll.donemId.clone(),
            accrualId: effective_accrual_id(payroll),
        };
        if payroll.status == BordroStatus::FINALIZED {
            blocked.insert(key.clone());
        }
        affected.insert(key);
    }

    for batch in &dataset.retroBatches {
        if !retro_batch_affected_by_mutation(index, dataset, batch, mutation)? {
            continue;
        }
        affected_retro_batches.insert(batch.id.clone());
        if batch.status == CompensationRevisionStatus::FINALIZED {
            blocked_retro_batches.insert(batch.id.clone());
        }
    }

    Ok(MutationImpact {
        affectedPayrolls: affected.into_iter().collect(),
        blockedByFinalized: blocked.into_iter().collect(),
        affectedRetroBatches: affected_retro_batches.into_iter().collect(),
        blockedByFinalizedRetroBatches: blocked_retro_batches.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AccrualType, BordroKaydi, CompensationRevisionStatus, DevredenPekKaydi, GelirKalemleri,
        KesintiKalemleri, PuantajOzeti, ResolvedStatutorySnapshot, RetroAdjustmentBatch,
        RetroAllocation, RetroEarningCode, RetroSettlementStatus, RetroSgkTreatment,
        RetroTaxTreatment,
    };
    use crate::payroll_engine::PayrollDatasetSnapshot;

    fn period_for_year(
        id: &str,
        year: i32,
        month: i32,
        tax_year: i32,
        tax_month: i32,
    ) -> BordroDonemi {
        let (end_year, end_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        BordroDonemi {
            id: id.into(),
            yil: year,
            ay: month,
            baslangicTarihi: format!("{year}-{month:02}-15"),
            bitisTarihi: format!("{end_year}-{end_month:02}-14"),
            donemAdi: id.into(),
            taxYear: tax_year,
            taxMonth: tax_month,
        }
    }

    fn period(id: &str, month: i32, tax_month: i32) -> BordroDonemi {
        period_for_year(id, 2026, month, 2026, tax_month)
    }

    fn payroll(period_id: &str, status: BordroStatus) -> BordroKaydi {
        BordroKaydi {
            id: format!("person-1_{period_id}"),
            personelId: "person-1".into(),
            donemId: period_id.into(),
            accrualId: format!("person-1_{period_id}"),
            accrualType: crate::models::AccrualType::NORMAL,
            paymentDate: "2026-02-14".into(),
            sequence: 0,
            accrualDescription: None,
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri::default(),
            gelirToplam: Default::default(),
            kesintiler: KesintiKalemleri::default(),
            kesintiToplam: Default::default(),
            netOdeme: Default::default(),
            status,
            olusturulmaTarihi: "2026-09-03T00:00:00Z".into(),
            sonGuncellemeTarihi: "2026-09-03T00:00:00Z".into(),
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
            statutorySnapshot: None,
            odenenRaporluGun: None,
            raporluGun: None,
        }
    }

    fn statutory_snapshot(source: StatutorySnapshotSource) -> ResolvedStatutorySnapshot {
        ResolvedStatutorySnapshot {
            source,
            segments: Vec::new(),
            sgkPrimGunSayisi: 0,
            pekAltSinir: Default::default(),
            pekUstSinir: Default::default(),
            sgkYemekIstisnasiToplam: Default::default(),
            gvYemekIstisnasiToplam: Default::default(),
            gvReferansGunlukAsgariUcret: Default::default(),
        }
    }

    fn supplementary(
        accrual_id: &str,
        period_id: &str,
        status: BordroStatus,
        source: StatutorySnapshotSource,
        payment_date: &str,
        sequence: i32,
        accrual_type: AccrualType,
    ) -> BordroKaydi {
        let mut record = payroll(period_id, status);
        record.id = accrual_id.into();
        record.accrualId = accrual_id.into();
        record.accrualType = accrual_type;
        record.paymentDate = payment_date.into();
        record.sequence = sequence;
        record.statutorySnapshot = Some(statutory_snapshot(source));
        record
    }

    fn legacy_supplementary(
        accrual_id: &str,
        period_id: &str,
        status: BordroStatus,
        payment_date: &str,
        sequence: i32,
        accrual_type: AccrualType,
    ) -> BordroKaydi {
        let mut record = supplementary(
            accrual_id,
            period_id,
            status,
            StatutorySnapshotSource::LegacyUnknown,
            payment_date,
            sequence,
            accrual_type,
        );
        // Missing statutorySnapshot is the legacy representation whose
        // provenance must be treated as LegacyUnknown by policy.
        record.statutorySnapshot = None;
        record
    }

    fn dataset_with_periods(
        periods: Vec<BordroDonemi>,
        payrolls: Vec<BordroKaydi>,
    ) -> PayrollDatasetSnapshot {
        PayrollDatasetSnapshot {
            periods,
            payrolls,
            ..PayrollDatasetSnapshot::default()
        }
    }

    fn has_accrual(impact: &MutationImpact, accrual_id: &str) -> bool {
        impact
            .affectedPayrolls
            .iter()
            .any(|key| key.accrualId == accrual_id)
    }

    fn has_blocked_accrual(impact: &MutationImpact, accrual_id: &str) -> bool {
        impact
            .blockedByFinalized
            .iter()
            .any(|key| key.accrualId == accrual_id)
    }

    fn dataset() -> PayrollDatasetSnapshot {
        PayrollDatasetSnapshot {
            periods: vec![
                period("2026-01", 1, 1),
                period("2026-02", 2, 2),
                period("2026-03", 3, 3),
            ],
            payrolls: vec![
                payroll("2026-03", BordroStatus::FINALIZED),
                payroll("2026-01", BordroStatus::CALCULATED),
            ],
            ..PayrollDatasetSnapshot::default()
        }
    }

    fn retro_batch(status: CompensationRevisionStatus) -> (RetroAdjustmentBatch, RetroAllocation) {
        let batch = RetroAdjustmentBatch {
            id: "retro-batch".into(),
            revisionId: "revision".into(),
            personnelId: "person-1".into(),
            paymentDate: "2026-06-20".into(),
            status,
            settlementStatus: if status == CompensationRevisionStatus::FINALIZED {
                RetroSettlementStatus::PAID
            } else {
                RetroSettlementStatus::UNSETTLED
            },
            totalGrossDelta: 10.into(),
            payableSettlementAmount: 10.into(),
            offsetSettlementAmount: 0.into(),
            recoveredAmount: 0.into(),
            recoverableAmount: 0.into(),
            outstandingReceivable: 0.into(),
            description: None,
            createdAt: None,
            calculatedAt: None,
            finalizedAt: None,
        };
        let allocation = RetroAllocation {
            id: "retro-allocation".into(),
            batchId: batch.id.clone(),
            personnelId: batch.personnelId.clone(),
            sourcePeriodId: "2026-01".into(),
            earningCode: RetroEarningCode::BASE_WAGE,
            originalRecognizedAmount: 0.into(),
            previousAuthoritativeRetroAmount: 0.into(),
            targetAmount: 10.into(),
            deltaAmount: 10.into(),
            sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
            incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
            stampTaxTreatment: RetroTaxTreatment::TAXABLE,
            originalPek: 0.into(),
            retroPekDelta: 0.into(),
            adjustedPek: 0.into(),
            workerSgkDelta: 0.into(),
            workerUnemploymentDelta: 0.into(),
            employerSgkDelta: 0.into(),
            employerUnemploymentDelta: 0.into(),
            originalEmployerLowerBound: 0.into(),
            targetEmployerLowerBound: 0.into(),
            employerLowerBoundDelta: 0.into(),
            employerLowerBoundPremiumDelta: 0.into(),
            originalSourceCarry: None,
            targetSourceCarry: None,
            payableSettlementAmount: 10.into(),
            offsetSettlementAmount: 0.into(),
            recoverableAmount: 0.into(),
            metadata: None,
        };
        (batch, allocation)
    }

    #[test]
    fn retro_source_carry_save_invalidates_downstream_and_blocks_finalized_history() {
        let (batch, mut allocation) = retro_batch(CompensationRevisionStatus::CALCULATED);
        allocation.originalSourceCarry = Some(Vec::new());
        allocation.targetSourceCarry = Some(vec![DevredenPekKaydi {
            tutar: 10.into(),
            kalanAySayisi: 1,
            kaynakDonemId: Some("2026-01".into()),
        }]);
        let downstream = supplementary(
            "downstream-payment",
            "2026-03",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::ProvisionalPaymentMonth,
            "2026-06-21",
            0,
            AccrualType::SUPPLEMENTAL,
        );
        let mut data = dataset();
        data.payrolls.push(downstream);
        data.retroBatches.push(batch);
        data.retroAllocations.push(allocation);

        let mutation = PayrollMutation::RetroBatchSave {
            personnelId: "person-1".into(),
            batchId: "retro-batch".into(),
            paymentDate: "2026-06-20".into(),
        };
        let impact = evaluate_payroll_invalidation(&data, &mutation)
            .expect("carry save mutation should evaluate");
        assert!(has_accrual(&impact, "downstream-payment"));
        assert!(!has_blocked_accrual(&impact, "downstream-payment"));

        data.payrolls
            .iter_mut()
            .find(|payroll| payroll.accrualId == "downstream-payment")
            .expect("downstream payroll")
            .status = BordroStatus::FINALIZED;
        let impact = evaluate_payroll_invalidation(&data, &mutation)
            .expect("carry save mutation should evaluate");
        assert!(has_blocked_accrual(&impact, "downstream-payment"));
    }

    #[test]
    fn source_mutation_invalidates_retro_ledger_and_finalized_retro_is_a_blocker() {
        let (batch, allocation) = retro_batch(CompensationRevisionStatus::CALCULATED);
        let mut data = dataset();
        data.retroBatches.push(batch);
        data.retroAllocations.push(allocation);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");
        assert_eq!(impact.affectedRetroBatches, vec!["retro-batch"]);
        assert!(impact.blockedByFinalizedRetroBatches.is_empty());

        let (finalized_batch, finalized_allocation) =
            retro_batch(CompensationRevisionStatus::FINALIZED);
        data.retroBatches = vec![finalized_batch];
        data.retroAllocations = vec![finalized_allocation];
        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");
        assert_eq!(impact.blockedByFinalizedRetroBatches, vec!["retro-batch"]);
    }

    #[test]
    fn source_mutation_returns_sorted_dependents_and_finalized_blockers() {
        let impact = evaluate_payroll_invalidation(
            &dataset(),
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");

        assert_eq!(
            impact.affectedPayrolls,
            vec![
                PayrollKey {
                    personnelId: "person-1".into(),
                    periodId: "2026-01".into(),
                    accrualId: "person-1_2026-01".into(),
                },
                PayrollKey {
                    personnelId: "person-1".into(),
                    periodId: "2026-03".into(),
                    accrualId: "person-1_2026-03".into(),
                },
            ]
        );
        assert_eq!(impact.blockedByFinalized, impact.affectedPayrolls[1..]);
    }

    #[test]
    fn payment_tax_year_mutation_invalidates_retro_payment_chain() {
        let (batch, allocation) = retro_batch(CompensationRevisionStatus::CALCULATED);
        let mut data = dataset();
        data.retroBatches.push(batch);
        data.retroAllocations.push(allocation);

        let impact =
            evaluate_payroll_invalidation(&data, &PayrollMutation::TaxYear { taxYear: 2026 })
                .expect("retro payment date must parse");
        assert_eq!(impact.affectedRetroBatches, vec!["retro-batch"]);
        assert!(impact.blockedByFinalizedRetroBatches.is_empty());

        let (finalized_batch, finalized_allocation) =
            retro_batch(CompensationRevisionStatus::FINALIZED);
        data.retroBatches = vec![finalized_batch];
        data.retroAllocations = vec![finalized_allocation];
        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonTaxYear {
                personnelId: "person-1".into(),
                taxYear: 2026,
            },
        )
        .expect("retro payment date must parse");
        assert_eq!(impact.blockedByFinalizedRetroBatches, vec!["retro-batch"]);
    }

    #[test]
    fn payroll_recalculation_does_not_block_on_its_current_record() {
        let impact = evaluate_payroll_invalidation(
            &dataset(),
            &PayrollMutation::PayrollCalculation {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");

        assert_eq!(impact.affectedPayrolls.len(), 1);
        assert_eq!(impact.affectedPayrolls[0].periodId, "2026-03");
        assert_eq!(impact.blockedByFinalized.len(), 1);
    }

    #[test]
    fn person_from_date_uses_period_end_and_person_scope() {
        let mut data = dataset();
        data.payrolls[1].personelId = "other-person".into();
        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonFromDate {
                personnelId: "person-1".into(),
                effectiveFrom: "2026-02-01".into(),
            },
        )
        .expect("date must parse");

        assert_eq!(impact.affectedPayrolls.len(), 1);
        assert_eq!(impact.affectedPayrolls[0].periodId, "2026-03");
    }

    #[test]
    fn attendance_mutation_includes_attendance_backed_before_normal_root() {
        let mut normal = payroll("2026-01", BordroStatus::CALCULATED);
        normal.paymentDate = "2026-01-14".into();
        let tediye = supplementary(
            "tediye",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::AttendanceBacked,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![tediye, normal]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");

        assert!(has_accrual(&impact, "tediye"));
        assert!(has_accrual(&impact, "person-1_2026-01"));
    }

    #[test]
    fn attendance_mutation_propagates_without_a_normal_root() {
        let tediye = supplementary(
            "tediye",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::AttendanceBacked,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let tis = supplementary(
            "tis",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::AttendanceBacked,
            "2026-01-20",
            0,
            AccrualType::TIS_IKRAMIYE,
        );
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![tediye, tis]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");

        assert!(has_accrual(&impact, "tediye"));
        assert!(has_accrual(&impact, "tis"));
    }

    #[test]
    fn attendance_mutation_propagates_from_provisional_root() {
        let tediye = supplementary(
            "tediye",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::ProvisionalPaymentMonth,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let tis = supplementary(
            "tis",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::ProvisionalPaymentMonth,
            "2026-01-20",
            0,
            AccrualType::TIS_IKRAMIYE,
        );
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![tediye, tis]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");

        assert!(has_accrual(&impact, "tediye"));
        assert!(has_accrual(&impact, "tis"));
    }

    #[test]
    fn attendance_backed_finalized_root_is_a_blocker() {
        let tediye = supplementary(
            "tediye",
            "2026-01",
            BordroStatus::FINALIZED,
            StatutorySnapshotSource::AttendanceBacked,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![tediye]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");

        assert!(has_blocked_accrual(&impact, "tediye"));
    }

    #[test]
    fn sick_leave_mutation_includes_attendance_backed_root_and_downstream() {
        let tediye = supplementary(
            "tediye",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::AttendanceBacked,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let tis = supplementary(
            "tis",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::AttendanceBacked,
            "2026-01-20",
            0,
            AccrualType::TIS_IKRAMIYE,
        );
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![tediye, tis]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonFromDate {
                personnelId: "person-1".into(),
                effectiveFrom: "2026-01-20".into(),
            },
        )
        .expect("date must parse");

        assert!(has_accrual(&impact, "tediye"));
        assert!(has_accrual(&impact, "tis"));
    }

    #[test]
    fn sick_leave_mutation_does_not_root_provisional_supplementary() {
        let tediye = supplementary(
            "tediye",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::ProvisionalPaymentMonth,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![tediye]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonFromDate {
                personnelId: "person-1".into(),
                effectiveFrom: "2026-01-20".into(),
            },
        )
        .expect("date must parse");

        assert!(impact.affectedPayrolls.is_empty());
        assert!(impact.blockedByFinalized.is_empty());
    }

    #[test]
    fn sick_leave_mutation_preserves_normal_root_invalidation() {
        let mut normal = payroll("2026-01", BordroStatus::CALCULATED);
        normal.paymentDate = "2026-01-14".into();
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![normal]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonFromDate {
                personnelId: "person-1".into(),
                effectiveFrom: "2026-01-20".into(),
            },
        )
        .expect("date must parse");

        assert!(has_accrual(&impact, "person-1_2026-01"));
    }

    #[test]
    fn unrelated_legacy_finalized_supplementary_does_not_block_mutation() {
        let mut ahmet = legacy_supplementary(
            "ahmet-legacy",
            "2024-01",
            BordroStatus::FINALIZED,
            "2024-01-10",
            0,
            AccrualType::TEDIYE,
        );
        ahmet.personelId = "ahmet".into();
        let mut mehmet = payroll("2026-01", BordroStatus::CALCULATED);
        mehmet.personelId = "mehmet".into();
        mehmet.id = "mehmet-normal".into();
        mehmet.accrualId = "mehmet-normal".into();
        mehmet.paymentDate = "2026-01-14".into();
        let data = dataset_with_periods(
            vec![
                period_for_year("2024-01", 2024, 1, 2024, 1),
                period("2026-01", 1, 1),
            ],
            vec![ahmet, mehmet],
        );

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "mehmet".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("unrelated legacy data must not cause a global blocker");

        assert!(has_accrual(&impact, "mehmet-normal"));
        assert!(!has_accrual(&impact, "ahmet-legacy"));
        assert!(!has_blocked_accrual(&impact, "ahmet-legacy"));
    }

    #[test]
    fn relevant_legacy_finalized_supplementary_fails_closed() {
        let legacy = supplementary(
            "legacy",
            "2026-01",
            BordroStatus::FINALIZED,
            StatutorySnapshotSource::LegacyUnknown,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let data = dataset_with_periods(vec![period("2026-01", 1, 1)], vec![legacy]);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("policy should return the relevant blocker");

        assert!(has_blocked_accrual(&impact, "legacy"));
    }

    #[test]
    fn relevant_legacy_finalized_downstream_supplementary_fails_closed() {
        let root = supplementary(
            "root",
            "2026-01",
            BordroStatus::CALCULATED,
            StatutorySnapshotSource::AttendanceBacked,
            "2026-01-10",
            0,
            AccrualType::TEDIYE,
        );
        let mut legacy = legacy_supplementary(
            "legacy",
            "2026-02",
            BordroStatus::FINALIZED,
            "2026-02-10",
            0,
            AccrualType::TIS_IKRAMIYE,
        );
        legacy.personelId = "person-1".into();
        let data = dataset_with_periods(
            vec![period("2026-01", 1, 1), period("2026-02", 2, 2)],
            vec![root, legacy],
        );

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("policy should return the relevant downstream blocker");

        assert!(has_blocked_accrual(&impact, "legacy"));
    }
}
