use payroll_core::{
    evaluate_payroll_invalidation, AccrualType, BordroDonemi, BordroKaydi, BordroStatus,
    CompensationRevisionStatus, DevredenPekKaydi, GelirKalemleri, KesintiKalemleri, MutationImpact,
    PayrollDatasetSnapshot, PayrollMutation, PuantajOzeti, ResolvedStatutorySnapshot,
    RetroAdjustmentBatch, RetroAllocation, RetroEarningCode, RetroSettlementStatus,
    RetroSgkTreatment, RetroTaxTreatment, StatutorySnapshotSource,
};

fn period_custom(id: &str, start: &str, end: &str, tax_year: i32, tax_month: i32) -> BordroDonemi {
    BordroDonemi {
        id: id.into(),
        yil: start[0..4].parse().unwrap(),
        ay: start[5..7].parse().unwrap(),
        baslangicTarihi: start.into(),
        bitisTarihi: end.into(),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

// This fixture keeps identity, ordering, status, and snapshot source explicit
// so each invalidation predicate can be exercised independently.
#[allow(clippy::too_many_arguments)]
fn payroll_for(
    person: &str,
    period_id: &str,
    accrual_id: &str,
    status: BordroStatus,
    accrual_type: AccrualType,
    payment_date: &str,
    sequence: i32,
    source: StatutorySnapshotSource,
) -> BordroKaydi {
    BordroKaydi {
        id: accrual_id.into(),
        personelId: person.into(),
        donemId: period_id.into(),
        accrualId: accrual_id.into(),
        accrualType: accrual_type,
        paymentDate: payment_date.into(),
        sequence,
        accrualDescription: None,
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri::default(),
        gelirToplam: Default::default(),
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: Default::default(),
        netOdeme: Default::default(),
        status,
        olusturulmaTarihi: "2026-09-15T00:00:00Z".into(),
        sonGuncellemeTarihi: "2026-09-15T00:00:00Z".into(),
        notlar: None,
        oncekiKumulatifGvMatrahi: None,
        oncekiKumulatifAsgariGvMatrahi: None,
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: None,
        pekDetay: None,
        isPrimiDetay: None,
        gvDetay: None,
        persistedGvBase: None,
        damgaDetay: None,
        statutorySnapshot: Some(ResolvedStatutorySnapshot {
            source,
            segments: Vec::new(),
            sgkPrimGunSayisi: 0,
            pekAltSinir: Default::default(),
            pekUstSinir: Default::default(),
            sgkYemekIstisnasiToplam: Default::default(),
            gvYemekIstisnasiToplam: Default::default(),
            gvReferansGunlukAsgariUcret: Default::default(),
            sgkIsciOraniYuzde: None,
            issizlikIsciOraniYuzde: None,
        }),
        odenenRaporluGun: None,
        raporluGun: None,
    }
}

fn dataset(periods: Vec<BordroDonemi>, payrolls: Vec<BordroKaydi>) -> PayrollDatasetSnapshot {
    PayrollDatasetSnapshot {
        periods,
        payrolls,
        ..PayrollDatasetSnapshot::default()
    }
}

fn affected(impact: &MutationImpact, accrual_id: &str) -> bool {
    impact
        .affectedPayrolls
        .iter()
        .any(|key| key.accrualId == accrual_id)
}

fn retro_fixture(
    person: &str,
    source_period: &str,
    batch_id: &str,
    status: CompensationRevisionStatus,
) -> (RetroAdjustmentBatch, RetroAllocation) {
    let batch = RetroAdjustmentBatch {
        id: batch_id.into(),
        revisionId: format!("revision-{batch_id}"),
        personnelId: person.into(),
        paymentDate: "2026-06-20".into(),
        status,
        settlementStatus: if status == CompensationRevisionStatus::FINALIZED {
            RetroSettlementStatus::PAID
        } else {
            RetroSettlementStatus::UNSETTLED
        },
        totalGrossDelta: 10.into(),
        payableSettlementAmount: 10.into(),
        offsetSettlementAmount: 0.into(),
        recoveredAmount: 0.into(),
        recoverableAmount: 0.into(),
        outstandingReceivable: 0.into(),
        description: None,
        createdAt: None,
        calculatedAt: None,
        finalizedAt: None,
    };
    let allocation = RetroAllocation {
        id: format!("allocation-{batch_id}"),
        batchId: batch.id.clone(),
        personnelId: person.into(),
        sourcePeriodId: source_period.into(),
        earningCode: RetroEarningCode::BASE_WAGE,
        originalRecognizedAmount: 0.into(),
        previousAuthoritativeRetroAmount: 0.into(),
        targetAmount: 10.into(),
        deltaAmount: 10.into(),
        sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
        incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
        stampTaxTreatment: RetroTaxTreatment::TAXABLE,
        originalPek: 0.into(),
        retroPekDelta: 0.into(),
        adjustedPek: 0.into(),
        workerSgkDelta: 0.into(),
        workerUnemploymentDelta: 0.into(),
        employerSgkDelta: 0.into(),
        employerUnemploymentDelta: 0.into(),
        originalEmployerLowerBound: 0.into(),
        targetEmployerLowerBound: 0.into(),
        employerLowerBoundDelta: 0.into(),
        employerLowerBoundPremiumDelta: 0.into(),
        originalSourceCarry: Some(Vec::new()),
        targetSourceCarry: Some(vec![DevredenPekKaydi {
            tutar: 10.into(),
            kalanAySayisi: 1,
            kaynakDonemId: Some(source_period.into()),
        }]),
        payableSettlementAmount: 10.into(),
        offsetSettlementAmount: 0.into(),
        recoverableAmount: 0.into(),
        metadata: None,
    };
    (batch, allocation)
}

#[test]
fn period_mutation_predicates_are_independently_observable_at_boundaries() {
    let source = period_custom("source", "2026-03-15", "2026-04-14", 2026, 3);
    let later_start = period_custom("later-start", "2026-04-15", "2026-05-14", 2025, 1);
    let later_tax_month = period_custom("later-tax", "2026-02-15", "2026-03-14", 2026, 4);
    let equal_start_equal_tax = period_custom("equal", "2026-03-15", "2026-04-14", 2026, 3);
    let earlier_equal_tax = period_custom("earlier-equal-tax", "2026-02-15", "2026-03-14", 2026, 3);
    let other_year_later_month = period_custom("other-year", "2026-02-15", "2026-03-14", 2025, 12);

    let payrolls = vec![
        payroll_for(
            "p",
            "source",
            "same-id",
            BordroStatus::CALCULATED,
            AccrualType::NORMAL,
            "2026-03-20",
            0,
            StatutorySnapshotSource::AttendanceBacked,
        ),
        payroll_for(
            "p",
            "later-start",
            "later-start",
            BordroStatus::CALCULATED,
            AccrualType::NORMAL,
            "2026-04-20",
            0,
            StatutorySnapshotSource::AttendanceBacked,
        ),
        payroll_for(
            "p",
            "later-tax",
            "later-tax",
            BordroStatus::CALCULATED,
            AccrualType::NORMAL,
            "2026-03-20",
            0,
            StatutorySnapshotSource::AttendanceBacked,
        ),
        payroll_for(
            "p",
            "equal",
            "equal",
            BordroStatus::CALCULATED,
            AccrualType::NORMAL,
            "2026-03-20",
            0,
            StatutorySnapshotSource::AttendanceBacked,
        ),
        payroll_for(
            "p",
            "earlier-equal-tax",
            "earlier-equal-tax",
            BordroStatus::CALCULATED,
            AccrualType::NORMAL,
            "2026-03-20",
            0,
            StatutorySnapshotSource::AttendanceBacked,
        ),
        payroll_for(
            "p",
            "other-year",
            "other-year",
            BordroStatus::CALCULATED,
            AccrualType::NORMAL,
            "2026-03-20",
            0,
            StatutorySnapshotSource::AttendanceBacked,
        ),
    ];
    let data = dataset(
        vec![
            source,
            later_start,
            later_tax_month,
            equal_start_equal_tax,
            earlier_equal_tax,
            other_year_later_month,
        ],
        payrolls,
    );

    let impact = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::Period {
            periodId: "source".into(),
        },
    )
    .unwrap();
    assert!(affected(&impact, "same-id"));
    assert!(affected(&impact, "later-start"));
    assert!(affected(&impact, "later-tax"));
    assert!(!affected(&impact, "equal"));
    assert!(!affected(&impact, "earlier-equal-tax"));
    assert!(!affected(&impact, "other-year"));
}

#[test]
fn person_and_tax_year_mutations_are_scoped_to_their_requested_dimension() {
    let current_year = period_custom("current-year", "2026-01-15", "2026-02-14", 2026, 1);
    let previous_year = period_custom("previous-year", "2025-01-15", "2025-02-14", 2025, 1);
    let data = dataset(
        vec![current_year, previous_year],
        vec![
            payroll_for(
                "person-1",
                "current-year",
                "person-1-current",
                BordroStatus::CALCULATED,
                AccrualType::NORMAL,
                "2026-02-10",
                0,
                StatutorySnapshotSource::AttendanceBacked,
            ),
            payroll_for(
                "person-1",
                "previous-year",
                "person-1-previous",
                BordroStatus::CALCULATED,
                AccrualType::NORMAL,
                "2025-02-10",
                0,
                StatutorySnapshotSource::AttendanceBacked,
            ),
            payroll_for(
                "person-2",
                "current-year",
                "person-2-current",
                BordroStatus::CALCULATED,
                AccrualType::NORMAL,
                "2026-02-10",
                0,
                StatutorySnapshotSource::AttendanceBacked,
            ),
        ],
    );

    let person = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::Person {
            personnelId: "person-1".into(),
        },
    )
    .unwrap();
    assert!(affected(&person, "person-1-current"));
    assert!(affected(&person, "person-1-previous"));
    assert!(!affected(&person, "person-2-current"));

    let person_year = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PersonTaxYear {
            personnelId: "person-1".into(),
            taxYear: 2026,
        },
    )
    .unwrap();
    assert!(affected(&person_year, "person-1-current"));
    assert!(!affected(&person_year, "person-1-previous"));
    assert!(!affected(&person_year, "person-2-current"));

    let year =
        evaluate_payroll_invalidation(&data, &PayrollMutation::TaxYear { taxYear: 2025 }).unwrap();
    assert!(!affected(&year, "person-1-current"));
    assert!(affected(&year, "person-1-previous"));
    assert!(!affected(&year, "person-2-current"));
}

