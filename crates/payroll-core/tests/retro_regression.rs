use chrono::{Duration, NaiveDate};
use payroll_core::{
    calculate_payroll, calculate_payroll_with_compensation_policy, canonical_sgk_earning_class,
    finalize_payroll, finalize_payroll_with_compensation_policy_checked,
    replay_retro_outstanding_receivables, retro_earning_policy, retro_payable_settlement_amount,
    retro_payment_income, round_sgk_amount, validate_v5_retro_outstanding_receivables, AccrualType,
    AnnualPayrollParameters, BordroDonemi, BordroKaydi, BordroStatus, CanonicalSgkEarningClass,
    CompensationBoundaryScope, CompensationPolicyGroup, CompensationPolicyState,
    CompensationPolicyVersion, CompensationRevision, CompensationRevisionOverride,
    CompensationRevisionReason, CompensationRevisionScope, CompensationRevisionStatus,
    DevredenPekKaydi, DonemselKurumDegerleri, ManualPayrollIncomeInput, PayrollAccrualInput,
    PayrollCalculationRequest, PayrollDatasetSnapshot, PayrollPeriodInstitutionSettings, Personel,
    PersonelPuantaj, RetroAdjustmentBatch, RetroAllocation, RetroEarningCode,
    RetroEntitlementEngine, RetroParameterKey, RetroSettlementStatus, RetroSgkTreatment,
    RetroTaxTreatment, RoadRoundingMode, SickLeaveRecord, StatutoryParameterSegment,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

#[path = "support/compensation_policy.rs"]
mod compensation_policy_test_support;

fn period(id: &str, start: &str, end: &str, tax_month: i32) -> BordroDonemi {
    BordroDonemi {
        id: id.into(),
        yil: 2026,
        ay: start[5..7].parse().expect("month"),
        baslangicTarihi: start.into(),
        bitisTarihi: end.into(),
        donemAdi: id.into(),
        taxYear: 2026,
        taxMonth: tax_month,
    }
}

fn person() -> Personel {
    Personel {
        id: "p1".into(),
        tcNo: "1".into(),
        ad: "Test".into(),
        soyad: "Personel".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "SGK-1".into(),
        iban: "TR000000000000000000000000".into(),
        hizmetYili: 5,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn settings(
    period_id: &str,
    daily_wage: Decimal,
    pek_multiplier: Decimal,
) -> DonemselKurumDegerleri {
    let mut value = DonemselKurumDegerleri {
        donemId: period_id.into(),
        gunlukTabanUcret: daily_wage,
        gunlukYemek: Decimal::ZERO,
        birlestirilmisSosyalYardim: Decimal::ZERO,
        gunlukVasitaYol: Decimal::ZERO,
        giyimYardimi: Decimal::ZERO,
        hizmetZammiBirimi: Decimal::ZERO,
        geceCalismaPrimiYuzde: Some(Decimal::ZERO),
        geceCalismaTatiliPrimiYuzde: Some(Decimal::ZERO),
        ekOdeme: Some(Decimal::ZERO),
        digerGelirVarsayilan: Some(Decimal::ZERO),
        gunlukAsgariUcret: Some(dec!(100)),
        pekTavanKatsayisi: Some(pek_multiplier),
        ..DonemselKurumDegerleri::default()
    };
    for group in value.isPrimiGruplari.as_mut().expect("default groups") {
        group.oran = Decimal::ZERO;
    }
    value
}

fn attendance(personnel_id: &str, period: &BordroDonemi, night_first_day: bool) -> PersonelPuantaj {
    let mut days = HashMap::new();
    let mut date = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut first = true;
    while date <= end {
        days.insert(
            date.format("%Y-%m-%d").to_string(),
            if night_first_day && first {
                "GÇ"
            } else {
                "Ç"
            }
            .into(),
        );
        first = false;
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("{}_{}", personnel_id, period.id),
        personelId: personnel_id.into(),
        donemId: period.id.clone(),
        gunler: days,
    }
}

fn dataset(
    periods: &[BordroDonemi],
    daily_wage: Decimal,
    pek_multiplier: Decimal,
) -> PayrollDatasetSnapshot {
    let legacy_settings = periods
        .iter()
        .map(|period| {
            (
                period.id.clone(),
                settings(&period.id, daily_wage, pek_multiplier),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut dataset = PayrollDatasetSnapshot {
        personnel: vec![person()],
        periods: periods.to_vec(),
        institutionSettings: legacy_settings
            .iter()
            .map(|(period_id, settings)| {
                (
                    period_id.clone(),
                    PayrollPeriodInstitutionSettings::from(settings),
                )
            })
            .collect(),
        attendances: periods
            .iter()
            .map(|period| attendance("p1", period, false))
            .collect(),
        payrolls: Vec::new(),
        taxOpenings: Vec::new(),
        sickLeaveRecords: Vec::new(),
        annualPayrollParameters: vec![AnnualPayrollParameters::default_for_2026()],
        compensationPolicyVersions: periods
            .iter()
            .filter_map(|period| {
                legacy_settings.get(&period.id).map(|settings| {
                    compensation_policy_test_support::baseline_version(period, settings)
                })
            })
            .collect(),
        ..PayrollDatasetSnapshot::default()
    };
    compensation_policy_test_support::seed_zero_tax_openings(&mut dataset);
    dataset
}

fn update_policy_for_period(
    dataset: &mut PayrollDatasetSnapshot,
    period_id: &str,
    update: impl FnOnce(&mut CompensationPolicyState),
) {
    let period = dataset
        .periods
        .iter()
        .find(|period| period.id == period_id)
        .expect("policy period")
        .clone();
    let mut state = dataset
        .compensationPolicyVersions
        .iter()
        .find(|version| version.effectiveFrom == period.baslangicTarihi)
        .expect("period baseline")
        .state
        .clone();
    update(&mut state);
    compensation_policy_test_support::replace_period_baseline_from_state(dataset, &period, state);
}

fn normal_payroll(
    dataset: &PayrollDatasetSnapshot,
    period_id: &str,
    payment_date: &str,
    sequence: i32,
) -> BordroKaydi {
    calculate_payroll(&PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: period_id.into(),
        calculatedAt: "2026-06-20T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: format!("p1_{period_id}"),
            accrualType: AccrualType::NORMAL,
            paymentDate: payment_date.into(),
            sequence,
            grossAmount: None,
            description: Some("Normal maaş".into()),
        }),
        dataset: dataset.clone(),
    })
    .expect("normal payroll fixture should calculate")
}

fn revision(id: &str, effective_from: &str) -> CompensationRevision {
    CompensationRevision {
        id: id.into(),
        reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
        title: "2026 TİS farkı".into(),
        effectiveFrom: effective_from.into(),
        effectiveTo: None,
        decisionDate: Some("2026-06-10".into()),
        signedAt: Some("2026-06-10".into()),
        description: Some("Retro test".into()),
        status: CompensationRevisionStatus::DRAFT,
        scope: CompensationRevisionScope::SELECTED_PERSONNEL,
        personnelIds: vec!["p1".into()],
        personnelGroup: None,
        createdAt: Some("2026-06-10T00:00:00Z".into()),
        updatedAt: None,
    }
}

fn wage_override(id: &str, revision_id: &str, value: Decimal) -> CompensationRevisionOverride {
    CompensationRevisionOverride {
        id: id.into(),
        revisionId: revision_id.into(),
        parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
        value,
        personnelId: None,
    }
}

fn retro_request(
    dataset: PayrollDatasetSnapshot,
    batch_id: &str,
    revision: CompensationRevision,
    overrides: Vec<CompensationRevisionOverride>,
    payment_date: &str,
) -> payroll_core::RetroCalculationRequest {
    payroll_core::RetroCalculationRequest {
        batchId: batch_id.into(),
        revision,
        overrides,
        personnelId: "p1".into(),
        paymentDate: payment_date.into(),
        calculatedAt: "2026-06-20T00:00:00Z".into(),
        description: Some("Retro test".into()),
        dataset,
    }
}

fn wage_retro_result(
    dataset: PayrollDatasetSnapshot,
    batch_id: &str,
    revision_id: &str,
    daily_wage: Decimal,
) -> payroll_core::RetroCalculationResult {
    RetroEntitlementEngine::calculate(&retro_request(
        dataset,
        batch_id,
        revision(revision_id, "2026-02-15"),
        vec![wage_override(
            &format!("{batch_id}-override"),
            revision_id,
            daily_wage,
        )],
        "2026-06-20",
    ))
    .expect("wage retro fixture should calculate")
}

fn append_retro_result(
    dataset: &mut PayrollDatasetSnapshot,
    result: payroll_core::RetroCalculationResult,
) {
    dataset.retroBatches.push(result.batch);
    dataset.retroAllocations.extend(result.allocations);
}

fn schema2_policy_state(
    daily_wage: Decimal,
    group_one_rate: Decimal,
    road_amount: Decimal,
    supplemental: Option<Decimal>,
    other_income: Option<Decimal>,
) -> CompensationPolicyState {
    CompensationPolicyState {
        gunlukTabanUcret: daily_wage,
        gunlukYemek: Decimal::ZERO,
        birlestirilmisSosyalYardim: Decimal::ZERO,
        gunlukVasitaYol: road_amount,
        giyimYardimi: Decimal::ZERO,
        hizmetZammiBirimi: Decimal::ZERO,
        isPrimiYuzde: Some(Decimal::ZERO),
        isPrimiGruplari: vec![
            CompensationPolicyGroup {
                id: "1. Grup".into(),
                ad: "1. Grup".into(),
                oran: group_one_rate,
                aktif: true,
            },
            CompensationPolicyGroup {
                id: "2. Grup".into(),
                ad: "2. Grup".into(),
                oran: dec!(75),
                aktif: true,
            },
        ],
        geceCalismaPrimiYuzde: Decimal::ZERO,
        geceCalismaTatiliPrimiYuzde: Decimal::ZERO,
        ekOdeme: supplemental,
        digerGelirVarsayilan: other_income,

        socialBenefitAccrualPolicies: None,
    }
}

fn schema2_policy_version(
    id: &str,
    effective_from: &str,
    state: CompensationPolicyState,
    boundary_scope: CompensationBoundaryScope,
) -> CompensationPolicyVersion {
    CompensationPolicyVersion {
        id: id.into(),
        effectiveFrom: effective_from.into(),
        state,
        boundaryScope: boundary_scope,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
        createdAt: "2026-02-01T00:00:00Z".into(),
        source: "ADMINISTRATIVE_DECISION".into(),
        reason: "Schema-2 retro fixture".into(),
        supersedesVersionId: None,
    }
}

fn schema2_source_with_payment_events(finalized: bool) -> PayrollDatasetSnapshot {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut january_reference = period("2025-12", "2025-12-15", "2026-01-14", 1);
    january_reference.yil = 2025;
    january_reference.ay = 12;
    let february_reference = period("2026-01", "2026-01-15", "2026-02-14", 2);
    let mut source = dataset(
        &[january_reference, february_reference, source_period.clone()],
        dec!(100),
        dec!(100),
    );
    let second_state = schema2_policy_state(
        dec!(110),
        dec!(15),
        Decimal::ZERO,
        None,
        Some(Decimal::ZERO),
    );
    let mut road_state = second_state.clone();
    road_state.gunlukVasitaYol = dec!(7.5);
    source.compensationPolicyVersions = vec![
        schema2_policy_version(
            "policy-base",
            &source_period.baslangicTarihi,
            schema2_policy_state(dec!(100), dec!(5), Decimal::ZERO, Some(Decimal::ZERO), None),
            CompensationBoundaryScope::FullEarning,
        ),
        schema2_policy_version(
            "policy-mid-period",
            "2026-02-20",
            second_state,
            CompensationBoundaryScope::FullEarning,
        ),
        schema2_policy_version(
            "policy-road-only",
            "2026-03-01",
            road_state,
            CompensationBoundaryScope::RoadOnly,
        ),
    ];
    let source_attendance = source
        .attendances
        .iter_mut()
        .find(|attendance| attendance.donemId == source_period.id)
        .expect("source attendance fixture should exist");
    source_attendance
        .gunler
        .insert("2026-02-20".into(), "R".into());
    source.sickLeaveRecords.push(SickLeaveRecord {
        id: "schema2-sick".into(),
        personnelId: "p1".into(),
        startDate: "2026-02-20".into(),
        endDate: "2026-02-20".into(),
        createdAt: None,
        updatedAt: None,
    });

    let early_supplement = calculate_payroll(&PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: source_period.id.clone(),
        calculatedAt: "2026-03-05T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: "p1_2026-02_supplement-early".into(),
            accrualType: AccrualType::SUPPLEMENTAL,
            paymentDate: "2026-03-05".into(),
            sequence: 0,
            grossAmount: Some(dec!(500)),
            description: Some("Early supplement".into()),
        }),
        dataset: source.clone(),
    })
    .expect("early supplemental event should calculate");
    source.payrolls.push(early_supplement);
    if finalized {
        let finalized_early_supplement = finalize_payroll(&PayrollCalculationRequest {
            personnelId: "p1".into(),
            periodId: source_period.id.clone(),
            calculatedAt: "2026-03-05T00:00:00Z".into(),
            manualIncome: None,
            accrual: Some(PayrollAccrualInput {
                accrualId: "p1_2026-02_supplement-early".into(),
                accrualType: AccrualType::SUPPLEMENTAL,
                paymentDate: "2026-03-05".into(),
                sequence: 0,
                grossAmount: Some(dec!(500)),
                description: Some("Early supplement".into()),
            }),
            dataset: source.clone(),
        })
        .expect("early supplemental event should finalize");
        source.payrolls[0] = finalized_early_supplement;
    }

    let normal_request = PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: source_period.id.clone(),
        calculatedAt: "2026-03-05T00:00:00Z".into(),
        manualIncome: Some(ManualPayrollIncomeInput {
            tediye: Some(Decimal::ZERO),
            tisIkramiyesi: None,
        }),
        accrual: Some(PayrollAccrualInput {
            accrualId: "p1_2026-02_normal".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-03-05".into(),
            sequence: 1,
            grossAmount: None,
            description: Some("Normal payroll".into()),
        }),
        dataset: source.clone(),
    };
    let normal = calculate_payroll_with_compensation_policy(&normal_request)
        .expect("schema-2 NORMAL event should calculate");
    assert!(normal
        .calculationMetadata
        .as_ref()
        .unwrap()
        .snapshot
        .is_some());
    source.payrolls.push(normal);
    if finalized {
        let finalized_normal =
            finalize_payroll_with_compensation_policy_checked(&PayrollCalculationRequest {
                dataset: source.clone(),
                ..normal_request
            })
            .expect("schema-2 NORMAL event should finalize");
        let normal_index = source
            .payrolls
            .iter()
            .position(|payroll| payroll.accrualType == AccrualType::NORMAL)
            .unwrap();
        source.payrolls[normal_index] = finalized_normal;
    }

    for (accrual_type, accrual_id, gross, sequence) in [
        (AccrualType::TEDIYE, "tediye-late", dec!(300), 2),
        (AccrualType::TIS_IKRAMIYE, "tis-late", dec!(400), 3),
    ] {
        let event = calculate_payroll(&PayrollCalculationRequest {
            personnelId: "p1".into(),
            periodId: source_period.id.clone(),
            calculatedAt: "2026-03-05T00:00:00Z".into(),
            manualIncome: None,
            accrual: Some(PayrollAccrualInput {
                accrualId: format!("p1_2026-02_{accrual_id}"),
                accrualType: accrual_type,
                paymentDate: "2026-03-05".into(),
                sequence,
                grossAmount: Some(gross),
                description: Some("Payment event".into()),
            }),
            dataset: source.clone(),
        })
        .expect("later source payment event should calculate");
        source.payrolls.push(event);
    }
    source
}

fn schema2_wage_retro(
    source: PayrollDatasetSnapshot,
    batch_id: &str,
    revision_id: &str,
    global_daily_wage: Decimal,
    personnel_daily_wage: Decimal,
) -> payroll_core::RetroCalculationResult {
    RetroEntitlementEngine::calculate(&retro_request(
        source,
        batch_id,
        revision(revision_id, "2026-02-15"),
        vec![
            wage_override(
                &format!("{batch_id}-global-wage"),
                revision_id,
                global_daily_wage,
            ),
            CompensationRevisionOverride {
                id: format!("{batch_id}-p1-wage"),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: personnel_daily_wage,
                personnelId: Some("p1".into()),
            },
        ],
        "2026-06-20",
    ))
    .expect("schema-2 payment-event replay should calculate")
}

