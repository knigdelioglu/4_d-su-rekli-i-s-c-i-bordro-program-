pub mod attendance_repo;
pub mod period_repo;
pub mod personnel_repo;
pub mod payroll_repo;
pub mod settings_repo;
pub mod sick_leave_repo;
pub mod tax_opening_repo;

use rust_decimal::prelude::*;
use rust_decimal::Decimal;

pub fn dec_to_kurus(d: Option<Decimal>) -> i64 {
    match d {
        Some(dec) => (dec * Decimal::from(100)).round().to_i64().unwrap_or(0),
        None => 0,
    }
}

pub fn opt_dec_to_kurus(d: Option<Decimal>) -> Option<i64> {
    d.map(|dec| (dec * Decimal::from(100)).round().to_i64().unwrap_or(0))
}

pub fn kurus_to_dec(k: i64) -> Decimal {
    Decimal::from(k) / Decimal::from(100)
}

pub fn opt_kurus_to_dec(k: Option<i64>) -> Option<Decimal> {
    k.map(|v| Decimal::from(v) / Decimal::from(100))
}
