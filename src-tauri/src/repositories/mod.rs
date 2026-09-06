use crate::domain::{DomainError, Result};
pub mod annual_payroll_parameters_repo;
pub mod attendance_repo;
pub mod payroll_invalidation_repo;
pub mod payroll_repo;
pub mod period_repo;
pub mod personnel_repo;
pub mod retro_repo;
pub mod settings_repo;
pub mod sick_leave_repo;
pub mod tax_opening_repo;

use rust_decimal::prelude::*;
use rust_decimal::Decimal;

fn decimal_to_kurus_i64(value: Decimal) -> Result<i64> {
    let scaled = value.checked_mul(Decimal::from(100)).ok_or_else(|| {
        DomainError::InvalidData(format!(
            "Parasal değer kuruşa çevrilirken Decimal taşması oluştu: {}",
            value
        ))
    })?;
    scaled.round().to_i64().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "Parasal değer SQLite i64 kuruş sınırını aşıyor: {}",
            value
        ))
    })
}

pub fn dec_to_kurus(d: Option<Decimal>) -> Result<i64> {
    decimal_to_kurus_i64(d.unwrap_or_default())
}

pub fn opt_dec_to_kurus(d: Option<Decimal>) -> Result<Option<i64>> {
    d.map(decimal_to_kurus_i64).transpose()
}

pub fn kurus_to_dec(k: i64) -> Decimal {
    Decimal::from(k) / Decimal::from(100)
}

pub fn opt_kurus_to_dec(k: Option<i64>) -> Option<Decimal> {
    k.map(|v| Decimal::from(v) / Decimal::from(100))
}
