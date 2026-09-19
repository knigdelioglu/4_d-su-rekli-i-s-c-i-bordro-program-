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

#[test]
fn raise_split_with_no_after_raise_premium_keeps_the_before_raise_group() {
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

    let payroll = calculate_payroll_checked(&request).expect("raise payroll without after premium");
    let detail = payroll.isPrimiDetay.expect("merged work premium detail");
    assert_eq!(detail.grupAd, "previous");
    assert_eq!(detail.oran, dec!(8));
    assert_eq!(detail.hakGunu, 1);
    assert_eq!(detail.tutar, dec!(160));
    assert_eq!(detail.gunlukIsPrimi, dec!(160));
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

#[test]
fn recalculating_existing_payment_event_does_not_count_it_as_prior_state() {
    let mut request = audit_support::request();
    let first = calculate_payroll_checked(&request).expect("initial payment event");
    request.dataset.payrolls.push(first.clone());

    let recalculated = calculate_payroll_checked(&request)
        .expect("recalculating the same payment event must remain deterministic");

    assert_eq!(recalculated.gelirToplam, first.gelirToplam);
    assert_eq!(recalculated.kesintiToplam, first.kesintiToplam);
    assert_eq!(recalculated.netOdeme, first.netOdeme);
    assert_eq!(
        recalculated.oncekiKumulatifGvMatrahi,
        first.oncekiKumulatifGvMatrahi
    );
    assert_eq!(
        recalculated.oncekiKumulatifAsgariGvMatrahi,
        first.oncekiKumulatifAsgariGvMatrahi
    );
    assert_eq!(
        serde_json::to_value(&recalculated.pekDetay).unwrap(),
        serde_json::to_value(&first.pekDetay).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&recalculated.gvDetay).unwrap(),
        serde_json::to_value(&first.gvDetay).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&recalculated.damgaDetay).unwrap(),
        serde_json::to_value(&first.damgaDetay).unwrap()
    );
    assert_eq!(recalculated.devredenPekGelen, first.devredenPekGelen);
    assert_eq!(recalculated.sonrakiDevredenPek, first.sonrakiDevredenPek);
}

#[test]
fn finalization_rejects_a_prior_non_finalized_payment_event_at_the_public_boundary() {
    let mut request = audit_support::request();
    let current = calculate_payroll_checked(&request).expect("current payment event");

    let mut previous_period = request.dataset.periods[0].clone();
    previous_period.id = "audit-dec".into();
    previous_period.yil = 2025;
    previous_period.ay = 12;
    previous_period.baslangicTarihi = "2025-12-15".into();
    previous_period.bitisTarihi = "2026-01-14".into();
    previous_period.taxYear = 2025;
    previous_period.taxMonth = 12;
    request.dataset.periods.push(previous_period);

    let mut previous = current.clone();
    previous.id = "audit-dec-payroll".into();
    previous.accrualId = "audit-dec-payroll".into();
    previous.donemId = "audit-dec".into();
    previous.paymentDate = "2025-12-31".into();
    previous.status = BordroStatus::CALCULATED;
    request.dataset.payrolls = vec![current, previous];

    let error = finalize_payroll(&request)
        .expect_err("a prior non-finalized payment event must block finalization");
    assert!(
        error.to_string().contains("FINALIZED"),
        "unexpected prior-event finalization error: {error:?}"
    );
}

#[test]
fn finalization_rejects_missing_calendar_attendance_at_the_public_boundary() {
    let mut request = audit_support::request();
    let payroll = calculate_payroll_checked(&request).expect("authoritative current payroll");
    request.dataset.payrolls.push(payroll);
    request.dataset.attendances[0].gunler.remove("2026-01-15");

    let error = finalize_payroll(&request)
        .expect_err("finalization must fail closed when one calendar day is missing");
    assert!(
        error.to_string().contains("eksik"),
        "unexpected missing-attendance finalization error: {error:?}"
    );
}

