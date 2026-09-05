//! Pure payroll mutation/invalidation policy shared by native and browser.
//!
//! This module deliberately returns keys and blockers only. Adapters decide
//! how to persist the mutation and how to mark the returned mutable records.

#![allow(non_snake_case)]

use crate::models::{BordroDonemi, BordroStatus};
use crate::payroll_engine::{accrual_order_for_payroll as payroll_order, PayrollDatasetSnapshot};
use crate::{DomainError, Result};
use chrono::NaiveDate;
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
    #[serde(rename = "ALL")]
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MutationImpact {
    pub affectedPayrolls: Vec<PayrollKey>,
    pub blockedByFinalized: Vec<PayrollKey>,
}

fn period_for<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    period_id: &str,
) -> Option<&'a BordroDonemi> {
    dataset.periods.iter().find(|period| period.id == period_id)
}

fn require_period<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    period_id: &str,
) -> Result<&'a BordroDonemi> {
    period_for(dataset, period_id).ok_or_else(|| {
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

fn affected_after_any_normal_root(
    dataset: &PayrollDatasetSnapshot,
    payroll: &crate::models::BordroKaydi,
    normal_roots: &[&crate::models::BordroKaydi],
) -> Result<bool> {
    if normal_roots.is_empty() {
        return Ok(false);
    }
    let candidate_order = payroll_order(dataset, payroll)?;
    normal_roots
        .iter()
        .try_fold(false, |affected, root| {
            if affected {
                Ok(true)
            } else {
                Ok(candidate_order >= payroll_order(dataset, root)?)
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
    dataset: &PayrollDatasetSnapshot,
    period_id: &str,
    payment_date: &str,
    sequence: i32,
    accrual_id: &str,
) -> Result<crate::payroll_engine::AccrualOrder> {
    crate::payroll_engine::payment_event_order(
        require_period(dataset, period_id)?, payment_date, sequence, accrual_id,
    )
}

fn affected_by_mutation(
    dataset: &PayrollDatasetSnapshot,
    payroll_personnel_id: &str,
    payroll_period_id: &str,
    payroll: &crate::models::BordroKaydi,
    mutation: &PayrollMutation,
) -> Result<bool> {
    let candidate = period_for(dataset, payroll_period_id);
    match mutation {
        PayrollMutation::Person { personnelId } => Ok(payroll_personnel_id == personnelId),
        PayrollMutation::PersonPeriod {
            personnelId,
            periodId,
        } => {
            let source = require_period(dataset, periodId)?;
            if payroll_personnel_id != personnelId || candidate.is_none() {
                return Ok(false);
            }
            // Puantaj is an input to NORMAL only. A supplementary event that
            // precedes the period's NORMAL root is independent and must not be
            // made STALE merely because attendance was later entered/changed.
            let normal_roots: Vec<&crate::models::BordroKaydi> = dataset
                .payrolls
                .iter()
                .filter(|root| {
                    root.personelId == *personnelId
                        && root.donemId == source.id
                        && root.accrualType == crate::models::AccrualType::NORMAL
                })
                .collect();
            affected_after_any_normal_root(dataset, payroll, &normal_roots)
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
            let source = require_period(dataset, periodId)?;
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
            let mut normal_roots = Vec::new();
            for root in dataset.payrolls.iter().filter(|root| {
                root.personelId == *personnelId
                    && root.accrualType == crate::models::AccrualType::NORMAL
            }) {
                let Some(root_period) = period_for(dataset, &root.donemId) else {
                    continue;
                };
                if is_person_from_date(root_period, effectiveFrom)? {
                    normal_roots.push(root);
                }
            }
            affected_after_any_normal_root(dataset, payroll, &normal_roots)
        }
        PayrollMutation::PayrollCalculation {
            personnelId,
            periodId,
        } => {
            let source = require_period(dataset, periodId)?;
            Ok(payroll_personnel_id == personnelId
                && candidate.is_some_and(|candidate| {
                    candidate.id != source.id && is_period_dependent(candidate, source)
                }))
        }
        PayrollMutation::AccrualCalculation {
            personnelId,
            periodId,
            accrualId,
        } | PayrollMutation::AccrualDelete { personnelId, periodId, accrualId } => {
            let source = dataset
                .payrolls
                .iter()
                .find(|candidate| {
                    candidate.personelId == *personnelId
                        && candidate.donemId == *periodId
                        && effective_accrual_id(candidate) == *accrualId
                })
                .ok_or_else(|| {
                    DomainError::ValidationError(format!(
                        "Invalidation kaynağı tahakkuk bulunamadı: {}.",
                        accrualId
                    ))
                })?;
            Ok(payroll_personnel_id == personnelId
                && (payroll_order(dataset, payroll)? > payroll_order(dataset, source)?
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
            && payroll_order(dataset, payroll)?
                > input_order(dataset, periodId, paymentDate, *sequence, accrualId)?),
        PayrollMutation::All => Ok(true),
    }
}

/// Computes the deterministic invalidation impact and FINALIZED blockers for
/// one source mutation. It performs no persistence and reads no clock.
pub fn evaluate_payroll_invalidation(
    dataset: &PayrollDatasetSnapshot,
    mutation: &PayrollMutation,
) -> Result<MutationImpact> {
    let mut affected = BTreeSet::new();
    let mut blocked = BTreeSet::new();

    for payroll in &dataset.payrolls {
        if !affected_by_mutation(
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

    Ok(MutationImpact {
        affectedPayrolls: affected.into_iter().collect(),
        blockedByFinalized: blocked.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BordroKaydi, GelirKalemleri, KesintiKalemleri, PuantajOzeti};
    use crate::payroll_engine::PayrollDatasetSnapshot;

    fn period(id: &str, month: i32, tax_month: i32) -> BordroDonemi {
        BordroDonemi {
            id: id.into(),
            yil: 2026,
            ay: month,
            baslangicTarihi: format!("2026-{month:02}-15"),
            bitisTarihi: format!("2026-{:02}-14", month + 1),
            donemAdi: id.into(),
            taxYear: 2026,
            taxMonth: tax_month,
        }
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
    fn attendance_mutation_keeps_independent_supplementary_before_normal_root_clean() {
        let mut data = dataset();
        data.payrolls[1].paymentDate = "2026-01-14".into();
        let mut supplementary = payroll("2026-01", BordroStatus::CALCULATED);
        supplementary.id = "person-1_2026-01_tediye".into();
        supplementary.accrualId = supplementary.id.clone();
        supplementary.accrualType = crate::models::AccrualType::TEDIYE;
        supplementary.paymentDate = "2026-01-10".into();
        supplementary.sequence = 0;
        data.payrolls.push(supplementary);

        let impact = evaluate_payroll_invalidation(
            &data,
            &PayrollMutation::PersonPeriod {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("period must exist");

        assert!(!impact
            .affectedPayrolls
            .iter()
            .any(|key| key.accrualId == "person-1_2026-01_tediye"));
        assert!(impact
            .affectedPayrolls
            .iter()
            .any(|key| key.accrualId == "person-1_2026-01"));
    }
}
