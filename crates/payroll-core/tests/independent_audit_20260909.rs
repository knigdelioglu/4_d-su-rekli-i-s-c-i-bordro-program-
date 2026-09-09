use payroll_core::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
#[path = "support/audit_fixture.rs"]
mod audit_fixture;
use audit_fixture::request;

fn export_wasm_case(name: &str, request: &PayrollCalculationRequest, result: &BordroKaydi) {
    if let Ok(directory) = std::env::var("PAYROLL_AUDIT_FIXTURES") {
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_vec(&json!({"request":request, "expected":result})).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn audit_retro_meal_must_not_reuse_payment_month_meal_exemption() {
    let mut req = request();
    let original = calculate_payroll_checked(&req).unwrap();
    req.dataset.payrolls.push(original);
    req.periodId = "payment".into();
    let normal = calculate_payroll_checked(&req).unwrap();
    req.dataset.payrolls.push(normal);
    let revision: CompensationRevision = serde_json::from_value(json!({
        "id":"meal-revision", "reason":"COLLECTIVE_AGREEMENT", "title":"Meal difference",
        "effectiveFrom":"2025-12-15", "effectiveTo":"2026-01-14", "scope":"ALL_PERSONNEL"
    }))
    .unwrap();
    let result = RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: "meal-retro".into(),
        revision,
        overrides: vec![CompensationRevisionOverride {
            id: "meal-rate".into(),
            revisionId: "meal-revision".into(),
            parameter: RetroParameterKey::GUNLUK_YEMEK,
            value: dec!(400),
            personnelId: None,
        }],
        personnelId: "audit".into(),
        paymentDate: "2026-02-20".into(),
        calculatedAt: req.calculatedAt.clone(),
        description: None,
        dataset: req.dataset.clone(),
    })
    .unwrap();
    assert_eq!(result.batch.totalGrossDelta, dec!(3100));
    assert!(result
        .allocations
        .iter()
        .all(|a| a.incomeTaxTreatment == RetroTaxTreatment::TAXABLE));
    req.dataset.retroBatches.push(result.batch);
    req.dataset.retroAllocations = result.allocations;
    req.accrual = Some(PayrollAccrualInput {
        accrualId: "meal-retro".into(),
        accrualType: AccrualType::RETRO_ADJUSTMENT,
        paymentDate: "2026-02-20".into(),
        sequence: 1,
        grossAmount: None,
        description: None,
    });
    let payment = calculate_payroll_checked(&req).unwrap();
    export_wasm_case("retro-meal", &req, &payment);
    // Source meal already exhausted its exemption: extra 3,100 less 434 SGK and 31 unemployment.
    assert_eq!(payment.gvDetay.unwrap().cariGvMatrahi, dec!(2635));
    assert_eq!(payment.kesintiler.gelirVergisi, Some(dec!(395.25)));
}

#[test]
fn audit_minimum_reference_uses_separately_rounded_worker_premiums() {
    // 30 * 1101.01 = 33030.30; SGK 4624.24 + unemployment 330.30.
    assert_eq!(
        calculate_aylik_asgari_ucret_gv_matrahi(dec!(1101.01), dec!(0.14), dec!(0.01)),
        dec!(28075.76)
    );
}

#[test]
fn audit_progressive_tax_independent_boundary_oracle() {
    let brackets = vec![
        TaxBracket {
            limit: dec!(100),
            oran: dec!(0.15),
        },
        TaxBracket {
            limit: dec!(200),
            oran: dec!(0.20),
        },
        TaxBracket {
            limit: dec!(300),
            oran: dec!(0.30),
        },
    ];
    for previous in 0..=350 {
        for current in [0, 1, 50, 100, 250] {
            let mut expected = Decimal::ZERO;
            for unit in previous..previous + current {
                expected += if unit < 100 {
                    dec!(0.15)
                } else if unit < 200 {
                    dec!(0.20)
                } else {
                    dec!(0.30)
                };
            }
            assert_eq!(
                calculate_gelir_vergisi_with_brackets(
                    Decimal::from(current),
                    Decimal::from(previous),
                    &brackets
                ),
                expected
            );
        }
    }
}

#[test]
fn audit_monthly_exemption_and_cumulative_event_chain() {
    for kind in [
        AccrualType::TEDIYE,
        AccrualType::TIS_IKRAMIYE,
        AccrualType::SUPPLEMENTAL,
    ] {
        let mut req = request();
        let normal = calculate_payroll_checked(&req).unwrap();
        let base = normal.gvDetay.as_ref().unwrap().cariGvMatrahi;
        let exemption = normal.gvDetay.as_ref().unwrap().uygulananGvIstisnasi;
        req.dataset.payrolls.push(normal);
        req.accrual = Some(PayrollAccrualInput {
            accrualId: "extra".into(),
            accrualType: kind,
            paymentDate: "2026-01-20".into(),
            sequence: 1,
            grossAmount: Some(dec!(10000)),
            description: None,
        });
        let extra = calculate_payroll_checked(&req).unwrap();
        let detail = extra.gvDetay.unwrap();
        assert_eq!(detail.oncekiKumulatifGvMatrahi, base);
        assert_eq!(detail.cariGvMatrahi, dec!(8500));
        assert_eq!(
            detail.uygulananGvIstisnasi + exemption,
            detail.asgariUcretGvIstisnasi
        );
        assert_eq!(
            extra.damgaDetay.unwrap().uygulananDamgaIstisnasi,
            Decimal::ZERO
        );
        assert_eq!(extra.netOdeme, extra.gelirToplam - extra.kesintiToplam);
    }
}

