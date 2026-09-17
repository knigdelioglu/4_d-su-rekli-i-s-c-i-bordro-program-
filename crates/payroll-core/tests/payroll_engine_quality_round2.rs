#[path = "support/independent_audit.rs"]
mod audit_support;

use chrono::NaiveDate;
use payroll_core::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn canonical_period(year: i32, month: i32) -> BordroDonemi {
    let (end_year, end_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    BordroDonemi {
        id: format!("quality-{year}-{month:02}"),
        yil: year,
        ay: month,
        baslangicTarihi: format!("{year}-{month:02}-15"),
        bitisTarihi: format!("{end_year}-{end_month:02}-14"),
        donemAdi: format!("Quality {year}-{month:02}"),
        taxYear: end_year,
        taxMonth: end_month,
    }
}

fn period_and_settings() -> (BordroDonemi, DonemselKurumDegerleri) {
    let request = audit_support::request();
    let period = request.dataset.periods[0].clone();
    let settings = request
        .dataset
        .institutionSettings
        .get(&period.id)
        .expect("audit settings")
        .clone();
    (period, settings)
}

fn statutory_segment(
    effective_from: &str,
    daily_minimum: Option<Decimal>,
    pek_multiplier: Option<Decimal>,
    sgk_meal: Option<Decimal>,
    gv_meal: Option<Decimal>,
) -> StatutoryParameterSegment {
    StatutoryParameterSegment {
        effectiveFrom: effective_from.into(),
        gunlukAsgariUcret: daily_minimum,
        pekTavanKatsayisi: pek_multiplier,
        gunlukYemekIstisnasiSGK: sgk_meal,
        gunlukYemekIstisnasiGV: gv_meal,
    }
}

#[test]
fn full_15_to_14_periods_have_exact_calendar_and_sgk_bounds() {
    let cases = [
        (2026, 1, 31),
        (2026, 2, 28),
        (2028, 2, 29),
        (2026, 4, 30),
        (2026, 12, 31),
    ];

    for (year, month, calendar_days) in cases {
        let request = audit_support::dated_request(year, month);
        let period = request.dataset.periods[0].clone();
        let settings = request
            .dataset
            .institutionSettings
            .get(&period.id)
            .expect("period settings");
        let attendance = &request.dataset.attendances[0];
        let full = resolve_statutory_snapshot_for_period(attendance, &period, settings)
            .expect("full attendance should resolve");

        assert_eq!(
            full.sgkPrimGunSayisi, 30,
            "full {year}-{month:02} period ({calendar_days} calendar days)"
        );
        assert_eq!(full.pekAltSinir, dec!(33030));
        assert_eq!(full.pekUstSinir, dec!(297270));
        assert_eq!(full.segments[0].sgkPrimGunSayisi, 30);

        let mut one_unpaid = attendance.clone();
        let unpaid_date = format!("{year}-{month:02}-15");
        one_unpaid.gunler.insert(unpaid_date, "R".into());
        let reduced = resolve_statutory_snapshot_for_period(&one_unpaid, &period, settings)
            .expect("one unpaid day should resolve");
        assert_eq!(
            reduced.sgkPrimGunSayisi,
            calendar_days - 1,
            "one unpaid day in {year}-{month:02}"
        );
        assert!(reduced.sgkPrimGunSayisi >= 0);
        assert!(reduced.sgkPrimGunSayisi <= 30);
    }
}

#[test]
fn attendance_classification_table_preserves_prim_and_meal_semantics() {
    let request = audit_support::dated_request(2026, 2);
    let period = request.dataset.periods[0].clone();
    let settings = request
        .dataset
        .institutionSettings
        .get(&period.id)
        .expect("period settings");
    let base_attendance = &request.dataset.attendances[0];
    let r_date = NaiveDate::from_ymd_opt(2026, 2, 20).expect("valid R date");

    let cases = [
        ("Ç", None, 30, 28),
        ("T", None, 30, 0),
        ("G", None, 30, 0),
        ("İ", None, 30, 0),
        ("GÇ", None, 30, 28),
        ("GÇT", None, 30, 0),
        ("R", Some(r_date), 30, 27),
        ("R", None, 27, 27),
    ];

    for (code, paid_sick_date, expected_sgk_days, expected_meal_days) in cases {
        let mut attendance = base_attendance.clone();
        if code == "R" {
            attendance.gunler.insert(r_date.to_string(), code.into());
        } else {
            for value in attendance.gunler.values_mut() {
                *value = code.into();
            }
        }
        let paid_dates = paid_sick_date.into_iter().collect::<Vec<_>>();
        let snapshot = resolve_statutory_snapshot_for_period_with_paid_sick_dates(
            &attendance,
            &period,
            settings,
            &paid_dates,
        )
        .expect("attendance code should resolve");

        assert_eq!(
            snapshot.sgkPrimGunSayisi, expected_sgk_days,
            "SGK day classification for {code}, paid={paid_sick_date:?}"
        );
        assert_eq!(snapshot.segments[0].fiiliYemekGunu, expected_meal_days);
    }
}

#[test]
fn paid_sick_wage_counts_two_days_with_decimal_exactness() {
    let mut request = audit_support::request();
    request.dataset.sickLeaveRecords.push(SickLeaveRecord {
        id: "paid-sick-two-days".into(),
        personnelId: "audit".into(),
        startDate: "2026-02-01".into(),
        endDate: "2026-02-02".into(),
        createdAt: None,
        updatedAt: None,
    });
    for date in ["2026-02-01", "2026-02-02"] {
        request.dataset.attendances[0]
            .gunler
            .insert(date.into(), "R".into());
    }

    let payroll = calculate_payroll_checked(&request).expect("paid sick payroll");
    assert_eq!(payroll.puantajOzeti.c, 29);
    assert_eq!(payroll.puantajOzeti.r, 2);
    assert_eq!(payroll.odenenRaporluGun, Some(2));
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(62000)));
    assert_eq!(
        payroll.statutorySnapshot.as_ref().unwrap().sgkPrimGunSayisi,
        30
    );
}

