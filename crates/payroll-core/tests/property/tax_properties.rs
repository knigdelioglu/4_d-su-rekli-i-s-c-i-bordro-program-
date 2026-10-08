use super::{decimal_from_cents, SimpleNormalParameters};
use crate::reference::tax_oracle;
use payroll_core::{
    calculate_aylik_asgari_ucret_gv_matrahi, calculate_gelir_vergisi_with_brackets,
    calculate_gv_hesap_detayi_with_brackets, calculate_is_primi_detayi,
    calculate_monthly_stamp_tax_state, calculate_segmented_meal_exemptions,
    calculate_total_tax_for_cumulative_matrah_with_brackets, AnnualPayrollParameters,
    IsPrimiGrupItem, MealExemptionSegment, TaxBracket,
};
use proptest::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;

#[test]
fn stamp_tax_half_kurus_matches_integer_rational_oracle() {
    let actual =
        calculate_monthly_stamp_tax_state(dec!(33500), dec!(33030), dec!(0.00759), Decimal::ZERO);
    let expected = crate::reference::exemptions_oracle::stamp_tax(
        dec!(33500),
        dec!(33030),
        dec!(0.00759),
        Decimal::ZERO,
    );

    assert_eq!(expected.gross, dec!(254.27));
    assert_eq!(expected.monthly_entitlement, dec!(250.70));
    assert_eq!(actual.brutDamgaVergisi, expected.gross);
    assert_eq!(actual.aylikDamgaIstisnaHakki, expected.monthly_entitlement);
    assert_eq!(actual.kesilenDamgaVergisi, expected.withheld);
    assert_eq!(expected.withheld, dec!(3.57));
}

#[test]
fn work_premium_half_kurus_matches_integer_rational_oracle() {
    let groups = [IsPrimiGrupItem {
        id: "half-kurus".into(),
        ad: "half-kurus".into(),
        oran: dec!(50),
        aktif: true,
    }];
    let actual = calculate_is_primi_detayi(dec!(1000.01), 1, Some("half-kurus"), Some(&groups))
        .expect("synthetic work premium group is valid");
    let expected = tax_oracle::round_percent_product_to_cents(dec!(1000.01), dec!(50), 1);

    assert_eq!(expected, dec!(500.01));
    assert_eq!(actual.gunlukIsPrimi, expected);
    assert_eq!(actual.tutar, expected);
}

#[test]
fn negative_half_kurus_matches_signed_integer_rational_oracle() {
    for (raw, expected) in [
        (dec!(1.005), dec!(1.01)),
        (dec!(-1.005), dec!(-1.01)),
        (dec!(1.004), dec!(1.00)),
        (dec!(-1.004), dec!(-1.00)),
    ] {
        assert_eq!(crate::reference::tax_oracle::round_money(raw), expected);
        assert_eq!(payroll_core::round_currency_amount(raw), expected);
    }
}

#[test]
fn statutory_meal_date_split_does_not_round_one_payment_twice() {
    let segments = [
        MealExemptionSegment {
            actual: dec!(10.005),
            sgk_capacity: dec!(100),
            gv_capacity: dec!(100),
        },
        MealExemptionSegment {
            actual: dec!(10.005),
            sgk_capacity: dec!(100),
            gv_capacity: dec!(100),
        },
    ];
    let actual = calculate_segmented_meal_exemptions(&segments);
    let expected = crate::reference::exemptions_oracle::segmented_meal_exemption(&[
        (dec!(10.005), dec!(100), dec!(100)),
        (dec!(10.005), dec!(100), dec!(100)),
    ]);

    assert_eq!(actual.sgk, expected.sgk);
    assert_eq!(actual.gv, expected.gv);
    assert_eq!(expected.sgk, dec!(20.01));
    assert_eq!(expected.gv, dec!(20.01));
}

