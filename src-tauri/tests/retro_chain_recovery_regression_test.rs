use bordro_programi_lib::services::payroll_service::PayrollService;
use payroll_core::{AccrualType, BordroStatus};

#[test]
#[ignore = "Requires a disposable SQLite copy supplied explicitly"]
fn september_2028_disposable_retro_chain_probe() {
    let path = std::env::var("BORDRO_RETRO_PROBE_COPY").expect("disposable DB path");
    assert!(path.starts_with("/private/tmp/"));
    let conn = rusqlite::Connection::open(path).unwrap();
    let dataset = PayrollService::build_dataset_snapshot(&conn).unwrap();
    let event = dataset
        .payrolls
        .iter()
        .find(|event| {
            event.donemId == "2028-09"
                && event.accrualType == AccrualType::RETRO_ADJUSTMENT
                && event.status == BordroStatus::STALE
        })
        .unwrap()
        .clone();
    let finalized_before = serde_json::to_value(
        dataset
            .payrolls
            .iter()
            .filter(|event| event.status == BordroStatus::FINALIZED)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let normal = PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        &event.personelId,
        &event.donemId,
        None,
        None,
    )
    .expect("September NORMAL");
    println!("NORMAL status: {:?}", normal.status);
    let before_recovery =
        serde_json::to_value(PayrollService::build_dataset_snapshot(&conn).unwrap()).unwrap();
    let error = PayrollService::recover_retro_batch(&conn, &event.accrualId)
        .expect_err("missing January source must not be invented");
    let message = error.to_string();
    assert!(message.contains("2027-01"), "{message}");
    assert!(message.contains("doğrulanmış yedek"), "{message}");
    let after = PayrollService::build_dataset_snapshot(&conn).unwrap();
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        before_recovery,
        "failed recovery must not change any ledger or payroll"
    );
    assert_eq!(
        serde_json::to_value(
            after
                .payrolls
                .iter()
                .filter(|event| event.status == BordroStatus::FINALIZED)
                .collect::<Vec<_>>()
        )
        .unwrap(),
        finalized_before
    );
    let source = after
        .periods
        .iter()
        .find(|period| period.id == "2027-01")
        .unwrap();
    let impact = payroll_core::evaluate_payroll_invalidation(
        &after,
        &payroll_core::PayrollMutation::AccrualInsert {
            personnelId: event.personelId.clone(),
            periodId: source.id.clone(),
            accrualId: format!("{}_{}", event.personelId, source.id),
            paymentDate: source.bitisTarihi.clone(),
            sequence: 0,
        },
    )
    .unwrap();
    assert!(
        !impact.blockedByFinalized.is_empty(),
        "inserting a replacement January NORMAL would affect finalized source history"
    );
    println!("September NORMAL calculated; recovery correctly names missing January 2027 authority; no recovery writes; inserting a historical replacement would affect {} finalized records.", impact.blockedByFinalized.len());
}
