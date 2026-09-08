#[allow(dead_code)]
#[path = "../../crates/payroll-core/tests/support/independent_audit.rs"]
mod support;
use bordro_programi_lib::{
    db::connection::create_in_memory_connection,
    repositories::{
        attendance_repo::AttendanceRepository, payroll_repo::PayrollRepository,
        period_repo::PeriodRepository, personnel_repo::PersonnelRepository,
        settings_repo::SettingsRepository,
    },
    services::payroll_service::PayrollService,
};
use payroll_core::*;
use rust_decimal_macros::dec;

fn setup() -> rusqlite::Connection {
    let conn = create_in_memory_connection().unwrap();
    let req = support::request();
    PersonnelRepository::save(&conn, &req.dataset.personnel[0]).unwrap();
    PeriodRepository::save(&conn, &req.dataset.periods[0]).unwrap();
    SettingsRepository::save_institution_settings(
        &conn,
        &req.dataset.institutionSettings[&req.periodId],
    )
    .unwrap();
    AttendanceRepository::save(&conn, &req.dataset.attendances[0]).unwrap();
    conn
}

#[test]
fn audit_failed_payroll_write_does_not_freeze_statutory_parameters() {
    let conn = setup();
    conn.execute_batch("CREATE TRIGGER audit_reject_payroll BEFORE INSERT ON payroll_records BEGIN SELECT RAISE(ABORT, 'audit injected failure'); END;").unwrap();
    let error = PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        "audit",
        "audit-jan",
        None,
        None,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("audit injected failure"),
        "{error}"
    );
    let settings = SettingsRepository::get_institution_settings(&conn, "audit-jan")
        .unwrap()
        .unwrap();
    assert!(
        settings.statutoryParameterSnapshot.is_none(),
        "failed write froze statutory settings"
    );
    assert!(PayrollRepository::get_all(&conn).unwrap().is_empty());
}

#[test]
fn audit_native_roundtrip_and_same_month_chain() {
    let conn = setup();
    let normal = PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        "audit",
        "audit-jan",
        None,
        None,
    )
    .unwrap();
    assert_eq!(normal.kesintiler.damgaVergisi, Some(dec!(219.88)));
    let request = support::supplementary(support::request(), AccrualType::TEDIYE);
    let extra = PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        "audit",
        "audit-jan",
        request.accrual.as_ref(),
        None,
    )
    .unwrap();
    assert_eq!(
        extra.oncekiKumulatifGvMatrahi,
        normal.gvDetay.as_ref().map(|g| g.cariGvMatrahi)
    );
    assert_eq!(
        extra.gvDetay.as_ref().unwrap().uygulananGvIstisnasi,
        dec!(0)
    );
    assert_eq!(extra.kesintiler.bes, Some(dec!(300)));
    let records = PayrollRepository::get_all(&conn).unwrap();
    for expected in [normal, extra] {
        let actual = records.iter().find(|p| p.id == expected.id).unwrap();
        assert_eq!(actual.gelirToplam, expected.gelirToplam);
        assert_eq!(actual.kesintiToplam, expected.kesintiToplam);
        assert_eq!(actual.netOdeme, expected.netOdeme);
        assert_eq!(
            actual.oncekiKumulatifGvMatrahi,
            expected.oncekiKumulatifGvMatrahi
        );
        assert_eq!(
            actual.oncekiKumulatifAsgariGvMatrahi,
            expected.oncekiKumulatifAsgariGvMatrahi
        );
        for (left, right) in [
            (
                serde_json::to_value(&actual.gvDetay).unwrap(),
                serde_json::to_value(&expected.gvDetay).unwrap(),
            ),
            (
                serde_json::to_value(&actual.pekDetay).unwrap(),
                serde_json::to_value(&expected.pekDetay).unwrap(),
            ),
            (
                serde_json::to_value(&actual.damgaDetay).unwrap(),
                serde_json::to_value(&expected.damgaDetay).unwrap(),
            ),
        ] {
            assert_eq!(left, right, "financial snapshot roundtrip");
        }
    }
}

#[test]
fn audit_tax_snapshot_change_invalidates_following_event_even_if_net_is_same() {
    let conn = setup();
    let mut normal = PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        "audit",
        "audit-jan",
        None,
        None,
    )
    .unwrap();
    let req = support::supplementary(support::request(), AccrualType::TEDIYE);
    let extra = PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        "audit",
        "audit-jan",
        req.accrual.as_ref(),
        None,
    )
    .unwrap();
    // The annual insurance balance is a dependency independently of this event's totals.
    normal
        .gvDetay
        .as_mut()
        .unwrap()
        .uygulanabilirSigortaGvIndirimi = dec!(0.01);
    PayrollRepository::save(&conn, &normal).unwrap();
    let records = PayrollRepository::get_all(&conn).unwrap();
    assert_eq!(
        records.iter().find(|p| p.id == extra.id).unwrap().status,
        BordroStatus::STALE
    );
}
