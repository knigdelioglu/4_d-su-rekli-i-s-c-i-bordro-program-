//! Read-only indexes for the immutable calculation snapshot.
//!
//! The snapshot remains the serialization contract. This index is an
//! in-memory acceleration layer built at the calculation boundary, so adding
//! it cannot change persisted data or the browser/native wire format. All
//! position lists retain source-vector order; callers that need payment-event
//! order still sort explicitly with the core's ordering rules.

use crate::models::{
    AnnualPayrollParameters, BordroDonemi, BordroKaydi, CompensationRevision,
    CompensationRevisionOverride, Personel, PersonelPuantaj, PersonelTaxOpening,
    RetroAdjustmentBatch, RetroAllocation, SickLeaveRecord,
};
use crate::payroll_engine::PayrollDatasetSnapshot;
use std::collections::HashMap;

type Positions = Vec<usize>;
type PersonKeyedPositions = HashMap<String, Positions>;
type PersonPeriodPositions = HashMap<String, PersonKeyedPositions>;
type PersonTaxMonthPositions = HashMap<String, HashMap<(i32, i32), Positions>>;
type PersonYearPositions = HashMap<String, HashMap<i32, Positions>>;

#[derive(Debug, Clone, Default)]
pub struct PayrollDatasetIndex {
    personnel_by_id: HashMap<String, usize>,
    periods_by_id: HashMap<String, usize>,
    periods_by_tax_month: HashMap<(i32, i32), Positions>,
    attendances_by_person_period: PersonPeriodPositions,
    payrolls_by_person_period: PersonPeriodPositions,
    payrolls_by_person_tax_month: PersonTaxMonthPositions,
    payrolls_by_person: PersonKeyedPositions,
    payrolls_by_accrual_id: PersonKeyedPositions,
    tax_openings_by_person_year: PersonYearPositions,
    annual_parameters_by_year: HashMap<i32, Positions>,
    sick_leave_by_person: PersonKeyedPositions,
    retro_batches_by_id: HashMap<String, usize>,
    retro_batches_by_person: PersonKeyedPositions,
    retro_batches_by_revision: PersonKeyedPositions,
    retro_allocations_by_batch: PersonKeyedPositions,
    compensation_revisions_by_id: HashMap<String, usize>,
    overrides_by_revision: PersonKeyedPositions,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BordroDonemi, Personel, PersonelPuantaj, SickLeaveRecord};
    use std::collections::HashMap;

    fn period(id: &str, tax_month: i32) -> BordroDonemi {
        BordroDonemi {
            id: id.into(),
            yil: 2026,
            ay: tax_month,
            baslangicTarihi: format!("2026-{tax_month:02}-15"),
            bitisTarihi: format!("2026-{tax_month:02}-28"),
            donemAdi: id.into(),
            taxYear: 2026,
            taxMonth: tax_month,
        }
    }

    fn person(id: &str) -> Personel {
        Personel {
            id: id.into(),
            tcNo: format!("TC-{id}"),
            ad: "Test".into(),
            soyad: "Personel".into(),
            grup: "1. Grup".into(),
            unvan: None,
            sgkSicilNo: String::new(),
            iban: String::new(),
            hizmetYili: 0,
            aciklama: None,
            devirKumulatifGvMatrahi: None,
            devirKumulatifGvMatrahiYili: None,
            devirKumulatifGvMatrahiBaslangicAyi: None,
            devirKumulatifAsgariGvMatrahi: None,
            devirKumulatifAsgariGvMatrahiYili: None,
            kesintiler: None,
        }
    }

    #[test]
    fn repeated_build_preserves_source_order_for_keyed_lookups() {
        let mut first_days = HashMap::new();
        first_days.insert("2026-01-15".into(), "Ç".into());
        let mut second_days = HashMap::new();
        second_days.insert("2026-02-15".into(), "R".into());
        let dataset = PayrollDatasetSnapshot {
            personnel: vec![person("person-1")],
            periods: vec![period("period-2", 2), period("period-1", 1)],
            attendances: vec![
                PersonelPuantaj {
                    id: "attendance-2".into(),
                    personelId: "person-1".into(),
                    donemId: "period-2".into(),
                    gunler: second_days,
                },
                PersonelPuantaj {
                    id: "attendance-1".into(),
                    personelId: "person-1".into(),
                    donemId: "period-1".into(),
                    gunler: first_days,
                },
            ],
            sickLeaveRecords: vec![
                SickLeaveRecord {
                    id: "sick-2".into(),
                    personnelId: "person-1".into(),
                    startDate: "2026-02-15".into(),
                    endDate: "2026-02-16".into(),
                    createdAt: None,
                    updatedAt: None,
                },
                SickLeaveRecord {
                    id: "sick-1".into(),
                    personnelId: "person-1".into(),
                    startDate: "2026-01-15".into(),
                    endDate: "2026-01-16".into(),
                    createdAt: None,
                    updatedAt: None,
                },
            ],
            ..PayrollDatasetSnapshot::default()
        };

        let first = PayrollDatasetIndex::build(&dataset);
        let second = PayrollDatasetIndex::build(&dataset);
        let period_ids = |index: &PayrollDatasetIndex| {
            index
                .periods_for_tax_month(&dataset, 2026, 2)
                .map(|period| period.id.clone())
                .collect::<Vec<_>>()
        };
        let attendance_ids = |index: &PayrollDatasetIndex| {
            index
                .attendances(&dataset, "person-1", "period-1")
                .map(|attendance| attendance.id.clone())
                .collect::<Vec<_>>()
        };
        let sick_ids = |index: &PayrollDatasetIndex| {
            index
                .sick_leave_for_person(&dataset, "person-1")
                .map(|record| record.id.clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(first.personnel(&dataset, "person-1").map(|p| p.id.as_str()), Some("person-1"));
        assert_eq!(period_ids(&first), period_ids(&second));
        assert_eq!(attendance_ids(&first), vec!["attendance-1".to_owned()]);
        assert_eq!(attendance_ids(&first), attendance_ids(&second));
        assert_eq!(sick_ids(&first), vec!["sick-2".to_owned(), "sick-1".to_owned()]);
        assert_eq!(sick_ids(&first), sick_ids(&second));
    }
}

fn positions<'a>(value: Option<&'a Positions>) -> &'a [usize] {
    value.map(Vec::as_slice).unwrap_or(&[])
}