#[test]
fn supplementary_accrual_boundaries_reject_negative_and_colliding_inputs() {
    let mut zero_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    zero_request.accrual.as_mut().unwrap().grossAmount = Some(Decimal::ZERO);
    let zero =
        calculate_payroll_checked(&zero_request).expect("zero supplementary amount is valid");
    assert_eq!(zero.gelirler.tediye, Some(Decimal::ZERO));

    let mut negative_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    negative_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(-0.01));
    let negative_error = calculate_payroll_checked(&negative_request)
        .expect_err("negative supplementary amount must fail closed");
    assert!(negative_error.to_string().contains("negatif"));

    let mut foreign_collision =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    let foreign_id = "shared-accrual";
    foreign_collision.dataset.payrolls.push({
        let mut existing = calculate_payroll(&foreign_collision).expect("collision fixture");
        existing.personelId = "another-person".into();
        existing.accrualId = foreign_id.into();
        existing
    });
    foreign_collision.accrual.as_mut().unwrap().accrualId = foreign_id.into();
    let collision_error = calculate_payroll_checked(&foreign_collision)
        .expect_err("an accrual id owned by another person must fail closed");
    assert!(collision_error.to_string().contains("başka bir personel"));

    let mut order_collision = audit_support::request();
    let existing = calculate_payroll_checked(&order_collision).expect("order fixture");
    let payment_date = existing.paymentDate.clone();
    let sequence = existing.sequence;
    order_collision.dataset.payrolls.push(existing);
    order_collision.accrual = Some(PayrollAccrualInput {
        accrualId: "same-order-different-id".into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: payment_date,
        sequence,
        grossAmount: Some(dec!(1)),
        description: None,
    });
    let order_error = calculate_payroll_checked(&order_collision)
        .expect_err("same tax month/date/sequence must be unique");
    assert!(order_error.to_string().contains("sıra numarası benzersiz"));
}

#[test]
fn accrual_resolution_uses_only_matching_tax_month_and_payment_date_metadata() {
    let mut request = audit_support::request();
    let baseline = calculate_payroll_checked(&request).expect("baseline event");

    let mut future_period = request.dataset.periods[0].clone();
    future_period.id = "audit-feb".into();
    future_period.ay = 2;
    future_period.baslangicTarihi = "2026-02-15".into();
    future_period.bitisTarihi = "2026-03-14".into();
    future_period.taxMonth = 2;
    request.dataset.periods.push(future_period);

    let mut mismatched_event = baseline;
    mismatched_event.id = "future-metadata-event".into();
    mismatched_event.accrualId = "future-metadata-event".into();
    mismatched_event.donemId = "audit-feb".into();
    mismatched_event.sequence = 9;
    request.dataset.payrolls.push(mismatched_event);

    let recalculated = calculate_payroll_checked(&request)
        .expect("a different tax month must not become a prior same-month event");
    assert_eq!(recalculated.sequence, 0);
}

#[test]
fn accrual_resolution_rejects_payment_date_with_only_one_wrong_tax_component() {
    let mut request = audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    request.accrual.as_mut().unwrap().paymentDate = "2026-02-01".into();

    let error = calculate_payroll_checked(&request)
        .expect_err("payment month mismatch must fail even when payment year matches");
    assert!(error.to_string().contains("uyumlu değil"));
}

#[test]
fn automatic_normal_resolution_ignores_existing_supplementary_count() {
    let first_request = audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    let first = calculate_payroll_checked(&first_request).expect("first supplementary event");

    let mut second_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TIS_IKRAMIYE);
    second_request.accrual.as_mut().unwrap().accrualId = "audit-extra-2".into();
    second_request.accrual.as_mut().unwrap().sequence = 2;
    second_request.dataset.payrolls.push(first.clone());
    let second = calculate_payroll_checked(&second_request).expect("second supplementary event");

    let mut normal_request = audit_support::request();
    normal_request.dataset.payrolls.extend([first, second]);
    let normal = calculate_payroll_checked(&normal_request)
        .expect("supplementary events must not count as duplicate NORMAL events");
    assert_eq!(normal.accrualType, AccrualType::NORMAL);
}