fn assert_schema2_source_replay_rejects(source: PayrollDatasetSnapshot, batch_id: &str) {
    let revision_id = format!("{batch_id}-revision");
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        batch_id,
        revision(&revision_id, "2026-02-15"),
        vec![wage_override(
            &format!("{batch_id}-wage"),
            &revision_id,
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect_err("an incomplete immutable payment input must not be reconstructed");
    assert!(
        error.to_string().contains("UNSUPPORTED_HISTORICAL_REPLAY"),
        "unexpected error for {batch_id}: {error}"
    );
}

fn assert_source_baseline_rejects(
    source: PayrollDatasetSnapshot,
    batch_id: &str,
    expected_message: &str,
) {
    let revision_id = format!("{batch_id}-revision");
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        batch_id,
        revision(&revision_id, "2026-02-15"),
        vec![wage_override(
            &format!("{batch_id}-wage"),
            &revision_id,
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect_err("retro cannot calculate without exactly one authoritative NORMAL baseline");
    assert!(
        error.to_string().contains(expected_message),
        "unexpected baseline error for {batch_id}: {error}"
    );
}

#[test]
fn schema2_normal_replays_real_non_normal_events_from_their_saved_payment_snapshots() {
    let calculated_source = schema2_source_with_payment_events(false);
    let calculated = schema2_wage_retro(
        calculated_source.clone(),
        "schema2-positive",
        "schema2-positive-revision",
        dec!(130),
        dec!(120),
    );
    let normal_snapshot = calculated_source.payrolls[1]
        .calculationMetadata
        .as_ref()
        .unwrap()
        .snapshot
        .as_ref()
        .unwrap();
    assert_eq!(normal_snapshot.usedInputs.personnelGroup, "1. Grup");
    assert_eq!(normal_snapshot.compensation.segments.len(), 3);
    assert_eq!(
        normal_snapshot.compensation.segments[0]
            .state
            .gunlukTabanUcret,
        dec!(100)
    );
    assert_eq!(
        normal_snapshot.compensation.segments[1]
            .state
            .gunlukTabanUcret,
        dec!(110)
    );
    assert_eq!(
        normal_snapshot.compensation.segments[2].boundaryScope,
        CompensationBoundaryScope::RoadOnly
    );
    assert_eq!(
        normal_snapshot.compensation.segments[0].state.ekOdeme,
        Some(Decimal::ZERO)
    );
    assert_eq!(
        normal_snapshot.compensation.segments[0]
            .state
            .digerGelirVarsayilan,
        None
    );
    assert_eq!(normal_snapshot.compensation.segments[1].state.ekOdeme, None);
    assert_eq!(
        normal_snapshot.compensation.segments[1]
            .state
            .digerGelirVarsayilan,
        Some(Decimal::ZERO)
    );
    assert_eq!(
        calculated_source.payrolls[0].accrualType,
        AccrualType::SUPPLEMENTAL
    );
    assert_eq!(
        calculated_source.payrolls[1].accrualType,
        AccrualType::NORMAL
    );
    assert_eq!(
        calculated_source.payrolls[1].status,
        BordroStatus::CALCULATED
    );
    assert_eq!(
        calculated_source.payrolls[2].accrualType,
        AccrualType::TEDIYE
    );
    assert_eq!(
        calculated_source.payrolls[3].accrualType,
        AccrualType::TIS_IKRAMIYE
    );
    assert_eq!(calculated.batch.totalGrossDelta, dec!(368));
    assert_eq!(
        calculated
            .allocations
            .iter()
            .filter(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
            .map(|allocation| allocation.deltaAmount)
            .sum::<Decimal>(),
        dec!(330)
    );
    assert_eq!(
        calculated
            .allocations
            .iter()
            .filter(|allocation| allocation.earningCode == RetroEarningCode::WORK_PREMIUM)
            .map(|allocation| allocation.deltaAmount)
            .sum::<Decimal>(),
        dec!(38)
    );
    assert_eq!(
        calculated
            .allocations
            .iter()
            .map(|allocation| allocation.retroPekDelta)
            .sum::<Decimal>(),
        dec!(330)
    );
    assert_eq!(
        calculated
            .allocations
            .iter()
            .map(|allocation| allocation.workerSgkDelta)
            .sum::<Decimal>(),
        dec!(46.20)
    );
    assert_eq!(
        calculated
            .allocations
            .iter()
            .map(|allocation| allocation.workerUnemploymentDelta)
            .sum::<Decimal>(),
        dec!(3.30)
    );
    assert_eq!(
        calculated
            .allocations
            .iter()
            .map(|allocation| allocation.employerSgkDelta)
            .sum::<Decimal>(),
        dec!(71.78)
    );
    assert_eq!(
        calculated
            .allocations
            .iter()
            .map(|allocation| allocation.employerUnemploymentDelta)
            .sum::<Decimal>(),
        dec!(6.60)
    );

    let mut changed_live_sources = calculated_source;
    changed_live_sources.personnel[0].soyad = "Changed after save".into();
    changed_live_sources.personnel[0].hizmetYili = 30;
    changed_live_sources.personnel[0].grup = "2. Grup".into();
    changed_live_sources.institutionSettings.insert(
        "2026-02".into(),
        PayrollPeriodInstitutionSettings::from(settings("2026-02", dec!(999), dec!(1))),
    );
    let live_source_attendance = changed_live_sources
        .attendances
        .iter_mut()
        .find(|attendance| attendance.donemId == "2026-02")
        .expect("source attendance fixture should exist");
    for day in live_source_attendance.gunler.values_mut() {
        *day = "X".into();
    }
    changed_live_sources.sickLeaveRecords.clear();
    let replay_after_live_mutation = schema2_wage_retro(
        changed_live_sources,
        "schema2-positive",
        "schema2-positive-revision",
        dec!(130),
        dec!(120),
    );
    assert_eq!(replay_after_live_mutation.batch.totalGrossDelta, dec!(368));
    assert_eq!(
        replay_after_live_mutation.allocations,
        calculated.allocations
    );

    let finalized_source = schema2_source_with_payment_events(true);
    assert_eq!(finalized_source.payrolls[1].status, BordroStatus::FINALIZED);
    let negative = schema2_wage_retro(
        finalized_source,
        "schema2-negative",
        "schema2-negative-revision",
        dec!(130),
        dec!(80),
    );
    assert_eq!(negative.batch.totalGrossDelta, dec!(-894));
    assert_eq!(
        negative
            .allocations
            .iter()
            .map(|allocation| allocation.retroPekDelta)
            .sum::<Decimal>(),
        dec!(-790)
    );
    assert_eq!(
        negative
            .allocations
            .iter()
            .filter(|allocation| allocation.earningCode == RetroEarningCode::WORK_PREMIUM)
            .map(|allocation| allocation.deltaAmount)
            .sum::<Decimal>(),
        dec!(-104)
    );
    assert_eq!(
        negative
            .allocations
            .iter()
            .map(|allocation| allocation.workerSgkDelta)
            .sum::<Decimal>(),
        dec!(-110.60)
    );
    assert_eq!(
        negative
            .allocations
            .iter()
            .map(|allocation| allocation.workerUnemploymentDelta)
            .sum::<Decimal>(),
        dec!(-7.90)
    );
}

#[test]
fn schema2_non_normal_source_requires_its_saved_statutory_snapshot() {
    let mut source = schema2_source_with_payment_events(false);
    source.payrolls[0].statutorySnapshot = None;
    let request = retro_request(
        source,
        "schema2-missing-statutory",
        revision("schema2-missing-statutory-revision", "2026-02-15"),
        vec![wage_override(
            "schema2-missing-statutory-override",
            "schema2-missing-statutory-revision",
            dec!(120),
        )],
        "2026-06-20",
    );
    let error = RetroEntitlementEngine::calculate(&request)
        .expect_err("schema-2 replay must not invent saved event statutory inputs");
    assert!(
        error.to_string().contains("UNSUPPORTED_HISTORICAL_REPLAY"),
        "unexpected error: {error}"
    );
    assert!(
        error.to_string().contains("supplement-early"),
        "unexpected error: {error}"
    );
}

#[test]
fn schema2_source_replay_rejects_forged_or_missing_event_financial_snapshots() {
    let source = schema2_source_with_payment_events(false);

    let mut mismatched_person = source.clone();
    mismatched_person
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::SUPPLEMENTAL)
        .expect("supplementary payment")
        .personelId = "p2".into();
    let revision_id = "schema2-event-person-mismatch-revision";
    let error = RetroEntitlementEngine::calculate(&retro_request(
        mismatched_person,
        "schema2-event-person-mismatch",
        revision(revision_id, "2026-02-15"),
        vec![wage_override(
            "schema2-event-person-mismatch-wage",
            revision_id,
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect_err("a payment event rebound to another person must be rejected");
    assert!(error
        .to_string()
        .contains("canonical financial replay source"));

    let mut incomplete_payment_date = source.clone();
    incomplete_payment_date
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::TEDIYE)
        .expect("tediye payment")
        .paymentDate = " ".into();
    assert_schema2_source_replay_rejects(incomplete_payment_date, "schema2-event-date-missing");

    let mut compensation_snapshot_on_supplementary = source.clone();
    let normal_snapshot = compensation_snapshot_on_supplementary
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualType == AccrualType::NORMAL)
        .and_then(|payroll| payroll.calculationMetadata.clone())
        .expect("normal snapshot");
    compensation_snapshot_on_supplementary
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::SUPPLEMENTAL)
        .expect("supplementary payment")
        .calculationMetadata = Some(normal_snapshot);
    assert_schema2_source_replay_rejects(
        compensation_snapshot_on_supplementary,
        "schema2-extra-compensation-snapshot",
    );

    let mut mismatched_net = source.clone();
    let event = mismatched_net
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::TEDIYE)
        .expect("tediye payment");
    event.netOdeme += dec!(0.01);
    assert_schema2_source_replay_rejects(mismatched_net, "schema2-mismatched-net");

    let mut missing_pek_carry = source.clone();
    missing_pek_carry
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::TEDIYE)
        .expect("tediye payment")
        .devredenPekGelen = None;
    assert_schema2_source_replay_rejects(missing_pek_carry, "schema2-missing-incoming-carry");

    let mut missing_gv_base = source.clone();
    missing_gv_base
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::TIS_IKRAMIYE)
        .expect("TİS payment")
        .persistedGvBase = None;
    assert_schema2_source_replay_rejects(missing_gv_base, "schema2-missing-gv-base");

    let mut pek_limit_mismatch = source;
    pek_limit_mismatch
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::TEDIYE)
        .expect("tediye payment")
        .pekDetay
        .as_mut()
        .expect("PEK snapshot")
        .pekAltSinir += dec!(0.01);
    assert_schema2_source_replay_rejects(pek_limit_mismatch, "schema2-pek-limit-mismatch");

    let mut mismatched_later_carry = schema2_source_with_payment_events(false);
    mismatched_later_carry
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::TEDIYE)
        .expect("tediye payment")
        .devredenPekGelen = Some(vec![DevredenPekKaydi {
        tutar: dec!(1),
        kalanAySayisi: 1,
        kaynakDonemId: Some("forged-carry".into()),
    }]);
    assert_schema2_source_replay_rejects(mismatched_later_carry, "schema2-event-carry-mismatch");
}

#[test]
fn retro_rejects_missing_stale_and_duplicate_normal_source_baselines() {
    let source = schema2_source_with_payment_events(false);

    let mut missing_normal = source.clone();
    missing_normal
        .payrolls
        .retain(|payroll| payroll.accrualType != AccrualType::NORMAL);
    assert_source_baseline_rejects(
        missing_normal,
        "retro-source-normal-missing",
        "hesaplanmış veya kesinleşmiş NORMAL bordrosu bulunamadı",
    );

    let mut stale_normal = source.clone();
    stale_normal
        .payrolls
        .iter_mut()
        .find(|payroll| payroll.accrualType == AccrualType::NORMAL)
        .expect("normal payment")
        .status = BordroStatus::STALE;
    assert_source_baseline_rejects(
        stale_normal,
        "retro-source-normal-stale",
        "kaynak döneminin hesaplanmış veya kesinleşmiş NORMAL bordrosu bulunamadı",
    );

    let mut duplicate_normal = source;
    let duplicate = duplicate_normal
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualType == AccrualType::NORMAL)
        .expect("normal payment")
        .clone();
    duplicate_normal.payrolls.push(duplicate);
    assert_source_baseline_rejects(
        duplicate_normal,
        "retro-source-normal-duplicate",
        "multiple authoritative NORMAL source records",
    );
}

#[test]
fn retro_request_rejects_empty_batch_identity_and_foreign_revision_overrides() {
    let source = schema2_source_with_payment_events(false);
    let mut empty_id = retro_request(
        source.clone(),
        "batch",
        revision("empty-id-revision", "2026-02-15"),
        vec![],
        "2026-06-20",
    );
    empty_id.batchId = "  ".into();
    let error = RetroEntitlementEngine::calculate(&empty_id)
        .expect_err("retro batch identity is required before it can be persisted");
    assert!(error.to_string().contains("batch kimliği boş"));

    let request = retro_request(
        source,
        "foreign-override-batch",
        revision("current-revision", "2026-02-15"),
        vec![wage_override("foreign-wage", "foreign-revision", dec!(120))],
        "2026-06-20",
    );
    let error = RetroEntitlementEngine::calculate(&request)
        .expect_err("an override from another revision cannot enter this retro calculation");
    assert!(error
        .to_string()
        .contains("başka bir revision'a ait override"));
}

#[test]
fn retro_batch_primary_id_cannot_be_rebound_to_another_revision() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source.retroBatches.push(RetroAdjustmentBatch {
        id: "shared-batch-id".into(),
        revisionId: "different-revision".into(),
        personnelId: "p1".into(),
        paymentDate: "2026-06-20".into(),
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
    });

    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "shared-batch-id",
        revision("current-revision", "2026-02-15"),
        vec![wage_override(
            "current-override",
            "current-revision",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect_err("an existing batch id must not be rebound");

    assert!(error.to_string().contains("primary id"));
}

#[test]
fn retro_replays_historical_periods_and_keeps_original_payrolls_immutable() {
    let periods = vec![
        period("2026-02", "2026-02-15", "2026-03-14", 3),
        period("2026-03", "2026-03-15", "2026-04-14", 4),
    ];
    let mut source = dataset(&periods, dec!(100), dec!(9));
    let first = normal_payroll(&source, "2026-02", "2026-03-10", 0);
    source.payrolls.push(first.clone());
    let second = normal_payroll(&source, "2026-03", "2026-04-10", 0);
    source.payrolls.push(second.clone());

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-a",
        revision("rev-a", "2026-02-15"),
        vec![wage_override("ov-a", "rev-a", dec!(120))],
        "2026-06-20",
    ))
    .expect("retro should calculate");

    assert_eq!(result.periods.len(), 2);
    assert_eq!(result.allocations.len(), 2);
    assert_eq!(result.batch.totalGrossDelta, dec!(1180));
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|allocation| allocation.deltaAmount)
            .sum::<Decimal>(),
        result.batch.totalGrossDelta
    );
    assert_eq!(source.payrolls[0].status, BordroStatus::CALCULATED);
    assert_eq!(source.payrolls[0].gelirToplam, first.gelirToplam);
    assert_eq!(source.payrolls[1].gelirToplam, second.gelirToplam);
}