fn records<'a, T>(values: &'a [T], positions: &'a [usize]) -> impl Iterator<Item = &'a T> + 'a {
    positions.iter().filter_map(|position| values.get(*position))
}

fn push_position(map: &mut PersonKeyedPositions, key: &str, position: usize) {
    map.entry(key.to_owned()).or_default().push(position);
}

fn push_nested_position(
    map: &mut PersonPeriodPositions,
    first_key: &str,
    second_key: &str,
    position: usize,
) {
    map.entry(first_key.to_owned())
        .or_default()
        .entry(second_key.to_owned())
        .or_default()
        .push(position);
}

impl PayrollDatasetIndex {
    pub fn build(dataset: &PayrollDatasetSnapshot) -> Self {
        let mut index = Self::default();

        for (position, person) in dataset.personnel.iter().enumerate() {
            index
                .personnel_by_id
                .entry(person.id.clone())
                .or_insert(position);
        }
        for (position, period) in dataset.periods.iter().enumerate() {
            index
                .periods_by_id
                .entry(period.id.clone())
                .or_insert(position);
            index
                .periods_by_tax_month
                .entry((period.taxYear, period.taxMonth))
                .or_default()
                .push(position);
        }
        for (position, attendance) in dataset.attendances.iter().enumerate() {
            push_nested_position(
                &mut index.attendances_by_person_period,
                &attendance.personelId,
                &attendance.donemId,
                position,
            );
        }
        for (position, payroll) in dataset.payrolls.iter().enumerate() {
            push_position(&mut index.payrolls_by_person, &payroll.personelId, position);
            push_nested_position(
                &mut index.payrolls_by_person_period,
                &payroll.personelId,
                &payroll.donemId,
                position,
            );
            if let Some(period_position) = index.periods_by_id.get(&payroll.donemId) {
                let period = &dataset.periods[*period_position];
                index
                    .payrolls_by_person_tax_month
                    .entry(payroll.personelId.clone())
                    .or_default()
                    .entry((period.taxYear, period.taxMonth))
                    .or_default()
                    .push(position);
            }
            if !payroll.accrualId.trim().is_empty() {
                push_position(&mut index.payrolls_by_accrual_id, &payroll.accrualId, position);
            }
            if payroll.id != payroll.accrualId {
                push_position(&mut index.payrolls_by_accrual_id, &payroll.id, position);
            }
        }
        for (position, opening) in dataset.taxOpenings.iter().enumerate() {
            index
                .tax_openings_by_person_year
                .entry(opening.personnelId.clone())
                .or_default()
                .entry(opening.year)
                .or_default()
                .push(position);
        }
        for (position, parameters) in dataset.annualPayrollParameters.iter().enumerate() {
            index
                .annual_parameters_by_year
                .entry(parameters.year)
                .or_default()
                .push(position);
        }
        for (position, record) in dataset.sickLeaveRecords.iter().enumerate() {
            push_position(&mut index.sick_leave_by_person, &record.personnelId, position);
        }
        for (position, batch) in dataset.retroBatches.iter().enumerate() {
            index
                .retro_batches_by_id
                .entry(batch.id.clone())
                .or_insert(position);
            push_position(&mut index.retro_batches_by_person, &batch.personnelId, position);
            push_position(
                &mut index.retro_batches_by_revision,
                &batch.revisionId,
                position,
            );
        }
        for (position, allocation) in dataset.retroAllocations.iter().enumerate() {
            push_position(
                &mut index.retro_allocations_by_batch,
                &allocation.batchId,
                position,
            );
        }
        for (position, revision) in dataset.compensationRevisions.iter().enumerate() {
            index
                .compensation_revisions_by_id
                .entry(revision.id.clone())
                .or_insert(position);
        }
        for (position, override_item) in dataset.compensationRevisionOverrides.iter().enumerate() {
            push_position(
                &mut index.overrides_by_revision,
                &override_item.revisionId,
                position,
            );
        }

        index
    }

