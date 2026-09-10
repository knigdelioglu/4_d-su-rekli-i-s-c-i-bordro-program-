use payroll_core::TaxBracket;
use rust_decimal::{Decimal, RoundingStrategy};

/// Test-only money rounding. This deliberately does not call a payroll-core
/// rounding helper so the differential test retains an independent boundary.
pub fn round_money(value: Decimal) -> Decimal {
    value.round_dp(2)
}

pub fn round_sgk(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

pub fn round_gv(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Calculates progressive tax by intersecting the cumulative base with each
/// bracket. It intentionally does not reuse payroll-core's total-tax helper.
pub fn total_progressive_tax(cumulative: Decimal, brackets: &[TaxBracket]) -> Decimal {
    let target = cumulative.max(Decimal::ZERO);
    if target.is_zero() {
        return Decimal::ZERO;
    }

    let mut lower = Decimal::ZERO;
    let mut tax = Decimal::ZERO;
    for bracket in brackets {
        let upper = bracket.limit;
        if target <= lower {
            break;
        }

        let taxable_end = target.min(upper);
        if taxable_end > lower {
            tax += (taxable_end - lower) * bracket.oran;
        }
        if target <= upper {
            return tax;
        }
        lower = upper;
    }

    brackets
        .last()
        .map(|last| tax + (target - lower).max(Decimal::ZERO) * last.oran)
        .unwrap_or(tax)
}

pub fn current_progressive_tax(
    current_base: Decimal,
    previous_cumulative: Decimal,
    brackets: &[TaxBracket],
) -> Decimal {
    if current_base <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    round_gv(
        total_progressive_tax(previous_cumulative + current_base, brackets)
            - total_progressive_tax(previous_cumulative, brackets),
    )
}

pub fn monthly_minimum_gv_base(
    daily_minimum: Decimal,
    worker_sgk_rate: Decimal,
    worker_unemployment_rate: Decimal,
) -> Decimal {
    let monthly_gross = round_money(daily_minimum.max(Decimal::ZERO) * Decimal::from(30));
    let worker_sgk = round_sgk(monthly_gross * worker_sgk_rate);
    let worker_unemployment = round_sgk(monthly_gross * worker_unemployment_rate);
    (monthly_gross - worker_sgk - worker_unemployment).max(Decimal::ZERO)
}
