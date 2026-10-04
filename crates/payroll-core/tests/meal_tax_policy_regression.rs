use payroll_core::*;
use rust_decimal_macros::dec;

fn request() -> PayrollCalculationRequest {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("golden/2026/G020-social-aid.json")).unwrap();
    let mut request: PayrollCalculationRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();
    let settings = request
        .dataset
        .institutionSettings
        .get_mut(&request.periodId)
        .unwrap();
    settings.gunlukYemek = dec!(350);
    settings.gunlukYemekIstisnasiGV = Some(dec!(300));
    settings.gunlukYemekIstisnasiSGK = Some(dec!(300));
    request
}

#[test]
fn selected_policy_exempts_all_meal_from_gv_but_none_from_stamp_without_changing_sgk() {
    let mut request = request();
    let on = calculate_payroll(&request).unwrap();
    assert_eq!(on.gelirler.yemek, Some(dec!(10850)));
    let gv_base = on.gelirToplam
        - dec!(10850)
        - on.kesintiler.isciSgkPrimi.unwrap()
        - on.kesintiler.isciIssizlikPrimi.unwrap()
        - on.kesintiler.sendikaAidati.unwrap();
    assert_eq!(on.gvDetay.as_ref().unwrap().cariGvMatrahi, gv_base);
    assert_eq!(
        on.damgaDetay.as_ref().unwrap().brutDamgaVergisi,
        (on.gelirToplam * dec!(0.00759)).round_dp(2)
    );

    request
        .dataset
        .institutionSettings
        .get_mut(&request.periodId)
        .unwrap()
        .yemekTamGvIstisnaDamgaTabi = false;
    let off = calculate_payroll(&request).unwrap();
    assert_eq!(
        off.gvDetay.as_ref().unwrap().cariGvMatrahi,
        gv_base + dec!(1550)
    );
    assert_eq!(
        off.damgaDetay.as_ref().unwrap().brutDamgaVergisi,
        ((off.gelirToplam - dec!(9300)) * dec!(0.00759)).round_dp(2)
    );
    assert_eq!(
        serde_json::to_value(&on.pekDetay).unwrap(),
        serde_json::to_value(&off.pekDetay).unwrap()
    );
    assert_eq!(on.gelirToplam, off.gelirToplam);
    assert_eq!(on.netOdeme, on.gelirToplam - on.kesintiToplam);
    assert_eq!(off.netOdeme, off.gelirToplam - off.kesintiToplam);
}

#[test]
fn missing_legacy_setting_defaults_on_and_false_survives_json_round_trip() {
    let mut value = serde_json::to_value(request()).unwrap();
    let id = value["periodId"].as_str().unwrap().to_owned();
    value["dataset"]["institutionSettings"][&id]
        .as_object_mut()
        .unwrap()
        .remove("yemekTamGvIstisnaDamgaTabi");
    let legacy: PayrollCalculationRequest = serde_json::from_value(value.clone()).unwrap();
    assert!(legacy.dataset.institutionSettings[&id].yemekTamGvIstisnaDamgaTabi);
    value["dataset"]["institutionSettings"][&id]["yemekTamGvIstisnaDamgaTabi"] = false.into();
    let restored: PayrollCalculationRequest = serde_json::from_value(value.clone()).unwrap();
    let restored: PayrollCalculationRequest =
        serde_json::from_str(&serde_json::to_string(&restored).unwrap()).unwrap();
    assert!(!restored.dataset.institutionSettings[&id].yemekTamGvIstisnaDamgaTabi);
    for invalid in [serde_json::Value::Null, "true".into(), 1.into()] {
        value["dataset"]["institutionSettings"][&id]["yemekTamGvIstisnaDamgaTabi"] = invalid;
        assert!(serde_json::from_value::<PayrollCalculationRequest>(value.clone()).is_err());
    }
}

#[test]
fn policy_does_not_copy_meal_exemption_into_a_supplementary_accrual() {
    let mut request = request();
    request.accrual.as_mut().unwrap().accrualType = AccrualType::SUPPLEMENTAL;
    request.accrual.as_mut().unwrap().grossAmount = Some(dec!(10000));
    let on = calculate_payroll(&request).unwrap();
    request
        .dataset
        .institutionSettings
        .get_mut(&request.periodId)
        .unwrap()
        .yemekTamGvIstisnaDamgaTabi = false;
    let off = calculate_payroll(&request).unwrap();
    assert_eq!(on.netOdeme, off.netOdeme);
    assert_eq!(on.gelirler.yemek, None);
}

#[test]
fn provided_payroll_gv_and_stamp_figures_match_the_selected_policy() {
    let brackets = default_gelir_vergisi_dilimleri_2026();
    let tax = calculate_canonical_tax_state(&CanonicalTaxCalculationInput {
        income_total: dec!(91792.97),
        retro_income_tax_exempt: dec!(0),
        retro_stamp_tax_exempt: dec!(0),
        normal_meal_tax_exemption: dec!(3952.34),
        normal_meal_stamp_tax_exemption: dec!(0),
        worker_sgk: dec!(12305.01),
        worker_unemployment: dec!(878.92),
        union_deduction: dec!(1700.57),
        insurance_wage_base: dec!(0),
        birth_military_gv_input: dec!(0),
        life_insurance_premium: dec!(0),
        health_insurance_premium: dec!(0),
        insurance_annual_cap: dec!(396360),
        insurance_used_before: dec!(0),
        previous_cumulative_gv: dec!(600000),
        monthly_asgari_gv_matrahi: dec!(28075.50),
        previous_cumulative_asgari_gv: dec!(224604),
        same_month_gv_used: dec!(0),
        tax_brackets: &brackets,
        monthly_minimum_gross: dec!(33030),
        stamp_rate: dec!(0.00759),
        same_month_stamp_used: dec!(0),
    });
    assert_eq!(tax.gv_base, dec!(72956.13));
    assert_eq!(tax.gv.brutGelirVergisi, dec!(19698.16));
    assert_eq!(tax.gv.kesilenGelirVergisi, dec!(14083.06));
    assert_eq!(tax.stamp.brutDamgaVergisi, dec!(696.71));
    assert_eq!(tax.stamp.kesilenDamgaVergisi, dec!(446.01));
}