    pub fn personnel<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        id: &str,
    ) -> Option<&'a Personel> {
        self.personnel_by_id
            .get(id)
            .and_then(|position| dataset.personnel.get(*position))
    }

    pub fn period<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        id: &str,
    ) -> Option<&'a BordroDonemi> {
        self.periods_by_id
            .get(id)
            .and_then(|position| dataset.periods.get(*position))
    }

    pub fn periods_for_tax_month<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        tax_year: i32,
        tax_month: i32,
    ) -> impl Iterator<Item = &'a BordroDonemi> + 'a {
        records(
            &dataset.periods,
            positions(self.periods_by_tax_month.get(&(tax_year, tax_month))),
        )
    }

    pub fn attendances<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        period_id: &str,
    ) -> impl Iterator<Item = &'a PersonelPuantaj> + 'a {
        let indexed_positions = self
            .attendances_by_person_period
            .get(personnel_id)
            .and_then(|periods| periods.get(period_id));
        records(&dataset.attendances, positions(indexed_positions))
    }

    pub fn payrolls_for_person_period<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        period_id: &str,
    ) -> impl Iterator<Item = &'a BordroKaydi> + 'a {
        let indexed_positions = self
            .payrolls_by_person_period
            .get(personnel_id)
            .and_then(|periods| periods.get(period_id));
        records(&dataset.payrolls, positions(indexed_positions))
    }

    pub fn payrolls_for_person_tax_month<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        tax_year: i32,
        tax_month: i32,
    ) -> impl Iterator<Item = &'a BordroKaydi> + 'a {
        let indexed_positions = self
            .payrolls_by_person_tax_month
            .get(personnel_id)
            .and_then(|months| months.get(&(tax_year, tax_month)));
        records(&dataset.payrolls, positions(indexed_positions))
    }

    pub fn payrolls_for_person<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
    ) -> impl Iterator<Item = &'a BordroKaydi> + 'a {
        records(
            &dataset.payrolls,
            positions(self.payrolls_by_person.get(personnel_id)),
        )
    }

    pub fn payrolls_for_accrual<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        accrual_id: &str,
    ) -> impl Iterator<Item = &'a BordroKaydi> + 'a {
        records(
            &dataset.payrolls,
            positions(self.payrolls_by_accrual_id.get(accrual_id)),
        )
    }

    pub fn payroll_for_accrual<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        period_id: &str,
        accrual_id: &str,
    ) -> Option<&'a BordroKaydi> {
        self.payrolls_for_accrual(dataset, accrual_id)
            .find(|payroll| payroll.personelId == personnel_id && payroll.donemId == period_id)
            .or_else(|| {
                self.payrolls_for_person_period(dataset, personnel_id, period_id)
                    .find(|payroll| {
                        payroll.accrualId == accrual_id
                            || (payroll.accrualId.trim().is_empty() && payroll.id == accrual_id)
                    })
            })
    }

    pub fn tax_opening<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        year: i32,
    ) -> Option<&'a PersonelTaxOpening> {
        self.tax_openings_by_person_year
            .get(personnel_id)
            .and_then(|years| years.get(&year))
            .and_then(|positions| positions.first())
            .and_then(|position| dataset.taxOpenings.get(*position))
    }

    pub fn annual_parameters<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        year: i32,
    ) -> Option<&'a AnnualPayrollParameters> {
        self.annual_parameters_by_year
            .get(&year)
            .and_then(|positions| positions.first())
            .and_then(|position| dataset.annualPayrollParameters.get(*position))
    }

    pub fn sick_leave_for_person<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
    ) -> impl Iterator<Item = &'a SickLeaveRecord> + 'a {
        records(
            &dataset.sickLeaveRecords,
            positions(self.sick_leave_by_person.get(personnel_id)),
        )
    }

    pub fn retro_batch<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        batch_id: &str,
    ) -> Option<&'a RetroAdjustmentBatch> {
        self.retro_batches_by_id
            .get(batch_id)
            .and_then(|position| dataset.retroBatches.get(*position))
    }

    pub fn retro_batches_for_person<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
    ) -> impl Iterator<Item = &'a RetroAdjustmentBatch> + 'a {
        records(
            &dataset.retroBatches,
            positions(self.retro_batches_by_person.get(personnel_id)),
        )
    }

    pub fn retro_batches_for_revision<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        revision_id: &str,
    ) -> impl Iterator<Item = &'a RetroAdjustmentBatch> + 'a {
        records(
            &dataset.retroBatches,
            positions(self.retro_batches_by_revision.get(revision_id)),
        )
    }

    pub fn retro_allocations_for_batch<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        batch_id: &str,
    ) -> impl Iterator<Item = &'a RetroAllocation> + 'a {
        records(
            &dataset.retroAllocations,
            positions(self.retro_allocations_by_batch.get(batch_id)),
        )
    }

    pub fn compensation_revision<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        revision_id: &str,
    ) -> Option<&'a CompensationRevision> {
        self.compensation_revisions_by_id
            .get(revision_id)
            .and_then(|position| dataset.compensationRevisions.get(*position))
    }

    pub fn overrides_for_revision<'a>(
        &'a self,
        dataset: &'a PayrollDatasetSnapshot,
        revision_id: &str,
    ) -> impl Iterator<Item = &'a CompensationRevisionOverride> + 'a {
        records(
            &dataset.compensationRevisionOverrides,
            positions(self.overrides_by_revision.get(revision_id)),
        )
    }
}
