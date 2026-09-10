use super::tax_oracle::round_sgk;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy)]
pub struct ContributionOracle {
    pub worker_sgk: Decimal,
    pub worker_unemployment: Decimal,
    pub employer_sgk: Decimal,
    pub employer_unemployment: Decimal,
    pub employer_lower_completion: Decimal,
    pub employer_total: Decimal,
}

pub fn contributions(
    prim_matrahi: Decimal,
    final_pek: Decimal,
    lower_completion: Decimal,
    worker_sgk_rate: Decimal,
    worker_unemployment_rate: Decimal,
    employer_sgk_rate: Decimal,
    employer_unemployment_rate: Decimal,
) -> ContributionOracle {
    let worker_sgk = round_sgk(prim_matrahi * worker_sgk_rate);
    let worker_unemployment = round_sgk(prim_matrahi * worker_unemployment_rate);
    let employer_sgk = round_sgk(final_pek * employer_sgk_rate);
    let employer_unemployment = round_sgk(final_pek * employer_unemployment_rate);
    let employer_lower_completion = round_sgk(lower_completion * worker_sgk_rate)
        + round_sgk(lower_completion * worker_unemployment_rate);
    ContributionOracle {
        worker_sgk,
        worker_unemployment,
        employer_sgk,
        employer_unemployment,
        employer_lower_completion,
        employer_total: employer_sgk + employer_unemployment + employer_lower_completion,
    }
}
