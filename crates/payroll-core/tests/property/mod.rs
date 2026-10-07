use payroll_core::{BordroDonemi, PayrollCalculationRequest, PersonelPuantaj};
use proptest::test_runner::Config as ProptestConfig;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

#[path = "../support/compensation_policy.rs"]
pub(crate) mod compensation_policy_test_support;

mod payment_event_properties;
mod payroll_properties;
mod pek_properties;
mod retro_properties;
mod tax_properties;
mod validation_properties;

pub(crate) fn proptest_config(cases: u32) -> ProptestConfig {
    let mut config = ProptestConfig::with_cases(cases);
    // Keep normal local/PR runs backed by proptest's default regression-file
    // persistence. Measurement jobs set PROPTEST_RNG_SEED and must use only
    // the versioned seed/case budget, not a mutable local regression corpus.
    if std::env::var_os("PROPTEST_RNG_SEED").is_some() {
        config.failure_persistence = None;
    }
    config
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SimpleNormalParameters {
    pub(crate) daily_base: Decimal,
    pub(crate) daily_meal: Decimal,
    pub(crate) daily_meal_sgk_capacity: Decimal,
    pub(crate) daily_meal_gv_capacity: Decimal,
    pub(crate) daily_minimum: Decimal,
    pub(crate) pek_multiplier: Decimal,
    pub(crate) worker_sgk_percent: Decimal,
    pub(crate) worker_unemployment_percent: Decimal,
    pub(crate) employer_sgk_percent: Decimal,
    pub(crate) employer_unemployment_percent: Decimal,
    pub(crate) stamp_rate_binde: Decimal,
}

impl Default for SimpleNormalParameters {
    fn default() -> Self {
        Self {
            daily_base: dec!(2000),
            daily_meal: Decimal::ZERO,
            daily_meal_sgk_capacity: Decimal::ZERO,
            daily_meal_gv_capacity: Decimal::ZERO,
            daily_minimum: dec!(1101),
            pek_multiplier: dec!(9),
            worker_sgk_percent: dec!(14),
            worker_unemployment_percent: dec!(1),
            employer_sgk_percent: dec!(21.75),
            employer_unemployment_percent: dec!(2),
            stamp_rate_binde: dec!(7.59),
        }
    }
}

pub(crate) fn decimal_from_cents(cents: i64) -> Decimal {
    Decimal::new(cents, 2)
}

pub(crate) fn simple_normal_request(
    parameters: SimpleNormalParameters,
) -> PayrollCalculationRequest {
    let mut request = crate::audit_fixture::request();
    request.calculatedAt = "2026-09-10T00:00:00Z".into();
    request.manualIncome = None;
    request.accrual = None;

    let period_id = request.periodId.clone();
    let period = request
        .dataset
        .periods
        .iter_mut()
        .find(|period| period.id == period_id)
        .expect("property period fixture");
    period.taxYear = 2026;
    period.taxMonth = 1;

    let settings = request
        .dataset
        .institutionSettings
        .get_mut(&period_id)
        .expect("property settings fixture");
    settings.gunlukYemekIstisnasiSGK = Some(parameters.daily_meal_sgk_capacity);
    settings.gunlukYemekIstisnasiGV = Some(parameters.daily_meal_gv_capacity);
    settings.gunlukAsgariUcret = Some(parameters.daily_minimum);
    settings.pekTavanKatsayisi = Some(parameters.pek_multiplier);
    settings.sgkIsciOraniYuzde = Some(parameters.worker_sgk_percent);
    settings.issizlikIsciOraniYuzde = Some(parameters.worker_unemployment_percent);
    settings.sgkIsverenOraniYuzde = Some(parameters.employer_sgk_percent);
    settings.issizlikIsverenOraniYuzde = Some(parameters.employer_unemployment_percent);
    settings.damgaVergisiOraniBinde = Some(parameters.stamp_rate_binde);
    settings.sabitSendikaAidati = Some(Decimal::ZERO);
    settings.sabitBesTutar = Some(Decimal::ZERO);
    let current_period = request
        .dataset
        .periods
        .iter()
        .find(|period| period.id == period_id)
        .expect("property period fixture");
    let policy = request
        .dataset
        .compensationPolicyVersions
        .iter_mut()
        .find(|policy| policy.effectiveFrom == current_period.baslangicTarihi)
        .expect("property baseline policy");
    policy.state.gunlukTabanUcret = parameters.daily_base;
    policy.state.gunlukYemek = parameters.daily_meal;
    policy.state.birlestirilmisSosyalYardim = Decimal::ZERO;
    policy.state.gunlukVasitaYol = Decimal::ZERO;
    policy.state.giyimYardimi = Decimal::ZERO;
    policy.state.hizmetZammiBirimi = Decimal::ZERO;
    policy.state.isPrimiYuzde = Some(Decimal::ZERO);
    policy.state.geceCalismaPrimiYuzde = Decimal::ZERO;
    policy.state.geceCalismaTatiliPrimiYuzde = Decimal::ZERO;
    policy.state.ekOdeme = None;
    policy.state.digerGelirVarsayilan = None;
    for group in &mut policy.state.isPrimiGruplari {
        group.oran = Decimal::ZERO;
    }

    request
        .dataset
        .personnel
        .first_mut()
        .expect("property personnel fixture")
        .kesintiler = None;
    request
}

pub(crate) fn base_payment_request() -> PayrollCalculationRequest {
    simple_normal_request(SimpleNormalParameters::default())
}

pub(crate) fn valid_period() -> BordroDonemi {
    BordroDonemi {
        id: "property-period".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Property period".into(),
        taxYear: 2026,
        taxMonth: 1,
    }
}

pub(crate) fn attendance_with_entry(date: &str, code: &str) -> PersonelPuantaj {
    PersonelPuantaj {
        id: "property-attendance".into(),
        personelId: "property-person".into(),
        donemId: "property-period".into(),
        gunler: HashMap::from([(date.into(), code.into())]),
    }
}