#[test]
fn same_month_reconciliation_has_exact_wage_priority_and_carry() {
    let mut tediye_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    tediye_request.dataset.attendances.clear();
    tediye_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(200000));
    let tediye = calculate_payroll(&tediye_request).expect("provisional tediye");

    let mut normal_request = audit_support::request();
    for value in normal_request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    for _ in 0..10 {
        normal_request.dataset.attendances[0]
            .gunler
            .insert(date.to_string(), "Ç".into());
        date += chrono::Duration::days(1);
    }
    normal_request.dataset.payrolls.push(tediye.clone());
    let normal = calculate_payroll(&normal_request).expect("reconciled normal");
    assert_eq!(
        tediye.pekDetay.as_ref().unwrap().aylikSonrasiPekTuketimi,
        Some(dec!(200000))
    );
    assert_eq!(
        normal.statutorySnapshot.as_ref().unwrap().sgkPrimGunSayisi,
        10
    );
    let pek = normal.pekDetay.as_ref().expect("normal PEK detail");
    assert_eq!(pek.aylikOncekiPekTuketimi, Some(Decimal::ZERO));
    assert_eq!(pek.aylikSonrasiPekTuketimi, Some(dec!(99090)));
    assert_eq!(pek.primMatrahi, dec!(20000));
    let outgoing = normal
        .sonrakiDevredenPek
        .as_ref()
        .expect("reconciled non-wage carry");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].tutar, dec!(120910));
    assert_eq!(outgoing[0].kalanAySayisi, 2);
}

#[test]
fn same_month_pek_state_accumulates_distinct_prior_payment_events_exactly() {
    let mut first_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    first_request.dataset.attendances.clear();
    first_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(200000));
    let first = calculate_payroll(&first_request).expect("first provisional payment");

    let mut second_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TIS_IKRAMIYE);
    second_request.dataset.attendances.clear();
    second_request.accrual.as_mut().unwrap().accrualId = "audit-extra-2".into();
    second_request.accrual.as_mut().unwrap().sequence = 2;
    second_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(10000));
    second_request.dataset.payrolls.push(first.clone());
    let second = calculate_payroll(&second_request).expect("second provisional payment");

    assert_eq!(
        first.pekDetay.as_ref().unwrap().aylikSonrasiPekTuketimi,
        Some(dec!(200000))
    );
    assert_eq!(
        second.pekDetay.as_ref().unwrap().aylikOncekiPekTuketimi,
        Some(dec!(200000))
    );
    assert_eq!(
        second.pekDetay.as_ref().unwrap().aylikSonrasiPekTuketimi,
        Some(dec!(210000))
    );

    let mut normal_request = audit_support::request();
    normal_request.dataset.payrolls.extend([first, second]);
    let normal = calculate_payroll(&normal_request).expect("normal payment after two events");
    let pek = normal.pekDetay.as_ref().expect("normal PEK detail");
    assert_eq!(pek.aylikOncekiPekTuketimi, Some(dec!(210000)));
    assert_eq!(pek.aylikSonrasiPekTuketimi, Some(dec!(272000)));
    assert_eq!(pek.primMatrahi, dec!(62000));
    assert_eq!(normal.sonrakiDevredenPek, Some(Vec::new()));
}

#[test]
fn same_month_reconciliation_uses_attendance_backed_prior_capacity_boundary() {
    let mut tediye_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    tediye_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(200000));
    let tediye = calculate_payroll(&tediye_request).expect("attendance-backed tediye");
    assert_eq!(
        tediye.statutorySnapshot.as_ref().unwrap().source,
        StatutorySnapshotSource::AttendanceBacked
    );

    let mut normal_request = audit_support::request();
    for value in normal_request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    for _ in 0..10 {
        normal_request.dataset.attendances[0]
            .gunler
            .insert(date.to_string(), "Ç".into());
        date += chrono::Duration::days(1);
    }
    normal_request.dataset.payrolls.push(tediye);

    let normal = calculate_payroll(&normal_request).expect("reconciled normal");
    let pek = normal.pekDetay.as_ref().expect("normal PEK detail");
    assert_eq!(pek.aylikOncekiPekTuketimi, Some(Decimal::ZERO));
    assert_eq!(pek.aylikSonrasiPekTuketimi, Some(dec!(99090)));
    assert_eq!(pek.primMatrahi, dec!(20000));
    let outgoing = normal
        .sonrakiDevredenPek
        .as_ref()
        .expect("reconciled non-wage carry");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].tutar, dec!(120910));
    assert_eq!(outgoing[0].kalanAySayisi, 2);
}

