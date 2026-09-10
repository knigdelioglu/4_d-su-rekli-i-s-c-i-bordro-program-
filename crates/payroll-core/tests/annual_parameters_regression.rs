use chrono::NaiveDate;
use payroll_core::{
    validate_annual_payroll_parameters, AnnualPayrollParameters, DonemselKurumDegerleri,
    TaxBracket, OPEN_ENDED_TAX_BRACKET_LIMIT,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatutoryReference {
    schema_version: u32,
    year: i32,
    verification_status: String,
    generated_from_production: bool,
    source: ReferenceSource,
    annual_payroll_parameters: AnnualReference,
    period_parameters: PeriodReference,
    references: Vec<ReferenceLink>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceSource {
    #[serde(rename = "type")]
    source_type: String,
    verified_at: String,
    notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceLink {
    name: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatutoryValue<T> {
    value: T,
    source: String,
    effective_from: String,
    verification_status: String,
    notes: String,
    #[serde(default)]
    authoritative_comparison: Option<AuthoritativeComparison>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthoritativeComparison {
    value: Decimal,
    source: String,
    effective_from: String,
    verification_status: String,
    notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceBracket {
    limit: Decimal,
    rate: Decimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnnualReference {
    year: StatutoryValue<i32>,
    income_tax_brackets: StatutoryValue<Vec<ReferenceBracket>>,
    annual_gross_minimum_wage: StatutoryValue<Decimal>,
    annual_insurance_gross_minimum_wage_cap: StatutoryValue<Decimal>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeriodReference {
    daily_minimum_wage: StatutoryValue<Decimal>,
    monthly_minimum_wage: StatutoryValue<Decimal>,
    monthly_minimum_wage_gv_base: StatutoryValue<Decimal>,
    worker_sgk_rate_percent: StatutoryValue<Decimal>,
    worker_unemployment_rate_percent: StatutoryValue<Decimal>,
    employer_sgk_rate_percent: StatutoryValue<Decimal>,
    employer_unemployment_rate_percent: StatutoryValue<Decimal>,
    pek_ceiling_multiplier: StatutoryValue<Decimal>,
    stamp_tax_rate_per_mille: StatutoryValue<Decimal>,
    gv_meal_exemption_daily: StatutoryValue<Decimal>,
    sgk_meal_exemption_daily: StatutoryValue<Decimal>,
    annual_minimum_wage_gv_exemption_reference: StatutoryValue<Decimal>,
}

fn valid_parameters() -> AnnualPayrollParameters {
    AnnualPayrollParameters::default_for_2026()
}

fn statutory_reference() -> StatutoryReference {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/statutory/2026.json");
    let payload = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} okunamadı: {error}", path.display()));
    serde_json::from_str(&payload)
        .unwrap_or_else(|error| panic!("{} JSON parse edilemedi: {error}", path.display()))
}

fn assert_value_metadata<T>(name: &str, value: &StatutoryValue<T>) {
    assert!(!value.source.trim().is_empty(), "{name}.source boş");
    assert!(!value.notes.trim().is_empty(), "{name}.notes boş");
    assert!(
        matches!(
            value.verification_status.as_str(),
            "verified" | "needs_authoritative_verification"
        ),
        "{name}.verificationStatus tanımsız: {}",
        value.verification_status
    );
    NaiveDate::parse_from_str(&value.effective_from, "%Y-%m-%d")
        .unwrap_or_else(|error| panic!("{name}.effectiveFrom geçersiz: {error}"));
}

fn assert_reference_metadata(reference: &StatutoryReference) {
    assert_eq!(reference.schema_version, 1);
    assert_eq!(reference.year, 2026);
    assert_eq!(reference.verification_status, "partial");
    assert!(!reference.generated_from_production);
    assert_eq!(
        reference.source.source_type,
        "independent_statutory_reference"
    );
    assert!(!reference.source.notes.trim().is_empty());
    NaiveDate::parse_from_str(&reference.source.verified_at, "%Y-%m-%d")
        .expect("statutory reference source.verifiedAt YYYY-MM-DD olmalı");

    let annual = &reference.annual_payroll_parameters;
    assert_value_metadata("annualPayrollParameters.year", &annual.year);
    assert_value_metadata(
        "annualPayrollParameters.incomeTaxBrackets",
        &annual.income_tax_brackets,
    );
    assert_value_metadata(
        "annualPayrollParameters.annualGrossMinimumWage",
        &annual.annual_gross_minimum_wage,
    );
    assert_value_metadata(
        "annualPayrollParameters.annualInsuranceGrossMinimumWageCap",
        &annual.annual_insurance_gross_minimum_wage_cap,
    );

    let period = &reference.period_parameters;
    assert_value_metadata(
        "periodParameters.dailyMinimumWage",
        &period.daily_minimum_wage,
    );
    assert_value_metadata(
        "periodParameters.monthlyMinimumWage",
        &period.monthly_minimum_wage,
    );
    assert_value_metadata(
        "periodParameters.monthlyMinimumWageGvBase",
        &period.monthly_minimum_wage_gv_base,
    );
    assert_value_metadata(
        "periodParameters.workerSgkRatePercent",
        &period.worker_sgk_rate_percent,
    );
    assert_value_metadata(
        "periodParameters.workerUnemploymentRatePercent",
        &period.worker_unemployment_rate_percent,
    );
    assert_value_metadata(
        "periodParameters.employerSgkRatePercent",
        &period.employer_sgk_rate_percent,
    );
    assert_value_metadata(
        "periodParameters.employerUnemploymentRatePercent",
        &period.employer_unemployment_rate_percent,
    );
    assert_value_metadata(
        "periodParameters.pekCeilingMultiplier",
        &period.pek_ceiling_multiplier,
    );
    assert_value_metadata(
        "periodParameters.stampTaxRatePerMille",
        &period.stamp_tax_rate_per_mille,
    );
    assert_value_metadata(
        "periodParameters.gvMealExemptionDaily",
        &period.gv_meal_exemption_daily,
    );
    assert_value_metadata(
        "periodParameters.sgkMealExemptionDaily",
        &period.sgk_meal_exemption_daily,
    );
    assert_value_metadata(
        "periodParameters.annualMinimumWageGvExemptionReference",
        &period.annual_minimum_wage_gv_exemption_reference,
    );
    assert_eq!(
        period.sgk_meal_exemption_daily.verification_status,
        "needs_authoritative_verification"
    );

    let comparison = period
        .sgk_meal_exemption_daily
        .authoritative_comparison
        .as_ref()
        .expect("SGK yemek alanının açık authoritative comparison kaydı olmalı");
    assert_eq!(comparison.value, dec!(158.00));
    assert!(!comparison.source.trim().is_empty());
    assert!(!comparison.notes.trim().is_empty());
    assert_eq!(
        comparison.verification_status,
        "needs_authoritative_verification"
    );
    NaiveDate::parse_from_str(&comparison.effective_from, "%Y-%m-%d")
        .expect("authoritativeComparison.effectiveFrom YYYY-MM-DD olmalı");

    assert!(reference.references.len() >= 6);
    for reference_link in &reference.references {
        assert!(!reference_link.name.trim().is_empty());
        assert!(reference_link.url.starts_with("https://"));
    }
}

#[test]
fn production_2026_parameters_match_independent_statutory_reference() {
    let reference = statutory_reference();
    assert_reference_metadata(&reference);

    let annual_reference = &reference.annual_payroll_parameters;
    let production_annual = AnnualPayrollParameters::default_for_2026();
    assert_eq!(production_annual.year, annual_reference.year.value);
    assert_eq!(production_annual.year, reference.year);
    assert_eq!(
        production_annual.sigortaGvYillikBrutAsgariUcretTavani,
        Some(annual_reference.annual_gross_minimum_wage.value)
    );
    assert_eq!(
        annual_reference.annual_gross_minimum_wage.value,
        annual_reference
            .annual_insurance_gross_minimum_wage_cap
            .value
    );
    assert_eq!(
        annual_reference.income_tax_brackets.value.len(),
        production_annual.gelirVergisiDilimleri.len()
    );
    for (actual, expected) in production_annual
        .gelirVergisiDilimleri
        .iter()
        .zip(&annual_reference.income_tax_brackets.value)
    {
        assert_eq!(actual.limit, expected.limit);
        assert_eq!(actual.oran, expected.rate);
    }
    assert!(validate_annual_payroll_parameters(&production_annual).is_ok());

    let period_reference = &reference.period_parameters;
    let production_period = DonemselKurumDegerleri::default();
    assert_eq!(
        production_period.gunlukAsgariUcret,
        Some(period_reference.daily_minimum_wage.value)
    );
    assert_eq!(
        period_reference.monthly_minimum_wage.value,
        period_reference.daily_minimum_wage.value * dec!(30)
    );
    assert_eq!(
        period_reference.monthly_minimum_wage_gv_base.value,
        period_reference.monthly_minimum_wage.value
            - period_reference.monthly_minimum_wage.value
                * period_reference.worker_sgk_rate_percent.value
                / dec!(100)
            - period_reference.monthly_minimum_wage.value
                * period_reference.worker_unemployment_rate_percent.value
                / dec!(100)
    );
    assert_eq!(
        production_period.sgkIsciOraniYuzde,
        Some(period_reference.worker_sgk_rate_percent.value)
    );
    assert_eq!(
        production_period.issizlikIsciOraniYuzde,
        Some(period_reference.worker_unemployment_rate_percent.value)
    );
    assert_eq!(
        production_period.sgkIsverenOraniYuzde,
        Some(period_reference.employer_sgk_rate_percent.value)
    );
    assert_eq!(
        production_period.issizlikIsverenOraniYuzde,
        Some(period_reference.employer_unemployment_rate_percent.value)
    );
    assert_eq!(
        production_period.pekTavanKatsayisi,
        Some(period_reference.pek_ceiling_multiplier.value)
    );
    assert_eq!(
        production_period.damgaVergisiOraniBinde,
        Some(period_reference.stamp_tax_rate_per_mille.value)
    );
    assert_eq!(
        production_period.gunlukYemekIstisnasiGV,
        Some(period_reference.gv_meal_exemption_daily.value)
    );
    assert_eq!(
        production_period.gunlukYemekIstisnasiSGK,
        Some(period_reference.sgk_meal_exemption_daily.value)
    );
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
    excessive_limit.gelirVergisiDilimleri[0].limit = dec!(1000000000000001);
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
