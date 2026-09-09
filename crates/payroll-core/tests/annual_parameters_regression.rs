use payroll_core::{
    validate_annual_payroll_parameters, AnnualPayrollParameters, TaxBracket,
    OPEN_ENDED_TAX_BRACKET_LIMIT,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn valid_parameters() -> AnnualPayrollParameters {
    AnnualPayrollParameters::default_for_2026()
}

#[test]
fn annual_parameter_semantics_match_the_authoritative_core_contract() {
    let mut empty_brackets = valid_parameters();
    empty_brackets.gelirVergisiDilimleri.clear();
    assert!(validate_annual_payroll_parameters(&empty_brackets).is_err());

    for year in [0, -1] {
        let mut invalid = valid_parameters();
        invalid.year = year;
        assert!(validate_annual_payroll_parameters(&invalid).is_err());
    }

    let mut equal_limits = valid_parameters();
    equal_limits.gelirVergisiDilimleri[1].limit = equal_limits.gelirVergisiDilimleri[0].limit;
    assert!(validate_annual_payroll_parameters(&equal_limits).is_err());

    let mut descending_limits = valid_parameters();
    descending_limits.gelirVergisiDilimleri[1].limit = dec!(100);
    assert!(validate_annual_payroll_parameters(&descending_limits).is_err());

    let mut nonpositive_limit = valid_parameters();
    nonpositive_limit.gelirVergisiDilimleri[0].limit = dec!(0);
    assert!(validate_annual_payroll_parameters(&nonpositive_limit).is_err());

    for rate in [dec!(-0.01), dec!(1.01)] {
        let mut invalid = valid_parameters();
        invalid.gelirVergisiDilimleri[0].oran = rate;
        assert!(validate_annual_payroll_parameters(&invalid).is_err());
    }

    let mut excessive_limit = valid_parameters();
    excessive_limit.gelirVergisiDilimleri[0].limit =
        dec!(1000000000000001);
    assert!(validate_annual_payroll_parameters(&excessive_limit).is_err());

    for cap in [dec!(0), dec!(-1)] {
        let mut invalid = valid_parameters();
        invalid.sigortaGvYillikBrutAsgariUcretTavani = Some(cap);
        assert!(validate_annual_payroll_parameters(&invalid).is_err());
    }

    let mut boundary = valid_parameters();
    boundary.gelirVergisiDilimleri = vec![TaxBracket {
        limit: Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT),
        oran: dec!(1),
    }];
    assert!(validate_annual_payroll_parameters(&boundary).is_ok());
    assert!(validate_annual_payroll_parameters(&valid_parameters()).is_ok());
}
