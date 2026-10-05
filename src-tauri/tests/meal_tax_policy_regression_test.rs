use bordro_programi_lib::{
    db::create_in_memory_connection,
    repositories::{period_repo::PeriodRepository, settings_repo::SettingsRepository},
};
use payroll_core::PayrollCalculationRequest;

#[test]
fn legacy_sqlite_settings_default_on_and_an_explicit_false_survives_save_reload() {
    let conn = create_in_memory_connection().unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../crates/payroll-core/tests/golden/2026/G020-social-aid.json"
    ))
    .unwrap();
    let request: PayrollCalculationRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();
    let period = &request.dataset.periods[0];
    PeriodRepository::save(&conn, period).unwrap();
    let original = &request.dataset.institutionSettings[&period.id];
    let mut legacy = serde_json::to_value(original).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("yemekTamGvIstisnaDamgaTabi");
    conn.execute("INSERT INTO institution_settings (period_id, settings_json, updated_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![period.id, legacy.to_string(), "2026-10-04"]).unwrap();
    let mut restored = SettingsRepository::get_institution_settings(&conn, &period.id)
        .unwrap()
        .unwrap();
    assert!(restored.yemekTamGvIstisnaDamgaTabi);
    assert_eq!(restored.gunlukYemek, original.gunlukYemek);
    restored.yemekTamGvIstisnaDamgaTabi = false;
    SettingsRepository::save_institution_settings(&conn, &restored).unwrap();
    let reloaded = SettingsRepository::get_institution_settings(&conn, &period.id)
        .unwrap()
        .unwrap();
    assert!(!reloaded.yemekTamGvIstisnaDamgaTabi);
    assert_eq!(reloaded.gunlukTabanUcret, original.gunlukTabanUcret);
    assert!(
        !SettingsRepository::get_all_institution_settings(&conn).unwrap()[&period.id]
            .yemekTamGvIstisnaDamgaTabi
    );
}

#[test]
fn legacy_sqlite_settings_with_recorded_payroll_history_keep_the_legacy_policy() {
    let mut conn = create_in_memory_connection().unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../crates/payroll-core/tests/golden/2026/G020-social-aid.json"
    ))
    .unwrap();
    let request: PayrollCalculationRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();
    let period = &request.dataset.periods[0];
    PeriodRepository::save(&conn, period).unwrap();
    let original = &request.dataset.institutionSettings[&period.id];
    let mut legacy = serde_json::to_value(original).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("yemekTamGvIstisnaDamgaTabi");
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![period.id, legacy.to_string(), "2026-10-04"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO personnel (id, tc_no, ad, soyad, grup, created_at, updated_at)
         VALUES ('legacy-person', '10000000000', 'Legacy', 'Person', '1. Grup', '', '')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO payroll_records (
            id, personnel_id, period_id, accrual_id, payment_date,
            gross_total, sgk_base, gv_base, previous_cumulative_gv,
            new_cumulative_gv, income_tax, stamp_tax, total_deductions,
            net_payment, status, puantaj_summary_json, calculated_at, updated_at
         ) VALUES (
            'legacy-payroll', 'legacy-person', ?1, 'legacy-payroll', '2026-02-28',
            0, 0, 0, 0, 0, 0, 0, 0, 0, 'FINALIZED', '{}', '', ''
         )",
        [&period.id],
    )
    .unwrap();

    // Simulate the version immediately before the compatibility migration.
    conn.pragma_update(None, "user_version", 8).unwrap();
    bordro_programi_lib::db::migrations::initialize_db(&mut conn).unwrap();

    let restored = SettingsRepository::get_institution_settings(&conn, &period.id)
        .unwrap()
        .unwrap();
    assert!(!restored.yemekTamGvIstisnaDamgaTabi);
}