#[test]
fn position_mutation_keeps_date_and_tax_position_predicates_independent() {
    let equal_date_other_year = period_custom("equal-date", "2026-03-15", "2026-04-14", 2025, 1);
    let earlier_same_tax_position =
        period_custom("same-tax-position", "2026-02-15", "2026-03-14", 2026, 3);
    let earlier_lower_month = period_custom("lower-month", "2026-02-15", "2026-03-14", 2026, 2);
    let earlier_other_year_high_month =
        period_custom("other-year-high", "2026-02-15", "2026-03-14", 2025, 12);
    let data = dataset(
        vec![
            equal_date_other_year,
            earlier_same_tax_position,
            earlier_lower_month,
            earlier_other_year_high_month,
        ],
        vec![
            payroll_for(
                "p",
                "equal-date",
                "equal-date",
                BordroStatus::CALCULATED,
                AccrualType::NORMAL,
                "2026-03-20",
                0,
                StatutorySnapshotSource::AttendanceBacked,
            ),
            payroll_for(
                "p",
                "same-tax-position",
                "same-tax-position",
                BordroStatus::CALCULATED,
                AccrualType::NORMAL,
                "2026-03-20",
                0,
                StatutorySnapshotSource::AttendanceBacked,
            ),
            payroll_for(
                "p",
                "lower-month",
                "lower-month",
                BordroStatus::CALCULATED,
                AccrualType::NORMAL,
                "2026-03-20",
                0,
                StatutorySnapshotSource::AttendanceBacked,
            ),
            payroll_for(
                "p",
                "other-year-high",
                "other-year-high",
                BordroStatus::CALCULATED,
                AccrualType::NORMAL,
                "2026-03-20",
                0,
                StatutorySnapshotSource::AttendanceBacked,
            ),
        ],
    );
    let impact = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PeriodFromPosition {
            startDate: "2026-03-15".into(),
            taxYear: 2026,
            taxMonth: 3,
        },
    )
    .unwrap();
    assert!(affected(&impact, "equal-date"));
    assert!(affected(&impact, "same-tax-position"));
    assert!(!affected(&impact, "lower-month"));
    assert!(!affected(&impact, "other-year-high"));
}

