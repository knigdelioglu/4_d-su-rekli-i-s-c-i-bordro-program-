use super::deductions_oracle::{contributions, ContributionOracle};
use super::exemptions_oracle::{
    income_tax_exemption, meal_exemption, stamp_tax, IncomeTaxExemptionOracle, StampTaxOracle,
};
use super::pek_oracle::{normal, NormalPekInput, PekOracle};
use super::tax_oracle::{current_progressive_tax, monthly_minimum_gv_base, round_money};
use payroll_core::TaxBracket;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy)]
pub struct NormalPayrollInput<'a> {
    pub hakedis_days: i32,
    pub worked_days: i32,
    pub statutory_days: i32,
    pub daily_base: Decimal,
    pub daily_meal: Decimal,
    pub daily_meal_sgk_capacity: Decimal,
    pub daily_meal_gv_capacity: Decimal,
    pub daily_minimum: Decimal,
    pub pek_multiplier: Decimal,
    pub worker_sgk_percent: Decimal,
    pub worker_unemployment_percent: Decimal,
    pub employer_sgk_percent: Decimal,
    pub employer_unemployment_percent: Decimal,
    pub stamp_rate_binde: Decimal,
    pub tax_brackets: &'a [TaxBracket],
}

#[derive(Debug, Clone, Copy)]
pub struct NormalPayrollOracle {
    pub base_income: Decimal,
    pub meal_income: Decimal,
    pub income_total: Decimal,
    pub statutory_meal_sgk_capacity: Decimal,
    pub statutory_meal_gv_capacity: Decimal,
    pub pek: PekOracle,
    pub contributions: ContributionOracle,
    pub gv_base: Decimal,
    pub monthly_asgari_gv_base: Decimal,
    pub gross_gv: Decimal,
    pub gv_exemption: IncomeTaxExemptionOracle,
    pub stamp: StampTaxOracle,
    pub deduction_total: Decimal,
    pub net_payment: Decimal,
}

pub fn calculate(input: NormalPayrollInput<'_>) -> NormalPayrollOracle {
    let hakedis_days = Decimal::from(input.hakedis_days.max(0));
    let worked_days = input.worked_days.max(0);
    let base_income = round_money(input.daily_base.max(Decimal::ZERO) * hakedis_days);
    let meal_income = round_money(input.daily_meal.max(Decimal::ZERO) * Decimal::from(worked_days));
    let income_total = round_money(base_income + meal_income);
    let meal_exemptions = meal_exemption(
        meal_income,
        worked_days,
        input.daily_meal_sgk_capacity,
        input.daily_meal_gv_capacity,
    );
    let statutory_meal_sgk_capacity =
        round_money(input.daily_meal_sgk_capacity.max(Decimal::ZERO) * Decimal::from(worked_days));
    let statutory_meal_gv_capacity =
        round_money(input.daily_meal_gv_capacity.max(Decimal::ZERO) * Decimal::from(worked_days));

    let worker_sgk_rate = input.worker_sgk_percent / Decimal::from(100);
    let worker_unemployment_rate = input.worker_unemployment_percent / Decimal::from(100);
    let employer_sgk_rate = input.employer_sgk_percent / Decimal::from(100);
    let employer_unemployment_rate = input.employer_unemployment_percent / Decimal::from(100);
    let pek = normal(NormalPekInput {
        gross_income: income_total,
        sgk_meal_exemption: meal_exemptions.sgk,
        statutory_days: input.statutory_days,
        daily_minimum: input.daily_minimum,
        pek_multiplier: input.pek_multiplier,
    });
    let contributions = contributions(
        pek.prim_matrahi,
        pek.final_pek,
        pek.lower_completion,
        worker_sgk_rate,
        worker_unemployment_rate,
        employer_sgk_rate,
        employer_unemployment_rate,
    );

    let gv_base = round_money(
        (income_total
            - meal_exemptions.gv
            - contributions.worker_sgk
            - contributions.worker_unemployment)
            .max(Decimal::ZERO),
    );
    let monthly_asgari_gv_base = monthly_minimum_gv_base(
        input.daily_minimum,
        worker_sgk_rate,
        worker_unemployment_rate,
    );
    let gross_gv = current_progressive_tax(gv_base, Decimal::ZERO, input.tax_brackets);
    let asgari_gv_entitlement =
        current_progressive_tax(monthly_asgari_gv_base, Decimal::ZERO, input.tax_brackets);
    let gv_exemption = income_tax_exemption(gross_gv, asgari_gv_entitlement, Decimal::ZERO);
    let stamp = stamp_tax(
        (income_total - meal_exemptions.gv).max(Decimal::ZERO),
        round_money(input.daily_minimum.max(Decimal::ZERO) * Decimal::from(30)),
        input.stamp_rate_binde / Decimal::from(1000),
        Decimal::ZERO,
    );
    let deduction_total = round_money(
        contributions.worker_sgk
            + contributions.worker_unemployment
            + gv_exemption.withheld
            + stamp.withheld,
    );

    NormalPayrollOracle {
        base_income,
        meal_income,
        income_total,
        statutory_meal_sgk_capacity,
        statutory_meal_gv_capacity,
        pek,
        contributions,
        gv_base,
        monthly_asgari_gv_base,
        gross_gv,
        gv_exemption,
        stamp,
        deduction_total,
        net_payment: round_money(income_total - deduction_total),
    }
}