#[test]
fn retro_replays_historical_attendance_into_separate_earning_code_allocations() {
    let periods = vec![period("2026-03", "2026-03-15", "2026-04-14", 4)];
    let mut source = dataset(&periods, dec!(100), dec!(9));
    let attendance_index = source
        .attendances
        .iter()
        .position(|attendance| attendance.donemId == "2026-03")
        .expect("historical attendance fixture should exist");
    let attendance = &mut source.attendances[attendance_index];
    attendance.gunler.insert("2026-03-15".into(), "GÇ".into());
    attendance.gunler.insert("2026-03-16".into(), "R".into());
    source.sickLeaveRecords.push(SickLeaveRecord {
        id: "sick-1".into(),
        personnelId: "p1".into(),
        startDate: "2026-03-16".into(),
        endDate: "2026-03-16".into(),
        createdAt: None,
        updatedAt: None,
    });
    let original = normal_payroll(&source, "2026-03", "2026-04-20", 0);
    source.payrolls.push(original);

    let revision = revision("rev-codes", "2026-03-15");
    let overrides = vec![
        wage_override("ov-base", "rev-codes", dec!(120)),
        CompensationRevisionOverride {
            id: "ov-night".into(),
            revisionId: "rev-codes".into(),
            parameter: RetroParameterKey::GECE_CALISMA_PRIMI_YUZDE,
            value: dec!(50),
            personnelId: None,
        },
        CompensationRevisionOverride {
            id: "ov-work".into(),
            revisionId: "rev-codes".into(),
            parameter: RetroParameterKey::IS_PRIMI_YUZDE,
            value: dec!(20),
            personnelId: None,
        },
    ];
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-codes",
        revision,
        overrides,
        "2026-06-20",
    ))
    .expect("historical attendance replay should calculate");

    let codes: Vec<RetroEarningCode> = result
        .allocations
        .iter()
        .map(|allocation| allocation.earningCode)
        .collect();
    assert!(codes.contains(&RetroEarningCode::BASE_WAGE));
    assert!(codes.contains(&RetroEarningCode::NIGHT_WORK));
    assert!(codes.contains(&RetroEarningCode::WORK_PREMIUM));
    assert!(result
        .allocations
        .iter()
        .all(|allocation| allocation.deltaAmount != Decimal::ZERO));
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|allocation| allocation.deltaAmount)
            .sum::<Decimal>(),
        result.batch.totalGrossDelta
    );
}

#[test]
fn second_authoritative_retro_uses_recognized_ledger_not_original_only() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let first = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-1",
        revision("rev-1", "2026-02-15"),
        vec![wage_override("ov-1", "rev-1", dec!(120))],
        "2026-06-20",
    ))
    .expect("first retro should calculate");
    source.retroBatches.push(first.batch);
    source.retroAllocations.extend(first.allocations);

    let second = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-2",
        revision("rev-2", "2026-02-15"),
        vec![wage_override("ov-2", "rev-2", dec!(130))],
        "2026-06-20",
    ))
    .expect("second retro should calculate");

    assert_eq!(second.batch.totalGrossDelta, dec!(280));
    assert!(second
        .allocations
        .iter()
        .all(|allocation| allocation.previousAuthoritativeRetroAmount == dec!(560)));
}

#[test]
fn same_revision_second_correction_is_delta_only() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let first = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-same-revision-1",
        revision("rev-same", "2026-02-15"),
        vec![wage_override("ov-same", "rev-same", dec!(120))],
        "2026-06-20",
    ))
    .expect("first correction should calculate");
    source.retroBatches.push(first.batch);
    source.retroAllocations.extend(first.allocations);

    let second = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-same-revision-2",
        revision("rev-same", "2026-02-15"),
        vec![wage_override("ov-same", "rev-same", dec!(130))],
        "2026-06-20",
    ))
    .expect("same revision may receive a later correction");

    assert_eq!(second.batch.totalGrossDelta, dec!(280));
    assert_eq!(second.allocations.len(), 1);
    assert_eq!(
        second.allocations[0].previousAuthoritativeRetroAmount,
        dec!(560)
    );
    assert_eq!(second.allocations[0].targetAmount, dec!(3640));
    assert_eq!(second.allocations[0].deltaAmount, dec!(280));
}

#[test]
fn chained_authoritative_retro_uses_only_incremental_source_delta_and_sgk() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let source_settings = source
        .institutionSettings
        .get_mut("2026-02")
        .expect("source settings");
    source_settings.sgkIsciOraniYuzde = Some(dec!(10));
    source_settings.issizlikIsciOraniYuzde = Some(dec!(2));
    source_settings.sgkIsverenOraniYuzde = Some(dec!(20));
    source_settings.issizlikIsverenOraniYuzde = Some(dec!(1));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let first = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-chain-1",
        revision("rev-chain-1", "2026-02-15"),
        vec![wage_override("ov-chain-1", "rev-chain-1", dec!(130))],
        "2026-06-20",
    ))
    .expect("first authoritative retro should calculate");

    assert_eq!(first.periods[0].sourcePeriodId, "2026-02");
    assert_eq!(first.periods[0].originalRecognizedAmount, dec!(2800));
    assert_eq!(
        first.periods[0].previousAuthoritativeRetroAmount,
        Decimal::ZERO
    );
    assert_eq!(first.periods[0].targetAmount, dec!(3640));
    assert_eq!(first.periods[0].deltaAmount, dec!(840));
    assert_eq!(first.batch.totalGrossDelta, dec!(840));
    assert_eq!(first.allocations.len(), 1);
    let first_allocation = first.allocations[0].clone();
    assert_eq!(first_allocation.sourcePeriodId, "2026-02");
    assert_eq!(first_allocation.earningCode, RetroEarningCode::BASE_WAGE);
    assert_eq!(first_allocation.retroPekDelta, dec!(840));
    assert_eq!(first_allocation.adjustedPek, dec!(3640));
    assert_eq!(first_allocation.workerSgkDelta, dec!(84));
    assert_eq!(first_allocation.workerUnemploymentDelta, dec!(16.8));
    assert_eq!(first_allocation.employerSgkDelta, dec!(128));
    assert_eq!(first_allocation.employerUnemploymentDelta, dec!(6.4));
    assert_eq!(first_allocation.originalEmployerLowerBound, dec!(200));
    assert_eq!(first_allocation.targetEmployerLowerBound, Decimal::ZERO);
    assert_eq!(first_allocation.employerLowerBoundDelta, dec!(-200));
    assert_eq!(first_allocation.employerLowerBoundPremiumDelta, dec!(-24));

    source.retroBatches.push(first.batch);
    source.retroAllocations.extend(first.allocations);

    let second = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-chain-2",
        revision("rev-chain-2", "2026-02-15"),
        vec![wage_override("ov-chain-2", "rev-chain-2", dec!(140))],
        "2026-06-20",
    ))
    .expect("second authoritative retro should calculate");

    assert_eq!(second.periods[0].sourcePeriodId, "2026-02");
    assert_eq!(second.periods[0].originalRecognizedAmount, dec!(2800));
    assert_eq!(
        second.periods[0].previousAuthoritativeRetroAmount,
        dec!(840)
    );
    assert_eq!(second.periods[0].targetAmount, dec!(3920));
    assert_eq!(second.periods[0].deltaAmount, dec!(280));
    assert_eq!(second.batch.totalGrossDelta, dec!(280));
    assert_eq!(second.allocations.len(), 1);
    let second_allocation = second.allocations[0].clone();
    assert_eq!(second_allocation.sourcePeriodId, "2026-02");
    assert_eq!(second_allocation.earningCode, RetroEarningCode::BASE_WAGE);
    assert_eq!(second_allocation.originalRecognizedAmount, dec!(2800));
    assert_eq!(
        second_allocation.previousAuthoritativeRetroAmount,
        dec!(840)
    );
    assert_eq!(second_allocation.targetAmount, dec!(3920));
    assert_eq!(second_allocation.deltaAmount, dec!(280));
    assert_eq!(second_allocation.retroPekDelta, dec!(280));
    assert_eq!(second_allocation.adjustedPek, dec!(3920));
    assert_eq!(second_allocation.workerSgkDelta, dec!(28));
    assert_eq!(second_allocation.workerUnemploymentDelta, dec!(5.6));
    assert_eq!(second_allocation.employerSgkDelta, dec!(56));
    assert_eq!(second_allocation.employerUnemploymentDelta, dec!(2.8));
    assert_eq!(second_allocation.originalEmployerLowerBound, dec!(200));
    assert_eq!(second_allocation.targetEmployerLowerBound, Decimal::ZERO);
    assert_eq!(second_allocation.employerLowerBoundDelta, Decimal::ZERO);
    assert_eq!(
        second_allocation.employerLowerBoundPremiumDelta,
        Decimal::ZERO
    );

    assert_eq!(
        first_allocation.retroPekDelta + second_allocation.retroPekDelta,
        dec!(1120)
    );
    assert_eq!(
        first_allocation.workerSgkDelta + second_allocation.workerSgkDelta,
        dec!(112)
    );
    assert_eq!(
        first_allocation.workerUnemploymentDelta + second_allocation.workerUnemploymentDelta,
        dec!(22.4)
    );
    assert_eq!(
        first_allocation.employerSgkDelta + second_allocation.employerSgkDelta,
        dec!(184)
    );
    assert_eq!(
        first_allocation.employerUnemploymentDelta + second_allocation.employerUnemploymentDelta,
        dec!(9.2)
    );

    source.retroBatches.push(second.batch.clone());
    source.retroAllocations.extend(second.allocations.clone());
    let (_, payment_allocations, payment_income, payment_month_pek_income) =
        retro_payment_income(&source, "retro-chain-2").expect("retro payment view");
    assert_eq!(payment_allocations.len(), 1);
    assert_eq!(payment_allocations[0].sourcePeriodId, "2026-02");
    assert_eq!(
        payment_allocations[0].earningCode,
        RetroEarningCode::BASE_WAGE
    );
    assert_eq!(payment_income.tabanBrutAylik, Some(dec!(280)));
    assert_eq!(payment_month_pek_income.tabanBrutAylik, None);
}

#[test]
fn zero_retro_delta_remains_unsettled_instead_of_becoming_offset_settled() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = wage_retro_result(source, "retro-zero-delta", "rev-zero-delta", dec!(100));

    assert_eq!(result.batch.totalGrossDelta, Decimal::ZERO);
    assert_eq!(result.batch.payableSettlementAmount, Decimal::ZERO);
    assert_eq!(result.batch.offsetSettlementAmount, Decimal::ZERO);
    assert_eq!(
        result.batch.settlementStatus,
        RetroSettlementStatus::UNSETTLED
    );
    assert!(result.allocations.is_empty());
}

#[test]
fn later_authoritative_retro_on_same_source_period_blocks_earlier_payment() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let later = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-later-payment",
        revision("rev-later-payment", "2026-02-15"),
        vec![wage_override(
            "ov-later-payment",
            "rev-later-payment",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("later authoritative retro should calculate");
    source.retroBatches.push(later.batch);
    source.retroAllocations.extend(later.allocations);

    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-earlier-payment",
        revision("rev-earlier-payment", "2026-02-15"),
        vec![wage_override(
            "ov-earlier-payment",
            "rev-earlier-payment",
            dec!(125),
        )],
        "2026-06-10",
    ))
    .expect_err("daha geç authoritative event daha erken ödeme hesabında yok sayılamamalı");
    assert!(error.to_string().contains("kronolojik"));
}

#[test]
fn same_day_authoritative_retro_is_recognized_as_an_earlier_appended_event() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let first = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-same-day-1",
        revision("rev-same-day-1", "2026-02-15"),
        vec![wage_override("ov-same-day-1", "rev-same-day-1", dec!(120))],
        "2026-06-20",
    ))
    .expect("first same-day retro should calculate");
    source.retroBatches.push(first.batch);
    source.retroAllocations.extend(first.allocations);

    let second = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-same-day-2",
        revision("rev-same-day-2", "2026-02-15"),
        vec![wage_override("ov-same-day-2", "rev-same-day-2", dec!(125))],
        "2026-06-20",
    ))
    .expect("a new same-day retro event is appended after existing authoritative events");

    assert_eq!(
        second.allocations[0].previousAuthoritativeRetroAmount,
        dec!(560)
    );
    assert_eq!(second.allocations[0].deltaAmount, dec!(140));
}

#[test]
fn draft_and_stale_retro_batches_are_not_recognized() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let calculated = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-not-authoritative-source",
        revision("rev-not-authoritative-source", "2026-02-15"),
        vec![wage_override(
            "ov-not-authoritative-source",
            "rev-not-authoritative-source",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("fixture retro should calculate");
    let mut draft_batch = calculated.batch.clone();
    draft_batch.id = "retro-draft".into();
    draft_batch.status = CompensationRevisionStatus::DRAFT;
    let mut draft_allocation = calculated.allocations[0].clone();
    draft_allocation.id = "allocation-draft".into();
    draft_allocation.batchId = draft_batch.id.clone();
    let mut stale_batch = calculated.batch;
    stale_batch.id = "retro-stale".into();
    stale_batch.status = CompensationRevisionStatus::STALE;
    let mut stale_allocation = calculated.allocations[0].clone();
    stale_allocation.id = "allocation-stale".into();
    stale_allocation.batchId = stale_batch.id.clone();
    source.retroBatches.extend([draft_batch, stale_batch]);
    source
        .retroAllocations
        .extend([draft_allocation, stale_allocation]);

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-authoritative-only",
        revision("rev-authoritative-only", "2026-02-15"),
        vec![wage_override(
            "ov-authoritative-only",
            "rev-authoritative-only",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("draft/stale rows must not block a new calculation");
    assert_eq!(result.batch.totalGrossDelta, dec!(560));
    assert_eq!(
        result.allocations[0].previousAuthoritativeRetroAmount,
        Decimal::ZERO
    );
}

#[test]
fn open_service_period_after_payment_date_is_not_replayed() {
    let periods = vec![
        period("2026-02", "2026-02-15", "2026-03-14", 3),
        period("2026-06", "2026-06-15", "2026-07-14", 7),
    ];
    let mut source = dataset(&periods, dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-closed-only",
        revision("rev-closed-only", "2026-02-15"),
        vec![wage_override(
            "ov-closed-only",
            "rev-closed-only",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("closed source period should calculate");

    assert_eq!(result.periods.len(), 1);
    assert_eq!(result.periods[0].sourcePeriodId, "2026-02");
    assert_eq!(result.batch.totalGrossDelta, dec!(560));
}

#[test]
fn selected_person_retro_skips_historical_periods_without_person_source_data() {
    let periods = vec![
        period("2026-02", "2026-02-15", "2026-03-14", 3),
        period("2026-03", "2026-03-15", "2026-04-14", 4),
    ];
    let mut source = dataset(&periods, dec!(100), dec!(9));
    source
        .attendances
        .retain(|attendance| attendance.donemId != "2026-02");
    let mut finalized = normal_payroll(&source, "2026-03", "2026-04-14", 0);
    finalized.status = BordroStatus::FINALIZED;
    source.payrolls.push(finalized.clone());

    let mut prior_all_personnel = revision("prior-all-personnel", "2026-02-15");
    prior_all_personnel.effectiveTo = Some("2026-03-14".into());
    prior_all_personnel.scope = CompensationRevisionScope::ALL_PERSONNEL;
    prior_all_personnel.personnelIds.clear();
    prior_all_personnel.status = CompensationRevisionStatus::CALCULATED;
    source.compensationRevisions.push(prior_all_personnel);
    source.compensationRevisionOverrides.push(wage_override(
        "prior-all-personnel-wage",
        "prior-all-personnel",
        dec!(110),
    ));

    let original_financial_data = serde_json::to_value(&source).expect("serialize source snapshot");
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-selected-person-periods",
        revision("selected-march-revision", "2026-03-15"),
        vec![wage_override(
            "selected-march-wage",
            "selected-march-revision",
            dec!(120),
        )],
        "2026-05-14",
    ))
    .expect("selected-person source period should calculate");

    assert_eq!(result.periods.len(), 1);
    assert_eq!(result.periods[0].sourcePeriodId, "2026-03");
    assert!(result.batch.totalGrossDelta > Decimal::ZERO);
    assert_eq!(source.payrolls[0].status, BordroStatus::FINALIZED);
    assert_eq!(source.payrolls[0].id, finalized.id);
    assert_eq!(
        serde_json::to_value(&source).expect("serialize source snapshot after preview"),
        original_financial_data,
        "preview must not mutate any source personnel or period financial data"
    );
}

#[test]
fn retro_source_periods_stop_at_revision_effective_to_even_when_paid_later() {
    // BUG-RETRO-001: the payment date is only the payment/tax event date. A
    // closed period that starts after effectiveTo is not affected by the
    // revision and must not be replayed just because it ended before payment.
    let periods = vec![
        period("2026-09", "2026-09-15", "2026-10-14", 10),
        period("2026-10", "2026-10-15", "2026-11-14", 11),
        period("2026-11", "2026-11-15", "2026-12-14", 12),
    ];
    let mut source = dataset(&periods, dec!(100), dec!(9));
    for (period_id, payment_date) in [
        ("2026-09", "2026-10-10"),
        ("2026-10", "2026-11-10"),
        ("2026-11", "2026-12-10"),
    ] {
        let payroll = normal_payroll(&source, period_id, payment_date, 0);
        source.payrolls.push(payroll);
    }

    let mut revision = revision("retro-september-october", "2026-09-15");
    revision.effectiveTo = Some("2026-11-14".into());
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-september-october-paid-december",
        revision.clone(),
        vec![wage_override(
            "retro-september-october-wage",
            &revision.id,
            dec!(120),
        )],
        "2026-12-14",
    ))
    .expect("source periods inside the revision window should replay");

    assert_eq!(
        result
            .periods
            .iter()
            .map(|period| period.sourcePeriodId.as_str())
            .collect::<Vec<_>>(),
        vec!["2026-09", "2026-10"],
        "2026-11 starts after effectiveTo and must not become a retro source"
    );
    assert!(result.periods[0].deltaAmount > Decimal::ZERO);
    assert!(result.periods[1].deltaAmount > Decimal::ZERO);
    assert!(result
        .allocations
        .iter()
        .all(|allocation| allocation.sourcePeriodId != "2026-11"));
}

#[test]
fn schema2_source_payroll_replays_without_live_attendance_rows() {
    let source_period = period("2026-03", "2026-03-15", "2026-04-14", 4);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let mut finalized = normal_payroll(&source, "2026-03", "2026-04-14", 0);
    finalized.status = BordroStatus::FINALIZED;
    source.payrolls.push(finalized);
    source.attendances.clear();

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-source-missing-attendance",
        revision("selected-march-revision", "2026-03-15"),
        vec![wage_override(
            "selected-march-wage",
            "selected-march-revision",
            dec!(120),
        )],
        "2026-05-14",
    ))
    .expect("schema-2 payroll retains the inputs needed for deterministic replay");
    assert_eq!(result.batch.totalGrossDelta, dec!(620));
}