#[test]
fn attendance_mutation_ignores_unrelated_person_and_preserves_legacy_dependency() {
    let period = period_custom("2026-01", "2026-01-15", "2026-02-14", 2026, 1);
    let legacy = payroll_for(
        "person-1",
        "2026-01",
        "legacy",
        BordroStatus::CALCULATED,
        AccrualType::TEDIYE,
        "2026-01-20",
        0,
        StatutorySnapshotSource::LegacyUnknown,
    );
    let unrelated = payroll_for(
        "other",
        "2026-01",
        "other",
        BordroStatus::CALCULATED,
        AccrualType::NORMAL,
        "2026-01-21",
        0,
        StatutorySnapshotSource::AttendanceBacked,
    );
    let data = dataset(vec![period], vec![legacy, unrelated]);
    let impact = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PersonPeriod {
            personnelId: "person-1".into(),
            periodId: "2026-01".into(),
        },
    )
    .unwrap();
    assert!(affected(&impact, "legacy"));
    assert!(!affected(&impact, "other"));
}

#[test]
fn payroll_and_accrual_mutations_respect_identity_order_and_person_scope() {
    let p1 = period_custom("p1", "2026-01-15", "2026-02-14", 2026, 1);
    let p2 = period_custom("p2", "2026-02-15", "2026-03-14", 2026, 2);
    let current = payroll_for(
        "person-1",
        "p1",
        "current",
        BordroStatus::CALCULATED,
        AccrualType::NORMAL,
        "2026-02-10",
        0,
        StatutorySnapshotSource::AttendanceBacked,
    );
    let same_period_later = payroll_for(
        "person-1",
        "p1",
        "later",
        BordroStatus::CALCULATED,
        AccrualType::TEDIYE,
        "2026-02-11",
        0,
        StatutorySnapshotSource::AttendanceBacked,
    );
    let later_period = payroll_for(
        "person-1",
        "p2",
        "later-period",
        BordroStatus::CALCULATED,
        AccrualType::NORMAL,
        "2026-03-10",
        0,
        StatutorySnapshotSource::AttendanceBacked,
    );
    let other_person = payroll_for(
        "other",
        "p2",
        "other-person",
        BordroStatus::CALCULATED,
        AccrualType::NORMAL,
        "2026-03-10",
        0,
        StatutorySnapshotSource::AttendanceBacked,
    );
    let data = dataset(
        vec![p1, p2],
        vec![current, same_period_later, later_period, other_person],
    );

    let recalculation = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PayrollCalculation {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
        },
    )
    .unwrap();
    assert!(!affected(&recalculation, "current"));
    assert!(!affected(&recalculation, "later"));
    assert!(affected(&recalculation, "later-period"));
    assert!(!affected(&recalculation, "other-person"));

    let accrual_recalc = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualCalculation {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "current".into(),
        },
    )
    .unwrap();
    assert!(!affected(&accrual_recalc, "current"));
    assert!(affected(&accrual_recalc, "later"));
    assert!(affected(&accrual_recalc, "later-period"));
    assert!(!affected(&accrual_recalc, "other-person"));

    let delete = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualDelete {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "current".into(),
        },
    )
    .unwrap();
    assert!(affected(&delete, "current"));
    assert!(affected(&delete, "later"));
    assert!(affected(&delete, "later-period"));
    assert!(!affected(&delete, "other-person"));

    let insert = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualInsert {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "zz-inserted".into(),
            paymentDate: "2026-02-11".into(),
            sequence: 0,
        },
    )
    .unwrap();
    assert!(!affected(&insert, "current"));
    assert!(!affected(&insert, "later"));
    assert!(affected(&insert, "later-period"));
    assert!(!affected(&insert, "other-person"));
}

