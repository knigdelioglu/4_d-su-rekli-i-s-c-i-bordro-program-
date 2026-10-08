//! Focused ledger assertions for the retro settlement paths.
//!
//! These cases deliberately assert the persisted ledger equations instead of
//! comparing two production calculations.  They protect the period/payment
//! month split and chronological receivable replay used by the financial
//! reconciliation flow.

use payroll_core::{
    replay_retro_outstanding_receivables, retro_sgk_ledger_totals,
    validate_v5_retro_outstanding_receivables, CompensationRevisionStatus, RetroAdjustmentBatch,
    RetroAllocation, RetroEarningCode, RetroSettlementStatus,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[allow(clippy::too_many_arguments)] // Keep ledger fields explicit in independent fixtures.
fn batch(
    id: &str,
    personnel_id: &str,
    payment_date: &str,
    status: CompensationRevisionStatus,
    recoverable: Decimal,
    offset: Decimal,
    recovered: Decimal,
    outstanding: Decimal,
) -> RetroAdjustmentBatch {
    RetroAdjustmentBatch {
        id: id.into(),
        revisionId: format!("revision-{id}"),
        personnelId: personnel_id.into(),
        paymentDate: payment_date.into(),
        status,
        settlementStatus: RetroSettlementStatus::UNSETTLED,
        totalGrossDelta: Decimal::ZERO,
        payableSettlementAmount: Decimal::ZERO,
        offsetSettlementAmount: offset,
        recoveredAmount: recovered,
        recoverableAmount: recoverable,
        outstandingReceivable: outstanding,
        description: None,
        createdAt: None,
        calculatedAt: None,
        finalizedAt: None,
    }
}

#[allow(clippy::too_many_arguments)] // Each argument maps to a distinct persisted ledger amount.
fn allocation(
    id: &str,
    source_period: &str,
    code: RetroEarningCode,
    original_pek: Decimal,
    adjusted_pek: Decimal,
    pek_delta: Decimal,
    worker_sgk: Decimal,
    worker_unemployment: Decimal,
    employer_sgk: Decimal,
    employer_unemployment: Decimal,
    delta: Decimal,
) -> RetroAllocation {
    let policy = payroll_core::retro_earning_policy(code);
    RetroAllocation {
        id: id.into(),
        batchId: "batch".into(),
        personnelId: "person".into(),
        sourcePeriodId: source_period.into(),
        earningCode: code,
        originalRecognizedAmount: Decimal::ZERO,
        previousAuthoritativeRetroAmount: Decimal::ZERO,
        targetAmount: delta,
        deltaAmount: delta,
        sgkTreatment: policy.sgkTreatment,
        incomeTaxTreatment: policy.incomeTaxTreatment,
        stampTaxTreatment: policy.stampTaxTreatment,
        originalPek: original_pek,
        retroPekDelta: pek_delta,
        adjustedPek: adjusted_pek,
        workerSgkDelta: worker_sgk,
        workerUnemploymentDelta: worker_unemployment,
        employerSgkDelta: employer_sgk,
        employerUnemploymentDelta: employer_unemployment,
        originalEmployerLowerBound: Decimal::ZERO,
        targetEmployerLowerBound: Decimal::ZERO,
        employerLowerBoundDelta: Decimal::ZERO,
        employerLowerBoundPremiumDelta: Decimal::ZERO,
        originalSourceCarry: None,
        targetSourceCarry: None,
        payableSettlementAmount: delta.max(Decimal::ZERO),
        offsetSettlementAmount: Decimal::ZERO,
        recoverableAmount: (-delta).max(Decimal::ZERO),
        metadata: None,
    }
}

#[test]
fn source_period_ledger_reconciles_positive_and_negative_components_independently() {
    let rows = vec![
        allocation(
            "wage",
            "2027-02",
            RetroEarningCode::BASE_WAGE,
            dec!(1000),
            dec!(1200),
            dec!(200),
            dec!(28),
            dec!(2),
            dec!(43.50),
            dec!(4),
            dec!(200),
        ),
        allocation(
            "premium",
            "2027-02",
            RetroEarningCode::WORK_PREMIUM,
            dec!(1200),
            dec!(1150),
            dec!(-50),
            dec!(-7),
            dec!(0),
            dec!(-10.88),
            dec!(-1),
            dec!(-50),
        ),
        allocation(
            "other",
            "2027-03",
            RetroEarningCode::OTHER,
            dec!(500),
            dec!(600),
            dec!(100),
            dec!(14),
            dec!(1),
            dec!(21.75),
            dec!(2),
            dec!(100),
        ),
    ];
    let totals = retro_sgk_ledger_totals(&rows);
    let feb = totals.get("2027-02").unwrap();
    assert_eq!(feb.0, dec!(1200));
    assert_eq!(feb.1, dec!(150));
    assert_eq!(feb.2, dec!(1200));
    assert_eq!(feb.3, dec!(21));
    assert_eq!(feb.4, dec!(2));
    assert_eq!(feb.5, dec!(32.62));
    assert_eq!(feb.6, dec!(3));
    assert_eq!(feb.7, dec!(150));
    assert_eq!(totals.get("2027-03").unwrap().7, dec!(100));
}

#[test]
fn v5_replay_carries_receivable_then_applies_offset_and_recovery() {
    let batches = vec![
        batch(
            "negative",
            "p1",
            "2027-03-14",
            CompensationRevisionStatus::CALCULATED,
            dec!(1000),
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(1000),
        ),
        batch(
            "zero",
            "p1",
            "2027-04-14",
            CompensationRevisionStatus::CALCULATED,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(1000),
        ),
        batch(
            "settled",
            "p1",
            "2027-05-14",
            CompensationRevisionStatus::FINALIZED,
            Decimal::ZERO,
            dec!(250),
            dec!(300),
            dec!(450),
        ),
    ];
    validate_v5_retro_outstanding_receivables(&batches).unwrap();
    assert_eq!(
        replay_retro_outstanding_receivables(&batches).unwrap()["p1"],
        dec!(450)
    );
}

#[test]
fn v5_replay_rejects_snapshot_that_would_hide_an_overpayment() {
    let batches = vec![
        batch(
            "negative",
            "p1",
            "2027-03-14",
            CompensationRevisionStatus::CALCULATED,
            dec!(100),
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100),
        ),
        batch(
            "over",
            "p1",
            "2027-04-14",
            CompensationRevisionStatus::CALCULATED,
            Decimal::ZERO,
            dec!(101),
            Decimal::ZERO,
            Decimal::ZERO,
        ),
    ];
    assert!(validate_v5_retro_outstanding_receivables(&batches).is_err());
}

