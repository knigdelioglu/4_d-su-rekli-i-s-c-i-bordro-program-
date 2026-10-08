//! Currency rounding rules used for Turkish-lira results.

use rust_decimal::{Decimal, RoundingStrategy};

/// Round a monetary result to kuruş. Turkish lira amounts use half-kuruş-up
/// rounding under Law 5083 Article 2; midpoint values must not use banker's
/// (nearest-even) rounding.
pub fn round_currency_amount(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}