#[test]
fn same_month_reconciliation_does_not_promote_a_normal_prior_event_to_provisional() {
    let mut previous_request = audit_support::request();
    previous_request.periodId = "audit-dec".into();
    let previous_period = &mut previous_request.dataset.periods[0];
    previous_period.id = "audit-dec".into();
    previous_period.yil = 2025;
    previous_period.ay = 12;
    previous_period.baslangicTarihi = "2025-12-15".into();
    previous_period.bitisTarihi = "2026-01-14".into();
    previous_period.taxYear = 2026;
    previous_period.taxMonth = 1;
    previous_period.donemAdi = "Audit December".into();

    let mut previous_settings = previous_request
        .dataset
        .institutionSettings
        .remove("audit-jan")
        .expect("previous settings");
    previous_settings.donemId = "audit-dec".into();
    previous_request
        .dataset
        .institutionSettings
        .insert("audit-dec".into(), previous_settings);
    let previous_attendance = &mut previous_request.dataset.attendances[0];
    previous_attendance.id = "audit-dec-attendance".into();
    previous_attendance.donemId = "audit-dec".into();
    previous_attendance.gunler.clear();
    let mut date = NaiveDate::from_ymd_opt(2025, 12, 15).unwrap();
    while date <= NaiveDate::from_ymd_opt(2026, 1, 14).unwrap() {
        previous_attendance
            .gunler
            .insert(date.to_string(), "Ç".into());
        date += chrono::Duration::days(1);
    }
    previous_request.manualIncome = Some(ManualPayrollIncomeInput {
        tediye: Some(dec!(200000)),
        ..ManualPayrollIncomeInput::default()
    });
    let previous = calculate_payroll_checked(&previous_request).expect("normal prior event");

    let mut current_request = audit_support::request();
    current_request
        .dataset
        .periods
        .push(previous_request.dataset.periods[0].clone());
    current_request.dataset.institutionSettings.insert(
        "audit-dec".into(),
        previous_request
            .dataset
            .institutionSettings
            .get("audit-dec")
            .unwrap()
            .clone(),
    );
    for value in current_request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    let mut current_date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    for _ in 0..10 {
        current_request.dataset.attendances[0]
            .gunler
            .insert(current_date.to_string(), "Ç".into());
        current_date += chrono::Duration::days(1);
    }
    current_request.dataset.payrolls.push(previous);

    let current = calculate_payroll(&current_request).expect("current partial normal event");
    let pek = current.pekDetay.as_ref().expect("current PEK detail");
    assert_eq!(pek.aylikOncekiPekTuketimi, Some(dec!(262000)));
    assert_eq!(pek.aylikSonrasiPekTuketimi, Some(dec!(262000)));
    assert_eq!(pek.primMatrahi, Decimal::ZERO);
    assert_eq!(current.sonrakiDevredenPek, Some(Vec::new()));
}

#[test]
fn legacy_supplementary_snapshots_preserve_wage_and_net_meal_components() {
    let mut first_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    first_request.dataset.attendances.clear();
    first_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(200000));
    let mut first = calculate_payroll(&first_request).expect("first legacy payment source");

    let mut second_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TIS_IKRAMIYE);
    second_request.dataset.attendances.clear();
    second_request.accrual.as_mut().unwrap().accrualId = "audit-extra-2".into();
    second_request.accrual.as_mut().unwrap().sequence = 2;
    second_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(10000));
    second_request.dataset.payrolls.push(first.clone());
    let mut second = calculate_payroll(&second_request).expect("second legacy payment source");

    // These fields model an imported pre-snapshot supplementary row.  The
    // current engine deliberately keeps the persisted payment event immutable
    // and reconciles its canonical wage/non-wage components at the public
    // calculation boundary.
    first.statutorySnapshot = None;
    first.gelirler.tabanBrutAylik = Some(dec!(5000));
    first.gelirler.yemek = Some(dec!(6000));
    first.gelirToplam += dec!(11000);
    first.netOdeme = first.gelirToplam - first.kesintiToplam;
    let first_pek = first.pekDetay.as_mut().unwrap();
    first_pek.aylikOncekiPekTuketimi = None;
    first_pek.aylikSonrasiPekTuketimi = None;
    first_pek.yemekIstisnasiTutar = dec!(3000);

    second.statutorySnapshot = None;
    second.gelirler.tabanBrutAylik = Some(dec!(7000));
    second.gelirToplam += dec!(7000);
    second.netOdeme = second.gelirToplam - second.kesintiToplam;
    let second_pek = second.pekDetay.as_mut().unwrap();
    second_pek.aylikOncekiPekTuketimi = None;
    second_pek.aylikSonrasiPekTuketimi = None;

    let mut normal_request = audit_support::request();
    for value in normal_request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    for _ in 0..10 {
        normal_request.dataset.attendances[0]
            .gunler
            .insert(date.to_string(), "Ç".into());
        date += chrono::Duration::days(1);
    }
    normal_request.dataset.payrolls.extend([first, second]);

    let normal = calculate_payroll(&normal_request).expect("normal payment with legacy sources");
    let pek = normal.pekDetay.as_ref().expect("normal PEK detail");
    assert_eq!(pek.aylikOncekiPekTuketimi, Some(dec!(15000)));
    assert_eq!(pek.aylikSonrasiPekTuketimi, Some(dec!(99090)));
    assert_eq!(pek.primMatrahi, dec!(20000));
    let outgoing = normal
        .sonrakiDevredenPek
        .as_ref()
        .expect("legacy non-wage carry");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].tutar, dec!(145910));
    assert_eq!(outgoing[0].kalanAySayisi, 2);
}