#[test]
fn replay_ignores_non_calculated_batches_and_keeps_people_separate() {
    let batches = vec![
        batch(
            "p1-open",
            "p1",
            "2027-02-14",
            CompensationRevisionStatus::CALCULATED,
            dec!(300),
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(300),
        ),
        batch(
            "p2-open",
            "p2",
            "2027-01-14",
            CompensationRevisionStatus::FINALIZED,
            dec!(125),
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(125),
        ),
        batch(
            "draft",
            "p1",
            "2027-03-14",
            CompensationRevisionStatus::DRAFT,
            dec!(999),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        ),
    ];
    let balances = replay_retro_outstanding_receivables(&batches).unwrap();
    assert_eq!(balances["p1"], dec!(300));
    assert_eq!(balances["p2"], dec!(125));
    assert_eq!(balances.len(), 2);
}

#[test]
fn ledger_rounds_each_sgk_component_and_preserves_signed_delta() {
    let rows = vec![
        allocation(
            "a",
            "period",
            RetroEarningCode::BASE_WAGE,
            dec!(1),
            dec!(1),
            dec!(0.005),
            dec!(0.005),
            dec!(0.005),
            dec!(0.005),
            dec!(0.005),
            dec!(0.005),
        ),
        allocation(
            "b",
            "period",
            RetroEarningCode::BASE_WAGE,
            dec!(1),
            dec!(1),
            dec!(0.005),
            dec!(0.005),
            dec!(0.005),
            dec!(0.005),
            dec!(0.005),
            dec!(-0.005),
        ),
    ];
    let row = retro_sgk_ledger_totals(&rows)["period"];
    assert_eq!(row.1, dec!(0.02));
    assert_eq!(row.3, dec!(0.02));
    assert_eq!(row.4, dec!(0.02));
    assert_eq!(row.5, dec!(0.02));
    assert_eq!(row.6, dec!(0.02));
    assert_eq!(row.7, dec!(0.01));
}

#[test]
fn replay_rejects_wrong_snapshot_even_when_recoverable_is_zero() {
    let batches = vec![batch(
        "zero-delta",
        "p1",
        "2027-06-14",
        CompensationRevisionStatus::CALCULATED,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(1),
    )];
    assert!(validate_v5_retro_outstanding_receivables(&batches).is_err());
}