#[test]
fn paid_sick_dates_must_match_r_attendance_at_the_public_boundary() {
    let mut request = audit_support::request();
    request.dataset.sickLeaveRecords.push(SickLeaveRecord {
        id: "paid-sick-attendance-mismatch".into(),
        personnelId: "audit".into(),
        startDate: "2026-02-01".into(),
        endDate: "2026-02-02".into(),
        createdAt: None,
        updatedAt: None,
    });

    let error = calculate_payroll_checked(&request)
        .expect_err("a paid sick date that is not marked R must fail closed");
    assert!(
        matches!(error, DomainError::InvalidData(ref message) if message.contains("puantajda")),
        "unexpected sick-attendance validation error: {error:?}"
    );
}

#[test]
fn manual_payroll_income_rejects_negative_optional_lines_at_public_boundary() {
    for (tediye, tis_ikramiyesi) in [(Some(dec!(-1)), None), (None, Some(dec!(-1)))] {
        let mut request = audit_support::request();
        request.manualIncome = Some(ManualPayrollIncomeInput {
            tediye,
            tisIkramiyesi: tis_ikramiyesi,
        });

        let error = calculate_payroll_checked(&request)
            .expect_err("negative manual income must fail closed");
        assert!(
            matches!(error, DomainError::ValidationError(ref message) if message.contains("negatif")),
            "unexpected manual-income validation error: {error:?}"
        );
    }
}

