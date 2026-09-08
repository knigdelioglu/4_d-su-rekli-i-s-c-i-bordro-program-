#[path = "support/independent_audit.rs"]
mod support;
use payroll_core::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[test]
fn audit_meal_is_exempt_from_stamp_as_well_as_income_tax() {
    let result = calculate_payroll_checked(&support::request()).unwrap();
    assert_eq!(result.gelirToplam, dec!(68200));
    assert_eq!(result.pekDetay.as_ref().unwrap().primMatrahi, dec!(62000));
    assert_eq!(result.gvDetay.as_ref().unwrap().cariGvMatrahi, dec!(52700));
    // 62,000 * .00759 - round(33,030 * .00759) = 219.88.
    assert_eq!(result.kesintiler.damgaVergisi, Some(dec!(219.88)));
}

#[test]
fn audit_zero_personal_fixed_bes_means_percentage_on_every_extra_kind() {
    for kind in [
        AccrualType::TEDIYE,
        AccrualType::TIS_IKRAMIYE,
        AccrualType::SUPPLEMENTAL,
    ] {
        let mut req = support::supplementary(support::request(), kind);
        req.dataset.personnel[0]
            .kesintiler
            .as_mut()
            .unwrap()
            .sabitBesTutar = Some(dec!(0));
        assert_eq!(
            calculate_payroll_checked(&req).unwrap().kesintiler.bes,
            Some(dec!(300)),
            "{kind:?}"
        );
    }
}

#[test]
fn audit_institution_fixed_bes_is_not_charged_again_as_percentage() {
    let mut req = support::supplementary(support::request(), AccrualType::TEDIYE);
    req.dataset
        .institutionSettings
        .get_mut(&req.periodId)
        .unwrap()
        .sabitBesTutar = Some(dec!(750));
    assert_eq!(
        calculate_payroll_checked(&req)
            .unwrap()
            .kesintiler
            .bes
            .unwrap_or_default(),
        dec!(0)
    );
}

#[test]
fn audit_invalid_tariffs_fail_at_calculation_boundary() {
    for brackets in [
        vec![],
        vec![TaxBracket {
            limit: dec!(100),
            oran: dec!(-0.15),
        }],
        vec![
            TaxBracket {
                limit: dec!(100),
                oran: dec!(0.15),
            },
            TaxBracket {
                limit: dec!(50),
                oran: dec!(0.2),
            },
        ],
    ] {
        let mut req = support::request();
        req.dataset.annualPayrollParameters[0].gelirVergisiDilimleri = brackets;
        assert!(
            calculate_payroll_checked(&req).is_err(),
            "invalid tariff accepted"
        );
    }
}

#[test]
fn audit_negative_personal_bes_rate_cannot_increase_net_payment() {
    let mut req = support::request();
    req.dataset.personnel[0]
        .kesintiler
        .as_mut()
        .unwrap()
        .oksOraniYuzde = Some(dec!(-3));
    assert!(calculate_payroll_checked(&req).is_err());
}

#[test]
fn audit_calendar_and_earning_formula_matrix() {
    for (code, meal_days, premium_days) in [
        ("Ç", 31, 30),
        ("GÇ", 31, 30),
        ("T", 0, 30),
        ("İ", 0, 30),
        ("G", 0, 30),
        ("GÇT", 0, 30),
    ] {
        let mut req = support::request();
        for value in req.dataset.attendances[0].gunler.values_mut() {
            *value = code.into();
        }
        let p = calculate_payroll_checked(&req).unwrap();
        assert_eq!(p.gelirler.tabanBrutAylik, Some(dec!(62000)));
        assert_eq!(p.gelirler.yemek, Some(Decimal::from(meal_days) * dec!(200)));
        assert_eq!(p.statutorySnapshot.unwrap().sgkPrimGunSayisi, premium_days);
        assert_eq!(p.netOdeme, p.gelirToplam - p.kesintiToplam);
    }
}

#[test]
fn audit_progressive_tax_against_independent_piecewise_oracle() {
    // Independently precomputed tax due at each wage tariff boundary.
    for (base, tax) in [
        (dec!(190000), dec!(28500)),
        (dec!(400000), dec!(70500)),
        (dec!(1500000), dec!(367500)),
        (dec!(5300000), dec!(1697500)),
        (dec!(5300100), dec!(1697540)),
    ] {
        assert_eq!(calculate_total_tax_for_cumulative_matrah(base), tax);
    }
}