#[test]
fn snapshotless_legacy_normal_is_rejected_instead_of_replayed_from_live_settings() {
    let source_period = period("2026-03", "2026-03-15", "2026-04-14", 4);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let mut legacy_normal = normal_payroll(&source, "2026-03", "2026-04-14", 0);
    legacy_normal.calculationMetadata = None;
    source.payrolls.push(legacy_normal);

    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-snapshotless-legacy-source",
        revision("snapshotless-legacy-revision", "2026-03-15"),
        vec![wage_override(
            "snapshotless-legacy-wage",
            "snapshotless-legacy-revision",
            dec!(120),
        )],
        "2026-05-14",
    ))
    .expect_err("retro must not reconstruct historical PEK from mutable live settings");
    assert!(
        error.to_string().contains("LEGACY_REPLAY_UNAVAILABLE"),
        "unexpected error: {error}"
    );
}

#[test]
fn overlapping_revisions_use_chronological_absolute_targets() {
    let periods = vec![
        period("2026-02", "2026-02-15", "2026-03-14", 3),
        period("2026-03", "2026-03-15", "2026-04-14", 4),
    ];
    let mut source = dataset(&periods, dec!(100), dec!(9));
    let february = normal_payroll(&source, "2026-02", "2026-03-10", 0);
    source.payrolls.push(february);
    let march = normal_payroll(&source, "2026-03", "2026-04-10", 0);
    source.payrolls.push(march);
    source.compensationRevisions.push({
        let mut prior = revision("rev-a", "2026-02-15");
        prior.status = CompensationRevisionStatus::CALCULATED;
        prior
    });
    source
        .compensationRevisionOverrides
        .push(wage_override("ov-a", "rev-a", dec!(110)));

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-overlap",
        revision("rev-b", "2026-03-15"),
        vec![wage_override("ov-b", "rev-b", dec!(120))],
        "2026-06-20",
    ))
    .expect("overlapping revisions should compose");

    let feb_target = result
        .periods
        .iter()
        .find(|preview| preview.sourcePeriodId == "2026-02")
        .expect("February preview");
    let march_target = result
        .periods
        .iter()
        .find(|preview| preview.sourcePeriodId == "2026-03")
        .expect("March preview");
    assert_eq!(feb_target.targetAmount, dec!(3080));
    assert_eq!(march_target.targetAmount, dec!(3720));
    assert_eq!(result.batch.totalGrossDelta, dec!(900));
}

#[test]
fn source_month_worker_sgk_is_deducted_once_from_retro_gv_base() {
    let periods = vec![
        period("2026-03", "2026-03-15", "2026-04-14", 4),
        period("2026-06", "2026-06-15", "2026-07-14", 6),
    ];
    let mut source = dataset(&periods, dec!(100), dec!(9));
    let source_settings = source
        .institutionSettings
        .get_mut("2026-03")
        .expect("source settings");
    source_settings.sgkIsciOraniYuzde = Some(dec!(10));
    source_settings.issizlikIsciOraniYuzde = Some(dec!(2));
    source_settings.sgkIsverenOraniYuzde = Some(dec!(20));
    source_settings.issizlikIsverenOraniYuzde = Some(dec!(1));
    let payment_settings = source
        .institutionSettings
        .get_mut("2026-06")
        .expect("payment settings");
    payment_settings.sgkIsciOraniYuzde = Some(dec!(20));
    payment_settings.issizlikIsciOraniYuzde = Some(dec!(3));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-03", "2026-04-10", 0));
    let retro_result = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-source-sgk",
        revision("rev-source-sgk", "2026-03-15"),
        vec![wage_override("ov-source-sgk", "rev-source-sgk", dec!(110))],
        "2026-06-20",
    ))
    .expect("source SGK correction should calculate");
    source.retroBatches.push(retro_result.batch.clone());
    source
        .retroAllocations
        .extend(retro_result.allocations.clone());

    let payment = calculate_payroll(&PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: "2026-06".into(),
        calculatedAt: "2026-06-20T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: retro_result.batch.id.clone(),
            accrualType: AccrualType::RETRO_ADJUSTMENT,
            paymentDate: "2026-06-20".into(),
            sequence: 0,
            grossAmount: Some(retro_result.batch.totalGrossDelta),
            description: Some("Kaynak SGK test retro".into()),
        }),
        dataset: source,
    })
    .expect("retro payment should calculate");
    let allocation = &retro_result.allocations[0];
    let expected_gv_base = retro_result.batch.totalGrossDelta
        - allocation.workerSgkDelta
        - allocation.workerUnemploymentDelta;
    assert_eq!(allocation.workerSgkDelta, dec!(31));
    assert_eq!(allocation.workerUnemploymentDelta, dec!(6.2));
    assert_eq!(
        payment.gvDetay.expect("GV detail").cariGvMatrahi,
        expected_gv_base
    );
    assert_eq!(
        payment.netOdeme,
        payment.gelirToplam - payment.kesintiToplam
    );
}

#[test]
fn retro_source_replay_uses_segmented_meal_capacity() {
    let source_period = period("2026-01", "2026-01-15", "2026-02-14", 2);
    let mut source = dataset(&[source_period], dec!(100), dec!(100));
    let settings = source
        .institutionSettings
        .get_mut("2026-01")
        .expect("source settings");
    settings.gunlukYemekIstisnasiSGK = Some(dec!(1000));
    settings.gunlukYemekIstisnasiGV = Some(dec!(1000));
    settings.statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
        effectiveFrom: "2026-02-01".into(),
        gunlukYemekIstisnasiSGK: Some(dec!(200)),
        gunlukYemekIstisnasiGV: Some(dec!(200)),
        ..StatutoryParameterSegment::default()
    }]);
    update_policy_for_period(&mut source, "2026-01", |state| {
        state.gunlukYemek = dec!(200);
    });
    let attendance = source.attendances.get_mut(0).expect("source attendance");
    for (date, code) in &mut attendance.gunler {
        *code = if date == "2026-01-15" || date == "2026-02-01" {
            "Ç".into()
        } else {
            "T".into()
        };
    }

    let original = normal_payroll(&source, "2026-01", "2026-02-10", 0);
    assert_eq!(original.gelirler.yemek, Some(dec!(400)));
    source.payrolls.push(original);

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-segmented-meal",
        revision("rev-segmented-meal", "2026-02-01"),
        vec![CompensationRevisionOverride {
            id: "ov-segmented-meal".into(),
            revisionId: "rev-segmented-meal".into(),
            parameter: RetroParameterKey::GUNLUK_YEMEK,
            value: dec!(4000),
            personnelId: None,
        }],
        "2026-06-20",
    ))
    .expect("segmented meal retro should calculate");

    let meal = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::MEAL)
        .expect("meal allocation should exist");
    assert_eq!(meal.originalRecognizedAmount, dec!(400));
    assert_eq!(meal.targetAmount, dec!(4200));
    assert_eq!(meal.deltaAmount, dec!(3800));
    assert_eq!(meal.retroPekDelta, dec!(3800));
}

#[test]
fn source_month_sgk_rounding_uses_aggregate_pek_across_earning_codes() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let mut source_attendance = attendance("p1", &source.periods[0], true);
    for (date, code) in &mut source_attendance.gunler {
        if date != "2026-02-15" {
            *code = "R".into();
        }
    }
    source.attendances = vec![source_attendance];
    update_policy_for_period(&mut source, "2026-02", |state| {
        state.geceCalismaPrimiYuzde = dec!(1);
    });
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let current_revision = revision("rev-sgk-rounding", "2026-02-15");
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-sgk-rounding",
        current_revision,
        vec![
            wage_override("ov-sgk-rounding-wage", "rev-sgk-rounding", dec!(100.05)),
            CompensationRevisionOverride {
                id: "ov-sgk-rounding-night".into(),
                revisionId: "rev-sgk-rounding".into(),
                parameter: RetroParameterKey::GECE_CALISMA_PRIMI_YUZDE,
                value: dec!(1.05),
                personnelId: None,
            },
        ],
        "2026-06-20",
    ))
    .expect("multi-code source SGK correction should calculate");

    assert_eq!(result.batch.totalGrossDelta, dec!(0.10));
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|allocation| allocation.retroPekDelta)
            .sum::<Decimal>(),
        dec!(0.10)
    );
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|allocation| allocation.workerSgkDelta)
            .sum::<Decimal>(),
        dec!(0.01)
    );
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|allocation| allocation.workerUnemploymentDelta)
            .sum::<Decimal>(),
        dec!(0.00)
    );
}

#[test]
fn partial_effective_dates_follow_the_15_to_14_service_geometry() {
    let february = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut feb_source = dataset(&[february], dec!(100), dec!(9));
    feb_source
        .payrolls
        .push(normal_payroll(&feb_source, "2026-02", "2026-03-10", 0));
    let feb_result = RetroEntitlementEngine::calculate(&retro_request(
        feb_source,
        "retro-effective-march-first",
        revision("rev-effective-march-first", "2026-03-01"),
        vec![wage_override(
            "ov-effective-march-first",
            "rev-effective-march-first",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("March 1 effective date should calculate");
    assert_eq!(feb_result.batch.totalGrossDelta, dec!(280));

    let march = period("2026-03", "2026-03-15", "2026-04-14", 4);
    let mut march_source = dataset(&[march], dec!(100), dec!(9));
    march_source
        .payrolls
        .push(normal_payroll(&march_source, "2026-03", "2026-04-10", 0));
    let march_result = RetroEntitlementEngine::calculate(&retro_request(
        march_source,
        "retro-effective-march-20",
        revision("rev-effective-march-20", "2026-03-20"),
        vec![wage_override(
            "ov-effective-march-20",
            "rev-effective-march-20",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("March 20 effective date should calculate");
    assert_eq!(march_result.batch.totalGrossDelta, dec!(520));
}

#[test]
fn negative_retro_delta_is_explicit_and_not_clamped_to_zero() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-negative",
        revision("rev-negative", "2026-02-15"),
        vec![wage_override("ov-negative", "rev-negative", dec!(90))],
        "2026-06-20",
    ))
    .expect("negative retro should remain a result");

    assert_eq!(result.batch.totalGrossDelta, dec!(-280));
    assert_eq!(
        result.batch.settlementStatus,
        RetroSettlementStatus::OVERPAYMENT
    );
    assert_eq!(result.allocations[0].deltaAmount, dec!(-280));
    assert_eq!(result.allocations[0].retroPekDelta, dec!(-280));
    assert!(result.allocations[0].workerSgkDelta < Decimal::ZERO);
    // Worker PEK falls, but the canonical final employer PEK remains at the
    // lower bound.  The employer burden therefore moves through completion,
    // not through a shortcut of current worker PEK × employer rate.
    assert_eq!(result.allocations[0].employerSgkDelta, Decimal::ZERO);
    assert!(result.allocations[0].employerLowerBoundDelta > Decimal::ZERO);
    assert!(result.allocations[0].employerLowerBoundPremiumDelta > Decimal::ZERO);
}

#[test]
fn same_date_retro_replay_uses_created_at_before_id_and_does_not_repeat_overpayment() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let positive = wage_retro_result(source.clone(), "z-positive", "rev-positive", dec!(120));
    assert!(positive.batch.totalGrossDelta > Decimal::ZERO);
    assert!(positive.batch.payableSettlementAmount > Decimal::ZERO);
    let mut positive = positive;
    positive.batch.createdAt = Some("2026-06-20T10:00:00Z".into());
    append_retro_result(&mut source, positive);

    let negative = wage_retro_result(source.clone(), "a-negative", "rev-negative", dec!(100));
    assert!(negative.batch.totalGrossDelta < Decimal::ZERO);
    assert!(negative.batch.recoverableAmount > Decimal::ZERO);
    assert_eq!(negative.batch.paymentDate, "2026-06-20");
    let mut negative = negative;
    negative.batch.createdAt = Some("2026-06-20T11:00:00Z".into());
    let negative_delta = negative.batch.totalGrossDelta;
    append_retro_result(&mut source, negative);

    let existing_batch_count = source.retroBatches.len();
    let existing_allocation_count = source.retroAllocations.len();
    let repeated = wage_retro_result(
        source.clone(),
        "0-repeated-preview",
        "rev-negative",
        dec!(100),
    );

    assert!("a-negative" < "z-positive");
    assert_eq!(negative_delta, -source.retroBatches[0].totalGrossDelta);
    assert_eq!(repeated.batch.totalGrossDelta, Decimal::ZERO);
    assert_eq!(repeated.batch.recoverableAmount, Decimal::ZERO);
    assert_eq!(repeated.batch.outstandingReceivable, dec!(560));
    assert_eq!(source.retroBatches.len(), existing_batch_count);
    assert_eq!(source.retroAllocations.len(), existing_allocation_count);
}

#[test]
fn signed_overpayment_ledger_can_be_reconciled_by_a_later_authoritative_revision() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let overpayment = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-overpayment-ledger",
        revision("rev-overpayment-ledger", "2026-02-15"),
        vec![wage_override(
            "ov-overpayment-ledger",
            "rev-overpayment-ledger",
            dec!(90),
        )],
        "2026-06-20",
    ))
    .expect("overpayment ledger should calculate");
    source.retroBatches.push(overpayment.batch);
    source.retroAllocations.extend(overpayment.allocations);

    let recovery = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-overpayment-recovery",
        revision("rev-overpayment-recovery", "2026-02-15"),
        vec![wage_override(
            "ov-overpayment-recovery",
            "rev-overpayment-recovery",
            dec!(95),
        )],
        "2026-06-20",
    ))
    .expect("later authoritative revision should reconcile signed ledger");

    assert_eq!(
        recovery.allocations[0].previousAuthoritativeRetroAmount,
        dec!(-280)
    );
    assert_eq!(recovery.allocations[0].targetAmount, dec!(2660));
    assert_eq!(recovery.allocations[0].deltaAmount, dec!(140));
    assert_eq!(recovery.allocations[0].retroPekDelta, dec!(140));
    assert_eq!(recovery.batch.offsetSettlementAmount, dec!(140));
    assert_eq!(recovery.batch.payableSettlementAmount, Decimal::ZERO);
    assert_eq!(recovery.batch.outstandingReceivable, dec!(140));
    assert_eq!(
        recovery.batch.settlementStatus,
        RetroSettlementStatus::SETTLED_BY_OFFSET
    );
}