#[test]
fn raise_split_merges_work_premium_details_with_exact_weighted_values() {
    let mut request = audit_support::request();
    request.dataset.zamAylari = vec![2];

    let current_settings = request
        .dataset
        .institutionSettings
        .get_mut(&request.periodId)
        .expect("current settings");
    current_settings.gunlukTabanUcret = dec!(1000);
    current_settings.gunlukYemek = Decimal::ZERO;
    let current_group = current_settings
        .isPrimiGruplari
        .as_mut()
        .expect("current groups")
        .first_mut()
        .expect("current group");
    current_group.ad = "current".into();
    current_group.oran = dec!(9);

    let mut previous_period = request.dataset.periods[0].clone();
    previous_period.id = "audit-dec".into();
    previous_period.yil = 2025;
    previous_period.ay = 12;
    previous_period.baslangicTarihi = "2025-12-15".into();
    previous_period.bitisTarihi = "2026-01-14".into();
    previous_period.taxYear = 2025;
    previous_period.taxMonth = 12;
    previous_period.donemAdi = "Audit December".into();

    let mut previous_settings = request
        .dataset
        .institutionSettings
        .get(&request.periodId)
        .expect("current settings")
        .clone();
    previous_settings.donemId = previous_period.id.clone();
    previous_settings.gunlukTabanUcret = dec!(2000);
    let previous_group = previous_settings
        .isPrimiGruplari
        .as_mut()
        .expect("previous groups")
        .first_mut()
        .expect("previous group");
    previous_group.ad = "previous".into();
    previous_group.oran = dec!(8);
    request.dataset.periods.push(previous_period);
    request
        .dataset
        .institutionSettings
        .insert("audit-dec".into(), previous_settings);

    for value in request.dataset.attendances[0].gunler.values_mut() {
        *value = "T".into();
    }
    request.dataset.attendances[0]
        .gunler
        .insert("2026-01-15".into(), "Ç".into());
    request.dataset.attendances[0]
        .gunler
        .insert("2026-01-16".into(), "Ç".into());
    request.dataset.attendances[0]
        .gunler
        .insert("2026-02-01".into(), "GÇ".into());
    request.dataset.attendances[0]
        .gunler
        .insert("2026-02-02".into(), "GÇT".into());
    request.dataset.attendances[0]
        .gunler
        .insert("2026-02-03".into(), "G".into());
    request.dataset.attendances[0]
        .gunler
        .insert("2026-02-04".into(), "İ".into());
    request.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
        sendikaUyesi: Some(true),
        besUyesi: Some(false),
        ..PersonelKesintileri::default()
    });

    let payroll = calculate_payroll_checked(&request).expect("raise payroll");
    let detail = payroll.isPrimiDetay.expect("merged work premium detail");
    assert_eq!(detail.grupId, "1. Grup");
    assert_eq!(detail.grupAd, "current");
    assert_eq!(detail.oran, dec!(9));
    assert_eq!(detail.hakGunu, 3);
    assert_eq!(detail.tutar, dec!(410));
    assert_eq!(detail.gunlukIsPrimi, dec!(136.67));
    assert_eq!(
        payroll.kesintiler.sendikaAidati,
        Some(dec!(1006.45)),
        "weighted before/after daily wage must reach the public union deduction"
    );
}

fn retro_source_carry_fixture(payment_date: &str) -> (RetroAdjustmentBatch, RetroAllocation) {
    let batch = RetroAdjustmentBatch {
        id: "audit-retro-carry".into(),
        revisionId: "audit-revision".into(),
        personnelId: "audit".into(),
        paymentDate: payment_date.into(),
        status: CompensationRevisionStatus::CALCULATED,
        settlementStatus: RetroSettlementStatus::UNSETTLED,
        totalGrossDelta: Decimal::ZERO,
        payableSettlementAmount: Decimal::ZERO,
        offsetSettlementAmount: Decimal::ZERO,
        recoveredAmount: Decimal::ZERO,
        recoverableAmount: Decimal::ZERO,
        outstandingReceivable: Decimal::ZERO,
        description: None,
        createdAt: None,
        calculatedAt: None,
        finalizedAt: None,
    };
    let allocation = RetroAllocation {
        id: "audit-retro-carry-allocation".into(),
        batchId: batch.id.clone(),
        personnelId: "audit".into(),
        sourcePeriodId: "audit-jan".into(),
        earningCode: RetroEarningCode::BASE_WAGE,
        originalRecognizedAmount: Decimal::ZERO,
        previousAuthoritativeRetroAmount: Decimal::ZERO,
        targetAmount: Decimal::ZERO,
        deltaAmount: Decimal::ZERO,
        sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
        incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
        stampTaxTreatment: RetroTaxTreatment::TAXABLE,
        originalPek: Decimal::ZERO,
        retroPekDelta: Decimal::ZERO,
        adjustedPek: Decimal::ZERO,
        workerSgkDelta: Decimal::ZERO,
        workerUnemploymentDelta: Decimal::ZERO,
        employerSgkDelta: Decimal::ZERO,
        employerUnemploymentDelta: Decimal::ZERO,
        originalEmployerLowerBound: Decimal::ZERO,
        targetEmployerLowerBound: Decimal::ZERO,
        employerLowerBoundDelta: Decimal::ZERO,
        employerLowerBoundPremiumDelta: Decimal::ZERO,
        originalSourceCarry: Some(Vec::new()),
        targetSourceCarry: Some(vec![DevredenPekKaydi {
            tutar: dec!(1),
            kalanAySayisi: 2,
            kaynakDonemId: Some("audit-jan".into()),
        }]),
        payableSettlementAmount: Decimal::ZERO,
        offsetSettlementAmount: Decimal::ZERO,
        recoverableAmount: Decimal::ZERO,
        metadata: None,
    };
    (batch, allocation)
}