#[test]
fn audit_twelve_month_cumulatives_and_year_reset() {
    let mut history = PayrollDatasetSnapshot::default();
    let mut expected_cumulative = dec!(0);
    for year in [2026, 2027] {
        if year == 2027 {
            expected_cumulative = dec!(0);
        }
        for month in 1..=12 {
            let mut req = support::dated_request(year, month);
            history.periods.extend(req.dataset.periods.clone());
            history
                .institutionSettings
                .extend(req.dataset.institutionSettings.clone());
            req.dataset.periods = history.periods.clone();
            req.dataset.institutionSettings = history.institutionSettings.clone();
            req.dataset.payrolls = history.payrolls.clone();
            let p = calculate_payroll_checked(&req).unwrap();
            let days = p.puantajOzeti.c;
            let base = Decimal::from(days) * dec!(2000);
            let expected_base =
                base - (base * dec!(0.14)).round_dp(2) - (base * dec!(0.01)).round_dp(2);
            let g = p.gvDetay.as_ref().unwrap();
            assert_eq!(
                g.oncekiKumulatifGvMatrahi, expected_cumulative,
                "{year}-{month}"
            );
            assert_eq!(g.cariGvMatrahi, expected_base);
            assert_eq!(
                p.oncekiKumulatifAsgariGvMatrahi,
                Some(dec!(28075.50) * Decimal::from(month - 1))
            );
            assert_eq!(p.statutorySnapshot.as_ref().unwrap().sgkPrimGunSayisi, 30);
            expected_cumulative += expected_base;
            history.payrolls.push(p);
        }
    }
}

#[test]
fn audit_opening_components_and_supplementary_exemption_budget() {
    let mut req = support::dated_request(2026, 7);
    req.dataset.taxOpenings.push(PersonelTaxOpening {
        id: "opening".into(),
        personnelId: req.personnelId.clone(),
        year: 2026,
        gvCumulativeOpening: Some(dec!(189000)),
        effectiveFromPeriodId: Some(req.periodId.clone()),
        asgariGvCumulativeOpening: Some(dec!(168453)),
        asgariGvEffectiveFromPeriodId: Some(req.periodId.clone()),
        createdAt: None,
        updatedAt: None,
    });
    let normal = calculate_payroll_checked(&req).unwrap();
    assert_eq!(normal.oncekiKumulatifGvMatrahi, Some(dec!(189000)));
    assert_eq!(normal.oncekiKumulatifAsgariGvMatrahi, Some(dec!(168453)));
    req.dataset.payrolls.push(normal.clone());
    req.accrual = Some(PayrollAccrualInput {
        accrualId: "july-extra".into(),
        accrualType: AccrualType::TIS_IKRAMIYE,
        paymentDate: "2026-07-31".into(),
        sequence: 1,
        grossAmount: Some(dec!(10000)),
        description: None,
    });
    let extra = calculate_payroll_checked(&req).unwrap();
    assert_eq!(
        extra.gvDetay.as_ref().unwrap().uygulananGvIstisnasi,
        dec!(0)
    );
    assert_eq!(
        extra.damgaDetay.as_ref().unwrap().uygulananDamgaIstisnasi,
        dec!(0)
    );
    assert_eq!(extra.kesintiler.gelirVergisi, Some(dec!(1700)));
    assert_eq!(extra.kesintiler.damgaVergisi, Some(dec!(75.90)));
}

#[test]
fn audit_pek_ceiling_and_nonwage_carry_conservation() {
    for excess in [dec!(0.01), dec!(10000), dec!(200000)] {
        let mut req = support::request();
        let normal = calculate_payroll_checked(&req).unwrap();
        let normal_pek = normal.pekDetay.as_ref().unwrap().primMatrahi;
        req.dataset.payrolls.push(normal);
        req = support::supplementary(req, AccrualType::TEDIYE);
        req.accrual.as_mut().unwrap().grossAmount = Some(dec!(297270) - normal_pek + excess);
        let p = calculate_payroll_checked(&req).unwrap();
        assert_eq!(
            p.pekDetay.as_ref().unwrap().primMatrahi + normal_pek,
            dec!(297270)
        );
        assert_eq!(
            p.sonrakiDevredenPek
                .unwrap()
                .iter()
                .map(|c| c.tutar)
                .sum::<Decimal>(),
            excess
        );
    }
}