#[test]
fn open_overpayment_does_not_create_duplicate_cash_payment() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let overpayment = wage_retro_result(
        source.clone(),
        "retro-open-overpayment",
        "rev-open-overpayment",
        dec!(90),
    );
    assert_eq!(overpayment.batch.totalGrossDelta, dec!(-280));
    assert_eq!(overpayment.batch.recoverableAmount, dec!(280));
    assert_eq!(overpayment.batch.outstandingReceivable, dec!(280));
    append_retro_result(&mut source, overpayment);

    let later = wage_retro_result(
        source.clone(),
        "retro-open-overpayment-later",
        "rev-open-overpayment-later",
        dec!(100),
    );
    assert_eq!(later.batch.totalGrossDelta, dec!(280));
    assert_eq!(later.batch.offsetSettlementAmount, dec!(280));
    assert_eq!(later.batch.payableSettlementAmount, Decimal::ZERO);
    assert_eq!(later.batch.outstandingReceivable, Decimal::ZERO);
    assert_eq!(
        later.batch.settlementStatus,
        RetroSettlementStatus::SETTLED_BY_OFFSET
    );
    assert_eq!(retro_payable_settlement_amount(&later.batch), Decimal::ZERO);

    let mut with_later = source;
    append_retro_result(&mut with_later, later.clone());
    assert!(retro_payment_income(&with_later, &later.batch.id).is_err());
}

#[test]
fn partial_overpayment_offset_leaves_only_the_payable_remainder() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let overpayment = wage_retro_result(
        source.clone(),
        "retro-partial-overpayment",
        "rev-partial-overpayment",
        dec!(96.428571428571428571),
    );
    assert_eq!(overpayment.batch.totalGrossDelta, dec!(-100));
    append_retro_result(&mut source, overpayment);

    let later = wage_retro_result(
        source,
        "retro-partial-overpayment-later",
        "rev-partial-overpayment-later",
        dec!(101.785714285714285714),
    );
    assert_eq!(later.batch.totalGrossDelta, dec!(150));
    assert_eq!(later.batch.offsetSettlementAmount, dec!(100));
    assert_eq!(later.batch.payableSettlementAmount, dec!(50));
    assert_eq!(later.batch.outstandingReceivable, Decimal::ZERO);
    assert_eq!(later.allocations[0].offsetSettlementAmount, dec!(100));
    assert_eq!(later.allocations[0].payableSettlementAmount, dec!(50));
}

#[test]
fn larger_overpayment_than_new_entitlement_remains_outstanding() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let overpayment = wage_retro_result(
        source.clone(),
        "retro-large-overpayment",
        "rev-large-overpayment",
        dec!(94.642857142857142857),
    );
    assert_eq!(overpayment.batch.totalGrossDelta, dec!(-150));
    append_retro_result(&mut source, overpayment);

    let later = wage_retro_result(
        source,
        "retro-large-overpayment-later",
        "rev-large-overpayment-later",
        dec!(98.214285714285714286),
    );
    assert_eq!(later.batch.totalGrossDelta, dec!(100));
    assert_eq!(later.batch.offsetSettlementAmount, dec!(100));
    assert_eq!(later.batch.payableSettlementAmount, Decimal::ZERO);
    assert_eq!(later.batch.outstandingReceivable, dec!(50));
    assert_eq!(
        later.batch.settlementStatus,
        RetroSettlementStatus::SETTLED_BY_OFFSET
    );
}

#[test]
fn retro_earning_policy_matches_canonical_normal_pek_buckets() {
    for code in [
        RetroEarningCode::WORK_PREMIUM,
        RetroEarningCode::TIS_BONUS,
        RetroEarningCode::TEDIYE,
        RetroEarningCode::SUPPLEMENTAL,
    ] {
        assert_eq!(
            canonical_sgk_earning_class(code),
            CanonicalSgkEarningClass::NonWage
        );
        assert_eq!(
            retro_earning_policy(code).sgkTreatment,
            RetroSgkTreatment::NON_WAGE_PAYMENT_MONTH
        );
    }
    for code in [
        RetroEarningCode::BASE_WAGE,
        RetroEarningCode::NIGHT_WORK,
        RetroEarningCode::NIGHT_HOLIDAY,
        RetroEarningCode::SOCIAL_AID,
        RetroEarningCode::MEAL,
        RetroEarningCode::TRANSPORT,
        RetroEarningCode::CLOTHING,
        RetroEarningCode::SERVICE_INCREMENT,
        RetroEarningCode::OTHER,
    ] {
        assert_eq!(
            canonical_sgk_earning_class(code),
            CanonicalSgkEarningClass::Wage
        );
        assert_eq!(
            retro_earning_policy(code).sgkTreatment,
            RetroSgkTreatment::WAGE_SOURCE_MONTH
        );
    }
}

#[test]
fn sgk_rounding_is_midpoint_away_from_zero_for_worker_and_employer_components() {
    assert_eq!(round_sgk_amount(dec!(0.005)), dec!(0.01));
    assert_eq!(round_sgk_amount(dec!(-0.005)), dec!(-0.01));
    assert_eq!(round_sgk_amount(dec!(0.025)), dec!(0.03));
    assert_eq!(round_sgk_amount(dec!(-0.025)), dec!(-0.03));
}

#[test]
fn multi_allocation_sgk_residual_matches_canonical_source_total() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(1), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-multi-allocation-rounding",
        revision("rev-multi-allocation-rounding", "2026-02-15"),
        vec![
            wage_override(
                "ov-multi-allocation-wage",
                "rev-multi-allocation-rounding",
                dec!(1.0089285714285714),
            ),
            CompensationRevisionOverride {
                id: "ov-multi-allocation-clothing".into(),
                revisionId: "rev-multi-allocation-rounding".into(),
                parameter: RetroParameterKey::GIYIM_YARDIMI,
                value: dec!(0.25),
                personnelId: None,
            },
        ],
        "2026-06-20",
    ))
    .expect("multi-allocation rounding fixture should calculate");

    let source_allocations = result
        .allocations
        .iter()
        .filter(|allocation| allocation.sgkTreatment == RetroSgkTreatment::WAGE_SOURCE_MONTH)
        .collect::<Vec<_>>();
    assert_eq!(source_allocations.len(), 2);
    assert_eq!(
        source_allocations
            .iter()
            .map(|allocation| allocation.retroPekDelta)
            .sum::<Decimal>(),
        dec!(0.50)
    );
    assert_eq!(
        source_allocations
            .iter()
            .map(|allocation| allocation.workerSgkDelta)
            .sum::<Decimal>(),
        round_sgk_amount(dec!(0.50) * dec!(0.14))
    );
    assert_eq!(
        source_allocations
            .iter()
            .map(|allocation| allocation.workerUnemploymentDelta)
            .sum::<Decimal>(),
        round_sgk_amount(dec!(0.50) * dec!(0.01))
    );
    // Independently rounding both 0.25 allocations would produce 0.04 +
    // 0.04 = 0.08 for worker SGK; the canonical source total is 0.07 and
    // the deterministic residual-cent allocation carries that correction.
    assert_eq!(source_allocations[0].workerSgkDelta, dec!(0.04));
    assert_eq!(source_allocations[1].workerSgkDelta, dec!(0.03));

    let base = source_allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage source allocation");
    let clothing = source_allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::CLOTHING)
        .expect("clothing source allocation");
    assert_eq!(base.employerSgkDelta, dec!(0.05));
    assert_eq!(clothing.employerSgkDelta, dec!(-0.05));
    assert_eq!(base.employerUnemploymentDelta, dec!(0.01));
    assert_eq!(clothing.employerUnemploymentDelta, dec!(-0.01));
}

#[test]
fn non_wage_only_retro_does_not_require_source_month_pek_allocations() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-non-wage-only",
        revision("rev-non-wage-only", "2026-02-15"),
        vec![CompensationRevisionOverride {
            id: "ov-non-wage-only".into(),
            revisionId: "rev-non-wage-only".into(),
            parameter: RetroParameterKey::IS_PRIMI_YUZDE,
            value: dec!(20),
            personnelId: None,
        }],
        "2026-06-20",
    ))
    .expect("a non-wage-only retro must not enter source-month SGK replay");

    assert_eq!(result.batch.totalGrossDelta, dec!(560));
    assert_eq!(result.allocations.len(), 1);
    assert_eq!(
        result.allocations[0].earningCode,
        RetroEarningCode::WORK_PREMIUM
    );
    assert_eq!(result.allocations[0].retroPekDelta, Decimal::ZERO);
    assert_eq!(result.allocations[0].workerSgkDelta, Decimal::ZERO);
    assert_eq!(result.allocations[0].workerUnemploymentDelta, Decimal::ZERO);
    assert_eq!(result.allocations[0].employerSgkDelta, Decimal::ZERO);
    assert_eq!(
        result.allocations[0].employerUnemploymentDelta,
        Decimal::ZERO
    );
}

#[test]
fn source_month_pek_ceiling_limits_incremental_retro_pek_once() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(90), dec!(1));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-cap",
        revision("rev-cap", "2026-02-15"),
        vec![wage_override("ov-cap", "rev-cap", dec!(110))],
        "2026-06-20",
    ))
    .expect("capped retro should calculate");

    let allocation = &result.allocations[0];
    assert_eq!(allocation.originalPek, dec!(2520));
    assert_eq!(allocation.retroPekDelta, dec!(480));
    assert_eq!(allocation.adjustedPek, dec!(3000));
}

#[test]
fn negative_retro_keeps_source_pek_at_ceiling_when_revised_gross_stays_above_it() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(1000), dec!(1));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = wage_retro_result(
        source,
        "retro-negative-cap-flat",
        "rev-negative-cap-flat",
        dec!(900),
    );
    let allocation = &result.allocations[0];
    assert_eq!(allocation.originalPek, dec!(3000));
    assert_eq!(allocation.targetAmount, dec!(25200));
    assert_eq!(allocation.retroPekDelta, Decimal::ZERO);
    assert_eq!(allocation.adjustedPek, dec!(3000));
}

#[test]
fn negative_retro_crossing_source_pek_ceiling_reconciles_only_the_lost_capacity() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(1000), dec!(1));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = wage_retro_result(
        source,
        "retro-negative-cap-cross",
        "rev-negative-cap-cross",
        dec!(90),
    );
    let allocation = &result.allocations[0];
    assert_eq!(allocation.originalPek, dec!(3000));
    assert_eq!(allocation.targetAmount, dec!(2520));
    assert_eq!(allocation.retroPekDelta, dec!(-480));
    assert_eq!(allocation.adjustedPek, dec!(2520));
}

#[test]
fn positive_retro_reconciles_decreased_employer_lower_bound_completion() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(20), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = wage_retro_result(
        source,
        "retro-lower-bound-down",
        "rev-lower-bound-down",
        dec!(25),
    );
    let allocation = &result.allocations[0];
    assert_eq!(allocation.originalEmployerLowerBound, dec!(2440));
    assert_eq!(allocation.targetEmployerLowerBound, dec!(2300));
    assert_eq!(allocation.employerLowerBoundDelta, dec!(-140));
    assert!(allocation.employerLowerBoundPremiumDelta < Decimal::ZERO);
    assert_eq!(allocation.employerSgkDelta, Decimal::ZERO);
}

#[test]
fn negative_retro_reconciles_increased_employer_lower_bound_completion() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(25), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = wage_retro_result(
        source,
        "retro-lower-bound-up",
        "rev-lower-bound-up",
        dec!(20),
    );
    let allocation = &result.allocations[0];
    assert_eq!(allocation.originalEmployerLowerBound, dec!(2300));
    assert_eq!(allocation.targetEmployerLowerBound, dec!(2440));
    assert_eq!(allocation.employerLowerBoundDelta, dec!(140));
    assert!(allocation.employerLowerBoundPremiumDelta > Decimal::ZERO);
    assert_eq!(allocation.employerSgkDelta, Decimal::ZERO);
}

#[test]
fn meal_and_clothing_retro_use_source_month_statutory_pek_policy() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    // 28 worked days: the historical SGK meal exemption is 8,400, so the
    // original 5,600 meal is fully exempt while the revised 11,200 meal has
    // only 2,800 of source-month PEK subject to SGK.
    update_policy_for_period(&mut source, "2026-02", |state| {
        state.gunlukYemek = dec!(200);
        state.giyimYardimi = dec!(100);
    });
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-meal-clothing-policy",
        revision("rev-meal-clothing-policy", "2026-02-15"),
        vec![
            CompensationRevisionOverride {
                id: "ov-meal-clothing-meal".into(),
                revisionId: "rev-meal-clothing-policy".into(),
                parameter: RetroParameterKey::GUNLUK_YEMEK,
                value: dec!(400),
                personnelId: None,
            },
            CompensationRevisionOverride {
                id: "ov-meal-clothing-clothing".into(),
                revisionId: "rev-meal-clothing-policy".into(),
                parameter: RetroParameterKey::GIYIM_YARDIMI,
                value: dec!(150),
                personnelId: None,
            },
        ],
        "2026-06-20",
    ))
    .expect("meal/clothing retro should calculate");

    let meal = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::MEAL)
        .expect("meal allocation");
    let clothing = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::CLOTHING)
        .expect("clothing allocation");
    assert_eq!(meal.deltaAmount, dec!(5600));
    assert_eq!(meal.retroPekDelta, dec!(2800));
    assert_eq!(meal.incomeTaxTreatment, RetroTaxTreatment::EXEMPT);
    assert_eq!(clothing.deltaAmount, dec!(50));
    assert_eq!(clothing.retroPekDelta, dec!(50));
    assert_eq!(clothing.workerSgkDelta, dec!(7));
    assert_eq!(clothing.workerUnemploymentDelta, dec!(0.5));
}

#[test]
fn separate_authoritative_payment_event_consumes_source_month_pek_ceiling() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(90), dec!(1));
    let normal = normal_payroll(&source, "2026-02", "2026-03-10", 0);
    source.payrolls.push(normal);

    let tediye = calculate_payroll(&PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: "2026-02".into(),
        calculatedAt: "2026-03-11T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: "p1_2026-02-tediye".into(),
            accrualType: AccrualType::TEDIYE,
            paymentDate: "2026-03-11".into(),
            sequence: 1,
            grossAmount: Some(dec!(500)),
            description: Some("Tediye".into()),
        }),
        dataset: source.clone(),
    })
    .expect("separate tediye event should calculate");
    assert_eq!(tediye.pekDetay.as_ref().unwrap().primMatrahi, dec!(480));
    source.payrolls.push(tediye);

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-source-pek-non-wage",
        revision("rev-source-pek-non-wage", "2026-02-15"),
        vec![wage_override(
            "ov-source-pek-non-wage",
            "rev-source-pek-non-wage",
            dec!(110),
        )],
        "2026-06-20",
    ))
    .expect("retro should calculate with a separate tediye event");

    let allocation = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage allocation should exist");
    // The source-month ceiling is shared by every authoritative SGK-subject
    // payment event in that month, including a separate TEDIYE event.  The
    // retro allocation itself therefore has no unused source PEK left.
    assert_eq!(allocation.originalPek, dec!(3000));
    assert_eq!(allocation.retroPekDelta, dec!(0));
    assert_eq!(allocation.adjustedPek, dec!(3000));
    assert!(allocation.originalSourceCarry.is_some());
    assert!(allocation.targetSourceCarry.is_some());
    assert_ne!(allocation.originalSourceCarry, allocation.targetSourceCarry);
}

#[test]
fn missing_original_accrual_fails_closed_instead_of_recognizing_zero() {
    // BUG-RETRO-001: a source period with attendance but no NORMAL payroll
    // used to be replayed as "recognized = 0", turning the whole month into
    // a retro difference (3.360 TL here). It must now fail closed.
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let source = dataset(&[source_period], dec!(100), dec!(9));
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-missing",
        revision("rev-missing", "2026-02-15"),
        vec![wage_override("ov-missing", "rev-missing", dec!(120))],
        "2026-06-20",
    ))
    .expect_err("missing NORMAL baseline must not be recognized as zero");
    assert!(
        error.to_string().contains("NORMAL bordrosu bulunamadı"),
        "{error}"
    );
}