#[test]
fn retro_source_carry_is_applied_at_payment_boundary_but_not_from_the_future() {
    for (payment_date, expected_carry_used) in [("2026-01-31", dec!(1)), ("2026-02-01", dec!(0))] {
        let mut request = audit_support::request();
        let (batch, allocation) = retro_source_carry_fixture(payment_date);
        request.dataset.retroBatches.push(batch);
        request.dataset.retroAllocations.push(allocation);

        let payroll = calculate_payroll(&request).expect("carry boundary payroll");
        assert_eq!(
            payroll.pekDetay.as_ref().unwrap().devredenPekKullanilan,
            expected_carry_used,
            "retro source carry payment boundary {payment_date}"
        );
    }
}

#[test]
fn retro_source_carry_rejects_negative_amounts_and_lifetime_but_accepts_zero() {
    for (amount, months, expected_ok) in [
        (dec!(-1), 2, false),
        (dec!(1), -1, false),
        (dec!(0), 0, true),
    ] {
        let mut request = audit_support::request();
        let (batch, mut allocation) = retro_source_carry_fixture("2026-01-31");
        allocation.targetSourceCarry = Some(vec![DevredenPekKaydi {
            tutar: amount,
            kalanAySayisi: months,
            kaynakDonemId: Some("audit-jan".into()),
        }]);
        request.dataset.retroBatches.push(batch);
        request.dataset.retroAllocations.push(allocation);

        let result = calculate_payroll(&request);
        assert_eq!(result.is_ok(), expected_ok, "carry {amount}/{months}");
        if let Err(error) = result {
            assert!(
                error.to_string().contains("negatif"),
                "unexpected carry validation error: {error:?}"
            );
        }
    }
}

