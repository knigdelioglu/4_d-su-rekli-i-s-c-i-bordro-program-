//! Independent boundary checks for the reusable A2 tax/PEK primitives.
use payroll_core::{
    calculate_aylik_asgari_ucret_gv_matrahi, calculate_gv_matrah,
    calculate_monthly_stamp_tax_state, calculate_total_tax_for_cumulative_matrah_with_brackets,
    validate_period, validate_tax_month_overlap, BordroDonemi, TaxBracket,
};
use rust_decimal_macros::dec;

#[path = "support/independent_audit.rs"]
mod audit_support;

fn period(tax_year: i32, tax_month: i32) -> BordroDonemi {
    BordroDonemi {
        id: "2026-12".into(),
        yil: 2026,
        ay: 12,
        baslangicTarihi: "2026-12-15".into(),
        bitisTarihi: "2027-01-14".into(),
        donemAdi: "Aralık 2026".into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

#[test]
fn december_period_accepts_either_overlapping_tax_month_and_rejects_unrelated_month() {
    let december = period(2026, 12);
    assert!(validate_period(&december).is_ok());
    assert!(validate_tax_month_overlap(&december).is_ok());
    let january = period(2027, 1);
    assert!(validate_tax_month_overlap(&january).is_ok());
    let unrelated = period(2027, 6);
    assert!(validate_tax_month_overlap(&unrelated).is_err());
}

#[test]
fn period_validation_rejects_bad_metadata_and_non_15_14_shape() {
    let mut bad = period(2026, 12);
    bad.ay = 11;
    assert!(validate_period(&bad).is_err());
    let mut malformed = period(2026, 12);
    malformed.baslangicTarihi = "2026-12-16".into();
    assert!(validate_period(&malformed).is_err());
}

#[test]
fn payment_month_snapshot_uses_full_capacity_without_attendance() {
    let request = audit_support::dated_request(2026, 12);
    let period = request.dataset.periods[0].clone();
    let settings = request.dataset.institutionSettings.get(&period.id).unwrap();
    let snapshot = payroll_core::payroll_engine::resolve_statutory_snapshot_for_payment_month(
        &period, settings,
    )
    .expect("supplementary payment-month snapshot should resolve");
    // A payment event without attendance receives normalized 30-day statutory
    // capacity; the separate attendance meal entitlement remains zero.
    assert_eq!(snapshot.sgkPrimGunSayisi, 30);
    assert_eq!(snapshot.pekAltSinir, dec!(33030));
    assert_eq!(
        snapshot
            .segments
            .iter()
            .map(|s| s.fiiliYemekGunu)
            .sum::<i32>(),
        0
    );
}

#[test]
fn finalization_request_rejects_missing_period_before_resolving_payment_event() {
    let mut request = audit_support::request();
    request.periodId = "missing-period".into();
    let error = payroll_core::payroll_engine::validate_payroll_finalization_request(&request)
        .expect_err("finalization must fail closed when period is absent");
    assert!(error.to_string().contains("Dönem bulunamadı"));
}

#[test]
fn calculated_payment_event_order_uses_payment_date_and_sequence() {
    let request = audit_support::dated_request(2026, 1);
    let payroll = payroll_core::calculate_payroll_checked(&request)
        .expect("synthetic full-month request should calculate");
    let order = payroll_core::payroll_engine::accrual_order_for_payroll(&request.dataset, &payroll)
        .expect("calculated event must have an order");
    // January 2026 payment is in the same tax month as the January period;
    // sequence and identity remain event metadata, independent of totals.
    let debug = format!("{order:?}");
    assert!(
        debug.contains("2026-01-31"),
        "unexpected payment order: {debug}"
    );
    assert!(
        debug.contains("sequence: 0"),
        "unexpected sequence: {debug}"
    );
    assert!(
        debug.contains(&payroll.id),
        "unexpected accrual identity: {debug}"
    );
}

#[test]
fn supplementary_accrual_rejects_negative_manual_gross_before_calculation() {
    let mut request = audit_support::supplementary(
        audit_support::dated_request(2026, 2),
        payroll_core::AccrualType::TEDIYE,
    );
    request.accrual.as_mut().unwrap().paymentDate = "2026-02-20".into();
    request.accrual.as_mut().unwrap().grossAmount = Some(dec!(-1));
    let error = payroll_core::calculate_payroll_checked(&request)
        .expect_err("negative supplementary gross must be rejected");
    assert!(
        error.to_string().contains("negatif"),
        "unexpected error: {error}"
    );
}

#[test]
fn supplementary_accrual_rejects_payment_date_outside_service_period() {
    let mut request = audit_support::supplementary(
        audit_support::dated_request(2026, 2),
        payroll_core::AccrualType::TIS_IKRAMIYE,
    );
    request.accrual.as_mut().unwrap().paymentDate = "2028-01-01".into();
    let error = payroll_core::calculate_payroll_checked(&request)
        .expect_err("payment event outside period must be rejected");
    assert!(
        error.to_string().contains("ödeme"),
        "unexpected error: {error}"
    );
}

#[test]
fn supplementary_accrual_requires_explicit_payment_date_and_gross_amount() {
    let mut missing_date = audit_support::supplementary(
        audit_support::dated_request(2026, 2),
        payroll_core::AccrualType::TEDIYE,
    );
    missing_date.accrual.as_mut().unwrap().paymentDate.clear();
    let error = payroll_core::calculate_payroll_checked(&missing_date).expect_err("date required");
    assert!(error.to_string().contains("tarihi açıkça belirtilmelidir"));

    let mut missing_gross = audit_support::supplementary(
        audit_support::dated_request(2026, 2),
        payroll_core::AccrualType::TIS_IKRAMIYE,
    );
    missing_gross.accrual.as_mut().unwrap().paymentDate = "2026-02-20".into();
    missing_gross.accrual.as_mut().unwrap().grossAmount = None;
    let error =
        payroll_core::calculate_payroll_checked(&missing_gross).expect_err("gross required");
    assert!(error.to_string().contains("brüt tutar zorunludur"));
}

#[test]
fn retro_without_known_batch_fails_closed_and_duplicate_normal_is_rejected() {
    let mut retro = audit_support::supplementary(
        audit_support::dated_request(2026, 2),
        payroll_core::AccrualType::RETRO_ADJUSTMENT,
    );
    retro.accrual.as_mut().unwrap().paymentDate = "2026-02-20".into();
    retro.accrual.as_mut().unwrap().grossAmount = None;
    let error = payroll_core::calculate_payroll_checked(&retro).expect_err("unknown batch");
    assert!(error
        .to_string()
        .contains("Retro adjustment batch bulunamadı"));

    let mut duplicate = audit_support::dated_request(2026, 2);
    let first = payroll_core::calculate_payroll_checked(&duplicate).expect("normal fixture");
    duplicate.dataset.payrolls.push(first.clone());
    duplicate.dataset.payrolls.push(first);
    let error = payroll_core::calculate_payroll_checked(&duplicate).expect_err("duplicate normal");
    assert!(error.to_string().contains("birden fazla NORMAL"));
}

#[test]
fn explicit_empty_accrual_identity_is_rejected_before_financial_resolution() {
    let mut request = audit_support::supplementary(
        audit_support::dated_request(2026, 2),
        payroll_core::AccrualType::TEDIYE,
    );
    request.accrual.as_mut().unwrap().accrualId = "   ".into();
    request.accrual.as_mut().unwrap().paymentDate = "2026-02-20".into();
    let error = payroll_core::calculate_payroll_checked(&request)
        .expect_err("empty accrual identity must be rejected");
    assert!(error.to_string().contains("Tahakkuk kimliği boş olamaz"));
}

#[test]
fn tax_oracle_handles_zero_and_open_last_bracket() {
    let brackets = vec![
        TaxBracket {
            limit: dec!(1000),
            oran: dec!(0.15),
        },
        TaxBracket {
            limit: dec!(2000),
            oran: dec!(0.20),
        },
    ];
    assert_eq!(
        calculate_total_tax_for_cumulative_matrah_with_brackets(dec!(0), &brackets),
        dec!(0)
    );
    // 1,000*.15 + 1,000*.20 + 500*.20
    assert_eq!(
        calculate_total_tax_for_cumulative_matrah_with_brackets(dec!(2500), &brackets),
        dec!(450)
    );
}

#[test]
fn minimum_wage_reference_rounds_each_worker_premium_before_subtraction() {
    // 30 * 1,101.00 = 33,030; SGK 14% = 4,624.20 and unemployment 1% = 330.30.
    assert_eq!(
        calculate_aylik_asgari_ucret_gv_matrahi(dec!(1101), dec!(0.14), dec!(0.01)),
        dec!(28075.50)
    );
}

#[test]
fn stamp_exemption_is_consumed_in_payment_order_and_never_goes_negative() {
    let first =
        calculate_monthly_stamp_tax_state(dec!(20_000), dec!(33_030), dec!(0.00759), dec!(0));
    assert_eq!(first.brutDamgaVergisi, dec!(151.80));
    assert_eq!(first.uygulananDamgaIstisnasi, dec!(151.80));
    let second = calculate_monthly_stamp_tax_state(
        dec!(50_000),
        dec!(33_030),
        dec!(0.00759),
        first.uygulananDamgaIstisnasi,
    );
    assert_eq!(second.uygulananDamgaIstisnasi, dec!(98.90));
    assert_eq!(second.kalanDamgaIstisnasi, dec!(0));
    assert_eq!(second.kesilenDamgaVergisi, dec!(280.60));
    assert_eq!(calculate_gv_matrah(dec!(100), dec!(70), dec!(40)), dec!(0));
}
