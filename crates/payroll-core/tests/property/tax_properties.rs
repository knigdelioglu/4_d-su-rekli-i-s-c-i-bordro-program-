use super::{decimal_from_cents, SimpleNormalParameters};
use crate::reference::tax_oracle;
use payroll_core::{
    calculate_aylik_asgari_ucret_gv_matrahi, calculate_gelir_vergisi_with_brackets,
    calculate_total_tax_for_cumulative_matrah_with_brackets, AnnualPayrollParameters,
};
use proptest::prelude::*;
use rust_decimal::Decimal;

fn tax_brackets() -> Vec<payroll_core::TaxBracket> {
    AnnualPayrollParameters::default_for_2026().gelirVergisiDilimleri
}

proptest! {
    #![proptest_config(crate::property::proptest_config(160))]

    #[test]
    fn progressive_total_tax_matches_independent_piecewise_oracle(
        cumulative_cents in 0i64..=600_000_000i64,
    ) {
        let cumulative = decimal_from_cents(cumulative_cents);
        let brackets = tax_brackets();
        let production = calculate_total_tax_for_cumulative_matrah_with_brackets(
            cumulative,
            &brackets,
        );
        let expected = tax_oracle::total_progressive_tax(cumulative, &brackets);
        prop_assert_eq!(production, expected);
    }

    #[test]
    fn marginal_tax_matches_oracle_and_cannot_decrease_when_income_increases(
        previous_cents in 0i64..=300_000_000i64,
        current_cents in 0i64..=200_000_000i64,
        additional_cents in 0i64..=200_000_000i64,
    ) {
        let previous = decimal_from_cents(previous_cents);
        let current = decimal_from_cents(current_cents);
        let additional = decimal_from_cents(additional_cents);
        let brackets = tax_brackets();
        let production = calculate_gelir_vergisi_with_brackets(current, previous, &brackets);
        let expected = tax_oracle::current_progressive_tax(current, previous, &brackets);
        let increased = calculate_gelir_vergisi_with_brackets(
            current + additional,
            previous,
            &brackets,
        );

        prop_assert_eq!(production, expected);
        prop_assert!(production >= rust_decimal::Decimal::ZERO);
        prop_assert!(increased >= production);
    }

    #[test]
    fn minimum_wage_gv_reference_matches_oracle_and_is_monotone(
        low_cents in 1i64..=200_000i64,
        increase_cents in 0i64..=200_000i64,
    ) {
        let low = decimal_from_cents(low_cents);
        let high = low + decimal_from_cents(increase_cents);
        let defaults = SimpleNormalParameters::default();
        let actual_low = calculate_aylik_asgari_ucret_gv_matrahi(
            low,
            defaults.worker_sgk_percent / Decimal::from(100),
            defaults.worker_unemployment_percent / Decimal::from(100),
        );
        let actual_high = calculate_aylik_asgari_ucret_gv_matrahi(
            high,
            defaults.worker_sgk_percent / Decimal::from(100),
            defaults.worker_unemployment_percent / Decimal::from(100),
        );
        let expected_low = tax_oracle::monthly_minimum_gv_base(
            low,
            defaults.worker_sgk_percent / Decimal::from(100),
            defaults.worker_unemployment_percent / Decimal::from(100),
        );

        prop_assert_eq!(actual_low, expected_low);
        prop_assert!(actual_high >= actual_low);
    }
}
