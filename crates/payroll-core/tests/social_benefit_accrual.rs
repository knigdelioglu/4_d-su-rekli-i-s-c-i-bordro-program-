#[path = "support/independent_audit.rs"]
#[allow(dead_code)]
mod audit_support;

use chrono::{Duration, NaiveDate};
use payroll_core::*;
use rust_decimal_macros::dec;

fn policy(
    method: BenefitPaymentMethod,
    eligible_day_types: Vec<BenefitDayType>,
    denominator: BenefitProrationDenominator,
) -> BenefitAccrualPolicy {
    BenefitAccrualPolicy {
        method,
        eligibleDayTypes: eligible_day_types,
        denominator,
    }
}

fn set_combined_aid(request: &mut PayrollCalculationRequest, rule: BenefitAccrualPolicy) {
    let state = &mut request.dataset.compensationPolicyVersions[0].state;
    state.birlestirilmisSosyalYardim = dec!(3000);
    state.socialBenefitAccrualPolicies = Some(SocialBenefitAccrualPolicies {
        birlestirilmisSosyalYardim: Some(rule),
        ..Default::default()
    });
}

fn set_worked_days(request: &mut PayrollCalculationRequest, worked_days: i64) {
    let attendance = &mut request.dataset.attendances[0];
    let start = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    for offset in 0..31 {
        let date = start + Duration::days(offset);
        attendance.gunler.insert(
            date.to_string(),
            if offset < worked_days { "Ç" } else { "Üİ" }.into(),
        );
    }
}

#[test]
fn combined_aid_supports_all_four_methods_and_zero_eligible_days() {
    let expected = [
        (BenefitPaymentMethod::Full, dec!(3000), dec!(3000)),
        (BenefitPaymentMethod::FullIfEligibleDay, dec!(3000), dec!(0)),
        (BenefitPaymentMethod::Proportional, dec!(1935.48), dec!(0)),
        (BenefitPaymentMethod::NoPayment, dec!(0), dec!(0)),
    ];

    for (method, with_eligible_days, no_eligible_days) in expected {
        for (eligible_days, expected_amount) in [(20, with_eligible_days), (0, no_eligible_days)] {
            let mut request = audit_support::request();
            set_worked_days(&mut request, eligible_days);
            set_combined_aid(
                &mut request,
                policy(
                    method,
                    vec![BenefitDayType::Worked],
                    BenefitProrationDenominator::CalendarDays,
                ),
            );
            let payroll = calculate_payroll_checked(&request).expect("benefit payroll");
            assert_eq!(
                payroll.gelirler.birlestirilmisSosyalYardim,
                Some(expected_amount),
                "method={method:?}, eligible_days={eligible_days}"
            );
            let gross = [
                payroll.gelirler.tabanBrutAylik,
                payroll.gelirler.tediye,
                payroll.gelirler.tisIkramiyesi,
                payroll.gelirler.ekOdeme,
                payroll.gelirler.yemek,
                payroll.gelirler.birlestirilmisSosyalYardim,
                payroll.gelirler.vasitaYol,
                payroll.gelirler.giyimYardimi,
                payroll.gelirler.isPrimi,
                payroll.gelirler.geceCalismasiUcreti,
                payroll.gelirler.geceCalismasiTatiliUcreti,
                payroll.gelirler.hizmetZammi,
                payroll.gelirler.digerGelir,
            ]
            .into_iter()
            .flatten()
            .sum::<rust_decimal::Decimal>();
            assert_eq!(payroll.gelirToplam, gross, "gross for {method:?}");
            assert_eq!(
                payroll.netOdeme,
                payroll.gelirToplam - payroll.kesintiToplam
            );
            let gv = payroll.gvDetay.as_ref().expect("GV breakdown");
            assert_eq!(
                payroll.kesintiler.gelirVergisi,
                Some(gv.kesilenGelirVergisi)
            );
            let stamp = payroll.damgaDetay.as_ref().expect("stamp breakdown");
            assert_eq!(
                payroll.kesintiler.damgaVergisi,
                Some(stamp.kesilenDamgaVergisi)
            );
            assert!(payroll.pekDetay.is_some(), "PEK breakdown for {method:?}");
            assert_eq!(
                validate_payroll_calculation_metadata_with_context(
                    &payroll,
                    &request.dataset.periods[0],
                    &[],
                    &[]
                )
                .unwrap(),
                PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
            );
        }
    }
}