#[test]
fn audit_pek_capacity_conservation_matrix() {
    let settings = DonemselKurumDegerleri {
        gunlukAsgariUcret: Some(dec!(1000)),
        pekTavanKatsayisi: Some(dec!(2)),
        ..Default::default()
    };
    for days in [0, 1, 15, 30] {
        for wage in [0, 500, 30000, 70000] {
            for bonus in [0, 1000, 80000] {
                let income = GelirKalemleri {
                    tabanBrutAylik: Some(Decimal::from(wage)),
                    tediye: Some(Decimal::from(bonus)),
                    ..Default::default()
                };
                let (pek, carry) = calculate_prime_esas_kazanc(
                    &income,
                    Some(&PuantajOzeti {
                        c: days,
                        ..Default::default()
                    }),
                    Some(&settings),
                    &[],
                );
                let capacity = Decimal::from(days * 2000);
                assert_eq!(pek.primMatrahi, Decimal::from(wage + bonus).min(capacity));
                assert_eq!(
                    carry.iter().map(|c| c.tutar).sum::<Decimal>(),
                    (Decimal::from(bonus) - (capacity - Decimal::from(wage)).max(Decimal::ZERO))
                        .max(Decimal::ZERO)
                );
                assert!(pek.finalPek <= capacity);
                assert_eq!(pek.finalPek, pek.primMatrahi + pek.altSinirTamamlamaFarki);
            }
        }
    }
}

#[test]
fn audit_opening_zero_nonzero_and_stale_history() {
    for opening in [dec!(0), dec!(189999)] {
        let mut req = request();
        req.dataset.taxOpenings.push(PersonelTaxOpening {
            id: "opening".into(),
            personnelId: "audit".into(),
            year: 2026,
            gvCumulativeOpening: Some(opening),
            effectiveFromPeriodId: Some("source".into()),
            asgariGvCumulativeOpening: Some(dec!(0)),
            asgariGvEffectiveFromPeriodId: Some("source".into()),
            createdAt: None,
            updatedAt: None,
        });
        let mut first = calculate_payroll_checked(&req).unwrap();
        assert_eq!(
            first.gvDetay.as_ref().unwrap().oncekiKumulatifGvMatrahi,
            opening
        );
        first.status = BordroStatus::STALE;
        req.dataset.payrolls.push(first);
        req.periodId = "payment".into();
        assert!(calculate_payroll_checked(&req).is_err());
    }
}

#[test]
fn audit_full_calendar_and_one_unpaid_day_at_year_boundary() {
    let mut req = request();
    let full = calculate_payroll_checked(&req).unwrap();
    assert_eq!(full.statutorySnapshot.unwrap().sgkPrimGunSayisi, 30);
    // December 15–January 14 contains 31 actual wage days; SGK normalizes full attendance to 30.
    assert_eq!(full.gelirler.tabanBrutAylik, Some(dec!(62000)));
    req.dataset.attendances[0]
        .gunler
        .insert("2025-12-31".into(), "R".into());
    let reduced = calculate_payroll_checked(&req).unwrap();
    assert_eq!(reduced.gelirler.tabanBrutAylik, Some(dec!(60000)));
    assert_eq!(reduced.statutorySnapshot.unwrap().sgkPrimGunSayisi, 30);
}

#[test]
fn audit_new_tax_year_resets_gv_but_preserves_pek_carry() {
    let mut req = request();
    req.dataset.periods[0].taxYear = 2025;
    req.dataset.periods[0].taxMonth = 12;
    req.dataset.periods[1].taxMonth = 1;
    let mut previous_year = AnnualPayrollParameters::default_for_2026();
    previous_year.year = 2025;
    req.dataset.annualPayrollParameters.push(previous_year);
    req.dataset.taxOpenings.push(PersonelTaxOpening {
        id: "2025-opening".into(),
        personnelId: "audit".into(),
        year: 2025,
        gvCumulativeOpening: Some(dec!(200000)),
        effectiveFromPeriodId: Some("source".into()),
        asgariGvCumulativeOpening: Some(dec!(200000)),
        asgariGvEffectiveFromPeriodId: Some("source".into()),
        createdAt: None,
        updatedAt: None,
    });
    req.manualIncome = Some(ManualPayrollIncomeInput {
        tediye: Some(dec!(400000)),
        tisIkramiyesi: None,
    });
    let previous = calculate_payroll_checked(&req).unwrap();
    let outgoing = previous.sonrakiDevredenPek.clone().unwrap();
    assert!(!outgoing.is_empty());
    req.dataset.payrolls.push(previous);
    req.periodId = "payment".into();
    req.manualIncome = None;
    let current = calculate_payroll_checked(&req).unwrap();
    assert_eq!(
        current.gvDetay.unwrap().oncekiKumulatifGvMatrahi,
        Decimal::ZERO
    );
    assert_eq!(current.oncekiKumulatifAsgariGvMatrahi, Some(Decimal::ZERO));
    assert_eq!(current.devredenPekGelen.unwrap(), outgoing);
    assert!(current.pekDetay.unwrap().devredenPekKullanilan > Decimal::ZERO);
}