#[test]
fn audit_retro_source_wage_premiums_and_oks_reach_payment() {
    let mut source = support::request();
    source
        .dataset
        .institutionSettings
        .get_mut(&source.periodId)
        .unwrap()
        .gunlukYemek = dec!(0);
    let original = calculate_payroll_checked(&source).unwrap();
    let mut req = support::dated_request(2026, 2);
    req.dataset.periods.extend(source.dataset.periods.clone());
    req.dataset
        .institutionSettings
        .extend(source.dataset.institutionSettings.clone());
    req.dataset
        .attendances
        .extend(source.dataset.attendances.clone());
    req.dataset.payrolls.push(original);
    let revision: CompensationRevision = serde_json::from_value(serde_json::json!({
        "id":"audit-revision","reason":"COLLECTIVE_AGREEMENT","title":"Audit wage revision",
        "effectiveFrom":"2026-01-15","effectiveTo":"2026-02-14","status":"DRAFT",
        "scope":"SELECTED_PERSONNEL","personnelIds":["audit"]
    }))
    .unwrap();
    let retro = RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: "audit-retro".into(),
        revision,
        overrides: vec![CompensationRevisionOverride {
            id: "audit-override".into(),
            revisionId: "audit-revision".into(),
            parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
            value: dec!(2200),
            personnelId: None,
        }],
        personnelId: "audit".into(),
        paymentDate: "2026-02-28".into(),
        calculatedAt: req.calculatedAt.clone(),
        description: None,
        dataset: req.dataset.clone(),
    })
    .unwrap();
    assert_eq!(
        retro
            .allocations
            .iter()
            .map(|a| a.deltaAmount)
            .sum::<Decimal>(),
        dec!(6200)
    );
    assert_eq!(
        retro
            .allocations
            .iter()
            .map(|a| a.workerSgkDelta)
            .sum::<Decimal>(),
        dec!(868)
    );
    req.dataset.retroBatches.push(retro.batch);
    req.dataset.retroAllocations.extend(retro.allocations);
    req.accrual = Some(PayrollAccrualInput {
        accrualId: "audit-retro".into(),
        accrualType: AccrualType::RETRO_ADJUSTMENT,
        paymentDate: "2026-02-28".into(),
        sequence: 0,
        grossAmount: None,
        description: None,
    });
    let p = calculate_payroll_checked(&req).unwrap();
    assert_eq!(p.gelirToplam, dec!(6200));
    assert_eq!(
        p.pekDetay.as_ref().unwrap().primMatrahi,
        dec!(0),
        "source wages must not consume payment-month PEK"
    );
    assert_eq!(p.kesintiler.isciSgkPrimi, Some(dec!(868)));
    assert_eq!(p.kesintiler.isciIssizlikPrimi, Some(dec!(62)));
    assert_eq!(
        p.kesintiler.bes,
        Some(dec!(186)),
        "wage differences require OKS too"
    );
}

#[test]
fn audit_retro_noop_after_split_raise_does_not_create_phantom_wages() {
    let mut source = support::request();
    let mut previous = support::dated_request(2025, 12);
    previous
        .dataset
        .institutionSettings
        .values_mut()
        .next()
        .unwrap()
        .gunlukTabanUcret = dec!(1000);
    source.dataset.periods.extend(previous.dataset.periods);
    source
        .dataset
        .institutionSettings
        .extend(previous.dataset.institutionSettings);
    source.dataset.zamAylari = vec![2];
    let original = calculate_payroll_checked(&source).unwrap();
    assert_eq!(original.gelirler.tabanBrutAylik, Some(dec!(45000)));
    source.dataset.payrolls.push(original);
    let revision: CompensationRevision = serde_json::from_value(serde_json::json!({
        "id":"noop-revision","reason":"COLLECTIVE_AGREEMENT","title":"No change to meal",
        "effectiveFrom":"2026-01-15","effectiveTo":"2026-02-14","status":"DRAFT",
        "scope":"SELECTED_PERSONNEL","personnelIds":["audit"]
    }))
    .unwrap();
    let result = RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: "noop-retro".into(),
        revision,
        overrides: vec![CompensationRevisionOverride {
            id: "noop-override".into(),
            revisionId: "noop-revision".into(),
            parameter: RetroParameterKey::GUNLUK_YEMEK,
            value: dec!(200),
            personnelId: None,
        }],
        personnelId: "audit".into(),
        paymentDate: "2026-02-28".into(),
        calculatedAt: source.calculatedAt.clone(),
        description: None,
        dataset: source.dataset,
    })
    .unwrap();
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|a| a.deltaAmount)
            .sum::<Decimal>(),
        dec!(0),
        "unchanged terms must not produce wage differences"
    );
}

