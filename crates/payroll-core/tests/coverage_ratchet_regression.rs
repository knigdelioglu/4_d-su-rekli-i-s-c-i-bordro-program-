//! Focused financial regressions for branches that are easy to miss in the
//! broad property corpus. Expected values are calculated from the stated
//! rates and bounds in this file, independently of the implementation.

use payroll_core::{
    calculate_gelir_vergisi_2026, calculate_gv_hesap_detayi_with_brackets, calculate_meal_income,
    calculate_paid_sick_dates_from_records, calculate_prime_esas_kazanc,
    canonical_sgk_income_components, validate_kurum_degerleri_for_payroll, validate_pek_bounds,
    validate_period_institution_settings_for_payroll, BordroDonemi, DonemselKurumDegerleri,
    GelirKalemleri, PayrollPeriodInstitutionSettings, PuantajOzeti, SickLeaveRecord, TaxBracket,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[test]
fn progressive_tax_crossing_bracket_matches_piecewise_oracle() {
    let brackets = vec![
        TaxBracket {
            limit: dec!(10000),
            oran: dec!(0.15),
        },
        TaxBracket {
            limit: dec!(25000),
            oran: dec!(0.20),
        },
        TaxBracket {
            limit: dec!(100000),
            oran: dec!(0.27),
        },
    ];
    // Previous cumulative 9,000 and current 20,000: the incremental tax is
    // 1,000 at 15%, 15,000 at 20%, and 4,000 at 27%, less prior 15% tax.
    let detail = calculate_gv_hesap_detayi_with_brackets(
        dec!(20000),
        dec!(9000),
        Decimal::ZERO,
        Decimal::ZERO,
        &brackets,
    );
    assert_eq!(detail.brutGelirVergisi, dec!(4230));
    assert_eq!(detail.kesilenGelirVergisi, dec!(4230));
    assert_eq!(detail.yeniKumulatifGvMatrahi, dec!(29000));
}

#[test]
fn period_settings_validation_covers_statutory_boundary_errors() {
    let valid = || PayrollPeriodInstitutionSettings {
        donemId: "period-validation".into(),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        sendikaAidatiYuzde: Some(dec!(1)),
        besOraniYuzde: Some(dec!(3)),
        gunlukYemekIstisnasiSGK: Some(dec!(100)),
        gunlukYemekIstisnasiGV: Some(dec!(100)),
        gunlukAsgariUcret: Some(dec!(1000)),
        pekTavanKatsayisi: Some(dec!(7)),
        sgkIsverenOraniYuzde: Some(dec!(20.5)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        damgaVergisiOraniBinde: Some(dec!(7.59)),
        ..Default::default()
    };
    assert!(validate_period_institution_settings_for_payroll(&valid()).is_ok());
    let mut missing = valid();
    missing.sgkIsciOraniYuzde = None;
    assert!(validate_period_institution_settings_for_payroll(&missing).is_err());
    let mut negative = valid();
    negative.gunlukAsgariUcret = Some(dec!(-1));
    assert!(validate_period_institution_settings_for_payroll(&negative).is_err());
    let mut over = valid();
    over.sgkIsciOraniYuzde = Some(dec!(101));
    assert!(validate_period_institution_settings_for_payroll(&over).is_err());
    let mut stamp = valid();
    stamp.damgaVergisiOraniBinde = Some(dec!(1001));
    assert!(validate_period_institution_settings_for_payroll(&stamp).is_err());
    let mut minimum = valid();
    minimum.gunlukAsgariUcret = Some(Decimal::ZERO);
    assert!(validate_period_institution_settings_for_payroll(&minimum).is_err());
    let mut cap = valid();
    cap.pekTavanKatsayisi = Some(dec!(0.5));
    assert!(validate_period_institution_settings_for_payroll(&cap).is_err());
    let mut fixed = valid();
    fixed.sabitBesTutar = Some(dec!(-1));
    assert!(validate_period_institution_settings_for_payroll(&fixed).is_err());
}

#[test]
fn pek_uses_daily_lower_and_upper_bounds_and_meal_exemption() {
    let settings = DonemselKurumDegerleri {
        gunlukAsgariUcret: Some(dec!(1000)),
        pekTavanKatsayisi: Some(dec!(5)),
        gunlukYemekIstisnasiSGK: Some(dec!(300)),
        ..Default::default()
    };
    let income = GelirKalemleri {
        tabanBrutAylik: Some(dec!(10000)),
        yemek: Some(dec!(1000)),
        ..Default::default()
    };
    let attendance = PuantajOzeti {
        c: 20,
        ..Default::default()
    };
    let (detail, carry) =
        calculate_prime_esas_kazanc(&income, Some(&attendance), Some(&settings), &[]);

    // Independent: meal PEK is max(1,000 - 20*300, 0) = 0; raw PEK 10,000.
    // Bounds are 20,000 and 100,000, so the lower bound completes PEK.
    assert_eq!(detail.hamPek, dec!(10000));
    assert_eq!(detail.pekAltSinir, dec!(20000));
    assert_eq!(detail.pekUstSinir, dec!(100000));
    assert_eq!(detail.altSinirTamamlamaFarki, dec!(10000));
    assert_eq!(detail.finalPek, dec!(20000));
    assert!(carry.is_empty());
}

#[test]
fn income_buckets_keep_non_wage_premiums_out_of_wage_bucket() {
    let income = GelirKalemleri {
        tabanBrutAylik: Some(dec!(10000)),
        isPrimi: Some(dec!(1250)),
        yemek: Some(dec!(700)),
        ..Default::default()
    };
    let (wage, non_wage) = canonical_sgk_income_components(&income, dec!(100));
    // Meal argument is already the taxable-to-SGK portion.
    assert_eq!(wage, dec!(10100));
    assert_eq!(non_wage, dec!(1250));
}

#[test]
fn meal_payment_rounds_once_at_half_kurus() {
    // 3 * 10.005 = 30.015, rounded away from zero to 30.02 under Law 5083.
    assert_eq!(calculate_meal_income(3, dec!(10.005)), dec!(30.02));
}

fn sick_period() -> BordroDonemi {
    BordroDonemi {
        id: "sick-coverage".into(),
        yil: 2027,
        ay: 1,
        baslangicTarihi: "2027-01-15".into(),
        bitisTarihi: "2027-02-14".into(),
        donemAdi: "Ocak".into(),
        taxYear: 2027,
        taxMonth: 1,
    }
}

fn sick(id: &str, start: &str, end: &str) -> SickLeaveRecord {
    SickLeaveRecord {
        id: id.into(),
        personnelId: "p1".into(),
        startDate: start.into(),
        endDate: end.into(),
        createdAt: None,
        updatedAt: None,
    }
}

#[test]
fn paid_sick_rule_pays_first_two_days_for_first_five_episodes_only() {
    let mut records = Vec::new();
    for index in 0..6 {
        let day = 15 + index * 3;
        records.push(sick(
            &format!("s{index}"),
            &format!("2027-01-{day:02}"),
            &format!("2027-01-{:02}", day + 1),
        ));
    }
    let dates = calculate_paid_sick_dates_from_records(&records, &sick_period()).unwrap();
    assert_eq!(dates.len(), 10);
    assert!(!dates.iter().any(|date| date.to_string() == "2027-01-30"));
}

#[test]
fn paid_sick_rejects_invalid_period_and_record_dates() {
    let mut period = sick_period();
    period.bitisTarihi = "not-a-date".into();
    assert!(calculate_paid_sick_dates_from_records(&[], &period).is_err());
    let bad = sick("bad", "2027-02-31", "2027-03-01");
    assert!(calculate_paid_sick_dates_from_records(&[bad], &sick_period()).is_err());
}

#[test]
fn legacy_gv_wrapper_uses_default_progressive_brackets() {
    // Independent 2026 brackets: 15% on the first 190,000 and 20% on the
    // next 210,000. The current slice starts after 180,000 cumulative.
    let actual = calculate_gelir_vergisi_2026(dec!(30000), dec!(180000));
    let expected = dec!(190000) * dec!(0.15) + dec!(20000) * dec!(0.20) - dec!(180000) * dec!(0.15);
    assert_eq!(actual, expected);
}

#[test]
fn pek_bounds_reject_negative_and_inverted_limits() {
    assert!(validate_pek_bounds(dec!(-1), dec!(10)).is_err());
    assert!(validate_pek_bounds(dec!(10), dec!(-1)).is_err());
    assert!(validate_pek_bounds(dec!(20), dec!(10)).is_err());
    assert!(validate_pek_bounds(dec!(10), dec!(20)).is_ok());
}

#[test]
fn institution_validation_rejects_financially_invalid_parameters() {
    let mut base = DonemselKurumDegerleri {
        donemId: "validation".into(),
        gunlukTabanUcret: dec!(100),
        ..Default::default()
    };
    let mut zero_base = base.clone();
    zero_base.gunlukTabanUcret = Decimal::ZERO;
    assert!(validate_kurum_degerleri_for_payroll(&zero_base).is_err());

    base.gunlukYemek = dec!(-1);
    assert!(validate_kurum_degerleri_for_payroll(&base).is_err());

    let mut rate = DonemselKurumDegerleri {
        donemId: "validation".into(),
        gunlukTabanUcret: dec!(100),
        sgkIsciOraniYuzde: Some(dec!(-1)),
        ..Default::default()
    };
    assert!(validate_kurum_degerleri_for_payroll(&rate).is_err());
    rate.sgkIsciOraniYuzde = Some(dec!(101));
    assert!(validate_kurum_degerleri_for_payroll(&rate).is_err());

    let invalid_stamp = DonemselKurumDegerleri {
        donemId: "validation".into(),
        gunlukTabanUcret: dec!(100),
        damgaVergisiOraniBinde: Some(dec!(1001)),
        ..Default::default()
    };
    assert!(validate_kurum_degerleri_for_payroll(&invalid_stamp).is_err());
}