#[test]
fn audit_paid_sick_episode_cross_year_does_not_duplicate_or_restart() {
    let req = request();
    let record: SickLeaveRecord = serde_json::from_value(json!({
        "id":"r", "personnelId":"audit", "startDate":"2025-12-31", "endDate":"2026-01-03"
    }))
    .unwrap();
    let mut records = vec![record.clone(), record];
    let days = calculate_paid_sick_dates_from_records(&records, &req.dataset.periods[0]).unwrap();
    assert_eq!(
        days.iter().map(ToString::to_string).collect::<Vec<_>>(),
        vec!["2025-12-31", "2026-01-01"]
    );
    // Five earlier episodes exhaust the code's contractual first-five quota in 2025.
    for day in [1, 4, 7, 10, 13] {
        records.push(serde_json::from_value(json!({"id":format!("r{day}"), "personnelId":"audit", "startDate":format!("2025-12-{day:02}"), "endDate":format!("2025-12-{day:02}")})).unwrap());
    }
    assert!(
        calculate_paid_sick_dates_from_records(&records, &req.dataset.periods[0])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn audit_serialized_calculation_is_exact_and_repeatable() {
    let req = request();
    let expected = calculate_payroll_checked(&req).unwrap();
    let encoded = serde_json::to_string(&req).unwrap();
    let restored: PayrollCalculationRequest = serde_json::from_str(&encoded).unwrap();
    let actual = calculate_payroll_checked(&restored).unwrap();
    export_wasm_case("normal", &restored, &actual);
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn audit_mixed_income_personal_deductions_and_tax_discount_oracle() {
    let mut req = request();
    req.dataset.personnel[0].hizmetYili = 3;
    req.dataset.personnel[0].kesintiler = Some(serde_json::from_value(json!({
        "sendikaUyesi":true, "sabitSendikaAidati":"1000", "besUyesi":true,
        "oksOraniYuzde":"3", "icraTutar":"500", "kisiBorcuTutar":"200",
        "gvIndirimleri":{"dogumAskerlikGvIndirimTutar":"100", "hayatSigortasiPrimiTutar":"1000", "saglikSigortasiPrimiTutar":"500"}
    })).unwrap());
    let settings = req.dataset.institutionSettings.get_mut("source").unwrap();
    settings.gunlukYemek = dec!(400);
    settings.gunlukVasitaYol = dec!(100);
    settings.birlestirilmisSosyalYardim = dec!(5000);
    settings.giyimYardimi = dec!(200);
    settings.hizmetZammiBirimi = dec!(10);
    settings.ekOdeme = Some(dec!(1000));
    settings.digerGelirVarsayilan = Some(dec!(500));
    settings.geceCalismaPrimiYuzde = Some(dec!(25));
    settings.geceCalismaTatiliPrimiYuzde = Some(dec!(50));
    for (date, code) in [
        ("2025-12-15", "GÇ"),
        ("2025-12-16", "GÇT"),
        ("2025-12-17", "İ"),
        ("2025-12-18", "R"),
    ] {
        req.dataset.attendances[0]
            .gunler
            .insert(date.into(), code.into());
    }
    let result = calculate_payroll_checked(&req).unwrap();
    assert_eq!(result.gelirToplam, dec!(87270));
    assert_eq!(result.pekDetay.unwrap().primMatrahi, dec!(78870));
    assert_eq!(result.kesintiler.isciSgkPrimi, Some(dec!(11041.8)));
    assert_eq!(result.kesintiler.isciIssizlikPrimi, Some(dec!(788.7)));
    assert_eq!(result.kesintiler.bes, Some(dec!(2366)));
    assert_eq!(result.gvDetay.unwrap().cariGvMatrahi, dec!(64939.5));
    // 87270 - 8400 meal - 11830.5 worker - 1000 union - 100 military - 1000 insurance.
    assert_eq!(result.kesintiler.gelirVergisi, Some(dec!(5529.60)));
    assert_eq!(result.netOdeme, result.gelirToplam - result.kesintiToplam);
}