#[test]
fn audit_unused_insurance_deduction_does_not_consume_annual_cap() {
    let mut req = support::request();
    req.dataset.personnel[0]
        .kesintiler
        .as_mut()
        .unwrap()
        .gvIndirimleri = Some(GvIndirimGirdileri {
        dogumAskerlikGvIndirimTutar: Some(dec!(52700)),
        hayatSigortasiPrimiTutar: None,
        saglikSigortasiPrimiTutar: Some(dec!(10000)),
    });
    let p = calculate_payroll_checked(&req).unwrap();
    let g = p.gvDetay.unwrap();
    assert_eq!(g.cariGvMatrahi, dec!(0));
    assert_eq!(g.sigortaGvIndirimAdayi, dec!(10000));
    assert_eq!(
        g.uygulanabilirSigortaGvIndirimi,
        dec!(0),
        "no tax base remained to absorb insurance"
    );
}

#[test]
fn audit_sick_leave_quota_dedup_and_unpaid_days() {
    let mut req = support::request();
    // Sixth episode in the calendar year: the first two days are no longer paid.
    for (i, day) in [1, 3, 5, 7, 9, 20].into_iter().enumerate() {
        let date = format!("2026-01-{day:02}");
        req.dataset.sickLeaveRecords.push(SickLeaveRecord {
            id: format!("sick-{i}"),
            personnelId: "audit".into(),
            startDate: date.clone(),
            endDate: date,
            createdAt: None,
            updatedAt: None,
        });
    }
    req.dataset.attendances[0]
        .gunler
        .insert("2026-01-20".into(), "R".into());
    let p = calculate_payroll_checked(&req).unwrap();
    assert_eq!(p.odenenRaporluGun, Some(0));
    assert_eq!(p.gelirler.tabanBrutAylik, Some(dec!(60000)));
    assert_eq!(p.statutorySnapshot.as_ref().unwrap().sgkPrimGunSayisi, 30);
    // Remove the five earlier episodes; identical duplicate episodes must not pay twice.
    req.dataset.sickLeaveRecords.drain(0..5);
    req.dataset
        .sickLeaveRecords
        .push(req.dataset.sickLeaveRecords[0].clone());
    let p = calculate_payroll_checked(&req).unwrap();
    assert_eq!(p.odenenRaporluGun, Some(1));
    assert_eq!(p.gelirler.tabanBrutAylik, Some(dec!(62000)));
    assert_eq!(p.gelirler.yemek, Some(dec!(6000)));
}

#[test]
fn audit_night_and_group_premiums_use_independent_days() {
    let mut req = support::request();
    for value in req.dataset.attendances[0].gunler.values_mut() {
        *value = "T".into();
    }
    req.dataset.attendances[0]
        .gunler
        .insert("2026-01-15".into(), "GÇ".into());
    req.dataset.attendances[0]
        .gunler
        .insert("2026-01-16".into(), "GÇT".into());
    let k = req
        .dataset
        .institutionSettings
        .get_mut(&req.periodId)
        .unwrap();
    k.geceCalismaPrimiYuzde = Some(dec!(20));
    k.geceCalismaTatiliPrimiYuzde = Some(dec!(30));
    k.isPrimiGruplari.as_mut().unwrap()[0].oran = dec!(9);
    let p = calculate_payroll_checked(&req).unwrap();
    assert_eq!(p.gelirler.geceCalismasiUcreti, Some(dec!(400)));
    assert_eq!(p.gelirler.geceCalismasiTatiliUcreti, Some(dec!(600)));
    assert_eq!(p.gelirler.isPrimi, Some(dec!(180)));
    assert_eq!(p.gelirler.yemek, Some(dec!(200)));
}

#[test]
fn audit_request_json_roundtrip_preserves_calculation() {
    let request = support::request();
    let json = serde_json::to_string(&request).unwrap();
    let parsed: PayrollCalculationRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(
        serde_json::to_value(calculate_payroll_checked(&request).unwrap()).unwrap(),
        serde_json::to_value(calculate_payroll_checked(&parsed).unwrap()).unwrap()
    );
    // Optional export lets the WASM audit exercise this exact native fixture.
    if let Ok(path) = std::env::var("PAYROLL_AUDIT_FIXTURE") {
        std::fs::write(path, json).unwrap();
    }
}