#[test]
fn missing_original_accrual_counts_paid_sick_r_day_in_source_statutory_snapshot() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source.attendances[0]
        .gunler
        .insert("2026-02-20".into(), "R".into());
    source.sickLeaveRecords.push(SickLeaveRecord {
        id: "paid-sick-source".into(),
        personnelId: "p1".into(),
        startDate: "2026-02-20".into(),
        endDate: "2026-02-20".into(),
        createdAt: None,
        updatedAt: None,
    });

    // An authoritative NORMAL baseline is required (BUG-RETRO-001); the
    // target replay must still count the paid sick R day.
    let normal = normal_payroll(&source, "2026-02", "2026-03-14", 0);
    source.payrolls.push(normal);
    let result = wage_retro_result(
        source,
        "retro-missing-paid-sick",
        "rev-missing-paid-sick",
        dec!(120),
    );
    let allocation = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage allocation");
    assert_eq!(allocation.targetAmount, dec!(3360));
    assert_eq!(
        allocation.deltaAmount,
        allocation.targetAmount - allocation.originalRecognizedAmount
    );
    assert_eq!(allocation.retroPekDelta, allocation.deltaAmount);
}

#[test]
fn one_retro_payment_shares_gv_dv_exemption_and_july_starts_fresh() {
    let periods = vec![
        period("2026-04", "2026-04-15", "2026-05-14", 5),
        period("2026-05", "2026-05-15", "2026-06-14", 6),
        period("2026-06", "2026-06-15", "2026-07-14", 7),
    ];
    let mut source = dataset(&periods, dec!(2443.28), dec!(9));
    let source_normal = normal_payroll(&source, "2026-04", "2026-05-10", 0);
    source.payrolls.push(source_normal);
    let june_normal = normal_payroll(&source, "2026-05", "2026-06-01", 0);
    source.payrolls.push(june_normal.clone());
    let batch = RetroAdjustmentBatch {
        id: "retro-tax".into(),
        revisionId: "rev-tax".into(),
        personnelId: "p1".into(),
        paymentDate: "2026-06-10".into(),
        status: CompensationRevisionStatus::CALCULATED,
        settlementStatus: RetroSettlementStatus::UNSETTLED,
        totalGrossDelta: dec!(100),
        payableSettlementAmount: dec!(100),
        offsetSettlementAmount: Decimal::ZERO,
        recoveredAmount: Decimal::ZERO,
        recoverableAmount: Decimal::ZERO,
        outstandingReceivable: Decimal::ZERO,
        description: Some("Haziran retro".into()),
        createdAt: None,
        calculatedAt: None,
        finalizedAt: None,
    };
    let allocation = RetroAllocation {
        id: "retro-tax-allocation".into(),
        batchId: batch.id.clone(),
        personnelId: "p1".into(),
        sourcePeriodId: "2026-04".into(),
        earningCode: RetroEarningCode::BASE_WAGE,
        originalRecognizedAmount: Decimal::ZERO,
        previousAuthoritativeRetroAmount: Decimal::ZERO,
        targetAmount: dec!(100),
        deltaAmount: dec!(100),
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
        originalSourceCarry: None,
        targetSourceCarry: None,
        payableSettlementAmount: dec!(100),
        offsetSettlementAmount: Decimal::ZERO,
        recoverableAmount: Decimal::ZERO,
        metadata: None,
    };
    source.retroBatches.push(batch);
    source.retroAllocations.push(allocation);

    let retro = calculate_payroll(&PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: "2026-05".into(),
        calculatedAt: "2026-06-10T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: "retro-tax".into(),
            accrualType: AccrualType::RETRO_ADJUSTMENT,
            paymentDate: "2026-06-10".into(),
            sequence: 1,
            grossAmount: Some(dec!(100)),
            description: Some("Haziran retro".into()),
        }),
        dataset: source.clone(),
    })
    .expect("retro payment event should calculate");

    let june_gv = june_normal.gvDetay.as_ref().expect("normal GV detail");
    let retro_gv = retro.gvDetay.as_ref().expect("retro GV detail");
    let june_dv = june_normal.damgaDetay.as_ref().expect("normal DV detail");
    let retro_dv = retro.damgaDetay.as_ref().expect("retro DV detail");
    assert_eq!(
        retro_gv.ayniAyOncekiKullanilanGvIstisnasi,
        june_gv.uygulananGvIstisnasi
    );
    assert_eq!(
        retro_dv.ayniAyOncekiKullanilanDamgaIstisnasi,
        june_dv.uygulananDamgaIstisnasi
    );
    assert!(
        retro_gv.uygulananGvIstisnasi + june_gv.uygulananGvIstisnasi
            <= retro_gv.asgariUcretGvIstisnasi
    );

    source.payrolls.push(retro.clone());
    let july = normal_payroll(&source, "2026-06", "2026-07-01", 0);
    let july_gv = july.gvDetay.as_ref().expect("July GV detail");
    assert_eq!(july_gv.ayniAyOncekiKullanilanGvIstisnasi, Decimal::ZERO);
    assert_eq!(
        july.oncekiKumulatifGvMatrahi,
        Some(retro.gvDetay.as_ref().unwrap().yeniKumulatifGvMatrahi)
    );
}

#[test]
fn event_specific_tediye_and_tis_overrides_fail_closed() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let request = retro_request(
        dataset(&[source_period], dec!(100), dec!(9)),
        "retro-event-specific",
        revision("rev-event-specific", "2026-02-15"),
        vec![CompensationRevisionOverride {
            id: "override-event-specific".into(),
            revisionId: "rev-event-specific".into(),
            parameter: RetroParameterKey::TEDIYE,
            value: dec!(5000),
            personnelId: None,
        }],
        "2026-06-20",
    );

    let error = RetroEntitlementEngine::calculate(&request)
        .expect_err("event tarihini modellemeyen tediye override'ı sessizce replay edilmemeli");
    assert!(error.to_string().contains("event tarihini"));
}

#[test]
fn retro_effective_to_date_is_inclusive_and_cannot_precede_effective_from() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let mut one_day_revision = revision("rev-one-day", "2026-02-15");
    one_day_revision.effectiveTo = Some("2026-02-15".into());
    let one_day = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-one-day",
        one_day_revision,
        vec![wage_override("ov-one-day", "rev-one-day", dec!(120))],
        "2026-06-20",
    ))
    .expect("an effective-to date equal to effective-from remains valid");
    assert_eq!(one_day.batch.totalGrossDelta, dec!(20));
    assert_eq!(one_day.allocations.len(), 1);
    assert_eq!(
        one_day.allocations[0].earningCode,
        RetroEarningCode::BASE_WAGE
    );
    assert_eq!(one_day.allocations[0].deltaAmount, dec!(20));

    let mut reversed_revision = revision("rev-reversed", "2026-02-15");
    reversed_revision.effectiveTo = Some("2026-02-14".into());
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-reversed",
        reversed_revision,
        vec![wage_override("ov-reversed", "rev-reversed", dec!(120))],
        "2026-06-20",
    ))
    .expect_err("end date before start date must be rejected");
    assert!(error.to_string().contains("bitiş tarihi"));
}

#[test]
fn retro_payment_date_must_not_precede_revision_effective_date() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let source = dataset(&[source_period], dec!(100), dec!(9));
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-future-revision",
        revision("rev-future-revision", "2026-06-21"),
        vec![wage_override("ov-future", "rev-future-revision", dec!(120))],
        "2026-06-20",
    ))
    .expect_err("future-effective revision must not be paid a day early");
    assert!(error.to_string().contains("sonra olamaz"));
}

#[test]
fn existing_retro_batch_id_rejects_changed_payment_date_and_finalized_replay() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    let first = wage_retro_result(
        source.clone(),
        "retro-reused-id",
        "rev-reused-id",
        dec!(120),
    );
    append_retro_result(&mut source, first.clone());

    let wrong_date_error = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-reused-id",
        revision("rev-reused-id", "2026-02-15"),
        vec![wage_override("ov-reused-id", "rev-reused-id", dec!(120))],
        "2026-06-21",
    ))
    .expect_err("an existing batch id must be bound to its original payment date");
    assert!(wrong_date_error.to_string().contains("primary id"));

    let mut finalized_source = source;
    finalized_source.retroBatches[0].status = CompensationRevisionStatus::FINALIZED;
    finalized_source.retroBatches[0].settlementStatus = RetroSettlementStatus::PAID;
    let finalized_error = RetroEntitlementEngine::calculate(&retro_request(
        finalized_source,
        "retro-reused-id",
        revision("rev-reused-id", "2026-02-15"),
        vec![wage_override("ov-reused-id", "rev-reused-id", dec!(120))],
        "2026-06-20",
    ))
    .expect_err("a finalized batch must not be recalculated in place");
    assert!(finalized_error.to_string().contains("FINALIZED"));
}

#[test]
fn retro_rate_override_boundaries_hold_for_each_percentage_parameter() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    for (index, parameter) in [
        RetroParameterKey::IS_PRIMI_YUZDE,
        RetroParameterKey::GECE_CALISMA_PRIMI_YUZDE,
        RetroParameterKey::GECE_CALISMA_TATILI_PRIMI_YUZDE,
    ]
    .into_iter()
    .enumerate()
    {
        for (label, value, accepted) in [
            ("zero", dec!(0), true),
            ("maximum", dec!(100), true),
            ("negative", dec!(-0.01), false),
            ("above-maximum", dec!(100.01), false),
        ] {
            let revision_id = format!("rev-rate-{index}-{label}");
            let batch_id = format!("retro-rate-{index}-{label}");
            let override_id = format!("ov-rate-{index}-{label}");
            let request = retro_request(
                source.clone(),
                &batch_id,
                revision(&revision_id, "2026-02-15"),
                vec![CompensationRevisionOverride {
                    id: override_id,
                    revisionId: revision_id,
                    parameter,
                    value,
                    personnelId: None,
                }],
                "2026-06-20",
            );
            let result = RetroEntitlementEngine::calculate(&request);
            assert_eq!(
                result.is_ok(),
                accepted,
                "unexpected {label} boundary for {parameter:?}: {result:?}"
            );
        }
    }
}

#[test]
fn personnel_specific_override_takes_precedence_over_global_and_other_parameters_stay_isolated() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let revision_id = "rev-specific-wins";
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-specific-wins",
        revision(revision_id, "2026-02-15"),
        vec![
            CompensationRevisionOverride {
                id: "ov-global-wage".into(),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(120),
                personnelId: None,
            },
            CompensationRevisionOverride {
                id: "ov-specific-wage".into(),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(130),
                personnelId: Some("p1".into()),
            },
            CompensationRevisionOverride {
                id: "ov-global-meal".into(),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_YEMEK,
                value: dec!(50),
                personnelId: None,
            },
        ],
        "2026-06-20",
    ))
    .expect("personnel-specific wage override should win without colliding with meal override");

    let base = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage allocation");
    assert_eq!(base.originalRecognizedAmount, dec!(2800));
    assert_eq!(base.targetAmount, dec!(3640));
    assert_eq!(base.deltaAmount, dec!(840));
}

#[test]
fn duplicate_personnel_specific_override_fails_closed() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let revision_id = "rev-duplicate-specific";
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-duplicate-specific",
        revision(revision_id, "2026-02-15"),
        vec![
            CompensationRevisionOverride {
                id: "ov-specific-1".into(),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(120),
                personnelId: Some("p1".into()),
            },
            CompensationRevisionOverride {
                id: "ov-specific-2".into(),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(130),
                personnelId: Some("p1".into()),
            },
        ],
        "2026-06-20",
    ))
    .expect_err("duplicate personnel-specific override must be rejected");
    assert!(error.to_string().contains("duplicate revision override"));
}

#[test]
fn duplicate_global_override_fails_closed_but_specific_plus_global_is_valid() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let revision_id = "rev-duplicate-global";
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "retro-duplicate-global",
        revision(revision_id, "2026-02-15"),
        vec![
            CompensationRevisionOverride {
                id: "ov-global-1".into(),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(120),
                personnelId: None,
            },
            CompensationRevisionOverride {
                id: "ov-global-2".into(),
                revisionId: revision_id.into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(130),
                personnelId: None,
            },
        ],
        "2026-06-20",
    ))
    .expect_err("duplicate global override must be rejected");
    assert!(error.to_string().contains("duplicate revision override"));

    let ok = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-global-plus-specific",
        revision("rev-global-plus-specific", "2026-02-15"),
        vec![
            CompensationRevisionOverride {
                id: "ov-global-only".into(),
                revisionId: "rev-global-plus-specific".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(120),
                personnelId: None,
            },
            CompensationRevisionOverride {
                id: "ov-specific-only".into(),
                revisionId: "rev-global-plus-specific".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(130),
                personnelId: Some("p1".into()),
            },
        ],
        "2026-06-20",
    ))
    .expect("one global plus one personnel-specific override is valid");
    assert_eq!(ok.batch.totalGrossDelta, dec!(840));
}

#[test]
fn irrelevant_historical_revisions_are_filtered_before_their_invalid_dates_are_validated() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut base = dataset(&[source_period], dec!(100), dec!(9));
    base.payrolls
        .push(normal_payroll(&base, "2026-02", "2026-03-10", 0));

    let current_id = "rev-filter-current";
    let current_override = || wage_override("ov-filter-current", current_id, dec!(120));

    let mut same_id = base.clone();
    let mut duplicate_current = revision(current_id, "2026-02-15");
    duplicate_current.status = CompensationRevisionStatus::CALCULATED;
    duplicate_current.effectiveTo = Some("2026-02-14".into());
    same_id.compensationRevisions.push(duplicate_current);
    assert!(
        RetroEntitlementEngine::calculate(&retro_request(
            same_id,
            "retro-filter-same-id",
            revision(current_id, "2026-02-15"),
            vec![current_override()],
            "2026-06-20",
        ))
        .is_ok(),
        "dataset copy of the current revision must be skipped before date validation"
    );

    let mut stale = base.clone();
    let mut stale_prior = revision("rev-filter-stale", "2026-02-15");
    stale_prior.status = CompensationRevisionStatus::STALE;
    stale_prior.effectiveTo = Some("2026-02-14".into());
    stale.compensationRevisions.push(stale_prior);
    assert!(
        RetroEntitlementEngine::calculate(&retro_request(
            stale,
            "retro-filter-stale",
            revision(current_id, "2026-02-15"),
            vec![current_override()],
            "2026-06-20",
        ))
        .is_ok(),
        "STALE historical revision must be skipped before date validation"
    );

    let mut out_of_scope = base.clone();
    let mut foreign_prior = revision("rev-filter-foreign", "2026-02-15");
    foreign_prior.status = CompensationRevisionStatus::CALCULATED;
    foreign_prior.effectiveTo = Some("2026-02-14".into());
    foreign_prior.personnelIds = vec!["other-personnel".into()];
    out_of_scope.compensationRevisions.push(foreign_prior);
    assert!(
        RetroEntitlementEngine::calculate(&retro_request(
            out_of_scope,
            "retro-filter-foreign",
            revision(current_id, "2026-02-15"),
            vec![current_override()],
            "2026-06-20",
        ))
        .is_ok(),
        "out-of-scope historical revision must be skipped before date validation"
    );

    let mut unauthoritative_draft = base;
    let mut draft_prior = revision("rev-filter-draft", "2026-02-15");
    draft_prior.effectiveTo = Some("2026-02-14".into());
    unauthoritative_draft
        .compensationRevisions
        .push(draft_prior);
    assert!(
        RetroEntitlementEngine::calculate(&retro_request(
            unauthoritative_draft,
            "retro-filter-draft",
            revision(current_id, "2026-02-15"),
            vec![current_override()],
            "2026-06-20",
        ))
        .is_ok(),
        "DRAFT historical revision without authoritative batch must be skipped"
    );
}

