use payroll_core::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[path = "support/audit_fixture.rs"]
mod fixture;

fn dataset(original_meal: Decimal, full_gv: bool) -> PayrollDatasetSnapshot {
    dataset_with_segments(original_meal, full_gv, vec![])
}

fn dataset_with_segments(
    original_meal: Decimal,
    full_gv: bool,
    segments: Vec<StatutoryParameterSegment>,
) -> PayrollDatasetSnapshot {
    let mut request = fixture::request();
    let source = request.periodId.clone();
    let settings = request
        .dataset
        .institutionSettings
        .get_mut(&source)
        .unwrap();
    settings.yemekTamGvIstisnaDamgaTabi = full_gv;
    settings.gunlukYemekIstisnasiGV = Some(dec!(300));
    settings.gunlukYemekIstisnasiSGK = Some(dec!(300));
    settings.statutoryParameterSegments = Some(segments);
    request.dataset.compensationPolicyVersions[0]
        .state
        .gunlukYemek = original_meal;
    for version in &mut request.dataset.compensationPolicyVersions {
        for group in &mut version.state.isPrimiGruplari {
            group.oran = dec!(0);
        }
    }
    let normal = calculate_payroll_checked(&request).unwrap();
    request.dataset.payrolls.push(normal);
    request.periodId = "payment".into();
    let normal = calculate_payroll_checked(&request).unwrap();
    request.dataset.payrolls.push(normal);
    request.dataset
}

fn entitlement(
    dataset: &PayrollDatasetSnapshot,
    id: &str,
    meal: Decimal,
) -> RetroCalculationResult {
    entitlement_with_wage(dataset, id, meal, None)
}

fn entitlement_with_wage(
    dataset: &PayrollDatasetSnapshot,
    id: &str,
    meal: Decimal,
    wage: Option<Decimal>,
) -> RetroCalculationResult {
    let mut overrides = vec![CompensationRevisionOverride {
        id: format!("{id}-meal"),
        revisionId: id.into(),
        parameter: RetroParameterKey::GUNLUK_YEMEK,
        value: meal,
        personnelId: None,
    }];
    if let Some(wage) = wage {
        overrides.push(CompensationRevisionOverride {
            id: format!("{id}-wage"),
            revisionId: id.into(),
            parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
            value: wage,
            personnelId: None,
        });
    }
    RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: id.into(),
        personnelId: "audit".into(),
        paymentDate: "2026-02-14".into(),
        calculatedAt: "2026-02-20T00:00:00Z".into(),
        description: None,
        revision: CompensationRevision {
            id: id.into(),
            reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
            title: "Meal correction".into(),
            effectiveFrom: "2025-12-15".into(),
            effectiveTo: Some("2026-01-14".into()),
            decisionDate: None,
            signedAt: None,
            description: None,
            status: CompensationRevisionStatus::DRAFT,
            scope: CompensationRevisionScope::ALL_PERSONNEL,
            personnelIds: vec![],
            personnelGroup: None,
            createdAt: None,
            updatedAt: None,
        },
        overrides,
        dataset: dataset.clone(),
    })
    .unwrap()
}

fn payment(dataset: &PayrollDatasetSnapshot, result: &RetroCalculationResult) -> BordroKaydi {
    let mut dataset = dataset.clone();
    dataset.retroBatches.push(result.batch.clone());
    dataset.retroAllocations.extend(result.allocations.clone());
    calculate_payroll_checked(&PayrollCalculationRequest {
        personnelId: "audit".into(),
        periodId: "payment".into(),
        calculatedAt: "2026-02-20T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: result.batch.id.clone(),
            accrualType: AccrualType::RETRO_ADJUSTMENT,
            paymentDate: result.batch.paymentDate.clone(),
            sequence: 1,
            grossAmount: None,
            description: None,
        }),
        dataset,
    })
    .unwrap()
}

#[test]
fn retro_meal_preserves_daily_exemption_below_across_and_above_cap() {
    for (original, target, exempt) in [
        (dec!(180), dec!(260), dec!(2480)),
        (dec!(260), dec!(350), dec!(1240)),
        (dec!(320), dec!(350), dec!(0)),
    ] {
        let dataset = dataset(original, false);
        let result = entitlement(&dataset, "meal", target);
        let allocation = &result.allocations[0];
        assert_eq!(
            retro_allocation_tax_exemptions(&result.batch, allocation).unwrap(),
            (exempt, exempt)
        );
        let payroll = payment(&dataset, &result);
        assert_eq!(
            payroll.gvDetay.as_ref().unwrap().cariGvMatrahi,
            payroll.gelirToplam
                - exempt
                - payroll.kesintiler.isciSgkPrimi.unwrap()
                - payroll.kesintiler.isciIssizlikPrimi.unwrap()
        );
        assert_eq!(
            payroll.damgaDetay.as_ref().unwrap().brutDamgaVergisi,
            round_currency_amount((payroll.gelirToplam - exempt) * dec!(0.00759))
        );
    }
}

#[test]
fn retro_meal_uses_each_source_statutory_segment_instead_of_one_monthly_cap() {
    let segment: StatutoryParameterSegment = serde_json::from_value(serde_json::json!({
        "effectiveFrom": "2026-01-01", "gunlukYemekIstisnasiGV": "400"
    }))
    .unwrap();
    let dataset = dataset_with_segments(dec!(260), false, vec![segment]);
    let result = entitlement(&dataset, "segmented-meal", dec!(350));
    // December 15–31: 17 × (300−260); January 1–14: 14 × (350−260).
    assert_eq!(
        retro_allocation_tax_exemptions(&result.batch, &result.allocations[0]).unwrap(),
        (dec!(1940), dec!(1940))
    );
    let payroll = payment(&dataset, &result);
    assert_eq!(payroll.damgaDetay.unwrap().brutDamgaVergisi, dec!(6.45));
}