#[test]
fn period_validation_matrix_rejects_order_shape_and_metadata_boundaries() {
    let valid = canonical_period(2026, 1);
    assert!(validate_period(&valid).is_ok());

    let mut equal = valid.clone();
    equal.bitisTarihi = equal.baslangicTarihi.clone();
    assert!(validate_period(&equal).is_err(), "start == end");

    let mut reversed = valid.clone();
    reversed.baslangicTarihi = "2026-02-15".into();
    reversed.bitisTarihi = "2026-01-14".into();
    reversed.yil = 2026;
    reversed.ay = 2;
    assert!(validate_period(&reversed).is_err(), "start > end");

    let mut bad_start_day = valid.clone();
    bad_start_day.baslangicTarihi = "2026-01-16".into();
    assert!(validate_period(&bad_start_day).is_err());

    let mut bad_end_day = valid.clone();
    bad_end_day.bitisTarihi = "2026-02-13".into();
    assert!(validate_period(&bad_end_day).is_err());

    let mut wrong_end_year = valid.clone();
    wrong_end_year.bitisTarihi = "2025-02-14".into();
    assert!(validate_period(&wrong_end_year).is_err());

    let mut wrong_end_month = valid.clone();
    wrong_end_month.bitisTarihi = "2026-03-14".into();
    assert!(validate_period(&wrong_end_month).is_err());

    let mut bad_year = valid.clone();
    bad_year.yil = 0;
    assert!(validate_period(&bad_year).is_err());

    let mut bad_month = valid.clone();
    bad_month.ay = 13;
    assert!(validate_period(&bad_month).is_err());

    let mut zero_year = valid.clone();
    zero_year.yil = 0;
    zero_year.baslangicTarihi = "0000-01-15".into();
    zero_year.bitisTarihi = "0000-02-14".into();
    zero_year.taxYear = 1;
    zero_year.taxMonth = 1;
    assert!(validate_period(&zero_year).is_err());

    let mut bad_tax_year = valid.clone();
    bad_tax_year.taxYear = 0;
    assert!(validate_period(&bad_tax_year).is_err());

    let mut bad_tax_month = valid.clone();
    bad_tax_month.taxMonth = 0;
    assert!(validate_period(&bad_tax_month).is_err());

    let mut wrong_metadata = valid.clone();
    wrong_metadata.yil = 2025;
    assert!(validate_period(&wrong_metadata).is_err());

    let mut wrong_metadata_month = valid.clone();
    wrong_metadata_month.ay = 2;
    assert!(validate_period(&wrong_metadata_month).is_err());

    assert!(validate_period(&canonical_period(2026, 12)).is_ok());
}

#[test]
fn tax_month_overlap_matrix_requires_a_real_start_or_end_match() {
    let january = canonical_period(2026, 1);
    assert!(validate_tax_month_overlap(&january).is_ok());

    let mut january_end_tax = january.clone();
    january_end_tax.taxYear = 2026;
    january_end_tax.taxMonth = 2;
    assert!(validate_tax_month_overlap(&january_end_tax).is_ok());

    let mut wrong_year = january.clone();
    wrong_year.taxYear = 2025;
    wrong_year.taxMonth = 1;
    assert!(validate_tax_month_overlap(&wrong_year).is_err());

    let mut wrong_month = january.clone();
    wrong_month.taxMonth = 3;
    assert!(validate_tax_month_overlap(&wrong_month).is_err());

    let december = canonical_period(2026, 12);
    assert!(validate_tax_month_overlap(&december).is_ok());

    let mut december_start_tax = december.clone();
    december_start_tax.taxYear = 2026;
    december_start_tax.taxMonth = 12;
    assert!(validate_tax_month_overlap(&december_start_tax).is_ok());

    let mut december_wrong_both = december;
    december_wrong_both.taxYear = 2025;
    december_wrong_both.taxMonth = 11;
    assert!(validate_tax_month_overlap(&december_wrong_both).is_err());
}