#[test]
fn legacy_wage_snapshot_handles_zero_and_non_wage_capacity_edges() {
    let mut prior_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    prior_request.dataset.attendances.clear();
    prior_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(200000));
    let mut prior = calculate_payroll(&prior_request).expect("legacy wage source");
    prior.statutorySnapshot = None;
    prior.gelirler.tediye = None;
    prior.gelirler.tabanBrutAylik = Some(dec!(200000));

    let mut zero_meal_request = audit_support::request();
    for value in zero_meal_request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    for _ in 0..10 {
        zero_meal_request.dataset.attendances[0]
            .gunler
            .insert(date.to_string(), "Ç".into());
        date += chrono::Duration::days(1);
    }
    zero_meal_request.dataset.payrolls.push(prior.clone());
    let zero_meal = calculate_payroll(&zero_meal_request).expect("zero non-wage capacity edge");
    assert_eq!(
        zero_meal.pekDetay.as_ref().unwrap().aylikOncekiPekTuketimi,
        Some(dec!(99090))
    );
    assert_eq!(zero_meal.sonrakiDevredenPek, Some(Vec::new()));

    let mut non_wage_request = audit_support::request();
    non_wage_request
        .dataset
        .institutionSettings
        .get_mut("audit-jan")
        .unwrap()
        .isPrimiGruplari
        .as_mut()
        .unwrap()
        .first_mut()
        .unwrap()
        .oran = dec!(10);
    for value in non_wage_request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    for _ in 0..10 {
        non_wage_request.dataset.attendances[0]
            .gunler
            .insert(date.to_string(), "Ç".into());
        date += chrono::Duration::days(1);
    }
    non_wage_request.dataset.payrolls.push(prior);
    let non_wage = calculate_payroll(&non_wage_request).expect("current non-wage edge");
    let pek = non_wage.pekDetay.as_ref().expect("non-wage PEK detail");
    assert_eq!(pek.aylikSonrasiPekTuketimi, Some(dec!(99090)));
    let outgoing = non_wage
        .sonrakiDevredenPek
        .as_ref()
        .expect("current meal excess carry");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].tutar, dec!(2000));
    assert_eq!(outgoing[0].kalanAySayisi, 2);
}

#[test]
fn all_unpaid_current_period_keeps_same_month_pek_state_non_negative() {
    let mut prior_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    prior_request.dataset.attendances.clear();
    prior_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(200000));
    let prior = calculate_payroll(&prior_request).expect("prior supplementary payment");

    let mut request = audit_support::request();
    for value in request.dataset.attendances[0].gunler.values_mut() {
        *value = "R".into();
    }
    request.dataset.payrolls.push(prior);

    let payroll = calculate_payroll(&request).expect("all-unpaid current period");
    assert_eq!(
        payroll.statutorySnapshot.as_ref().unwrap().sgkPrimGunSayisi,
        0
    );
    let pek = payroll.pekDetay.as_ref().expect("all-unpaid PEK detail");
    assert_eq!(pek.aylikOncekiPekTuketimi, Some(Decimal::ZERO));
    assert_eq!(pek.aylikSonrasiPekTuketimi, Some(Decimal::ZERO));
}

