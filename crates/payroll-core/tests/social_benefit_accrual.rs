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
        allowMonthlyAmountExcess: None,
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
fn proportional_preview_covers_calendar_periods_and_explicit_thirty_day_cap() {
    for (period_days, denominator, eligible_days, expected_capped, expected_uncapped) in [
        (
            28,
            BenefitProrationDenominator::CalendarDays,
            28,
            dec!(3000),
            dec!(3000),
        ),
        (
            29,
            BenefitProrationDenominator::CalendarDays,
            29,
            dec!(3000),
            dec!(3000),
        ),
        (
            30,
            BenefitProrationDenominator::CalendarDays,
            30,
            dec!(3000),
            dec!(3000),
        ),
        (
            31,
            BenefitProrationDenominator::CalendarDays,
            31,
            dec!(3000),
            dec!(3000),
        ),
        (
            28,
            BenefitProrationDenominator::ThirtyDayMonth,
            28,
            dec!(2800),
            dec!(2800),
        ),
        (
            29,
            BenefitProrationDenominator::ThirtyDayMonth,
            29,
            dec!(2900),
            dec!(2900),
        ),
        (
            30,
            BenefitProrationDenominator::ThirtyDayMonth,
            30,
            dec!(3000),
            dec!(3000),
        ),
        (
            31,
            BenefitProrationDenominator::ThirtyDayMonth,
            31,
            dec!(3000),
            dec!(3100),
        ),
    ] {
        let capped = policy(
            BenefitPaymentMethod::Proportional,
            vec![BenefitDayType::Worked],
            denominator,
        );
        let mut uncapped = capped.clone();
        uncapped.allowMonthlyAmountExcess = Some(true);
        let mut capped = capped;
        capped.allowMonthlyAmountExcess = Some(false);
        assert_eq!(
            calculate_social_benefit_preview(dec!(3000), period_days, eligible_days, Some(&capped))
                .unwrap(),
            expected_capped,
            "capped {period_days} day period with {denominator:?}"
        );
        assert_eq!(
            calculate_social_benefit_preview(
                dec!(3000),
                period_days,
                eligible_days,
                Some(&uncapped)
            )
            .unwrap(),
            expected_uncapped,
            "uncapped {period_days} day period with {denominator:?}"
        );
    }

    let no_days = policy(
        BenefitPaymentMethod::Proportional,
        vec![BenefitDayType::Worked],
        BenefitProrationDenominator::ThirtyDayMonth,
    );
    assert_eq!(
        calculate_social_benefit_preview(dec!(3000), 31, 0, Some(&no_days)).unwrap(),
        dec!(0)
    );
    assert!(calculate_social_benefit_preview(dec!(3000), 0, 0, Some(&no_days)).is_err());
    assert!(calculate_social_benefit_preview(dec!(3000), 30, 31, Some(&no_days)).is_err());
}

#[test]
fn explicit_cap_and_legacy_uncapped_policy_apply_to_each_mid_period_slice_once() {
    let mut request = audit_support::request();
    set_worked_days(&mut request, 31);
    set_combined_aid(
        &mut request,
        policy(
            BenefitPaymentMethod::Proportional,
            vec![BenefitDayType::Worked],
            BenefitProrationDenominator::ThirtyDayMonth,
        ),
    );
    let first = &mut request.dataset.compensationPolicyVersions[0].state;
    first
        .socialBenefitAccrualPolicies
        .as_mut()
        .unwrap()
        .birlestirilmisSosyalYardim
        .as_mut()
        .unwrap()
        .allowMonthlyAmountExcess = Some(false);
    let mut changed = request.dataset.compensationPolicyVersions[0].clone();
    changed.id = "capped-aid-policy-mid-period".into();
    changed.effectiveFrom = "2026-01-31".into();
    changed.state.birlestirilmisSosyalYardim = dec!(6000);
    changed.supersedesVersionId = None;
    request.dataset.compensationPolicyVersions.push(changed);

    let payroll = calculate_payroll_checked(&request).expect("capped split policy payroll");
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(4451.61)),
        "the cap is the sum of each policy slice's monthly share"
    );
    let saved_segments = &payroll
        .calculationMetadata
        .as_ref()
        .unwrap()
        .snapshot
        .as_ref()
        .unwrap()
        .compensation
        .segments;
    assert_eq!(saved_segments.len(), 2);
    assert!(saved_segments.iter().all(|segment| segment
        .state
        .socialBenefitAccrualPolicies
        .as_ref()
        .unwrap()
        .birlestirilmisSosyalYardim
        .as_ref()
        .unwrap()
        .allowMonthlyAmountExcess
        == Some(false)));

    let mut legacy = request.clone();
    for version in &mut legacy.dataset.compensationPolicyVersions {
        version
            .state
            .socialBenefitAccrualPolicies
            .as_mut()
            .unwrap()
            .birlestirilmisSosyalYardim
            .as_mut()
            .unwrap()
            .allowMonthlyAmountExcess = None;
    }
    assert_eq!(
        calculate_payroll_checked(&legacy)
            .unwrap()
            .gelirler
            .birlestirilmisSosyalYardim,
        Some(dec!(4600)),
        "missing option retains each historical 31/30 slice result"
    );
}

#[test]
fn full_if_eligible_day_is_slice_scoped_and_zero_eligible_slice_is_not_paid() {
    let mut request = audit_support::request();
    set_worked_days(&mut request, 15);
    set_combined_aid(
        &mut request,
        policy(
            BenefitPaymentMethod::FullIfEligibleDay,
            vec![BenefitDayType::Worked],
            BenefitProrationDenominator::CalendarDays,
        ),
    );
    let mut changed = request.dataset.compensationPolicyVersions[0].clone();
    changed.id = "full-if-aid-mid-period".into();
    changed.effectiveFrom = "2026-01-31".into();
    changed.state.birlestirilmisSosyalYardim = dec!(6000);
    changed
        .state
        .socialBenefitAccrualPolicies
        .as_mut()
        .unwrap()
        .birlestirilmisSosyalYardim
        .as_mut()
        .unwrap()
        .method = BenefitPaymentMethod::FullIfEligibleDay;
    changed.supersedesVersionId = None;
    request.dataset.compensationPolicyVersions.push(changed);

    let payroll = calculate_payroll_checked(&request).expect("slice-scoped full-if payroll");
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(1548.39)),
        "15 eligible days in the first 16-day slice pays that slice's share only"
    );
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

    let old_rule_json = serde_json::json!({
        "method": "PROPORTIONAL",
        "eligibleDayTypes": ["WORKED"],
        "denominator": "THIRTY_DAY_MONTH"
    });
    let old_rule: BenefitAccrualPolicy = serde_json::from_value(old_rule_json)
        .expect("old rule without the cap field should deserialize");
    assert_eq!(old_rule.allowMonthlyAmountExcess, None);

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

    let mut capped_case = request.clone();
    set_worked_days(&mut capped_case, 31);
    set_combined_aid(&mut capped_case, old_rule);
    assert_eq!(
        calculate_payroll_checked(&capped_case)
            .unwrap()
            .gelirler
            .birlestirilmisSosyalYardim,
        Some(dec!(3100)),
        "old policy keeps its historical 31/30 result"
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