#[test]
fn full_gv_taxable_stamp_preference_is_unchanged() {
    let dataset = dataset(dec!(180), true);
    let result = entitlement(&dataset, "meal", dec!(350));
    let amount = result.batch.totalGrossDelta;
    assert_eq!(
        retro_allocation_tax_exemptions(&result.batch, &result.allocations[0]).unwrap(),
        (amount, dec!(0))
    );
    assert!(result.allocations[0]
        .metadata
        .as_ref()
        .unwrap()
        .find("mealTaxExemption")
        .is_none());
    let payroll = payment(&dataset, &result);
    assert_eq!(payroll.gvDetay.as_ref().unwrap().cariGvMatrahi, dec!(0));
    assert_eq!(
        payroll.damgaDetay.as_ref().unwrap().brutDamgaVergisi,
        round_currency_amount(amount * dec!(0.00759))
    );
}

#[test]
fn subsequent_batch_cannot_consume_the_same_meal_exemption_twice() {
    let mut dataset = dataset(dec!(180), false);
    let first = entitlement(&dataset, "meal-1", dec!(260));
    dataset.retroBatches.push(first.batch);
    dataset.retroAllocations.extend(first.allocations);
    let second = entitlement(&dataset, "meal-2", dec!(350));
    assert_eq!(second.batch.totalGrossDelta, dec!(2790));
    assert_eq!(
        retro_allocation_tax_exemptions(&second.batch, &second.allocations[0]).unwrap(),
        (dec!(1240), dec!(1240))
    );
    let repeated = entitlement(&dataset, "meal-2", dec!(260));
    assert!(repeated.allocations.is_empty());
}

#[test]
fn partial_settlement_prorates_exemption_and_legacy_metadata_preserves_its_result() {
    let dataset = dataset(dec!(260), false);
    let mut result = entitlement(&dataset, "meal", dec!(350));
    result.allocations[0].payableSettlementAmount = dec!(1395);
    assert_eq!(
        retro_allocation_tax_exemptions(&result.batch, &result.allocations[0]).unwrap(),
        (dec!(620), dec!(620))
    );
    result.allocations[0].metadata = Some(r#"{"revisionId":"legacy"}"#.into());
    assert_eq!(
        retro_allocation_tax_exemptions(&result.batch, &result.allocations[0]).unwrap(),
        (dec!(0), dec!(0))
    );
}

#[test]
fn saved_exemption_survives_json_and_ignores_mutable_source_settings() {
    let mut dataset = dataset(dec!(180), false);
    let result = entitlement(&dataset, "meal", dec!(260));
    let result: RetroCalculationResult =
        serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
    dataset
        .institutionSettings
        .get_mut("source")
        .unwrap()
        .gunlukYemekIstisnasiGV = Some(dec!(0));
    let payroll = payment(&dataset, &result);
    assert_eq!(payroll.gvDetay.as_ref().unwrap().cariGvMatrahi, dec!(0));
    assert_eq!(
        payroll.damgaDetay.as_ref().unwrap().brutDamgaVergisi,
        dec!(0)
    );
}

#[test]
fn positive_cash_retro_accepts_signed_worker_premiums_without_weakening_other_deductions() {
    let dataset = dataset(dec!(180), false);
    let result = entitlement_with_wage(&dataset, "mixed-correction", dec!(280), Some(dec!(1950)));
    let payroll = payment(&dataset, &result);
    assert_eq!(payroll.gelirToplam, dec!(1550));
    assert_eq!(payroll.kesintiler.isciSgkPrimi, Some(dec!(-217)));
    assert_eq!(payroll.kesintiler.isciIssizlikPrimi, Some(dec!(-15.50)));
    assert_eq!(
        payroll.gvDetay.as_ref().unwrap().cariGvMatrahi,
        dec!(232.50)
    );
    assert_eq!(
        payroll.netOdeme,
        payroll.gelirToplam - payroll.kesintiToplam
    );
    validate_ordinary_payroll_snapshot(&payroll).unwrap();
    assert!(validate_ordinary_payroll_line_items(&payroll.gelirler, &payroll.kesintiler).is_err());
    let mut invalid = payroll.clone();
    invalid.kesintiler.icra = Some(dec!(-1));
    assert!(validate_ordinary_payroll_snapshot(&invalid).is_err());
    invalid = payroll.clone();
    invalid.gelirler.yemek = Some(dec!(-1));
    assert!(validate_ordinary_payroll_snapshot(&invalid).is_err());
}

#[test]
fn malformed_or_excessive_exemption_snapshot_is_rejected() {
    let dataset = dataset(dec!(180), false);
    let mut result = entitlement(&dataset, "meal", dec!(260));
    for snapshot in [
        serde_json::json!({}),
        serde_json::json!({
            "schemaVersion": 1, "originalAmount": "0", "previousAmount": "0", "targetAmount": "999999"
        }),
    ] {
        result.allocations[0].metadata =
            Some(serde_json::json!({"mealTaxExemption":snapshot}).to_string());
        assert!(retro_allocation_tax_exemptions(&result.batch, &result.allocations[0]).is_err());
    }
}
