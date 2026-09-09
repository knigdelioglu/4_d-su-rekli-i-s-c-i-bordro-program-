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
pub mod transaction;

use rusqlite::types::{Type, Value};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

fn money_to_kurus_i64(value: Decimal) -> Result<i64> {
    if value.normalize().scale() > 2 {
        return Err(DomainError::ValidationError(format!(
            "Parasal değer 2'den fazla ondalık basamak içeremez (kuruş hassasiyeti aşıldı): {}",
            value
        )));
    }
    let scaled = value.checked_mul(Decimal::from(100)).ok_or_else(|| {
        DomainError::InvalidData(format!(
            "Parasal değer kuruşa çevrilirken Decimal taşması oluştu: {}",
            value
        ))
    })?;
    scaled.to_i64().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "Parasal değer SQLite i64 kuruş sınırını aşıyor: {}",
            value
        ))
    })
}

pub fn money_to_kurus(d: Option<Decimal>) -> Result<i64> {
    money_to_kurus_i64(d.unwrap_or_default())
}

pub fn opt_money_to_kurus(d: Option<Decimal>) -> Result<Option<i64>> {
    d.map(money_to_kurus_i64).transpose()
}

/// Backward-compatible alias for callers that used the old generic name.
/// New persistence code must use the explicit money converter.
pub fn dec_to_kurus(d: Option<Decimal>) -> Result<i64> {
    money_to_kurus(d)
}

/// Backward-compatible alias for callers that used the old generic name.
/// New persistence code must use the explicit money converter.
pub fn opt_dec_to_kurus(d: Option<Decimal>) -> Result<Option<i64>> {
    opt_money_to_kurus(d)
}

const RATE_TEXT_PREFIX: &str = "rate:";

/// Persists percentage/rate values without applying the money precision rule.
///
/// Existing rows keep their INTEGER representation (`rate * 100`). Values
/// that cannot be represented by that legacy scale are stored as tagged TEXT
/// in the same SQLite column, which preserves the old schema and exact Decimal
/// precision without silently rounding or truncating the rate.
pub(crate) fn opt_rate_to_sql_value(d: Option<Decimal>) -> Result<Option<Value>> {
    d.map(rate_to_sql_value).transpose()
}

fn rate_to_sql_value(value: Decimal) -> Result<Value> {
    let normalized = value.normalize();
    if normalized.scale() <= 2 {
        let scaled = normalized.checked_mul(Decimal::from(100)).ok_or_else(|| {
            DomainError::InvalidData(format!(
                "Oran SQLite değerine çevrilirken Decimal taşması oluştu: {}",
                value
            ))
        })?;
        let scaled = scaled.to_i64().ok_or_else(|| {
            DomainError::InvalidData(format!("Oran SQLite i64 sınırını aşıyor: {}", value))
        })?;
        return Ok(Value::Integer(scaled));
    }

    Ok(Value::Text(format!("{RATE_TEXT_PREFIX}{normalized}")))
}

fn rate_sql_conversion_error(
    column_index: usize,
    data_type: Type,
    message: impl Into<String>,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column_index,
        data_type,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        )),
    )
}

pub(crate) fn rate_sql_value_to_decimal(
    value: Value,
    column_index: usize,
) -> rusqlite::Result<Option<Decimal>> {
    let data_type = value.data_type();
    match value {
        Value::Null => Ok(None),
        Value::Integer(scaled) => Ok(Some(Decimal::from(scaled) / Decimal::from(100))),
        Value::Text(encoded) => {
            let Some(raw) = encoded.strip_prefix(RATE_TEXT_PREFIX) else {
                return Err(rate_sql_conversion_error(
                    column_index,
                    data_type,
                    "Etiketlenmemiş oran SQLite metni okunamadı.",
                ));
            };
            Decimal::from_str_exact(raw).map(Some).map_err(|error| {
                rate_sql_conversion_error(column_index, data_type, error.to_string())
            })
        }
        _ => Err(rate_sql_conversion_error(
            column_index,
            data_type,
            "Oran SQLite değeri INTEGER veya etiketli TEXT olmalıdır.",
        )),
    }
}

pub fn kurus_to_dec(k: i64) -> Decimal {
    Decimal::from(k) / Decimal::from(100)
}

pub fn opt_kurus_to_dec(k: Option<i64>) -> Option<Decimal> {
    k.map(|v| Decimal::from(v) / Decimal::from(100))
}
