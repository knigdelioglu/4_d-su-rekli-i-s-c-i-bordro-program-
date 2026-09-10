use super::tax_oracle::{round_gv, round_money};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy)]
pub struct MealExemptionOracle {
    pub sgk: Decimal,
    pub gv: Decimal,
}

pub fn meal_exemption(
    actual_meal: Decimal,
    worked_days: i32,
    daily_sgk_capacity: Decimal,
    daily_gv_capacity: Decimal,
) -> MealExemptionOracle {
    let days = Decimal::from(worked_days.max(0));
    let actual = round_money(actual_meal.max(Decimal::ZERO));
    let sgk_capacity = round_money(daily_sgk_capacity.max(Decimal::ZERO) * days);
    let gv_capacity = round_money(daily_gv_capacity.max(Decimal::ZERO) * days);
    MealExemptionOracle {
        sgk: actual.min(sgk_capacity),
        gv: actual.min(gv_capacity),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IncomeTaxExemptionOracle {
    pub monthly_entitlement: Decimal,
    pub used_before: Decimal,
    pub remaining_before: Decimal,
    pub applied_current: Decimal,
    pub remaining_after: Decimal,
    pub withheld: Decimal,
}

pub fn income_tax_exemption(
    gross_tax: Decimal,
    monthly_entitlement: Decimal,
    used_before: Decimal,
) -> IncomeTaxExemptionOracle {
    let gross_tax = round_gv(gross_tax).max(Decimal::ZERO);
    let monthly_entitlement = round_gv(monthly_entitlement).max(Decimal::ZERO);
    let used_before = round_gv(used_before).max(Decimal::ZERO);
    let remaining_before = (monthly_entitlement - used_before).max(Decimal::ZERO);
    let applied_current = gross_tax.min(remaining_before);
    IncomeTaxExemptionOracle {
        monthly_entitlement,
        used_before,
        remaining_before,
        applied_current,
        remaining_after: remaining_before - applied_current,
        withheld: gross_tax - applied_current,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StampTaxOracle {
    pub gross: Decimal,
    pub monthly_entitlement: Decimal,
    pub used_before: Decimal,
    pub applied: Decimal,
    pub remaining_after: Decimal,
    pub withheld: Decimal,
}

pub fn stamp_tax(
    taxable_income: Decimal,
    monthly_minimum_gross: Decimal,
    stamp_rate: Decimal,
    used_before: Decimal,
) -> StampTaxOracle {
    let gross = round_money(taxable_income.max(Decimal::ZERO) * stamp_rate);
    let monthly_entitlement = round_money(monthly_minimum_gross.max(Decimal::ZERO) * stamp_rate);
    let used_before = used_before.max(Decimal::ZERO);
    let remaining_before = (monthly_entitlement - used_before).max(Decimal::ZERO);
    let applied = gross.min(remaining_before);
    StampTaxOracle {
        gross,
        monthly_entitlement,
        used_before,
        applied,
        remaining_after: remaining_before - applied,
        withheld: (gross - applied).max(Decimal::ZERO),
    }
}
