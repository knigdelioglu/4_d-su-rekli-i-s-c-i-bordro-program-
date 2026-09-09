use bordro_programi_lib::{
    db::create_in_memory_connection,
    repositories::{
        annual_payroll_parameters_repo::AnnualPayrollParametersRepository,
        attendance_repo::AttendanceRepository, payroll_repo::PayrollRepository,
        period_repo::PeriodRepository, personnel_repo::PersonnelRepository,
        settings_repo::SettingsRepository,
    },
    services::payroll_service::PayrollService,
};
use payroll_core::*;
use rust_decimal_macros::dec;
#[path = "../../crates/payroll-core/tests/support/audit_fixture.rs"]
mod audit_fixture;

#[test]
fn audit_native_roundtrip_recalculation_and_finalized_atomicity() {
    let req = audit_fixture::request();
    let conn = create_in_memory_connection().unwrap();
    PersonnelRepository::save(&conn, &req.dataset.personnel[0]).unwrap();
    AnnualPayrollParametersRepository::save(&conn, &req.dataset.annualPayrollParameters[0])
        .unwrap();
    for period in &req.dataset.periods {
        PeriodRepository::save(&conn, period).unwrap();
        SettingsRepository::save_institution_settings(
            &conn,
            &req.dataset.institutionSettings[&period.id],
        )
        .unwrap();
    }
    for attendance in &req.dataset.attendances {
        AttendanceRepository::save(&conn, attendance).unwrap();
    }
    let expected = calculate_payroll_checked(&req).unwrap();
    let calculated =
        PayrollService::calculate_payroll_for_accrual_checked(&conn, "audit", "source", None, None)
            .unwrap();
    assert_eq!(calculated.gelirToplam, expected.gelirToplam);
    assert_eq!(calculated.kesintiToplam, expected.kesintiToplam);
    let saved = PayrollRepository::get_all(&conn).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        serde_json::to_value(&saved[0].gvDetay).unwrap(),
        serde_json::to_value(&calculated.gvDetay).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&saved[0].pekDetay).unwrap(),
        serde_json::to_value(&calculated.pekDetay).unwrap()
    );
    assert_eq!(saved[0].netOdeme, calculated.netOdeme);
    PayrollService::calculate_payroll_for_accrual_checked(&conn, "audit", "source", None, None)
        .unwrap();
    assert_eq!(PayrollRepository::get_all(&conn).unwrap().len(), 1);
    PayrollService::finalize_payroll_for_personnel(&conn, "audit", "source").unwrap();
    let before = serde_json::to_value(PayrollRepository::get_all(&conn).unwrap()).unwrap();
    let mut changed = req.dataset.institutionSettings["source"].clone();
    changed.gunlukTabanUcret += dec!(1);
    assert!(SettingsRepository::save_institution_settings(&conn, &changed).is_err());
    assert!(PayrollService::calculate_payroll_for_accrual_checked(
        &conn, "audit", "source", None, None
    )
    .is_err());
    assert_eq!(
        serde_json::to_value(PayrollRepository::get_all(&conn).unwrap()).unwrap(),
        before
    );
}