#[test]
fn policy_eligibility_counts_paid_and_unpaid_reports_and_leave_once() {
    let mut request = audit_support::request();
    let attendance = &mut request.dataset.attendances[0];
    let cases = [
        ("2026-01-16", "T"),
        ("2026-01-17", "R"),
        ("2026-01-18", "R"),
        ("2026-01-19", "Üİ"),
        ("2026-01-20", "D"),
    ];
    for (date, code) in cases {
        attendance.gunler.insert(date.into(), code.into());
    }
    request.dataset.sickLeaveRecords.push(SickLeaveRecord {
        id: "paid-day-only".into(),
        personnelId: "audit".into(),
        startDate: "2026-01-17".into(),
        endDate: "2026-01-17".into(),
        createdAt: None,
        updatedAt: None,
    });
    set_combined_aid(
        &mut request,
        policy(
            BenefitPaymentMethod::Proportional,
            vec![
                BenefitDayType::Worked,
                BenefitDayType::PaidRestHolidayOrLeave,
                BenefitDayType::PaidSickLeave,
            ],
            BenefitProrationDenominator::CalendarDays,
        ),
    );

    let payroll = calculate_payroll_checked(&request).expect("mixed attendance payroll");
    // 26 worked days, one paid rest day, and one paid report day are eligible.
    // Unpaid report, unpaid leave, and outside-employment each occupy one
    // distinct calendar day and cannot be counted again.
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(2709.68))
    );
    assert_eq!(payroll.puantajOzeti.r, 2);
    assert_eq!(payroll.puantajOzeti.ui, 1);
    assert_eq!(payroll.puantajOzeti.d, 1);
    assert_eq!(
        payroll.statutorySnapshot.as_ref().unwrap().sgkPrimGunSayisi,
        28
    );
}

#[test]
fn proportional_denominator_is_explicit_and_policy_change_replays_from_saved_segments() {
    let mut request = audit_support::request();
    set_combined_aid(
        &mut request,
        policy(
            BenefitPaymentMethod::Proportional,
            vec![BenefitDayType::Worked],
            BenefitProrationDenominator::ThirtyDayMonth,
        ),
    );
    set_worked_days(&mut request, 20);
    assert_eq!(
        calculate_payroll_checked(&request)
            .unwrap()
            .gelirler
            .birlestirilmisSosyalYardim,
        Some(dec!(2000))
    );

    let mut changed = request.dataset.compensationPolicyVersions[0].clone();
    changed.id = "aid-policy-mid-period".into();
    changed.effectiveFrom = "2026-01-31".into();
    changed.state.birlestirilmisSosyalYardim = dec!(6000);
    changed.supersedesVersionId = None;
    request.dataset.compensationPolicyVersions.push(changed);
    let payroll = calculate_payroll_checked(&request).expect("split-policy payroll");
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(2400))
    );
    let period = request.dataset.periods[0].clone();
    assert_eq!(
        validate_payroll_calculation_metadata_with_context(&payroll, &period, &[], &[]).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );

    // Later live policy edits must not change the calculated row's frozen
    // compensation segments or its canonical financial replay.
    request.dataset.compensationPolicyVersions[0]
        .state
        .birlestirilmisSosyalYardim = dec!(9999);
    request.dataset.compensationPolicyVersions[1]
        .state
        .birlestirilmisSosyalYardim = dec!(9999);
    assert_eq!(
        validate_payroll_calculation_metadata_with_context(&payroll, &period, &[], &[]).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );
}

#[test]
fn old_compensation_policy_json_without_accrual_rules_remains_compatible() {
    let request = audit_support::request();
    let mut value = serde_json::to_value(&request.dataset.compensationPolicyVersions[0].state)
        .expect("serialize old-compatible state");
    value
        .as_object_mut()
        .unwrap()
        .remove("socialBenefitAccrualPolicies");
    let restored: CompensationPolicyState =
        serde_json::from_value(value).expect("missing optional rules should migrate as legacy");
    assert_eq!(restored.socialBenefitAccrualPolicies, None);
    let mut configured = restored.clone();
    configured.socialBenefitAccrualPolicies = Some(SocialBenefitAccrualPolicies {
        birlestirilmisSosyalYardim: Some(policy(
            BenefitPaymentMethod::Proportional,
            vec![BenefitDayType::Worked, BenefitDayType::PaidSickLeave],
            BenefitProrationDenominator::ThirtyDayMonth,
        )),
        ..Default::default()
    });
    let backup_json = serde_json::to_string(&configured).expect("configured backup JSON");
    assert_eq!(
        serde_json::from_str::<CompensationPolicyState>(&backup_json).unwrap(),
        configured
    );

    let payroll = calculate_payroll_checked(&request).expect("legacy policy payroll");
    let mut v2 = payroll.clone();
    v2.calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .calculationVersion = PAYROLL_COMPATIBLE_CALCULATION_VERSION.into();
    assert_eq!(
        validate_payroll_calculation_metadata_with_context(
            &v2,
            &request.dataset.periods[0],
            &[],
            &[]
        )
        .unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );
}

#[test]
fn oks_percentage_deduction_keeps_full_lira_flooring() {
    let mut request = audit_support::request();
    request.dataset.compensationPolicyVersions[0]
        .state
        .gunlukTabanUcret = dec!(2000.01);
    request.dataset.personnel[0]
        .kesintiler
        .as_mut()
        .unwrap()
        .oksOraniYuzde = Some(dec!(3));

    let payroll = calculate_payroll_checked(&request).expect("fractional OKS basis payroll");
    let pek = payroll.pekDetay.as_ref().unwrap().primMatrahi;
    let expected = (pek * dec!(3) / dec!(100)).floor();
    assert_eq!(payroll.kesintiler.bes, Some(expected));
    assert_eq!(expected.fract(), dec!(0));
}