#[test]
fn skipped_tax_month_expires_a_two_month_pek_carry_exactly() {
    let mut october_request = audit_support::dated_request(2026, 10);
    for month in 1..=9 {
        let historical = audit_support::dated_request(2026, month);
        october_request
            .dataset
            .periods
            .push(historical.dataset.periods[0].clone());
        october_request.dataset.institutionSettings.insert(
            historical.periodId.clone(),
            historical
                .dataset
                .institutionSettings
                .get(&historical.periodId)
                .expect("historical settings")
                .clone(),
        );
    }
    october_request.accrual = Some(PayrollAccrualInput {
        accrualId: "october-carry-source".into(),
        accrualType: AccrualType::SUPPLEMENTAL,
        paymentDate: "2026-10-20".into(),
        sequence: 1,
        grossAmount: Some(dec!(500000)),
        description: None,
    });
    let october = calculate_payroll_checked(&october_request).expect("October carry source");
    assert_eq!(
        october
            .sonrakiDevredenPek
            .as_ref()
            .expect("October carry")
            .first()
            .expect("October carry item")
            .kalanAySayisi,
        2
    );

    let mut december_request = audit_support::dated_request(2026, 12);
    for month in 1..=11 {
        let historical = audit_support::dated_request(2026, month);
        december_request
            .dataset
            .periods
            .push(historical.dataset.periods[0].clone());
        december_request.dataset.institutionSettings.insert(
            historical.periodId.clone(),
            historical
                .dataset
                .institutionSettings
                .get(&historical.periodId)
                .expect("historical settings")
                .clone(),
        );
    }
    december_request
        .dataset
        .institutionSettings
        .get_mut(&december_request.periodId)
        .expect("December settings")
        .gunlukTabanUcret = dec!(9000);
    december_request.dataset.payrolls.push(october);

    let december = calculate_payroll_checked(&december_request)
        .expect("a carry at its exact two-month expiry must remain calculable");
    let pek = december.pekDetay.as_ref().expect("December PEK detail");
    assert_eq!(pek.devredenPekKullanilan, dec!(18270));
    assert_eq!(pek.primMatrahi, dec!(297270));
    assert_eq!(december.sonrakiDevredenPek, Some(Vec::new()));
}

#[test]
fn legacy_stamp_snapshot_reconstructs_used_exemption_from_gross_and_deduction() {
    let mut prior_request =
        audit_support::supplementary(audit_support::request(), AccrualType::TEDIYE);
    prior_request.dataset.attendances.clear();
    prior_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(20000));
    let mut prior = calculate_payroll(&prior_request).expect("legacy stamp source");

    // Simulate a pre-stamp-snapshot row.  The persisted income and stamp
    // deduction are the independent source values used by the migration path.
    prior.damgaDetay = None;
    prior.kesintiler.damgaVergisi = Some(dec!(50));

    let mut request = audit_support::request();
    request.dataset.payrolls.push(prior);
    let current = calculate_payroll(&request).expect("current event after legacy stamp row");
    let stamp = current.damgaDetay.as_ref().expect("current stamp detail");
    assert_eq!(stamp.ayniAyOncekiKullanilanDamgaIstisnasi, dec!(101.80));
    assert_eq!(stamp.uygulananDamgaIstisnasi, dec!(148.90));
    assert_eq!(stamp.kalanDamgaIstisnasi, Decimal::ZERO);
}