#[test]
fn authoritative_batch_makes_a_draft_historical_revision_participate_and_validate() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let prior_id = "rev-authoritative-draft";
    let mut prior = revision(prior_id, "2026-02-15");
    prior.effectiveTo = Some("2026-02-14".into());
    source.compensationRevisions.push(prior);
    source.retroBatches.push(RetroAdjustmentBatch {
        id: "retro-authoritative-draft".into(),
        revisionId: prior_id.into(),
        personnelId: "p1".into(),
        paymentDate: "2026-06-10".into(),
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
    });

    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-after-authoritative-draft",
        revision("rev-current-after-draft", "2026-02-15"),
        vec![wage_override(
            "ov-current-after-draft",
            "rev-current-after-draft",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect_err("authoritative batch should make the historical DRAFT revision participate");
    assert!(error.to_string().contains("bitiş tarihi"));
}

#[test]
fn authoritative_legacy_tediye_is_preserved_in_target_without_becoming_a_retro_delta() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let tediye = calculate_payroll(&PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: "2026-02".into(),
        calculatedAt: "2026-03-11T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: "p1_2026-02-legacy-tediye".into(),
            accrualType: AccrualType::TEDIYE,
            paymentDate: "2026-03-11".into(),
            sequence: 1,
            grossAmount: Some(dec!(500)),
            description: Some("Legacy tediye".into()),
        }),
        dataset: source.clone(),
    })
    .expect("legacy tediye fixture should calculate");
    source.payrolls.push(tediye);

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-preserve-legacy-tediye",
        revision("rev-preserve-legacy-tediye", "2026-02-15"),
        vec![wage_override(
            "ov-preserve-legacy-tediye",
            "rev-preserve-legacy-tediye",
            dec!(110),
        )],
        "2026-06-20",
    ))
    .expect("retro should preserve the independent legacy tediye event");

    assert_eq!(result.batch.totalGrossDelta, dec!(280));
    assert!(
        result
            .allocations
            .iter()
            .all(|allocation| allocation.earningCode != RetroEarningCode::TEDIYE),
        "unchanged legacy tediye must not be converted into a retro delta"
    );
    let base = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage delta");
    assert_eq!(base.deltaAmount, dec!(280));
}

#[test]
fn personnel_group_scope_requires_exact_match_for_current_and_historical_revisions() {
    let periods = vec![
        period("2026-02", "2026-02-15", "2026-03-14", 3),
        period("2026-03", "2026-03-15", "2026-04-14", 4),
    ];
    let mut base = dataset(&periods, dec!(100), dec!(9));
    base.payrolls
        .push(normal_payroll(&base, "2026-02", "2026-03-10", 0));
    base.payrolls
        .push(normal_payroll(&base, "2026-03", "2026-04-10", 0));

    let mut current_match = revision("rev-current-group-match", "2026-03-15");
    current_match.scope = CompensationRevisionScope::PERSONNEL_GROUP;
    current_match.personnelIds.clear();
    current_match.personnelGroup = Some("1. Grup".into());
    assert!(
        RetroEntitlementEngine::calculate(&retro_request(
            base.clone(),
            "retro-current-group-match",
            current_match,
            vec![wage_override(
                "ov-current-group-match",
                "rev-current-group-match",
                dec!(120),
            )],
            "2026-06-20",
        ))
        .is_ok(),
        "current revision should accept the personnel's exact group"
    );

    let mut current_mismatch = revision("rev-current-group-mismatch", "2026-03-15");
    current_mismatch.scope = CompensationRevisionScope::PERSONNEL_GROUP;
    current_mismatch.personnelIds.clear();
    current_mismatch.personnelGroup = Some("Other Group".into());
    let mismatch_error = RetroEntitlementEngine::calculate(&retro_request(
        base.clone(),
        "retro-current-group-mismatch",
        current_mismatch,
        vec![wage_override(
            "ov-current-group-mismatch",
            "rev-current-group-mismatch",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect_err("current revision with another personnel group must be rejected");
    assert!(mismatch_error.to_string().contains("grubu kapsamı dışında"));

    let mut historical_match_source = base.clone();
    let mut historical_match = revision("rev-historical-group-match", "2026-02-15");
    historical_match.status = CompensationRevisionStatus::CALCULATED;
    historical_match.scope = CompensationRevisionScope::PERSONNEL_GROUP;
    historical_match.personnelIds.clear();
    historical_match.personnelGroup = Some("1. Grup".into());
    historical_match_source
        .compensationRevisions
        .push(historical_match);
    historical_match_source
        .compensationRevisionOverrides
        .push(wage_override(
            "ov-historical-group-match",
            "rev-historical-group-match",
            dec!(110),
        ));
    let with_match = RetroEntitlementEngine::calculate(&retro_request(
        historical_match_source,
        "retro-after-group-match",
        revision("rev-after-group-match", "2026-03-15"),
        vec![wage_override(
            "ov-after-group-match",
            "rev-after-group-match",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("matching historical group revision should participate");
    let february = with_match
        .periods
        .iter()
        .find(|preview| preview.sourcePeriodId == "2026-02")
        .expect("February preview");
    assert_eq!(february.targetAmount, dec!(3080));

    let mut historical_mismatch_source = base;
    let mut historical_mismatch = revision("rev-historical-group-mismatch", "2026-02-15");
    historical_mismatch.status = CompensationRevisionStatus::CALCULATED;
    historical_mismatch.scope = CompensationRevisionScope::PERSONNEL_GROUP;
    historical_mismatch.personnelIds.clear();
    historical_mismatch.personnelGroup = Some("Other Group".into());
    historical_mismatch_source
        .compensationRevisions
        .push(historical_mismatch);
    historical_mismatch_source
        .compensationRevisionOverrides
        .push(wage_override(
            "ov-historical-group-mismatch",
            "rev-historical-group-mismatch",
            dec!(110),
        ));
    let without_match = RetroEntitlementEngine::calculate(&retro_request(
        historical_mismatch_source,
        "retro-after-group-mismatch",
        revision("rev-after-group-mismatch", "2026-03-15"),
        vec![wage_override(
            "ov-after-group-mismatch",
            "rev-after-group-mismatch",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("non-matching historical group revision should be ignored");
    assert!(
        without_match
            .periods
            .iter()
            .all(|preview| preview.sourcePeriodId != "2026-02"),
        "out-of-scope historical revision must not pull the earlier February period into replay"
    );
}

#[test]
fn split_replay_prorates_fixed_monthly_allowances_without_duplication() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    update_policy_for_period(&mut source, "2026-02", |state| {
        state.birlestirilmisSosyalYardim = dec!(280);
        state.giyimYardimi = dec!(140);
        state.hizmetZammiBirimi = dec!(10);
        state.ekOdeme = Some(dec!(70));
        state.digerGelirVarsayilan = Some(dec!(35));
    });
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-split-fixed",
        revision("rev-split-fixed", "2026-03-01"),
        vec![wage_override(
            "ov-split-fixed",
            "rev-split-fixed",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("mid-period wage revision should replay two segments");

    let base = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage delta");
    assert_eq!(base.originalRecognizedAmount, dec!(2800));
    assert_eq!(base.targetAmount, dec!(3080));
    assert_eq!(base.deltaAmount, dec!(280));

    for code in [
        RetroEarningCode::SOCIAL_AID,
        RetroEarningCode::CLOTHING,
        RetroEarningCode::SERVICE_INCREMENT,
        RetroEarningCode::SUPPLEMENTAL,
        RetroEarningCode::OTHER,
    ] {
        assert!(
            result
                .allocations
                .iter()
                .all(|allocation| allocation.earningCode != code),
            "{code:?} is a fixed monthly amount and must not gain a retro delta merely because replay split the period"
        );
    }
    assert_eq!(result.batch.totalGrossDelta, dec!(280));
}

#[test]
fn work_premium_override_updates_only_the_active_group_matching_the_personnel() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source.personnel[0].grup = "target-group".into();
    update_policy_for_period(&mut source, "2026-02", |state| {
        let groups = &mut state.isPrimiGruplari;
        groups[0].id = "unrelated-id".into();
        groups[0].ad = "Unrelated Group".into();
        groups[0].oran = Decimal::ZERO;
        groups[0].aktif = true;
        groups[1].id = "target-group".into();
        groups[1].ad = "Target Alias".into();
        groups[1].oran = Decimal::ZERO;
        groups[1].aktif = true;
        groups[2].oran = Decimal::ZERO;
    });
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let revision_id = "rev-work-premium-group";
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-work-premium-group",
        revision(revision_id, "2026-02-15"),
        vec![CompensationRevisionOverride {
            id: "ov-work-premium-group".into(),
            revisionId: revision_id.into(),
            parameter: RetroParameterKey::IS_PRIMI_YUZDE,
            value: dec!(10),
            personnelId: None,
        }],
        "2026-06-20",
    ))
    .expect("work premium override should update the matching active group only");

    let premium = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::WORK_PREMIUM)
        .expect("work premium allocation");
    assert_eq!(premium.originalRecognizedAmount, Decimal::ZERO);
    assert_eq!(premium.targetAmount, dec!(280));
    assert_eq!(premium.deltaAmount, dec!(280));
    assert_eq!(result.batch.totalGrossDelta, dec!(280));
}

#[test]
fn zero_delta_authoritative_prior_batch_does_not_block_later_retro() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    source.retroBatches.push(RetroAdjustmentBatch {
        id: "retro-zero-prior".into(),
        revisionId: "rev-zero-prior".into(),
        personnelId: "p1".into(),
        paymentDate: "2026-06-10".into(),
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
    });

    let later = wage_retro_result(
        source,
        "retro-after-zero-prior",
        "rev-after-zero-prior",
        dec!(120),
    );
    assert_eq!(later.batch.totalGrossDelta, dec!(560));
    assert_eq!(
        later.allocations[0].previousAuthoritativeRetroAmount,
        Decimal::ZERO
    );
}

#[test]
fn settled_by_offset_requires_a_strictly_positive_offset_amount() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));
    source.retroBatches.push(RetroAdjustmentBatch {
        id: "retro-zero-offset-invalid".into(),
        revisionId: "rev-zero-offset-invalid".into(),
        personnelId: "p1".into(),
        paymentDate: "2026-06-10".into(),
        status: CompensationRevisionStatus::CALCULATED,
        settlementStatus: RetroSettlementStatus::SETTLED_BY_OFFSET,
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
    });

    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-after-zero-offset-invalid",
        revision("rev-after-zero-offset-invalid", "2026-02-15"),
        vec![wage_override(
            "ov-after-zero-offset-invalid",
            "rev-after-zero-offset-invalid",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect_err("SETTLED_BY_OFFSET with a zero offset amount must be rejected");
    assert!(error.to_string().contains("settlement status"));
}

#[test]
fn legacy_v4_zero_flow_prior_batch_remains_authoritative_for_incremental_retro() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source
        .payrolls
        .push(normal_payroll(&source, "2026-02", "2026-03-10", 0));

    let mut legacy = wage_retro_result(
        source.clone(),
        "retro-legacy-v4-prior",
        "rev-legacy-v4-prior",
        dec!(120),
    );
    legacy.batch.payableSettlementAmount = Decimal::ZERO;
    legacy.batch.offsetSettlementAmount = Decimal::ZERO;
    legacy.batch.recoveredAmount = Decimal::ZERO;
    legacy.batch.recoverableAmount = Decimal::ZERO;
    legacy.batch.outstandingReceivable = Decimal::ZERO;
    legacy.batch.settlementStatus = RetroSettlementStatus::UNSETTLED;
    for allocation in &mut legacy.allocations {
        allocation.payableSettlementAmount = Decimal::ZERO;
        allocation.offsetSettlementAmount = Decimal::ZERO;
        allocation.recoverableAmount = Decimal::ZERO;
    }
    append_retro_result(&mut source, legacy);

    let later = wage_retro_result(
        source,
        "retro-after-legacy-v4",
        "rev-after-legacy-v4",
        dec!(130),
    );
    assert_eq!(later.batch.totalGrossDelta, dec!(280));
    assert_eq!(
        later.allocations[0].previousAuthoritativeRetroAmount,
        dec!(560)
    );
    assert_eq!(later.allocations[0].deltaAmount, dec!(280));
}

#[test]
fn missing_original_accrual_adds_two_paid_sick_days_by_multiplication_not_division() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    source.attendances[0]
        .gunler
        .insert("2026-02-20".into(), "R".into());
    source.attendances[0]
        .gunler
        .insert("2026-02-21".into(), "R".into());
    source.sickLeaveRecords.push(SickLeaveRecord {
        id: "paid-sick-two-days".into(),
        personnelId: "p1".into(),
        startDate: "2026-02-20".into(),
        endDate: "2026-02-21".into(),
        createdAt: None,
        updatedAt: None,
    });

    let normal = normal_payroll(&source, "2026-02", "2026-03-14", 0);
    source.payrolls.push(normal);
    let result = wage_retro_result(
        source,
        "retro-missing-two-paid-sick",
        "rev-missing-two-paid-sick",
        dec!(120),
    );
    let base = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage allocation");
    assert!(base.originalRecognizedAmount > Decimal::ZERO);
    assert_eq!(base.targetAmount, dec!(3360));
    assert_eq!(
        base.deltaAmount,
        base.targetAmount - base.originalRecognizedAmount
    );
}

// ---------------------------------------------------------------------------
// BUG-RETRO-001 regression: effective window, missing NORMAL baseline and
// payment-date independence. Mirrors the live scenario (revision for one
// 15-14 service period, payment three months later, the payment period has
// attendance but no NORMAL payroll yet).
// ---------------------------------------------------------------------------

fn bug_retro_001_dataset(old_daily_wage: Decimal) -> PayrollDatasetSnapshot {
    let periods = vec![
        period("2026-03", "2026-03-15", "2026-04-14", 4),
        period("2026-04", "2026-04-15", "2026-05-14", 5),
        period("2026-05", "2026-05-15", "2026-06-14", 6),
        period("2026-06", "2026-06-15", "2026-07-14", 7),
    ];
    let mut source = dataset(&periods, old_daily_wage, dec!(9));
    // NORMAL payrolls exist for March-May; June (the payment period of a
    // 2026-07-14 payment) has attendance only, exactly like Mart 2028 live.
    for (period_id, payment_date) in [
        ("2026-03", "2026-04-14"),
        ("2026-04", "2026-05-14"),
        ("2026-05", "2026-06-14"),
    ] {
        let payroll = normal_payroll(&source, period_id, payment_date, 0);
        source.payrolls.push(payroll);
    }
    source
}

fn bug_retro_001_revision(effective_to: Option<&str>) -> CompensationRevision {
    let mut value = revision("bug-retro-001", "2026-03-15");
    value.effectiveTo = effective_to.map(Into::into);
    value
}

fn source_ids(result: &payroll_core::RetroCalculationResult) -> Vec<String> {
    result
        .periods
        .iter()
        .map(|period| period.sourcePeriodId.clone())
        .collect()
}

#[test]
fn bug_retro_001_test_a_period_after_effective_to_is_never_a_candidate() {
    let source = bug_retro_001_dataset(dec!(2443.28));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "bug-retro-001-a",
        bug_retro_001_revision(Some("2026-04-14")),
        vec![wage_override(
            "bug-retro-001-a-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-07-14",
    ))
    .expect("only the revision window must be replayed");

    assert_eq!(source_ids(&result), vec!["2026-03".to_string()]);
    assert!(result
        .allocations
        .iter()
        .all(|allocation| allocation.sourcePeriodId == "2026-03"));
    // No full-month fake difference: the delta is a daily-wage difference,
    // not a whole month's entitlement.
    assert!(result.batch.totalGrossDelta < dec!(5000));
}

#[test]
fn bug_retro_001_effective_to_boundary_is_inclusive_on_period_start() {
    let source = bug_retro_001_dataset(dec!(2443.28));
    // effectiveTo on the day before 2026-04 starts: April is excluded.
    let before = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "bug-retro-001-boundary-before",
        bug_retro_001_revision(Some("2026-04-14")),
        vec![wage_override(
            "bug-retro-001-bb-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-06-14",
    ))
    .expect("boundary before");
    assert_eq!(source_ids(&before), vec!["2026-03".to_string()]);

    // effectiveTo exactly on the first day of 2026-04: the period intersects
    // the revision for one day and is replayed for that day only.
    let on_start = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "bug-retro-001-boundary-on",
        bug_retro_001_revision(Some("2026-04-15")),
        vec![wage_override(
            "bug-retro-001-bo-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-06-14",
    ))
    .expect("boundary on period start");
    assert_eq!(
        source_ids(&on_start),
        vec!["2026-03".to_string(), "2026-04".to_string()]
    );
    let april_base = on_start
        .allocations
        .iter()
        .find(|allocation| {
            allocation.sourcePeriodId == "2026-04"
                && allocation.earningCode == RetroEarningCode::BASE_WAGE
        })
        .expect("one covered April day must produce a base-wage delta");
    assert_eq!(april_base.deltaAmount, dec!(56.72));
}

#[test]
fn bug_retro_001_test_b_missing_normal_inside_window_fails_closed() {
    let mut source = bug_retro_001_dataset(dec!(2443.28));
    // Open-ended revision: June (attendance, no NORMAL) is inside the window.
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "bug-retro-001-b-open",
        bug_retro_001_revision(None),
        vec![wage_override(
            "bug-retro-001-b-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-07-14",
    ))
    .expect_err("a period without NORMAL must not be recognized as zero");
    let message = error.to_string();
    assert!(message.contains("2026-06"), "{message}");
    assert!(message.contains("NORMAL bordrosu bulunamadı"), "{message}");

    // Bounded revision whose window contains a period whose NORMAL is
    // missing must fail the same way instead of returning a full month.
    source
        .payrolls
        .retain(|payroll| payroll.donemId != "2026-04");
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "bug-retro-001-b-bounded",
        bug_retro_001_revision(Some("2026-05-14")),
        vec![wage_override(
            "bug-retro-001-b2-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-07-14",
    ))
    .expect_err("missing NORMAL inside a bounded window fails closed");
    assert!(error.to_string().contains("2026-04"), "{error}");
}

#[test]
fn bug_retro_001_stale_normal_inside_window_names_the_period() {
    let mut source = bug_retro_001_dataset(dec!(2443.28));
    source
        .payrolls
        .iter_mut()
        .filter(|payroll| payroll.donemId == "2026-03")
        .for_each(|payroll| payroll.status = BordroStatus::STALE);
    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "bug-retro-001-stale",
        bug_retro_001_revision(Some("2026-04-14")),
        vec![wage_override(
            "bug-retro-001-s-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-07-14",
    ))
    .expect_err("stale NORMAL baseline is not authoritative");
    let message = error.to_string();
    assert!(message.contains("2026-03"), "{message}");
    assert!(message.contains("güncelliğini yitirmiş"), "{message}");
}

#[test]
fn bug_retro_001_test_c_later_payment_date_never_widens_sources() {
    let source = bug_retro_001_dataset(dec!(2443.28));
    let mut seen = Vec::new();
    for (index, payment_date) in ["2026-04-14", "2026-05-14", "2026-07-14"]
        .into_iter()
        .enumerate()
    {
        let result = RetroEntitlementEngine::calculate(&retro_request(
            source.clone(),
            &format!("bug-retro-001-c-{index}"),
            bug_retro_001_revision(Some("2026-04-14")),
            vec![wage_override(
                &format!("bug-retro-001-c-{index}-w"),
                "bug-retro-001",
                dec!(2500),
            )],
            payment_date,
        ))
        .unwrap_or_else(|error| panic!("{payment_date}: {error}"));
        seen.push((source_ids(&result), result.batch.totalGrossDelta));
    }
    assert!(seen
        .iter()
        .all(|(ids, total)| ids == &vec!["2026-03".to_string()] && *total == seen[0].1));
}

#[test]
fn bug_retro_001_test_d_valid_daily_wage_retro_matches_normal_entitlement() {
    // Live reference: 2.443,28 -> 2.500,00 TL daily, 31 service days.
    let source = bug_retro_001_dataset(dec!(2443.28));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source.clone(),
        "bug-retro-001-d",
        bug_retro_001_revision(Some("2026-04-14")),
        vec![wage_override(
            "bug-retro-001-d-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-07-14",
    ))
    .expect("valid retro");
    let base = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("base wage delta");
    assert_eq!(base.deltaAmount, dec!(1758.32), "56,72 TL x 31 gün");

    // Independent oracle: a NORMAL payroll computed with the new wage must
    // equal recognized + retro delta for every earning code
    // (new entitlement - previously recognized - previous retro).
    let mut new_wage = source.clone();
    update_policy_for_period(&mut new_wage, "2026-03", |state| {
        state.gunlukTabanUcret = dec!(2500);
    });
    let expected = normal_payroll(&new_wage, "2026-03", "2026-04-14", 0);
    let original = source
        .payrolls
        .iter()
        .find(|payroll| payroll.donemId == "2026-03")
        .expect("original NORMAL");
    let expected_total = expected.gelirToplam - original.gelirToplam;
    assert_eq!(result.batch.totalGrossDelta, expected_total);

    // Recalculating the same revision after the first batch is authoritative
    // yields 0,00 TL (previous retro adjustments are recognized).
    let mut with_first = source;
    append_retro_result(&mut with_first, result);
    let second = RetroEntitlementEngine::calculate(&retro_request(
        with_first,
        "bug-retro-001-d-second",
        bug_retro_001_revision(Some("2026-04-14")),
        vec![wage_override(
            "bug-retro-001-d2-w",
            "bug-retro-001",
            dec!(2500),
        )],
        "2026-07-14",
    ))
    .expect("second calculation");
    assert_eq!(second.batch.totalGrossDelta, Decimal::ZERO);
    assert_eq!(second.batch.payableSettlementAmount, Decimal::ZERO);
}

#[test]
fn receivable_replay_matches_runtime_order_for_zero_and_unpaid_overpayment_batches() {
    fn batch(
        id: &str,
        total: Decimal,
        recoverable: Decimal,
        outstanding: Decimal,
        settlement_status: RetroSettlementStatus,
        created_at: Option<&str>,
    ) -> RetroAdjustmentBatch {
        RetroAdjustmentBatch {
            id: id.into(),
            revisionId: format!("revision-{id}"),
            personnelId: "p1".into(),
            paymentDate: "2027-03-14".into(),
            status: CompensationRevisionStatus::CALCULATED,
            settlementStatus: settlement_status,
            totalGrossDelta: total,
            payableSettlementAmount: Decimal::ZERO,
            offsetSettlementAmount: Decimal::ZERO,
            recoveredAmount: Decimal::ZERO,
            recoverableAmount: recoverable,
            outstandingReceivable: outstanding,
            description: None,
            createdAt: created_at.map(str::to_owned),
            calculatedAt: created_at.map(str::to_owned),
            finalizedAt: None,
        }
    }

    let prior_overpayment = batch(
        "retro-43d8c24e-6c31-416b-b203-c334568697c1",
        dec!(-160872.98),
        dec!(160872.98),
        dec!(160872.98),
        RetroSettlementStatus::OVERPAYMENT,
        None,
    );
    let zero_recovery = batch(
        "retro-0cbb12f3-4999-43d6-8215-9b710093c5d4",
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(160872.98),
        RetroSettlementStatus::UNSETTLED,
        Some("2026-09-30T11:43:10.586Z"),
    );
    let later_overpayment = batch(
        "retro-f0000000-0000-4000-8000-000000000001",
        dec!(-10.00),
        dec!(10.00),
        dec!(160882.98),
        RetroSettlementStatus::OVERPAYMENT,
        Some("2026-10-01T11:43:10.586Z"),
    );

    let replay = replay_retro_outstanding_receivables(&[
        later_overpayment.clone(),
        zero_recovery.clone(),
        prior_overpayment.clone(),
    ])
    .expect("runtime-valid receivable chain should replay");
    assert_eq!(replay.get("p1"), Some(&dec!(160882.98)));

    let mut corrupted_zero_recovery = zero_recovery;
    corrupted_zero_recovery.outstandingReceivable = Decimal::ZERO;
    let error = validate_v5_retro_outstanding_receivables(&[
        prior_overpayment,
        corrupted_zero_recovery,
        later_overpayment,
    ])
    .expect_err("an incorrect zero-difference receivable snapshot must remain invalid");
    assert!(error
        .to_string()
        .contains("retro-0cbb12f3-4999-43d6-8215-9b710093c5d4"));
}

#[test]
fn receivable_replay_filters_drafts_and_rejects_invalid_v5_settlement_flows() {
    fn batch(
        id: &str,
        status: CompensationRevisionStatus,
        amounts: (Decimal, Decimal, Decimal, Decimal, Decimal),
        settlement_status: RetroSettlementStatus,
    ) -> RetroAdjustmentBatch {
        let (total, recoverable, offset, recovered, outstanding) = amounts;
        RetroAdjustmentBatch {
            id: id.into(),
            revisionId: format!("revision-{id}"),
            personnelId: "p1".into(),
            paymentDate: "2027-03-14".into(),
            status,
            settlementStatus: settlement_status,
            totalGrossDelta: total,
            payableSettlementAmount: Decimal::ZERO,
            offsetSettlementAmount: offset,
            recoveredAmount: recovered,
            recoverableAmount: recoverable,
            outstandingReceivable: outstanding,
            description: None,
            createdAt: Some("2027-03-14T12:00:00Z".into()),
            calculatedAt: Some("2027-03-14T12:00:00Z".into()),
            finalizedAt: None,
        }
    }

    // Draft and stale rows are historical edits, not payment events, and must
    // never alter the replayed balance. The legacy negative row is still
    // accepted by the runtime compatibility path and derives its receivable
    // from the signed entitlement delta.
    let draft = batch(
        "draft",
        CompensationRevisionStatus::DRAFT,
        (
            dec!(-100),
            dec!(100),
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100),
        ),
        RetroSettlementStatus::OVERPAYMENT,
    );
    let stale = batch(
        "stale",
        CompensationRevisionStatus::STALE,
        (dec!(-50), dec!(50), Decimal::ZERO, Decimal::ZERO, dec!(50)),
        RetroSettlementStatus::OVERPAYMENT,
    );
    let legacy = batch(
        "legacy-negative",
        CompensationRevisionStatus::CALCULATED,
        (
            dec!(-25),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        ),
        RetroSettlementStatus::OVERPAYMENT,
    );
    let replay = replay_retro_outstanding_receivables(&[draft, stale, legacy])
        .expect("non-authoritative rows must be ignored and legacy flow replayed");
    assert_eq!(replay.get("p1"), Some(&dec!(25)));

    let mut over_recovered = batch(
        "over-recovered",
        CompensationRevisionStatus::CALCULATED,
        (dec!(-100), dec!(100), Decimal::ZERO, dec!(101), dec!(100)),
        RetroSettlementStatus::OVERPAYMENT,
    );
    let error = validate_v5_retro_outstanding_receivables(&[over_recovered.clone()])
        .expect_err("recovery cannot exceed the available receivable");
    assert!(error.to_string().contains("over-recovered"));

    over_recovered.recoveredAmount = Decimal::ZERO;
    over_recovered.outstandingReceivable = dec!(99);
    let error = validate_v5_retro_outstanding_receivables(&[over_recovered])
        .expect_err("V5 snapshot must equal chronological replay");
    assert!(error.to_string().contains("outstanding receivable"));
}

#[test]
fn dated_transport_replay_preserves_historical_schedule_and_applies_transport_override() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    update_policy_for_period(&mut source, "2026-02", |state| {
        state.gunlukVasitaYol = dec!(10);
    });
    let mut march_road = source.compensationPolicyVersions[0].clone();
    march_road.id = "transport-road-march".into();
    march_road.effectiveFrom = "2026-03-05".into();
    march_road.state.gunlukVasitaYol = dec!(20);
    march_road.boundaryScope = CompensationBoundaryScope::RoadOnly;
    march_road.supersedesVersionId = None;
    source.compensationPolicyVersions.push(march_road);
    let normal = normal_payroll(&source, "2026-02", "2026-03-14", 0);
    assert_eq!(normal.gelirler.vasitaYol, Some(dec!(380)));
    source.payrolls.push(normal);
    let wage_only = wage_retro_result(
        source.clone(),
        "transport-replay-wage",
        "rev-wage",
        dec!(110),
    );
    assert!(!wage_only
        .allocations
        .iter()
        .any(|allocation| allocation.earningCode == RetroEarningCode::TRANSPORT));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "transport-retro",
        revision("rev-transport", "2026-02-15"),
        vec![CompensationRevisionOverride {
            id: "transport-override".into(),
            revisionId: "rev-transport".into(),
            parameter: RetroParameterKey::GUNLUK_VASITA_YOL,
            value: dec!(30),
            personnelId: None,
        }],
        "2026-06-20",
    ))
    .unwrap();
    let transport = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::TRANSPORT)
        .unwrap();
    // 28 working days × 30 minus 18×10 + 10×20 already paid.
    assert_eq!(transport.deltaAmount, dec!(460));
}

