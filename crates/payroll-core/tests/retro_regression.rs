use chrono::{Duration, NaiveDate};
use payroll_core::{
    calculate_payroll, AccrualType, AnnualPayrollParameters, BordroDonemi, BordroKaydi,
    BordroStatus, CompensationRevision, CompensationRevisionOverride, CompensationRevisionReason,
    CompensationRevisionScope, CompensationRevisionStatus, DonemselKurumDegerleri,
    PayrollAccrualInput, PayrollCalculationRequest, PayrollDatasetSnapshot, Personel,
    PersonelPuantaj, RetroAdjustmentBatch, RetroAllocation, RetroEarningCode,
    RetroEntitlementEngine, RetroParameterKey, RetroSgkTreatment, RetroTaxTreatment,
    RetroSettlementStatus, SickLeaveRecord,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

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
    PayrollDatasetSnapshot {
        personnel: vec![person()],
        periods: periods.to_vec(),
        institutionSettings: periods
            .iter()
            .map(|period| {
                (
                    period.id.clone(),
                    settings(&period.id, daily_wage, pek_multiplier),
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
        ..PayrollDatasetSnapshot::default()
    }
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
        description: None,
        createdAt: None,
        calculatedAt: None,
        finalizedAt: None,
    });

    let error = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "shared-batch-id",
        revision("current-revision", "2026-02-15"),
        vec![wage_override("current-override", "current-revision", dec!(120))],
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
    assert_eq!(second.allocations[0].previousAuthoritativeRetroAmount, dec!(560));
    assert_eq!(second.allocations[0].targetAmount, dec!(3640));
    assert_eq!(second.allocations[0].deltaAmount, dec!(280));
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
        vec![wage_override("ov-later-payment", "rev-later-payment", dec!(120))],
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
        vec![wage_override(
            "ov-same-day-1",
            "rev-same-day-1",
            dec!(120),
        )],
        "2026-06-20",
    ))
    .expect("first same-day retro should calculate");
    source.retroBatches.push(first.batch);
    source.retroAllocations.extend(first.allocations);

    let second = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-same-day-2",
        revision("rev-same-day-2", "2026-02-15"),
        vec![wage_override(
            "ov-same-day-2",
            "rev-same-day-2",
            dec!(125),
        )],
        "2026-06-20",
    ))
    .expect("a new same-day retro event is appended after existing authoritative events");

    assert_eq!(second.allocations[0].previousAuthoritativeRetroAmount, dec!(560));
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
    source.retroAllocations.extend([draft_allocation, stale_allocation]);

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
    assert_eq!(result.allocations[0].previousAuthoritativeRetroAmount, Decimal::ZERO);
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
        vec![wage_override("ov-closed-only", "rev-closed-only", dec!(120))],
        "2026-06-20",
    ))
    .expect("closed source period should calculate");

    assert_eq!(result.periods.len(), 1);
    assert_eq!(result.periods[0].sourcePeriodId, "2026-02");
    assert_eq!(result.batch.totalGrossDelta, dec!(560));
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
    source
        .institutionSettings
        .get_mut("2026-02")
        .expect("source settings")
        .geceCalismaPrimiYuzde = Some(dec!(1));
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
    assert!(result.allocations[0].employerSgkDelta < Decimal::ZERO);
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

    assert_eq!(recovery.allocations[0].previousAuthoritativeRetroAmount, dec!(-280));
    assert_eq!(recovery.allocations[0].targetAmount, dec!(2660));
    assert_eq!(recovery.allocations[0].deltaAmount, dec!(140));
    assert_eq!(recovery.allocations[0].retroPekDelta, dec!(140));
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
fn meal_and_clothing_retro_use_source_month_statutory_pek_policy() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let mut source = dataset(&[source_period], dec!(100), dec!(9));
    let source_settings = source
        .institutionSettings
        .get_mut("2026-02")
        .expect("source settings");
    // 28 worked days: the historical SGK meal exemption is 8,400, so the
    // original 5,600 meal is fully exempt while the revised 11,200 meal has
    // only 2,800 of source-month PEK subject to SGK.
    source_settings.gunlukYemek = dec!(200);
    source_settings.giyimYardimi = dec!(100);
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
}

#[test]
fn missing_original_accrual_uses_historical_attendance_and_statutory_snapshot() {
    let source_period = period("2026-02", "2026-02-15", "2026-03-14", 3);
    let source = dataset(&[source_period], dec!(100), dec!(9));
    let result = RetroEntitlementEngine::calculate(&retro_request(
        source,
        "retro-missing",
        revision("rev-missing", "2026-02-15"),
        vec![wage_override("ov-missing", "rev-missing", dec!(120))],
        "2026-06-20",
    ))
    .expect("missing-accrual retro should use attendance-backed PEK ceiling");

    assert_eq!(result.batch.totalGrossDelta, dec!(3360));
    assert_eq!(
        result.allocations[0].originalRecognizedAmount,
        Decimal::ZERO
    );
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