#[test]
fn insurance_gv_usage_is_carried_only_from_prior_tax_events() {
    let mut previous_request = audit_support::dated_request(2026, 1);
    previous_request.dataset.personnel[0]
        .kesintiler
        .as_mut()
        .expect("person deductions")
        .gvIndirimleri = Some(GvIndirimGirdileri {
        dogumAskerlikGvIndirimTutar: None,
        hayatSigortasiPrimiTutar: None,
        saglikSigortasiPrimiTutar: Some(dec!(10000)),
    });
    let previous = calculate_payroll(&previous_request).expect("January insurance payroll");
    assert_eq!(
        previous
            .gvDetay
            .as_ref()
            .expect("January GV detail")
            .uygulanabilirSigortaGvIndirimi,
        dec!(9300)
    );

    let mut current_request = audit_support::dated_request(2026, 2);
    current_request.dataset.periods.push(
        previous_request
            .dataset
            .periods
            .first()
            .expect("January period")
            .clone(),
    );
    current_request.dataset.institutionSettings.insert(
        "audit-2026-01".into(),
        previous_request
            .dataset
            .institutionSettings
            .get("audit-2026-01")
            .expect("January settings")
            .clone(),
    );

    // These rows are deliberately outside the current tax chronology.  They
    // must not consume the annual insurance-GV limit of February.
    let mut future_same_year = previous.clone();
    future_same_year.id = "future-same-year".into();
    future_same_year.accrualId = "future-same-year".into();
    future_same_year.donemId = "audit-2026-03".into();
    future_same_year.paymentDate = "2026-03-31".into();
    future_same_year.sequence = 9;
    future_same_year
        .gvDetay
        .as_mut()
        .expect("future GV detail")
        .uygulanabilirSigortaGvIndirimi = dec!(111);
    let mut future_period = current_request
        .dataset
        .periods
        .first()
        .expect("February period")
        .clone();
    future_period.id = "audit-2026-03".into();
    future_period.yil = 2026;
    future_period.ay = 3;
    future_period.baslangicTarihi = "2026-03-15".into();
    future_period.bitisTarihi = "2026-04-14".into();
    future_period.taxYear = 2026;
    future_period.taxMonth = 3;
    current_request.dataset.periods.push(future_period);
    let mut previous_year = previous.clone();
    previous_year.id = "previous-year".into();
    previous_year.accrualId = "previous-year".into();
    previous_year.donemId = "audit-2025-01".into();
    previous_year.paymentDate = "2025-01-31".into();
    let mut previous_year_period = previous_request
        .dataset
        .periods
        .first()
        .expect("January period")
        .clone();
    previous_year_period.id = "audit-2025-01".into();
    previous_year_period.yil = 2025;
    previous_year_period.ay = 1;
    previous_year_period.baslangicTarihi = "2025-01-15".into();
    previous_year_period.bitisTarihi = "2025-02-14".into();
    previous_year_period.taxYear = 2025;
    previous_year_period.taxMonth = 1;
    current_request.dataset.periods.push(previous_year_period);
    current_request
        .dataset
        .payrolls
        .extend([previous.clone(), future_same_year, previous_year]);

    let mut same_month_earlier = previous.clone();
    same_month_earlier.id = "same-month-earlier".into();
    same_month_earlier.accrualId = "same-month-earlier".into();
    same_month_earlier.accrualType = AccrualType::TEDIYE;
    same_month_earlier.donemId = current_request.periodId.clone();
    same_month_earlier.paymentDate = "2026-02-27".into();
    same_month_earlier.sequence = 9;
    same_month_earlier
        .gvDetay
        .as_mut()
        .expect("earlier GV detail")
        .uygulanabilirSigortaGvIndirimi = dec!(444);
    same_month_earlier
        .gvDetay
        .as_mut()
        .expect("earlier GV detail")
        .uygulananGvIstisnasi = Decimal::ZERO;
    same_month_earlier
        .pekDetay
        .as_mut()
        .expect("earlier PEK detail")
        .aylikSonrasiPekTuketimi = Some(Decimal::ZERO);
    same_month_earlier
        .damgaDetay
        .as_mut()
        .expect("earlier stamp detail")
        .uygulananDamgaIstisnasi = Decimal::ZERO;

    let mut same_month_later = previous;
    same_month_later.id = "same-month-later".into();
    same_month_later.accrualId = "same-month-later".into();
    same_month_later.accrualType = AccrualType::TEDIYE;
    same_month_later.donemId = current_request.periodId.clone();
    same_month_later.paymentDate = "2026-02-28".into();
    same_month_later.sequence = 9;
    same_month_later
        .gvDetay
        .as_mut()
        .expect("later GV detail")
        .uygulanabilirSigortaGvIndirimi = dec!(333);
    same_month_later
        .gvDetay
        .as_mut()
        .expect("later GV detail")
        .uygulananGvIstisnasi = Decimal::ZERO;
    same_month_later
        .pekDetay
        .as_mut()
        .expect("later PEK detail")
        .aylikSonrasiPekTuketimi = Some(Decimal::ZERO);
    same_month_later
        .damgaDetay
        .as_mut()
        .expect("later stamp detail")
        .uygulananDamgaIstisnasi = Decimal::ZERO;
    current_request.accrual = Some(PayrollAccrualInput {
        accrualId: "current-february".into(),
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-02-28".into(),
        sequence: 0,
        grossAmount: None,
        description: None,
    });
    current_request
        .dataset
        .payrolls
        .extend([same_month_earlier, same_month_later]);

    let current = calculate_payroll(&current_request).expect("February insurance payroll");
    let current_gv = current.gvDetay.as_ref().expect("February GV detail");
    assert_eq!(current_gv.sigortaGvYillikKalanLimiti, dec!(386616));
    assert_eq!(current_gv.uygulanabilirSigortaGvIndirimi, Decimal::ZERO);
}
