use payroll_core::TaxBracket;
use rust_decimal::Decimal;

/// Test-only money rounding expressed as integer rational arithmetic. This
/// deliberately does not call a payroll-core rounding helper or Decimal's
/// rounding API, so half-kuruş expectations stay independent of production.
pub fn round_money(value: Decimal) -> Decimal {
    let denominator = pow10(value.scale());
    round_ratio_to_cents(value.mantissa(), denominator)
}

/// Rounds an exact rational amount to kuruş, with half-kuruş values away from
/// zero, as required by Law 5083 Article 2. Inputs in the test oracle are
/// bounded payroll amounts, so checked integer arithmetic makes overflow
/// explicit instead of silently changing the expected value.
pub fn round_ratio_to_cents(numerator: i128, denominator: i128) -> Decimal {
    assert!(denominator > 0, "oracle denominator must be positive");
    let signed_cents = numerator
        .checked_mul(100)
        .expect("oracle amount fits i128 cents");
    let sign = if signed_cents < 0 { -1 } else { 1 };
    let magnitude = signed_cents
        .checked_abs()
        .expect("oracle amount magnitude fits i128");
    let mut cents = magnitude / denominator;
    let remainder = magnitude % denominator;
    if remainder
        .checked_mul(2)
        .expect("oracle remainder fits i128")
        >= denominator
    {
        cents += 1;
    }
    Decimal::from_i128_with_scale(sign * cents, 2)
}

pub fn round_product_to_cents(left: Decimal, right: Decimal) -> Decimal {
    let numerator = left
        .mantissa()
        .checked_mul(right.mantissa())
        .expect("oracle product fits i128");
    let denominator = pow10(left.scale() + right.scale());
    round_ratio_to_cents(numerator, denominator)
}

pub fn round_percent_product_to_cents(base: Decimal, percent: Decimal, units: i32) -> Decimal {
    let numerator = base
        .mantissa()
        .checked_mul(percent.mantissa())
        .and_then(|value| value.checked_mul(i128::from(units)))
        .expect("oracle percentage product fits i128");
    let denominator = pow10(base.scale() + percent.scale())
        .checked_mul(100)
        .expect("oracle percentage denominator fits i128");
    round_ratio_to_cents(numerator, denominator)
}

fn pow10(scale: u32) -> i128 {
    10_i128
        .checked_pow(scale)
        .expect("oracle decimal scale fits i128")
}

pub fn round_sgk(value: Decimal) -> Decimal {
    round_money(value)
}

pub fn round_gv(value: Decimal) -> Decimal {
    round_money(value)
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
