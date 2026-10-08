use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, DonemselKurumDegerleri};
use bordro_programi_lib::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use payroll_core::{MutationImpact, PayrollKey, TransportRateChange};
use rusqlite::params;

fn period(id: &str, start: &str, end: &str, tax_year: i32, tax_month: i32) -> BordroDonemi {
    let (year, month) = id.split_once('-').unwrap();
    BordroDonemi {
        id: id.into(),
        yil: year.parse().unwrap(),
        ay: month.parse().unwrap(),
        baslangicTarihi: start.into(),
        bitisTarihi: end.into(),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

fn insert_period(conn: &rusqlite::Connection, p: &BordroDonemi) {
    conn.execute(
        "INSERT INTO payroll_periods
         (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'test')",
        params![
            p.id,
            p.yil,
            p.ay,
            p.baslangicTarihi,
            p.bitisTarihi,
            p.donemAdi,
            p.taxYear,
            p.taxMonth
        ],
    )
    .unwrap();
}

#[test]
fn period_repository_handles_empty_selection_and_work_period_neighbors() {
    let conn = create_in_memory_connection().unwrap();
    assert!(PeriodRepository::get_by_ids(&conn, &[]).unwrap().is_empty());

    let previous = period("2027-01", "2027-01-15", "2027-02-14", 2027, 2);
    let active = period("2027-02", "2027-02-15", "2027-03-14", 2027, 3);
    let next = period("2027-03", "2027-03-15", "2027-04-14", 2027, 4);
    insert_period(&conn, &previous);
    insert_period(&conn, &active);
    insert_period(&conn, &next);

    assert_eq!(
        PeriodRepository::get_previous_by_work_period(&conn, &active)
            .unwrap()
            .unwrap()
            .id,
        previous.id
    );
    assert_eq!(
        PeriodRepository::get_next_by_work_period(&conn, &active)
            .unwrap()
            .unwrap()
            .id,
        next.id
    );
    assert_eq!(
        PeriodRepository::get_before_by_work_period(&conn, &active)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        PeriodRepository::get_by_tax_year_before_month(&conn, 2027, 4)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn period_repository_returns_none_for_missing_id_and_successor() {
    let conn = create_in_memory_connection().unwrap();
    let only = period("2027-01", "2027-01-15", "2027-02-14", 2027, 2);
    insert_period(&conn, &only);
    assert!(PeriodRepository::get_by_id(&conn, "missing")
        .unwrap()
        .is_none());
    assert!(PeriodRepository::get_next_by_work_period(&conn, &only)
        .unwrap()
        .is_none());
    assert!(PeriodRepository::get_previous_by_work_period(&conn, &only)
        .unwrap()
        .is_none());
}

#[test]
fn period_repository_rejects_invalid_period_metadata_before_querying() {
    let conn = create_in_memory_connection().unwrap();
    let invalid = period("2027-02", "2027-02-14", "2027-03-14", 2027, 3);
    let error = PeriodRepository::get_previous_by_work_period(&conn, &invalid).unwrap_err();
    assert!(error.to_string().contains("15-14"));
}

#[test]
fn settings_repository_preserves_raw_legacy_fields_and_app_setting_upsert() {
    let conn = create_in_memory_connection().unwrap();
    insert_period(
        &conn,
        &period("2027-01", "2027-01-15", "2027-02-14", 2027, 2),
    );
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2027-01', ?1, 'test')",
        params![r#"{"donemId":"2027-01","legacyOnly":true}"#],
    )
    .unwrap();

    let raw = SettingsRepository::get_raw_institution_settings(&conn, "2027-01")
        .unwrap()
        .unwrap();
    assert_eq!(raw["legacyOnly"], true);
    SettingsRepository::merge_legacy_compensation_settings_in_transaction(
        &conn,
        "2027-01",
        &serde_json::json!({"gunlukYemek": 12.34, "ignored": 99}),
    )
    .unwrap();
    let merged = SettingsRepository::get_raw_institution_settings(&conn, "2027-01")
        .unwrap()
        .unwrap();
    assert_eq!(merged["gunlukYemek"], 12.34);
    assert!(merged.get("ignored").is_none());

    assert_eq!(
        SettingsRepository::get_app_setting(&conn, "theme").unwrap(),
        None
    );
    SettingsRepository::set_app_setting(&conn, "theme", "dark").unwrap();
    SettingsRepository::set_app_setting(&conn, "theme", "light").unwrap();
    assert_eq!(
        SettingsRepository::get_app_setting(&conn, "theme").unwrap(),
        Some("light".into())
    );
}

#[test]
fn settings_repository_reports_malformed_raw_json_without_defaulting_money() {
    let conn = create_in_memory_connection().unwrap();
    insert_period(
        &conn,
        &period("2027-01", "2027-01-15", "2027-02-14", 2027, 2),
    );
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2027-01', '{broken', 'test')",
        [],
    )
    .unwrap();
    let error = SettingsRepository::get_raw_institution_settings(&conn, "2027-01").unwrap_err();
    assert!(error.to_string().contains("bozuk JSON"));
}

#[test]
fn typed_settings_reads_reject_malformed_json_in_every_scope() {
    let conn = create_in_memory_connection().unwrap();
    insert_period(
        &conn,
        &period("2027-01", "2027-01-15", "2027-02-14", 2027, 2),
    );
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2027-01', '{broken', 'test')",
        [],
    )
    .unwrap();

    let all_error = SettingsRepository::get_all_institution_settings(&conn).unwrap_err();
    assert!(all_error.to_string().contains("2027-01"));
    assert!(
        SettingsRepository::get_institution_settings(&conn, "2027-01")
            .unwrap_err()
            .to_string()
            .contains("bozuk JSON")
    );
    assert!(
        SettingsRepository::get_for_periods(&conn, &["2027-01".into()])
            .unwrap_err()
            .to_string()
            .contains("2027-01")
    );
}

#[test]
fn legacy_settings_merge_rejects_corrupt_non_object_stored_json() {
    let conn = create_in_memory_connection().unwrap();
    insert_period(
        &conn,
        &period("2027-01", "2027-01-15", "2027-02-14", 2027, 2),
    );
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2027-01', '17', 'test')",
        [],
    )
    .unwrap();

    let error = SettingsRepository::merge_legacy_compensation_settings_in_transaction(
        &conn,
        "2027-01",
        &serde_json::json!({"gunlukYemek": 12.34}),
    )
    .unwrap_err();
    assert!(error.to_string().contains("JSON nesnesi olmalıdır"));
    assert_eq!(
        SettingsRepository::get_raw_institution_settings(&conn, "2027-01")
            .unwrap()
            .unwrap(),
        serde_json::json!(17)
    );
}

#[test]
fn active_settings_decode_and_legacy_merge_ignore_non_objects() {
    let conn = create_in_memory_connection().unwrap();
    insert_period(
        &conn,
        &period("2027-01", "2027-01-15", "2027-02-14", 2027, 2),
    );
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2027-01', '{}', 'test')",
        [],
    )
    .unwrap();
    let active = SettingsRepository::get_all_active_institution_settings(&conn).unwrap();
    assert_eq!(active["2027-01"].donemId, "2027-01");
    SettingsRepository::merge_legacy_compensation_settings_in_transaction(
        &conn,
        "2027-01",
        &serde_json::json!("legacy scalar"),
    )
    .unwrap();
    assert_eq!(
        SettingsRepository::get_raw_institution_settings(&conn, "2027-01")
            .unwrap()
            .unwrap(),
        serde_json::json!({})
    );
}

#[test]
fn settings_repository_missing_scopes_are_empty_and_absent() {
    let conn = create_in_memory_connection().unwrap();
    assert!(
        SettingsRepository::get_institution_settings(&conn, "missing")
            .unwrap()
            .is_none()
    );
    assert!(
        SettingsRepository::get_raw_institution_settings(&conn, "missing")
            .unwrap()
            .is_none()
    );
    assert!(SettingsRepository::get_for_periods(&conn, &[])
        .unwrap()
        .is_empty());
    assert!(
        SettingsRepository::get_for_periods(&conn, &["missing".into()])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn settings_repository_save_and_get_roundtrip_for_valid_period() {
    let conn = create_in_memory_connection().unwrap();
    let p = period("2027-01", "2027-01-15", "2027-02-14", 2027, 2);
    insert_period(&conn, &p);
    let mut settings = DonemselKurumDegerleri {
        donemId: p.id.clone(),
        ..DonemselKurumDegerleri::default()
    };
    settings.gunlukAsgariUcret = Some(rust_decimal_macros::dec!(1000));
    settings.pekTavanKatsayisi = Some(rust_decimal_macros::dec!(7.5));
    settings.gunlukYemekIstisnasiSGK = Some(rust_decimal_macros::dec!(100));
    settings.gunlukYemekIstisnasiGV = Some(rust_decimal_macros::dec!(100));
    settings.statutoryParameterSegments = Some(Vec::new());
    SettingsRepository::save_institution_settings(&conn, &settings).unwrap();
    let loaded = SettingsRepository::get_institution_settings(&conn, &p.id)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.donemId, p.id);
    assert_eq!(loaded.gunlukAsgariUcret, settings.gunlukAsgariUcret);
    assert_eq!(
        SettingsRepository::get_all_institution_settings(&conn)
            .unwrap()
            .len(),
        1
    );
    SettingsRepository::persist_statutory_snapshot_if_missing(&conn, &p.id).unwrap();
    SettingsRepository::persist_statutory_snapshot_if_missing(&conn, &p.id).unwrap();
    SettingsRepository::set_app_setting_in_transaction(&conn, "audit", "ok").unwrap();
    assert_eq!(
        SettingsRepository::get_app_setting(&conn, "audit").unwrap(),
        Some("ok".into())
    );
    let scoped =
        SettingsRepository::get_for_periods(&conn, &[p.id.clone(), "missing".into()]).unwrap();
    assert_eq!(scoped.len(), 1);
    assert!(scoped.contains_key(&p.id));
    let _ = SettingsRepository::validate_statutory_segments_for_period(&p, &settings);
    let active = payroll_core::PayrollPeriodInstitutionSettings {
        donemId: p.id.clone(),
        ..Default::default()
    };
    assert!(SettingsRepository::save_active_institution_settings(&conn, &active).is_err());
}

#[test]
fn settings_repository_rejects_save_for_missing_period_and_missing_snapshot_source() {
    let conn = create_in_memory_connection().unwrap();
    let settings = DonemselKurumDegerleri {
        donemId: "missing".into(),
        ..DonemselKurumDegerleri::default()
    };
    assert!(
        SettingsRepository::save_institution_settings(&conn, &settings)
            .unwrap_err()
            .to_string()
            .contains("bulunamadı")
    );
    assert!(
        SettingsRepository::persist_statutory_snapshot_if_missing(&conn, "missing")
            .unwrap_err()
            .to_string()
            .contains("bulunamadı")
    );
}

#[test]
fn active_settings_reject_a_missing_period_before_any_write() {
    let conn = create_in_memory_connection().unwrap();
    let settings = payroll_core::PayrollPeriodInstitutionSettings {
        donemId: "missing".into(),
        ..Default::default()
    };
    let error = SettingsRepository::save_active_institution_settings(&conn, &settings)
        .expect_err("active settings cannot be attached to an unknown service period");
    assert!(error.to_string().contains("bulunamadı"));
    assert!(
        SettingsRepository::get_raw_institution_settings(&conn, "missing")
            .unwrap()
            .is_none()
    );
}

#[test]
fn legacy_meal_exemption_falls_back_to_sgk_and_identical_save_is_idempotent() {
    let conn = create_in_memory_connection().unwrap();
    let p = period("2027-01", "2027-01-15", "2027-02-14", 2027, 2);
    insert_period(&conn, &p);
    let mut settings = DonemselKurumDegerleri {
        donemId: p.id.clone(),
        gunlukYemekIstisnasiGV: None,
        ..DonemselKurumDegerleri::default()
    };
    settings.gunlukYemekIstisnasiSGK = Some(rust_decimal_macros::dec!(105));
    SettingsRepository::save_institution_settings(&conn, &settings).unwrap();
    let loaded = SettingsRepository::get_institution_settings(&conn, &p.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.gunlukYemekIstisnasiGV,
        Some(rust_decimal_macros::dec!(105))
    );

    let mut canonical = settings;
    canonical.gunlukYemekIstisnasiGV = Some(rust_decimal_macros::dec!(105));
    SettingsRepository::save_institution_settings(&conn, &canonical)
        .expect("a byte-identical settings save must not invalidate payroll history");
}

#[test]
fn legacy_transport_only_settings_roundtrip_does_not_mutate_finalized_payroll() {
    let conn = create_in_memory_connection().unwrap();
    let p = period("2027-01", "2027-01-15", "2027-02-14", 2027, 2);
    insert_period(&conn, &p);
    conn.execute(
        "INSERT INTO personnel
         (id, tc_no, ad, soyad, grup, sgk_sicil_no, iban, hizmet_yili, created_at, updated_at)
         VALUES ('p1', '10000000001', 'Test', 'Person', '1', '', '', 1, 'test', 'test')",
        [],
    )
    .unwrap();
    let settings = DonemselKurumDegerleri {
        donemId: p.id.clone(),
        ..DonemselKurumDegerleri::default()
    };
    SettingsRepository::save_institution_settings(&conn, &settings).unwrap();
    conn.execute(
        "INSERT INTO payroll_records
         (id, personnel_id, period_id, accrual_id, payment_date, gross_total, sgk_base, gv_base,
          previous_cumulative_gv, new_cumulative_gv, income_tax, stamp_tax, total_deductions,
          net_payment, status, puantaj_summary_json, calculated_at, updated_at)
         VALUES ('final', 'p1', '2027-01', 'normal', '2027-02-10', 10000, 10000, 8500,
                 0, 8500, 1000, 75, 1500, 8500, 'FINALIZED', '{}', 'test', 'test')",
        [],
    )
    .unwrap();

    let mut legacy_roundtrip = settings;
    legacy_roundtrip.transportRateChanges = Some(vec![TransportRateChange {
        effectiveFrom: "2027-01-15".into(),
        gunlukVasitaYol: rust_decimal_macros::dec!(25),
    }]);
    SettingsRepository::save_institution_settings(&conn, &legacy_roundtrip)
        .expect("a legacy transport-only edit does not change the immutable payroll policy");

    let stored = SettingsRepository::get_institution_settings(&conn, &p.id)
        .unwrap()
        .expect("saved settings");
    assert_eq!(
        stored.transportRateChanges.unwrap()[0].gunlukVasitaYol,
        rust_decimal_macros::dec!(25)
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM payroll_records WHERE id = 'final'",
            [],
            |row| { row.get::<_, String>(0) }
        )
        .unwrap(),
        "FINALIZED"
    );
}

#[test]
fn app_setting_with_non_text_storage_returns_a_database_error() {
    let conn = create_in_memory_connection().unwrap();
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES ('bad', x'ff', 'test')",
        [],
    )
    .unwrap();
    assert!(SettingsRepository::get_app_setting(&conn, "bad").is_err());
}

#[test]
fn payroll_invalidation_apply_marks_only_mutable_affected_records_stale() {
    let conn = create_in_memory_connection().unwrap();
    conn.execute(
        "INSERT INTO personnel (id, tc_no, ad, soyad, grup, sgk_sicil_no, iban, hizmet_yili, created_at, updated_at)
         VALUES ('p1', '10000000001', 'Test', 'Person', '1', '', '', 1, 'test', 'test')",
        [],
    )
    .unwrap();
    insert_period(
        &conn,
        &period("2027-01", "2027-01-15", "2027-02-14", 2027, 2),
    );
    insert_period(
        &conn,
        &period("2027-02", "2027-02-15", "2027-03-14", 2027, 3),
    );
    insert_period(
        &conn,
        &period("2027-03", "2027-03-15", "2027-04-14", 2027, 4),
    );
    for (id, period_id, accrual, status) in [
        ("pay-c", "2027-01", "normal", "CALCULATED"),
        ("pay-d", "2027-02", "retro", "DRAFT"),
        ("pay-f", "2027-03", "final", "FINALIZED"),
    ] {
        conn.execute(
            "INSERT INTO payroll_records
             (id, personnel_id, period_id, accrual_id, payment_date, sequence, gross_total, sgk_base,
              gv_base, previous_cumulative_gv, new_cumulative_gv, income_tax, stamp_tax,
              total_deductions, net_payment, status, puantaj_summary_json, calculated_at, updated_at)
             VALUES (?1, 'p1', ?2, ?3, '2027-02-10', ?5, 10000, 10000, 8500, 0, 8500,
                     1000, 75, 1500, 8500, ?4, '{}', 'test', 'test')",
            params![id, period_id, accrual, status, if id == "pay-c" { 0 } else if id == "pay-d" { 1 } else { 2 }],
        )
        .unwrap();
    }
    let impact = MutationImpact {
        affectedPayrolls: vec![
            PayrollKey {
                personnelId: "p1".into(),
                periodId: "2027-01".into(),
                accrualId: "normal".into(),
            },
            PayrollKey {
                personnelId: "p1".into(),
                periodId: "2027-02".into(),
                accrualId: "retro".into(),
            },
            PayrollKey {
                personnelId: "p1".into(),
                periodId: "2027-03".into(),
                accrualId: "final".into(),
            },
        ],
        ..MutationImpact::default()
    };
    assert_eq!(
        PayrollInvalidationRepository::apply_impact(&conn, &impact).unwrap(),
        2
    );
    let statuses: Vec<(String, String)> = conn
        .prepare("SELECT id, status FROM payroll_records ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        statuses,
        vec![
            ("pay-c".into(), "STALE".into()),
            ("pay-d".into(), "STALE".into()),
            ("pay-f".into(), "FINALIZED".into())
        ]
    );
    let error = PayrollInvalidationRepository::assert_mutation_allowed(
        &conn,
        &payroll_core::PayrollMutation::All,
    )
    .unwrap_err();
    assert!(error.to_string().contains("Test Person"));
}

#[test]
fn payroll_invalidation_apply_rejects_blocked_impact_before_writing() {
    let conn = create_in_memory_connection().unwrap();
    let impact = MutationImpact {
        blockedByFinalized: vec![PayrollKey {
            personnelId: "missing-person".into(),
            periodId: "missing-period".into(),
            accrualId: "normal".into(),
        }],
        ..MutationImpact::default()
    };
    let error = PayrollInvalidationRepository::apply_impact(&conn, &impact).unwrap_err();
    assert!(error.to_string().contains("Kesinleştirilmiş"));
}

#[test]
fn payroll_invalidation_rejects_retro_effect_blocker_before_writing() {
    let conn = create_in_memory_connection().unwrap();
    let impact = MutationImpact {
        retroEffects: vec![payroll_core::RetroMutationEffect {
            batchId: "batch-final".into(),
            sourcePeriodIds: vec!["2027-01".into()],
            linkedPaymentEventIds: vec!["pay-final".into()],
            action: payroll_core::RetroMutationAction::ImmutableBlocker,
            blockers: vec![payroll_core::RetroMutationBlockerReason::FinalizedBatch],
        }],
        ..MutationImpact::default()
    };
    assert!(PayrollInvalidationRepository::apply_impact(&conn, &impact).is_err());
}

#[test]
fn active_settings_reader_reports_malformed_json() {
    let conn = create_in_memory_connection().unwrap();
    insert_period(
        &conn,
        &period("2027-01", "2027-01-15", "2027-02-14", 2027, 2),
    );
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2027-01', '{bad', 'test')",
        [],
    )
    .unwrap();
    assert!(
        SettingsRepository::get_all_active_institution_settings(&conn)
            .unwrap_err()
            .to_string()
            .contains("bozuk JSON")
    );
}

#[test]
fn active_settings_reader_propagates_a_missing_settings_table() {
    let conn = create_in_memory_connection().unwrap();
    conn.execute("DROP TABLE institution_settings", []).unwrap();

    assert!(SettingsRepository::get_all_active_institution_settings(&conn).is_err());
}

#[test]
fn active_settings_save_retains_legacy_fields_and_freezes_snapshot_once() {
    let conn = create_in_memory_connection().unwrap();
    let p = period("2027-01", "2027-01-15", "2027-02-14", 2027, 2);
    insert_period(&conn, &p);
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2027-01', ?1, 'test')",
        params![r#"{"legacyOnly":true,"gunlukYemekIstisnasiSGK":10}"#],
    )
    .unwrap();
    let active = payroll_core::PayrollPeriodInstitutionSettings {
        donemId: p.id.clone(),
        gunlukAsgariUcret: Some(rust_decimal_macros::dec!(100)),
        pekTavanKatsayisi: Some(rust_decimal_macros::dec!(7.5)),
        gunlukYemekIstisnasiSGK: Some(rust_decimal_macros::dec!(10)),
        gunlukYemekIstisnasiGV: None,
        sgkIsciOraniYuzde: Some(rust_decimal_macros::dec!(14)),
        issizlikIsciOraniYuzde: Some(rust_decimal_macros::dec!(1)),
        sendikaAidatiYuzde: Some(rust_decimal_macros::dec!(0)),
        besOraniYuzde: Some(rust_decimal_macros::dec!(0)),
        sgkIsverenOraniYuzde: Some(rust_decimal_macros::dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(rust_decimal_macros::dec!(2)),
        damgaVergisiOraniBinde: Some(rust_decimal_macros::dec!(7.59)),
        ..Default::default()
    };
    SettingsRepository::save_active_institution_settings(&conn, &active).unwrap();
    let stored = SettingsRepository::get_raw_institution_settings(&conn, &p.id)
        .unwrap()
        .unwrap();
    assert_eq!(stored["legacyOnly"], true);
    assert_eq!(stored["gunlukYemekIstisnasiGV"], "10");
    SettingsRepository::persist_statutory_snapshot_if_missing(&conn, &p.id).unwrap();
    let first = SettingsRepository::get_raw_institution_settings(&conn, &p.id)
        .unwrap()
        .unwrap()["statutoryParameterSnapshot"]
        .clone();
    assert!(!first.is_null());
    SettingsRepository::persist_statutory_snapshot_if_missing(&conn, &p.id).unwrap();
    let second = SettingsRepository::get_raw_institution_settings(&conn, &p.id)
        .unwrap()
        .unwrap()["statutoryParameterSnapshot"]
        .clone();
    assert_eq!(first, second);
}

#[test]
fn payroll_invalidation_apply_handles_missing_retro_batch_without_counting_it() {
    let conn = create_in_memory_connection().unwrap();
    let impact = MutationImpact {
        affectedRetroBatches: vec!["missing-retro-batch".into()],
        ..MutationImpact::default()
    };
    assert_eq!(
        PayrollInvalidationRepository::apply_impact(&conn, &impact).unwrap(),
        0
    );
}

#[test]
fn payroll_invalidation_evaluate_and_assert_allow_empty_mutation() {
    let conn = create_in_memory_connection().unwrap();
    let impact = PayrollInvalidationRepository::evaluate_mutation(
        &conn,
        &payroll_core::PayrollMutation::All,
    )
    .unwrap();
    assert!(impact.affectedPayrolls.is_empty());
    assert!(PayrollInvalidationRepository::assert_mutation_allowed(
        &conn,
        &payroll_core::PayrollMutation::All,
    )
    .is_ok());
}
