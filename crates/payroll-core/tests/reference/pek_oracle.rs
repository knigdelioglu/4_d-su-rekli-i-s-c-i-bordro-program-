use super::tax_oracle::round_money;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy)]
pub struct NormalPekInput {
    pub gross_income: Decimal,
    pub sgk_meal_exemption: Decimal,
    pub statutory_days: i32,
    pub daily_minimum: Decimal,
    pub pek_multiplier: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub struct PekOracle {
    pub ham_pek: Decimal,
    pub prim_matrahi: Decimal,
    pub lower_bound: Decimal,
    pub upper_bound: Decimal,
    pub lower_completion: Decimal,
    pub final_pek: Decimal,
}

/// Independent no-carry PEK model used by the generated normal-payroll
/// differential tests. Lower-bound completion is employer-only, so it never
/// increases the worker's priming base.
pub fn normal(input: NormalPekInput) -> PekOracle {
    let ham_pek = round_money((input.gross_income - input.sgk_meal_exemption).max(Decimal::ZERO));
    let days = Decimal::from(input.statutory_days.max(0));
    let lower_bound = round_money(input.daily_minimum.max(Decimal::ZERO) * days);
    let upper_bound = round_money(
        input.daily_minimum.max(Decimal::ZERO) * input.pek_multiplier.max(Decimal::ZERO) * days,
    );
    let prim_matrahi = ham_pek.min(upper_bound).max(Decimal::ZERO);
    let lower_completion = if prim_matrahi > Decimal::ZERO && prim_matrahi < lower_bound {
        round_money(lower_bound - prim_matrahi)
    } else {
        Decimal::ZERO
    };
    PekOracle {
        ham_pek,
        prim_matrahi,
        lower_bound,
        upper_bound,
        lower_completion,
        final_pek: prim_matrahi + lower_completion,
    }
}