#[test]
fn retro_batch_mutations_enforce_person_source_and_settlement_identity() {
    let p1 = period_custom("p1", "2026-01-15", "2026-02-14", 2026, 1);
    let p2 = period_custom("p2", "2026-02-15", "2026-03-14", 2026, 2);
    let (batch, allocation) = retro_fixture(
        "person-1",
        "p1",
        "retro-batch",
        CompensationRevisionStatus::CALCULATED,
    );
    let mut data = dataset(vec![p1, p2], Vec::new());
    data.retroBatches.push(batch);
    data.retroAllocations.push(allocation);

    let person = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::Person {
            personnelId: "person-1".into(),
        },
    )
    .unwrap();
    assert_eq!(person.affectedRetroBatches, vec!["retro-batch"]);

    let wrong_person = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::Person {
            personnelId: "other".into(),
        },
    )
    .unwrap();
    assert!(wrong_person.affectedRetroBatches.is_empty());

    let person_tax_year = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PersonTaxYear {
            personnelId: "person-1".into(),
            taxYear: 2026,
        },
    )
    .unwrap();
    assert_eq!(person_tax_year.affectedRetroBatches, vec!["retro-batch"]);

    let wrong_tax_year =
        evaluate_payroll_invalidation(&data, &PayrollMutation::TaxYear { taxYear: 2025 }).unwrap();
    assert!(wrong_tax_year.affectedRetroBatches.is_empty());

    let payroll_calculation = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PayrollCalculation {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
        },
    )
    .unwrap();
    assert_eq!(
        payroll_calculation.affectedRetroBatches,
        vec!["retro-batch"]
    );

    let wrong_payroll_calculation = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PayrollCalculation {
            personnelId: "person-1".into(),
            periodId: "p2".into(),
        },
    )
    .unwrap();
    assert!(wrong_payroll_calculation.affectedRetroBatches.is_empty());

    let person_from_date = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PersonFromDate {
            personnelId: "person-1".into(),
            effectiveFrom: "2026-01-01".into(),
        },
    )
    .unwrap();
    assert_eq!(person_from_date.affectedRetroBatches, vec!["retro-batch"]);

    let source_delete = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualDelete {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "source-payroll".into(),
        },
    )
    .unwrap();
    assert_eq!(source_delete.affectedRetroBatches, vec!["retro-batch"]);

    let person_period = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PersonPeriod {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
        },
    )
    .unwrap();
    assert_eq!(person_period.affectedRetroBatches, vec!["retro-batch"]);
    let wrong_person = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PersonPeriod {
            personnelId: "other".into(),
            periodId: "p1".into(),
        },
    )
    .unwrap();
    assert!(wrong_person.affectedRetroBatches.is_empty());
    let wrong_period = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::PersonPeriod {
            personnelId: "person-1".into(),
            periodId: "p2".into(),
        },
    )
    .unwrap();
    assert!(wrong_period.affectedRetroBatches.is_empty());

    let settlement_recalc = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualCalculation {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "retro-batch".into(),
        },
    )
    .unwrap();
    assert!(settlement_recalc.affectedRetroBatches.is_empty());
    let source_recalc = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualCalculation {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "source-payroll".into(),
        },
    )
    .unwrap();
    assert_eq!(source_recalc.affectedRetroBatches, vec!["retro-batch"]);

    let settlement_delete = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualDelete {
            personnelId: "person-1".into(),
            periodId: "p2".into(),
            accrualId: "retro-batch".into(),
        },
    )
    .unwrap();
    assert_eq!(settlement_delete.affectedRetroBatches, vec!["retro-batch"]);
    let unrelated_delete = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualDelete {
            personnelId: "other".into(),
            periodId: "p2".into(),
            accrualId: "retro-batch".into(),
        },
    )
    .unwrap();
    assert!(unrelated_delete.affectedRetroBatches.is_empty());

    let settlement_insert = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualInsert {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "retro-batch".into(),
            paymentDate: "2026-06-20".into(),
            sequence: 0,
        },
    )
    .unwrap();
    assert!(settlement_insert.affectedRetroBatches.is_empty());
    let source_insert = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::AccrualInsert {
            personnelId: "person-1".into(),
            periodId: "p1".into(),
            accrualId: "new-source".into(),
            paymentDate: "2026-02-10".into(),
            sequence: 0,
        },
    )
    .unwrap();
    assert_eq!(source_insert.affectedRetroBatches, vec!["retro-batch"]);
}