#[test]
fn statutory_segment_validation_is_closed_at_period_edges_and_values() {
    let (period, base_settings) = period_and_settings();

    for effective in [&period.baslangicTarihi, &period.bitisTarihi] {
        let mut settings = base_settings.clone();
        settings.statutoryParameterSegments = Some(vec![statutory_segment(
            effective,
            Some(dec!(1101)),
            None,
            None,
            None,
        )]);
        assert!(validate_statutory_segments_for_period(&period, &settings).is_ok());
    }

    for effective in ["2026-01-14", "2026-02-15"] {
        let mut settings = base_settings.clone();
        settings.statutoryParameterSegments = Some(vec![statutory_segment(
            effective,
            Some(dec!(1101)),
            None,
            None,
            None,
        )]);
        assert!(validate_statutory_segments_for_period(&period, &settings).is_err());
    }

    let mut duplicate = base_settings.clone();
    duplicate.statutoryParameterSegments = Some(vec![
        statutory_segment("2026-01-15", Some(dec!(1101)), None, None, None),
        statutory_segment("2026-01-15", None, Some(dec!(9)), None, None),
    ]);
    assert!(validate_statutory_segments_for_period(&period, &duplicate).is_err());

    let mut descending = base_settings.clone();
    descending.statutoryParameterSegments = Some(vec![
        statutory_segment("2026-02-01", Some(dec!(1101)), None, None, None),
        statutory_segment("2026-01-31", None, Some(dec!(9)), None, None),
    ]);
    assert!(validate_statutory_segments_for_period(&period, &descending).is_err());

    let mut empty_override = base_settings.clone();
    empty_override.statutoryParameterSegments = Some(vec![statutory_segment(
        "2026-01-15",
        None,
        None,
        None,
        None,
    )]);
    assert!(validate_statutory_segments_for_period(&period, &empty_override).is_err());

    for (value, expected_ok) in [(dec!(0), false), (dec!(1), true), (dec!(1.01), true)] {
        let mut settings = base_settings.clone();
        settings.statutoryParameterSegments = Some(vec![statutory_segment(
            "2026-01-15",
            Some(value),
            None,
            None,
            None,
        )]);
        assert_eq!(
            validate_statutory_segments_for_period(&period, &settings).is_ok(),
            expected_ok,
            "daily minimum {value}"
        );
    }

    for (value, expected_ok) in [(dec!(0.99), false), (dec!(1), true), (dec!(9), true)] {
        let mut settings = base_settings.clone();
        settings.statutoryParameterSegments = Some(vec![statutory_segment(
            "2026-01-15",
            None,
            Some(value),
            None,
            None,
        )]);
        assert_eq!(
            validate_statutory_segments_for_period(&period, &settings).is_ok(),
            expected_ok,
            "PEK multiplier {value}"
        );
    }

    for (sgk_meal, gv_meal, expected_ok) in [
        (Some(dec!(0)), Some(dec!(0)), true),
        (Some(dec!(-0.01)), Some(dec!(0)), false),
        (Some(dec!(0)), Some(dec!(-0.01)), false),
    ] {
        let mut settings = base_settings.clone();
        settings.statutoryParameterSegments = Some(vec![statutory_segment(
            "2026-01-15",
            None,
            None,
            sgk_meal,
            gv_meal,
        )]);
        assert_eq!(
            validate_statutory_segments_for_period(&period, &settings).is_ok(),
            expected_ok,
            "meal SGK/GV boundary"
        );
    }
}

fn normal_with_icra(icra: Decimal) -> PayrollCalculationRequest {
    let mut request = audit_support::request();
    request.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
        icraTutar: Some(icra),
        ..PersonelKesintileri::default()
    });
    let settings = request
        .dataset
        .institutionSettings
        .get_mut(&request.periodId)
        .expect("settings");
    settings.sgkIsciOraniYuzde = Some(Decimal::ZERO);
    settings.issizlikIsciOraniYuzde = Some(Decimal::ZERO);
    settings.gelirVergisiOraniYuzde = Some(Decimal::ZERO);
    settings.damgaVergisiOraniBinde = Some(Decimal::ZERO);
    settings.sendikaAidatiYuzde = Some(Decimal::ZERO);
    settings.besOraniYuzde = Some(Decimal::ZERO);
    settings.gunlukTabanUcret = dec!(100);
    settings.gunlukYemek = Decimal::ZERO;
    for value in request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    request.dataset.attendances[0]
        .gunler
        .insert("2026-01-15".into(), "Ç".into());
    request
}

#[test]
fn negative_net_boundary_allows_zero_and_reports_exact_positive_deficit() {
    let zero = calculate_payroll_checked(&normal_with_icra(dec!(100)))
        .expect("income equal to deductions is valid");
    assert_eq!(zero.gelirToplam, dec!(100));
    assert_eq!(zero.kesintiToplam, dec!(100));
    assert_eq!(zero.netOdeme, Decimal::ZERO);

    let error = calculate_payroll_checked(&normal_with_icra(dec!(101)))
        .expect_err("deductions above income must fail closed");
    match error {
        DomainError::NegativeNetPayment {
            gelir,
            kesinti,
            fark,
        } => {
            assert_eq!(gelir, dec!(100));
            assert_eq!(kesinti, dec!(101));
            assert_eq!(fark, dec!(1));
        }
        other => panic!("expected NegativeNetPayment, got {other:?}"),
    }
}
