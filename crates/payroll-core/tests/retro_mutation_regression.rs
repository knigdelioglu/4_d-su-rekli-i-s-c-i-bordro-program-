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

fn dataset(batch: RetroAdjustmentBatch, allocations: Vec<RetroAllocation>) -> PayrollDatasetSnapshot {
    PayrollDatasetSnapshot {
        periods: vec![period("p1")],
        retroBatches: vec![batch],
        retroAllocations: allocations,
        ..PayrollDatasetSnapshot::default()
    }
}

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
    assert_eq!(retro_payable_allocation_amount(&legacy, &positive), Decimal::ZERO);

    let (_, allocations, income, _) =
        retro_payment_income(&dataset(legacy, vec![positive, negative]), "legacy")
            .expect("legacy mixed-sign batch should normalize locally");
    assert_eq!(allocations.iter().map(|a| a.payableSettlementAmount).sum::<Decimal>(), dec!(70));
    assert_eq!(allocations.iter().map(|a| a.recoverableAmount).sum::<Decimal>(), Decimal::ZERO);
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
    assert!(retro_payment_income(&dataset(valid_batch.clone(), vec![unknown_period]), "b1").is_err());

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
    assert!(retro_payment_income(&dataset(valid_batch.clone(), vec![negative_original]), "b1").is_err());

    let mut negative_carry_amount = valid.clone();
    negative_carry_amount.originalSourceCarry = Some(vec![DevredenPekKaydi {
        tutar: dec!(-1),
        kalanAySayisi: 1,
        kaynakDonemId: Some("p1".into()),
    }]);
    assert!(retro_payment_income(&dataset(valid_batch.clone(), vec![negative_carry_amount]), "b1").is_err());

    let mut negative_carry_months = valid;
    negative_carry_months.targetSourceCarry = Some(vec![DevredenPekKaydi {
        tutar: dec!(1),
        kalanAySayisi: -1,
        kaynakDonemId: Some("p1".into()),
    }]);
    assert!(retro_payment_income(&dataset(valid_batch, vec![negative_carry_months]), "b1").is_err());
}

#[test]
fn payment_ledger_rejects_settlement_flows_that_exceed_signed_entitlement() {
    let batch = batch(
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
    assert!(retro_payment_income(&dataset(batch, vec![overpaid]), "b1").is_err());

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
    assert_eq!(retro_payable_settlement_amount(&offset_only_batch), Decimal::ZERO);
    assert!(retro_payment_income(&dataset(offset_only_batch, vec![offset]), "b2").is_err());
}