#[test]
fn retro_source_carry_save_obeys_replay_and_payment_date_boundaries() {
    let p1 = period_custom("p1", "2026-01-15", "2026-02-14", 2026, 1);
    let p2 = period_custom("p2", "2026-02-15", "2026-03-14", 2026, 2);
    let before = payroll_for(
        "person-1",
        "p1",
        "before",
        BordroStatus::CALCULATED,
        AccrualType::SUPPLEMENTAL,
        "2026-06-19",
        0,
        StatutorySnapshotSource::ProvisionalPaymentMonth,
    );
    let at_boundary = payroll_for(
        "person-1",
        "p2",
        "at-boundary",
        BordroStatus::CALCULATED,
        AccrualType::SUPPLEMENTAL,
        "2026-06-20",
        0,
        StatutorySnapshotSource::ProvisionalPaymentMonth,
    );
    let other_person = payroll_for(
        "other",
        "p2",
        "other-person",
        BordroStatus::CALCULATED,
        AccrualType::SUPPLEMENTAL,
        "2026-06-21",
        0,
        StatutorySnapshotSource::ProvisionalPaymentMonth,
    );
    let (batch, allocation) = retro_fixture(
        "person-1",
        "p1",
        "retro-batch",
        CompensationRevisionStatus::CALCULATED,
    );
    let mut data = dataset(vec![p1, p2], vec![before, at_boundary, other_person]);
    data.retroBatches.push(batch);
    data.retroAllocations.push(allocation);

    let mutation = PayrollMutation::RetroBatchSave {
        personnelId: "person-1".into(),
        batchId: "retro-batch".into(),
        paymentDate: "2026-06-20".into(),
    };
    let impact = evaluate_payroll_invalidation(&data, &mutation).unwrap();
    assert!(!affected(&impact, "before"));
    assert!(affected(&impact, "at-boundary"));
    assert!(!affected(&impact, "other-person"));

    data.retroAllocations[0].originalSourceCarry =
        data.retroAllocations[0].targetSourceCarry.clone();
    let no_replay = evaluate_payroll_invalidation(&data, &mutation).unwrap();
    assert!(no_replay.affectedPayrolls.is_empty());
}

