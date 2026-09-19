use payroll_core::{
    retro_earning_policy, retro_payable_allocation_amount, retro_payable_settlement_amount,
    retro_payment_income, retro_sgk_ledger_totals, BordroDonemi, CompensationRevisionStatus,
    DevredenPekKaydi, PayrollDatasetSnapshot, RetroAdjustmentBatch, RetroAllocation,
    RetroEarningCode, RetroSettlementStatus,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn period(id: &str) -> BordroDonemi {
    BordroDonemi {
        id: id.into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: id.into(),
        taxYear: 2026,
        taxMonth: 1,
    }
}

fn batch(
    id: &str,
    total: Decimal,
    payable: Decimal,
    offset: Decimal,
    recoverable: Decimal,
    status: CompensationRevisionStatus,
    settlement: RetroSettlementStatus,
) -> RetroAdjustmentBatch {
    RetroAdjustmentBatch {
        id: id.into(),
        revisionId: format!("revision-{id}"),
        personnelId: "p1".into(),
        paymentDate: "2026-06-20".into(),
        status,
        settlementStatus: settlement,
        totalGrossDelta: total,
        payableSettlementAmount: payable,
        offsetSettlementAmount: offset,
        recoveredAmount: Decimal::ZERO,
        recoverableAmount: recoverable,
        outstandingReceivable: recoverable,
        description: None,
        createdAt: None,
        calculatedAt: None,
        finalizedAt: None,
    }
}

// Each ledger field is an independent mutation boundary in these fixtures.
#[allow(clippy::too_many_arguments)]
fn allocation(
    id: &str,
    batch_id: &str,
    code: RetroEarningCode,
    original: Decimal,
    previous: Decimal,
    target: Decimal,
    delta: Decimal,
    payable: Decimal,
    offset: Decimal,
    recoverable: Decimal,
) -> RetroAllocation {
    let policy = retro_earning_policy(code);
    RetroAllocation {
        id: id.into(),
        batchId: batch_id.into(),
        personnelId: "p1".into(),
        sourcePeriodId: "p1".into(),
        earningCode: code,
        originalRecognizedAmount: original,
        previousAuthoritativeRetroAmount: previous,
        targetAmount: target,
        deltaAmount: delta,
        sgkTreatment: policy.sgkTreatment,
        incomeTaxTreatment: policy.incomeTaxTreatment,
        stampTaxTreatment: policy.stampTaxTreatment,
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
        payableSettlementAmount: payable,
        offsetSettlementAmount: offset,
        recoverableAmount: recoverable,
        metadata: None,
    }
}

fn dataset(
    batch: RetroAdjustmentBatch,
    allocations: Vec<RetroAllocation>,
) -> PayrollDatasetSnapshot {
    PayrollDatasetSnapshot {
        periods: vec![period("p1")],
        retroBatches: vec![batch],
        retroAllocations: allocations,
        ..PayrollDatasetSnapshot::default()
    }
}

type NegativeAllocationSnapshotMutation = (&'static str, fn(&mut RetroAllocation));

#[test]
fn sgk_ledger_totals_sum_deltas_but_take_maximum_authoritative_snapshots() {
    let mut first = allocation(
        "a1",
        "b1",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(40),
        dec!(40),
        dec!(40),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    first.originalPek = dec!(100);
    first.retroPekDelta = dec!(20);
    first.adjustedPek = dec!(120);
    first.workerSgkDelta = dec!(2.80);
    first.workerUnemploymentDelta = dec!(0.20);
    first.employerSgkDelta = dec!(4.35);
    first.employerUnemploymentDelta = dec!(0.40);

    let mut second = allocation(
        "a2",
        "b1",
        RetroEarningCode::NIGHT_WORK,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(60),
        dec!(60),
        dec!(60),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    second.originalPek = dec!(150);
    second.retroPekDelta = dec!(30);
    second.adjustedPek = dec!(180);
    second.workerSgkDelta = dec!(4.20);
    second.workerUnemploymentDelta = dec!(0.30);
    second.employerSgkDelta = dec!(6.53);
    second.employerUnemploymentDelta = dec!(0.60);

    let totals = retro_sgk_ledger_totals(&[first, second]);
    let p1 = totals.get("p1").expect("source period ledger should exist");
    assert_eq!(p1.0, dec!(150));
    assert_eq!(p1.1, dec!(50));
    assert_eq!(p1.2, dec!(180));
    assert_eq!(p1.3, dec!(7.00));
    assert_eq!(p1.4, dec!(0.50));
    assert_eq!(p1.5, dec!(10.88));
    assert_eq!(p1.6, dec!(1.00));
    assert_eq!(p1.7, dec!(100));
}

#[test]
fn payment_income_keeps_source_wage_out_of_payment_month_pek() {
    let batch = batch(
        "b1",
        dec!(150),
        dec!(150),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let wage = allocation(
        "wage",
        "b1",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    let premium = allocation(
        "premium",
        "b1",
        RetroEarningCode::WORK_PREMIUM,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(50),
        dec!(50),
        dec!(50),
        Decimal::ZERO,
        Decimal::ZERO,
    );

    let (_, allocations, income, payment_month_pek) =
        retro_payment_income(&dataset(batch, vec![wage, premium]), "b1")
            .expect("valid retro payment ledger should resolve");
    assert_eq!(allocations.len(), 2);
    assert_eq!(income.tabanBrutAylik, Some(dec!(100)));
    assert_eq!(income.isPrimi, Some(dec!(50)));
    assert_eq!(payment_month_pek.tabanBrutAylik, None);
    assert_eq!(payment_month_pek.isPrimi, Some(dec!(50)));
}

#[test]
fn legacy_mixed_sign_batch_is_materialized_as_one_net_cash_flow() {
    let legacy = batch(
        "legacy",
        dec!(70),
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let positive = allocation(
        "positive",
        "legacy",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
    );
    let negative = allocation(
        "negative",
        "legacy",
        RetroEarningCode::WORK_PREMIUM,
        dec!(30),
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(-30),
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
    );

    assert_eq!(retro_payable_settlement_amount(&legacy), dec!(70));
    assert_eq!(
        retro_payable_allocation_amount(&legacy, &positive),
        Decimal::ZERO
    );

    let (_, allocations, income, _) =
        retro_payment_income(&dataset(legacy, vec![positive, negative]), "legacy")
            .expect("legacy mixed-sign batch should normalize locally");
    assert_eq!(
        allocations
            .iter()
            .map(|a| a.payableSettlementAmount)
            .sum::<Decimal>(),
        dec!(70)
    );
    assert_eq!(
        allocations
            .iter()
            .map(|a| a.recoverableAmount)
            .sum::<Decimal>(),
        Decimal::ZERO
    );
    assert_eq!(income.tabanBrutAylik, Some(dec!(70)));
    assert_eq!(income.isPrimi, Some(Decimal::ZERO));
}

#[test]
fn payment_ledger_rejects_broken_identity_policy_and_equation_invariants() {
    let valid_batch = batch(
        "b1",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let valid = allocation(
        "a1",
        "b1",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );

    let mut wrong_person = valid.clone();
    wrong_person.personnelId = "other".into();
    assert!(retro_payment_income(&dataset(valid_batch.clone(), vec![wrong_person]), "b1").is_err());

    let mut unknown_period = valid.clone();
    unknown_period.sourcePeriodId = "missing".into();
    assert!(
        retro_payment_income(&dataset(valid_batch.clone(), vec![unknown_period]), "b1").is_err()
    );

    let mut wrong_policy = valid.clone();
    wrong_policy.sgkTreatment = retro_earning_policy(RetroEarningCode::WORK_PREMIUM).sgkTreatment;
    assert!(retro_payment_income(&dataset(valid_batch.clone(), vec![wrong_policy]), "b1").is_err());

    let mut wrong_delta = valid.clone();
    wrong_delta.targetAmount = dec!(101);
    assert!(retro_payment_income(&dataset(valid_batch, vec![wrong_delta]), "b1").is_err());
}

#[test]
fn payment_ledger_rejects_negative_authoritative_fields_and_invalid_carry() {
    let valid_batch = batch(
        "b1",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let valid = allocation(
        "a1",
        "b1",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );

    let mut negative_original = valid.clone();
    negative_original.originalRecognizedAmount = dec!(-1);
    negative_original.targetAmount = dec!(99);
    assert!(
        retro_payment_income(&dataset(valid_batch.clone(), vec![negative_original]), "b1").is_err()
    );

    let mut negative_carry_amount = valid.clone();
    negative_carry_amount.originalSourceCarry = Some(vec![DevredenPekKaydi {
        tutar: dec!(-1),
        kalanAySayisi: 1,
        kaynakDonemId: Some("p1".into()),
    }]);
    assert!(retro_payment_income(
        &dataset(valid_batch.clone(), vec![negative_carry_amount]),
        "b1"
    )
    .is_err());

    let mut negative_carry_months = valid;
    negative_carry_months.targetSourceCarry = Some(vec![DevredenPekKaydi {
        tutar: dec!(1),
        kalanAySayisi: -1,
        kaynakDonemId: Some("p1".into()),
    }]);
    assert!(
        retro_payment_income(&dataset(valid_batch, vec![negative_carry_months]), "b1").is_err()
    );
}

#[test]
fn payment_ledger_rejects_each_negative_authoritative_allocation_snapshot() {
    let cases: [NegativeAllocationSnapshotMutation; 8] = [
        ("original PEK", |allocation| {
            allocation.originalPek = dec!(-1)
        }),
        ("adjusted PEK", |allocation| {
            allocation.adjustedPek = dec!(-1)
        }),
        ("original lower bound", |allocation| {
            allocation.originalEmployerLowerBound = dec!(-1)
        }),
        ("target lower bound", |allocation| {
            allocation.targetEmployerLowerBound = dec!(-1)
        }),
        ("payable", |allocation| {
            allocation.payableSettlementAmount = dec!(-1)
        }),
        ("offset", |allocation| {
            allocation.offsetSettlementAmount = dec!(-1)
        }),
        ("recoverable", |allocation| {
            allocation.recoverableAmount = dec!(-1)
        }),
        ("original recognized", |allocation| {
            allocation.originalRecognizedAmount = dec!(-1);
            allocation.targetAmount = dec!(99);
        }),
    ];

    for (label, mutate) in cases {
        let batch = batch(
            "negative-snapshot",
            dec!(100),
            dec!(100),
            Decimal::ZERO,
            Decimal::ZERO,
            CompensationRevisionStatus::CALCULATED,
            RetroSettlementStatus::UNSETTLED,
        );
        let mut allocation = allocation(
            "negative-snapshot-allocation",
            "negative-snapshot",
            RetroEarningCode::BASE_WAGE,
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100),
            dec!(100),
            dec!(100),
            Decimal::ZERO,
            Decimal::ZERO,
        );
        mutate(&mut allocation);
        assert!(
            retro_payment_income(&dataset(batch, vec![allocation]), "negative-snapshot").is_err(),
            "negative {label} must be rejected"
        );
    }

    let batch = batch(
        "negative-target",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let negative_target = allocation(
        "negative-target-allocation",
        "negative-target",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(-1),
        dec!(-1),
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
    );
    let positive_companion = allocation(
        "positive-target-allocation",
        "negative-target",
        RetroEarningCode::WORK_PREMIUM,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(101),
        dec!(101),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(batch, vec![negative_target, positive_companion]),
        "negative-target"
    )
    .is_err());
}

#[test]
fn payment_ledger_rejects_settlement_flows_that_exceed_signed_entitlement() {
    let cash_flow_batch = batch(
        "b1",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let overpaid = allocation(
        "a1",
        "b1",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        dec!(101),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(&dataset(cash_flow_batch, vec![overpaid]), "b1").is_err());

    let offset_only_batch = batch(
        "b2",
        dec!(100),
        Decimal::ZERO,
        dec!(100),
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::SETTLED_BY_OFFSET,
    );
    let offset = allocation(
        "a2",
        "b2",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        dec!(100),
        Decimal::ZERO,
    );
    assert_eq!(
        retro_payable_settlement_amount(&offset_only_batch),
        Decimal::ZERO
    );
    assert!(retro_payment_income(&dataset(offset_only_batch, vec![offset]), "b2").is_err());
}

#[test]
fn explicit_partial_offset_requires_unsettled_status_until_cash_is_paid() {
    let valid_batch = batch(
        "partial-flow",
        dec!(100),
        dec!(40),
        dec!(60),
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let valid_allocation = allocation(
        "partial-flow-a",
        "partial-flow",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        dec!(40),
        dec!(60),
        Decimal::ZERO,
    );

    let (_, allocations, income, _) = retro_payment_income(
        &dataset(valid_batch.clone(), vec![valid_allocation.clone()]),
        "partial-flow",
    )
    .expect("a split between cash and offset is a valid unsettled payment");
    assert_eq!(allocations[0].payableSettlementAmount, dec!(40));
    assert_eq!(allocations[0].offsetSettlementAmount, dec!(60));
    assert_eq!(income.tabanBrutAylik, Some(dec!(40)));

    for wrong_status in [
        RetroSettlementStatus::SETTLED_BY_OFFSET,
        RetroSettlementStatus::PAID,
        RetroSettlementStatus::OVERPAYMENT,
    ] {
        let mut invalid_batch = valid_batch.clone();
        invalid_batch.settlementStatus = wrong_status;
        assert!(
            retro_payment_income(
                &dataset(invalid_batch, vec![valid_allocation.clone()]),
                "partial-flow"
            )
            .is_err(),
            "cash plus offset cannot be declared {wrong_status:?}"
        );
    }

    let mut paid = valid_batch;
    paid.status = CompensationRevisionStatus::FINALIZED;
    paid.settlementStatus = RetroSettlementStatus::PAID;
    assert!(retro_payment_income(&dataset(paid, vec![valid_allocation]), "partial-flow").is_ok());
}

#[test]
fn allocation_policy_snapshot_checks_sgk_income_tax_and_stamp_tax_independently() {
    let valid_batch = batch(
        "policy-fields",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let valid_allocation = allocation(
        "policy-fields-a",
        "policy-fields",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100),
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(valid_batch.clone(), vec![valid_allocation.clone()]),
        "policy-fields"
    )
    .is_ok());

    let mut wrong_sgk = valid_allocation.clone();
    wrong_sgk.sgkTreatment = payroll_core::RetroSgkTreatment::NON_WAGE_PAYMENT_MONTH;
    let mut wrong_gv = valid_allocation.clone();
    wrong_gv.incomeTaxTreatment = payroll_core::RetroTaxTreatment::EXEMPT;
    let mut wrong_dv = valid_allocation;
    wrong_dv.stampTaxTreatment = payroll_core::RetroTaxTreatment::EXEMPT;

    for (label, broken) in [
        ("SGK", wrong_sgk),
        ("income tax", wrong_gv),
        ("stamp tax", wrong_dv),
    ] {
        assert!(
            retro_payment_income(&dataset(valid_batch.clone(), vec![broken]), "policy-fields")
                .is_err(),
            "{label} policy snapshot mismatch must fail independently"
        );
    }
}

#[test]
fn balanced_batch_totals_do_not_hide_one_allocation_exceeding_its_entitlement() {
    let valid_batch = batch(
        "balanced-batch",
        dec!(100),
        dec!(40),
        dec!(60),
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let first = allocation(
        "balanced-first",
        "balanced-batch",
        RetroEarningCode::BASE_WAGE,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(60),
        dec!(60),
        dec!(30),
        dec!(30),
        Decimal::ZERO,
    );
    let second = allocation(
        "balanced-second",
        "balanced-batch",
        RetroEarningCode::WORK_PREMIUM,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(40),
        dec!(40),
        dec!(10),
        dec!(30),
        Decimal::ZERO,
    );
    let (_, allocations, income, _) = retro_payment_income(
        &dataset(valid_batch.clone(), vec![first.clone(), second.clone()]),
        "balanced-batch",
    )
    .expect("both allocations should be valid at their own entitlement boundary");
    assert_eq!(allocations.len(), 2);
    assert_eq!(income.tabanBrutAylik, Some(dec!(30)));
    assert_eq!(income.isPrimi, Some(dec!(10)));

    // The batch total still reconciles, but the first allocation exceeds its
    // own positive delta; a total-only ledger check would silently accept it.
    let mut excess_cash = first.clone();
    excess_cash.payableSettlementAmount = dec!(31);
    let mut deficient_cash = second.clone();
    deficient_cash.payableSettlementAmount = dec!(9);
    assert!(retro_payment_income(
        &dataset(valid_batch.clone(), vec![excess_cash, deficient_cash]),
        "balanced-batch"
    )
    .is_err());

    // Repeat the same invariant with offset amounts, independently of cash.
    let mut deficient_offset = first;
    deficient_offset.offsetSettlementAmount = dec!(29);
    let mut excess_offset = second;
    excess_offset.offsetSettlementAmount = dec!(31);
    assert!(retro_payment_income(
        &dataset(valid_batch, vec![deficient_offset, excess_offset]),
        "balanced-batch"
    )
    .is_err());
}
