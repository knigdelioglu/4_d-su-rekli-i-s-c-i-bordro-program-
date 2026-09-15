use payroll_core::{
    retro_earning_policy, retro_payable_settlement_amount, retro_payment_income, BordroDonemi,
    CompensationRevisionStatus, DevredenPekKaydi, PayrollDatasetSnapshot, RetroAdjustmentBatch,
    RetroAllocation, RetroEarningCode, RetroSettlementStatus,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "p1".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "p1".into(),
        taxYear: 2026,
        taxMonth: 1,
    }
}

fn make_batch(
    id: &str,
    total: Decimal,
    payable: Decimal,
    offset: Decimal,
    recoverable: Decimal,
    status: CompensationRevisionStatus,
    settlement_status: RetroSettlementStatus,
) -> RetroAdjustmentBatch {
    RetroAdjustmentBatch {
        id: id.into(),
        revisionId: format!("revision-{id}"),
        personnelId: "person-1".into(),
        paymentDate: "2026-06-20".into(),
        status,
        settlementStatus: settlement_status,
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

fn make_allocation(
    id: &str,
    batch_id: &str,
    code: RetroEarningCode,
    delta: Decimal,
    payable: Decimal,
    offset: Decimal,
    recoverable: Decimal,
) -> RetroAllocation {
    let policy = retro_earning_policy(code);
    RetroAllocation {
        id: id.into(),
        batchId: batch_id.into(),
        personnelId: "person-1".into(),
        sourcePeriodId: "p1".into(),
        earningCode: code,
        originalRecognizedAmount: Decimal::ZERO,
        previousAuthoritativeRetroAmount: Decimal::ZERO,
        targetAmount: delta,
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

fn dataset(batch: RetroAdjustmentBatch, allocations: Vec<RetroAllocation>) -> PayrollDatasetSnapshot {
    PayrollDatasetSnapshot {
        periods: vec![period()],
        retroBatches: vec![batch],
        retroAllocations: allocations,
        ..PayrollDatasetSnapshot::default()
    }
}

#[test]
fn legacy_payable_fallback_requires_every_explicit_flow_to_be_zero() {
    let legacy = make_batch(
        "legacy",
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    assert_eq!(retro_payable_settlement_amount(&legacy), dec!(100));

    let mut explicit_payable = legacy.clone();
    explicit_payable.payableSettlementAmount = dec!(1);
    assert_eq!(retro_payable_settlement_amount(&explicit_payable), dec!(1));

    let mut offset = legacy.clone();
    offset.offsetSettlementAmount = dec!(1);
    assert_eq!(retro_payable_settlement_amount(&offset), Decimal::ZERO);

    let mut recovered = legacy.clone();
    recovered.recoveredAmount = dec!(1);
    assert_eq!(retro_payable_settlement_amount(&recovered), Decimal::ZERO);

    let mut recoverable = legacy.clone();
    recoverable.recoverableAmount = dec!(1);
    assert_eq!(retro_payable_settlement_amount(&recoverable), Decimal::ZERO);

    let mut outstanding = legacy.clone();
    outstanding.outstandingReceivable = dec!(1);
    assert_eq!(retro_payable_settlement_amount(&outstanding), Decimal::ZERO);

    let mut settled_by_offset = legacy.clone();
    settled_by_offset.settlementStatus = RetroSettlementStatus::SETTLED_BY_OFFSET;
    assert_eq!(
        retro_payable_settlement_amount(&settled_by_offset),
        Decimal::ZERO
    );

    let mut overpayment = legacy.clone();
    overpayment.settlementStatus = RetroSettlementStatus::OVERPAYMENT;
    assert_eq!(retro_payable_settlement_amount(&overpayment), Decimal::ZERO);

    let mut zero_total = legacy.clone();
    zero_total.totalGrossDelta = Decimal::ZERO;
    assert_eq!(retro_payable_settlement_amount(&zero_total), Decimal::ZERO);

    let mut negative_total = legacy;
    negative_total.totalGrossDelta = dec!(-1);
    assert_eq!(retro_payable_settlement_amount(&negative_total), Decimal::ZERO);
}

#[test]
fn zero_valued_carry_snapshot_is_valid_boundary_data() {
    let batch = make_batch(
        "b1",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let mut allocation = make_allocation(
        "a1",
        "b1",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    let zero_carry = vec![DevredenPekKaydi {
        tutar: Decimal::ZERO,
        kalanAySayisi: 0,
        kaynakDonemId: Some("p1".into()),
    }];
    allocation.originalSourceCarry = Some(zero_carry.clone());
    allocation.targetSourceCarry = Some(zero_carry);

    assert!(retro_payment_income(&dataset(batch, vec![allocation]), "b1").is_ok());
}

#[test]
fn finalized_payment_requires_paid_status_and_positive_cash_payable() {
    let paid_batch = make_batch(
        "paid",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::FINALIZED,
        RetroSettlementStatus::PAID,
    );
    let paid_allocation = make_allocation(
        "paid-a",
        "paid",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(&dataset(paid_batch, vec![paid_allocation]), "paid").is_ok());

    let wrong_status_batch = make_batch(
        "wrong-status",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::PAID,
    );
    let wrong_status_allocation = make_allocation(
        "wrong-status-a",
        "wrong-status",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(wrong_status_batch, vec![wrong_status_allocation]),
        "wrong-status"
    )
    .is_err());

    let finalized_offset_only = make_batch(
        "offset-finalized",
        dec!(100),
        Decimal::ZERO,
        dec!(100),
        Decimal::ZERO,
        CompensationRevisionStatus::FINALIZED,
        RetroSettlementStatus::PAID,
    );
    let offset_allocation = make_allocation(
        "offset-finalized-a",
        "offset-finalized",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        Decimal::ZERO,
        dec!(100),
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(finalized_offset_only, vec![offset_allocation]),
        "offset-finalized"
    )
    .is_err());
}

#[test]
fn explicit_settlement_totals_must_match_allocations_and_entitlement() {
    let batch_total_mismatch = make_batch(
        "allocation-mismatch",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let allocation_total_mismatch = make_allocation(
        "allocation-mismatch-a",
        "allocation-mismatch",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(99),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(batch_total_mismatch, vec![allocation_total_mismatch]),
        "allocation-mismatch"
    )
    .is_err());

    let entitlement_mismatch = make_batch(
        "entitlement-mismatch",
        dec!(100),
        dec!(60),
        dec!(30),
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let entitlement_mismatch_allocation = make_allocation(
        "entitlement-mismatch-a",
        "entitlement-mismatch",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(60),
        dec!(30),
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(
            entitlement_mismatch,
            vec![entitlement_mismatch_allocation]
        ),
        "entitlement-mismatch"
    )
    .is_err());

    let valid_split = make_batch(
        "valid-split",
        dec!(100),
        dec!(60),
        dec!(40),
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let valid_split_allocation = make_allocation(
        "valid-split-a",
        "valid-split",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(60),
        dec!(40),
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(valid_split, vec![valid_split_allocation]),
        "valid-split"
    )
    .is_ok());
}

#[test]
fn explicit_settlement_balances_cannot_be_negative() {
    let mut negative_recovered = make_batch(
        "negative-recovered",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    negative_recovered.recoveredAmount = dec!(-1);
    let recovered_allocation = make_allocation(
        "negative-recovered-a",
        "negative-recovered",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(negative_recovered, vec![recovered_allocation]),
        "negative-recovered"
    )
    .is_err());

    let mut negative_outstanding = make_batch(
        "negative-outstanding",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    negative_outstanding.outstandingReceivable = dec!(-1);
    let outstanding_allocation = make_allocation(
        "negative-outstanding-a",
        "negative-outstanding",
        RetroEarningCode::BASE_WAGE,
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(negative_outstanding, vec![outstanding_allocation]),
        "negative-outstanding"
    )
    .is_err());
}

#[test]
fn allocation_identity_and_source_code_pairs_are_unique_within_batch() {
    let duplicate_id_batch = make_batch(
        "duplicate-id",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let duplicate_id_first = make_allocation(
        "same-id",
        "duplicate-id",
        RetroEarningCode::BASE_WAGE,
        dec!(40),
        dec!(40),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    let duplicate_id_second = make_allocation(
        "same-id",
        "duplicate-id",
        RetroEarningCode::WORK_PREMIUM,
        dec!(60),
        dec!(60),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(
            duplicate_id_batch,
            vec![duplicate_id_first, duplicate_id_second]
        ),
        "duplicate-id"
    )
    .is_err());

    let duplicate_key_batch = make_batch(
        "duplicate-key",
        dec!(100),
        dec!(100),
        Decimal::ZERO,
        Decimal::ZERO,
        CompensationRevisionStatus::CALCULATED,
        RetroSettlementStatus::UNSETTLED,
    );
    let duplicate_key_first = make_allocation(
        "key-a",
        "duplicate-key",
        RetroEarningCode::BASE_WAGE,
        dec!(40),
        dec!(40),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    let duplicate_key_second = make_allocation(
        "key-b",
        "duplicate-key",
        RetroEarningCode::BASE_WAGE,
        dec!(60),
        dec!(60),
        Decimal::ZERO,
        Decimal::ZERO,
    );
    assert!(retro_payment_income(
        &dataset(
            duplicate_key_batch,
            vec![duplicate_key_first, duplicate_key_second]
        ),
        "duplicate-key"
    )
    .is_err());
}