#[test]
fn retro_source_carry_replay_ignores_non_wage_carry_changes() {
    let p1 = period_custom("p1", "2026-01-15", "2026-02-14", 2026, 1);
    let p2 = period_custom("p2", "2026-02-15", "2026-03-14", 2026, 2);
    let at_boundary = payroll_for(
        "person-1",
        "p2",
        "at-boundary",
        BordroStatus::CALCULATED,
        AccrualType::SUPPLEMENTAL,
        "2026-06-20",
        0,
        StatutorySnapshotSource::ProvisionalPaymentMonth,
    );
    let (batch, mut wage) = retro_fixture(
        "person-1",
        "p1",
        "retro-batch",
        CompensationRevisionStatus::CALCULATED,
    );
    wage.originalSourceCarry = wage.targetSourceCarry.clone();
    let mut non_wage = wage.clone();
    non_wage.id = "non-wage-carry-change".into();
    non_wage.earningCode = RetroEarningCode::WORK_PREMIUM;
    non_wage.sgkTreatment = RetroSgkTreatment::NON_WAGE_PAYMENT_MONTH;
    non_wage.originalSourceCarry = None;
    non_wage.targetSourceCarry = Some(vec![DevredenPekKaydi {
        tutar: 10.into(),
        kalanAySayisi: 1,
        kaynakDonemId: Some("p1".into()),
    }]);

    let mut data = dataset(vec![p1, p2], vec![at_boundary]);
    data.retroBatches.push(batch);
    data.retroAllocations.extend([wage, non_wage]);
    let impact = evaluate_payroll_invalidation(
        &data,
        &PayrollMutation::RetroBatchSave {
            personnelId: "person-1".into(),
            batchId: "retro-batch".into(),
            paymentDate: "2026-06-20".into(),
        },
    )
    .unwrap();

    assert!(impact.affectedPayrolls.is_empty());
}