#[test]
fn coverage_authoritative_wage_retro_reconciles_source_pek_and_worker_premium() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let normal = normal_payroll(&source, "2026-02", "2026-03-14", 0);
    assert_eq!(normal.gelirler.tabanBrutAylik, Some(dec!(2800)));
    source.payrolls.push(normal);

    let result = wage_retro_result(source, "coverage-wage", "coverage-rev", dec!(110));
    let wage = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("wage allocation");
    // 28 service days × (110 - 100) = 280 TL.
    assert_eq!(wage.deltaAmount, dec!(280));
    assert_eq!(wage.retroPekDelta, dec!(280));
    assert_eq!(wage.workerSgkDelta, dec!(39.20));
    assert_eq!(wage.workerUnemploymentDelta, dec!(2.80));
    assert_eq!(result.batch.totalGrossDelta, dec!(280));
}

#[test]
fn coverage_retro_payment_income_separates_wage_source_and_non_wage_payment_month() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let normal = normal_payroll(&source, "2026-02", "2026-03-14", 0);
    source.payrolls.push(normal);

    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "coverage-mixed",
        revision("coverage-mixed-rev", "2026-02-15"),
        vec![
            wage_override("coverage-wage-override", "coverage-mixed-rev", dec!(110)),
            CompensationRevisionOverride {
                id: "coverage-premium-override".into(),
                revisionId: "coverage-mixed-rev".into(),
                parameter: RetroParameterKey::IS_PRIMI_YUZDE,
                value: dec!(10),
                personnelId: None,
            },
        ],
        "2026-06-20",
    ))
    .expect("mixed authoritative retro should calculate");

    let wage = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("wage allocation");
    let premium = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::WORK_PREMIUM)
        .expect("work premium allocation");
    assert_eq!(wage.deltaAmount, dec!(280));
    assert_eq!(premium.deltaAmount, dec!(308));
    assert_eq!(wage.sgkTreatment, RetroSgkTreatment::WAGE_SOURCE_MONTH);
    assert_eq!(
        premium.sgkTreatment,
        RetroSgkTreatment::NON_WAGE_PAYMENT_MONTH
    );
    assert_eq!(wage.retroPekDelta, dec!(280));
    assert_eq!(premium.retroPekDelta, dec!(0));
    assert_eq!(result.batch.totalGrossDelta, dec!(588));
}

#[test]
fn coverage_authoritative_wage_retro_emits_negative_delta_without_payable_cash() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let normal = normal_payroll(&source, "2026-02", "2026-03-14", 0);
    assert_eq!(normal.gelirler.tabanBrutAylik, Some(dec!(2800)));
    source.payrolls.push(normal);

    let result = wage_retro_result(
        source,
        "coverage-negative-wage",
        "coverage-negative-rev",
        dec!(90),
    );
    let wage = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == RetroEarningCode::BASE_WAGE)
        .expect("negative wage allocation");
    // 28 service days × (90 - 100) = -280 TL; negative entitlement is
    // recoverable and must not become a positive payment.
    assert_eq!(wage.deltaAmount, dec!(-280));
    assert_eq!(wage.retroPekDelta, dec!(-280));
    assert_eq!(wage.workerSgkDelta, dec!(-39.20));
    assert_eq!(wage.workerUnemploymentDelta, dec!(-2.80));
    assert_eq!(result.batch.totalGrossDelta, dec!(-280));
    assert_eq!(result.batch.payableSettlementAmount, dec!(0));
    assert_eq!(result.batch.recoverableAmount, dec!(280));
}

#[test]
fn coverage_authoritative_wage_retro_zero_delta_is_idempotent() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let normal = normal_payroll(&source, "2026-02", "2026-03-14", 0);
    source.payrolls.push(normal);

    let result = wage_retro_result(source, "coverage-zero-wage", "coverage-zero-rev", dec!(100));
    assert_eq!(result.batch.totalGrossDelta, Decimal::ZERO);
    assert_eq!(result.batch.payableSettlementAmount, Decimal::ZERO);
    assert_eq!(result.batch.recoverableAmount, Decimal::ZERO);
    assert!(result
        .allocations
        .iter()
        .all(|allocation| allocation.deltaAmount == Decimal::ZERO));
}