#[test]
fn synthetic_uat_payment_month_boundary_resets_cumulative_gv() {
    // These synthetic 2027/2028 brackets and input bases reproduce one
    // ordinary UAT row on each side of the service-period year boundary.
    // Expected taxes are calculated independently from the piecewise tariff,
    // not copied from the stored payroll result.
    let brackets = [
        TaxBracket {
            limit: dec!(100000),
            oran: dec!(0.15),
        },
        TaxBracket {
            limit: dec!(200000),
            oran: dec!(0.20),
        },
        TaxBracket {
            limit: dec!(300000),
            oran: dec!(0.27),
        },
        TaxBracket {
            limit: dec!(400000),
            oran: dec!(0.40),
        },
    ];

    // 2027-11 service period: 15 Nov–14 Dec, paid/taxed in 2027 month 12.
    let november_expected_gross =
        tax_oracle::current_progressive_tax(dec!(92972.22), dec!(721430.08), &brackets);
    let november_minimum_base = dec!(28075.50);
    let november_expected_exemption =
        tax_oracle::current_progressive_tax(november_minimum_base, dec!(308830.50), &brackets);
    let november_expected_withheld =
        tax_oracle::round_money(november_expected_gross - november_expected_exemption);
    let november_actual = calculate_gv_hesap_detayi_with_brackets(
        dec!(92972.22),
        dec!(721430.08),
        november_minimum_base,
        dec!(308830.50),
        &brackets,
    );

    assert_eq!(november_expected_gross, dec!(37188.89));
    assert_eq!(november_expected_exemption, dec!(11230.20));
    assert_eq!(november_expected_withheld, dec!(25958.69));
    assert_eq!(november_actual.brutGelirVergisi, november_expected_gross);
    assert_eq!(
        november_actual.asgariUcretGvIstisnasi,
        november_expected_exemption
    );
    assert_eq!(
        november_actual.kesilenGelirVergisi,
        november_expected_withheld
    );

    // 2027-12 service period: 15 Dec–14 Jan, paid/taxed in 2028 month 1.
    // The income-tax and minimum-wage reference cumulatives both restart at 0.
    let december_expected_gross =
        tax_oracle::current_progressive_tax(dec!(96047.68), Decimal::ZERO, &brackets);
    let december_expected_exemption =
        tax_oracle::current_progressive_tax(november_minimum_base, Decimal::ZERO, &brackets);
    let december_expected_withheld =
        tax_oracle::round_money(december_expected_gross - december_expected_exemption);
    let december_actual = calculate_gv_hesap_detayi_with_brackets(
        dec!(96047.68),
        Decimal::ZERO,
        november_minimum_base,
        Decimal::ZERO,
        &brackets,
    );

    assert_eq!(december_expected_gross, dec!(14407.15));
    assert_eq!(december_expected_exemption, dec!(4211.33));
    assert_eq!(december_expected_withheld, dec!(10195.82));
    assert_eq!(december_actual.brutGelirVergisi, december_expected_gross);
    assert_eq!(
        december_actual.asgariUcretGvIstisnasi,
        december_expected_exemption
    );
    assert_eq!(
        december_actual.kesilenGelirVergisi,
        december_expected_withheld
    );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UatCohortFixture {
    synthetic_brackets: Vec<UatBracketFixture>,
    periods: Vec<UatTaxPeriodFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UatBracketFixture {
    limit_cents: i64,
    rate_numerator: i64,
    rate_denominator: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UatTaxPeriodFixture {
    service_period_id: String,
    tax_year: i32,
    tax_month: i32,
    rows: Vec<UatTaxRowFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UatTaxRowFixture {
    tax_base_cents: i64,
    previous_tax_cumulative_cents: i64,
    minimum_tax_base_cents: i64,
    previous_minimum_tax_cumulative_cents: i64,
    observed_gross_tax_cents: i64,
    observed_minimum_exemption_cents: i64,
    observed_withheld_tax_cents: i64,
}

#[test]
fn synthetic_uat_november_december_cohort_matches_independent_tax_oracle() {
    // The persisted UAT fields in this fixture are the observed side only.
    // Expected values are recomputed from integer-cent bases, the recorded
    // prior cumulative inputs, and the explicit synthetic piecewise tariff.
    // The fixture deliberately contains no identifying worker data.
    let fixture: UatCohortFixture = serde_json::from_str(include_str!(
        "../fixtures/uat_2027_tax_boundary_reference.json"
    ))
    .expect("the versioned synthetic UAT cohort fixture is valid JSON");
    let brackets: Vec<TaxBracket> = fixture
        .synthetic_brackets
        .iter()
        .map(|bracket| TaxBracket {
            limit: decimal_from_cents(bracket.limit_cents),
            oran: Decimal::from(bracket.rate_numerator) / Decimal::from(bracket.rate_denominator),
        })
        .collect();

    assert_eq!(fixture.periods.len(), 2);
    let mut totals = Vec::new();
    for period in &fixture.periods {
        assert_eq!(period.rows.len(), 20);
        match period.service_period_id.as_str() {
            "2027-11" => {
                assert_eq!((period.tax_year, period.tax_month), (2027, 12));
            }
            "2027-12" => {
                assert_eq!((period.tax_year, period.tax_month), (2028, 1));
                assert!(period
                    .rows
                    .iter()
                    .all(|row| row.previous_tax_cumulative_cents == 0
                        && row.previous_minimum_tax_cumulative_cents == 0));
            }
            other => panic!("unexpected synthetic UAT period {other}"),
        }

        let mut gross_tax_total = Decimal::ZERO;
        let mut exemption_total = Decimal::ZERO;
        let mut withheld_total = Decimal::ZERO;
        for row in &period.rows {
            let tax_base = decimal_from_cents(row.tax_base_cents);
            let previous_tax = decimal_from_cents(row.previous_tax_cumulative_cents);
            let minimum_base = decimal_from_cents(row.minimum_tax_base_cents);
            let previous_minimum = decimal_from_cents(row.previous_minimum_tax_cumulative_cents);
            let expected_gross =
                tax_oracle::current_progressive_tax(tax_base, previous_tax, &brackets);
            let expected_exemption =
                tax_oracle::current_progressive_tax(minimum_base, previous_minimum, &brackets);
            let expected_withheld = tax_oracle::round_money(expected_gross - expected_exemption);
            let production = calculate_gv_hesap_detayi_with_brackets(
                tax_base,
                previous_tax,
                minimum_base,
                previous_minimum,
                &brackets,
            );

            assert_eq!(production.brutGelirVergisi, expected_gross);
            assert_eq!(production.asgariUcretGvIstisnasi, expected_exemption);
            assert_eq!(production.kesilenGelirVergisi, expected_withheld);
            assert_eq!(
                expected_gross,
                decimal_from_cents(row.observed_gross_tax_cents)
            );
            assert_eq!(
                expected_exemption,
                decimal_from_cents(row.observed_minimum_exemption_cents)
            );
            assert_eq!(
                expected_withheld,
                decimal_from_cents(row.observed_withheld_tax_cents)
            );

            gross_tax_total += expected_gross;
            exemption_total += expected_exemption;
            withheld_total += expected_withheld;
        }

        match period.service_period_id.as_str() {
            "2027-11" => {
                assert_eq!(gross_tax_total, dec!(736516.61));
                assert_eq!(exemption_total, dec!(224604.00));
                assert_eq!(withheld_total, dec!(511912.61));
            }
            "2027-12" => {
                assert_eq!(gross_tax_total, dec!(285396.27));
                assert_eq!(exemption_total, dec!(84226.60));
                assert_eq!(withheld_total, dec!(201169.67));
            }
            _ => unreachable!(),
        }
        totals.push((gross_tax_total, exemption_total, withheld_total));
    }

    // All 40 worker rows reconcile to the UAT 299k TL change before
    // non-income-tax deductions are considered.
    assert_eq!(totals[0].2 - totals[1].2, dec!(310742.94));
}

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
