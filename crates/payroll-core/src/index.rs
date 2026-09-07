//! Read-only indexes for the immutable calculation snapshot.
//!
//! The snapshot remains the serialization contract. This index is an
//! in-memory acceleration layer built at the calculation boundary, so adding
//! it cannot change persisted data or the browser/native wire format.

use crate::models::{
    AnnualPayrollParameters, BordroDonemi, BordroKaydi, Personel, PersonelPuantaj,
    PersonelTaxOpening,
};
use crate::payroll_engine::PayrollDatasetSnapshot;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct PayrollDatasetIndex {
    personnel_by_id: HashMap<String, usize>,
    periods_by_id: HashMap<String, usize>,
    attendances_by_person_period: HashMap<(String, String), Vec<usize>>,
    payrolls_by_person_period: HashMap<(String, String), Vec<usize>>,
    payrolls_by_person: HashMap<String, Vec<usize>>,
    tax_openings_by_person_year: HashMap<(String, i32), usize>,
    annual_parameters_by_year: HashMap<i32, usize>,
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
        }
        for (position, attendance) in dataset.attendances.iter().enumerate() {
            index
                .attendances_by_person_period
                .entry((attendance.personelId.clone(), attendance.donemId.clone()))
                .or_default()
                .push(position);
        }
        for (position, payroll) in dataset.payrolls.iter().enumerate() {
            index
                .payrolls_by_person
                .entry(payroll.personelId.clone())
                .or_default()
                .push(position);
            index
                .payrolls_by_person_period
                .entry((payroll.personelId.clone(), payroll.donemId.clone()))
                .or_default()
                .push(position);
        }
        for (position, opening) in dataset.taxOpenings.iter().enumerate() {
            index
                .tax_openings_by_person_year
                .entry((opening.personnelId.clone(), opening.year))
                .or_insert(position);
        }
        for (position, parameters) in dataset.annualPayrollParameters.iter().enumerate() {
            index
                .annual_parameters_by_year
                .entry(parameters.year)
                .or_insert(position);
        }

        index
    }

    pub fn personnel<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        id: &str,
    ) -> Option<&'a Personel> {
        self.personnel_by_id
            .get(id)
            .and_then(|position| dataset.personnel.get(*position))
    }

    pub fn period<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        id: &str,
    ) -> Option<&'a BordroDonemi> {
        self.periods_by_id
            .get(id)
            .and_then(|position| dataset.periods.get(*position))
    }

    pub fn attendances<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        period_id: &str,
    ) -> Vec<&'a PersonelPuantaj> {
        self.attendances_by_person_period
            .get(&(personnel_id.to_owned(), period_id.to_owned()))
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(|position| dataset.attendances.get(*position))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn payrolls_for_person_period<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        period_id: &str,
    ) -> Vec<&'a BordroKaydi> {
        self.payrolls_by_person_period
            .get(&(personnel_id.to_owned(), period_id.to_owned()))
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(|position| dataset.payrolls.get(*position))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn payrolls_for_person<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
    ) -> Vec<&'a BordroKaydi> {
        self.payrolls_by_person
            .get(personnel_id)
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(|position| dataset.payrolls.get(*position))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn payroll_for_accrual<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        period_id: &str,
        accrual_id: &str,
    ) -> Option<&'a BordroKaydi> {
        self.payrolls_for_person_period(dataset, personnel_id, period_id)
            .into_iter()
            .find(|payroll| {
                payroll.accrualId == accrual_id
                    || (payroll.accrualId.trim().is_empty() && payroll.id == accrual_id)
            })
    }

    pub fn tax_opening<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        personnel_id: &str,
        year: i32,
    ) -> Option<&'a PersonelTaxOpening> {
        self.tax_openings_by_person_year
            .get(&(personnel_id.to_owned(), year))
            .and_then(|position| dataset.taxOpenings.get(*position))
    }

    pub fn annual_parameters<'a>(
        &self,
        dataset: &'a PayrollDatasetSnapshot,
        year: i32,
    ) -> Option<&'a AnnualPayrollParameters> {
        self.annual_parameters_by_year
            .get(&year)
            .and_then(|position| dataset.annualPayrollParameters.get(*position))
    }
}
