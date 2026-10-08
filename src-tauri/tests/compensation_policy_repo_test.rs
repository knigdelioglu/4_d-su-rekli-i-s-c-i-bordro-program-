use bordro_programi_lib::db::{
    create_in_memory_connection,
    migrations::{
        get_migrations, CALCULATION_METADATA_MIGRATION_VERSION,
        COMPENSATION_POLICY_FLAGS_MIGRATION_VERSION,
    },
};
use bordro_programi_lib::domain::models::{
    CompensationBoundaryScope, CompensationPolicyGroup, CompensationPolicyState,
    CompensationPolicyVersion, RoadRoundingMode,
};
use bordro_programi_lib::repositories::compensation_policy_repo::CompensationPolicyRepository;
use bordro_programi_lib::repositories::transaction::with_transaction;
use payroll_core::resolve_compensation_segments;
use rusqlite::{params, Connection};
use rust_decimal_macros::dec;
use serde_json::json;

fn policy_state(base: rust_decimal::Decimal) -> CompensationPolicyState {
    CompensationPolicyState {
        gunlukTabanUcret: base,
        gunlukYemek: dec!(10.12345),
        birlestirilmisSosyalYardim: dec!(20),
        gunlukVasitaYol: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        isPrimiYuzde: None,
        isPrimiGruplari: vec![CompensationPolicyGroup {
            id: "g1".into(),
            ad: "1. Grup".into(),
            oran: dec!(9.125),
            aktif: true,
        }],
        geceCalismaPrimiYuzde: dec!(0),
        geceCalismaTatiliPrimiYuzde: dec!(0),
        ekOdeme: None,
        digerGelirVarsayilan: Some(dec!(0)),
    }
}

fn version(
    id: &str,
    date: &str,
    base: rust_decimal::Decimal,
    parent: Option<&str>,
) -> CompensationPolicyVersion {
    CompensationPolicyVersion {
        id: id.into(),
        effectiveFrom: date.into(),
        state: policy_state(base),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
        createdAt: "2026-01-01T00:00:00Z".into(),
        source: "ADMINISTRATIVE_DECISION".into(),
        reason: format!("reason-{id}"),
        supersedesVersionId: parent.map(str::to_owned),
    }
}

fn insert_raw_policy(
    conn: &Connection,
    id: &str,
    date: &str,
    state_json: &str,
    supersedes: Option<&str>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO compensation_policy_versions
            (id, effective_from, state_json, created_at, source, reason,
             supersedes_version_id, boundary_scope, road_rounding_mode)
         VALUES (?1, ?2, ?3, 'test', 'test', 'test', ?4, 'FULL_EARNING', 'PER_COMPENSATION_SLICE')",
        params![id, date, state_json, supersedes],
    )
}

fn seed_previous_schema(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    // Schema version 20 is the immediately previous schema; migration 21 is
    // applied below after representative persisted data is inserted.
    get_migrations().to_version(conn, 20)?;
    conn.execute(
        "INSERT INTO personnel
            (id, tc_no, ad, soyad, grup, sgk_sicil_no, iban, hizmet_yili, created_at, updated_at)
         VALUES ('p1', '10000000000', 'Test', 'Person', '1. Grup', '', '', 3, 't', 't')",
        [],
    )?;
    conn.execute(
        "INSERT INTO payroll_periods
            (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, created_at)
         VALUES ('2026-01', 2026, 1, '2026-01-15', '2026-02-14', 'Ocak', 't')",
        [],
    )?;
    conn.execute(
        "INSERT INTO payroll_records
            (id, personnel_id, period_id, accrual_id, payment_date, gross_total, sgk_base,
             gv_base, previous_cumulative_gv, new_cumulative_gv, income_tax, stamp_tax,
             total_deductions, net_payment, puantaj_summary_json, calculated_at, updated_at)
         VALUES ('pay1', 'p1', '2026-01', 'normal1', '2026-02-10', 10000, 10000,
                 8500, 0, 8500, 1000, 75, 1500, 8500, '{}', 't', 't')",
        [],
    )?;
    conn.execute(
        "INSERT INTO institution_settings (period_id, settings_json, updated_at)
         VALUES ('2026-01', '{}', 't')",
        [],
    )?;
    conn.execute(
        "INSERT INTO compensation_revisions (id, reason, title, effective_from)
         VALUES ('rev1', 'PAY_CORRECTION', 'Correction', '2026-01-15')",
        [],
    )?;
    conn.execute(
        "INSERT INTO retro_adjustment_batches
            (id, revision_id, personnel_id, payment_date, total_gross_delta)
         VALUES ('batch1', 'rev1', 'p1', '2026-02-10', 100)",
        [],
    )?;
    conn.execute(
        "INSERT INTO retro_adjustment_allocations
            (id, batch_id, personnel_id, source_period_id, earning_code,
             original_recognized_amount, target_amount, delta_amount,
             sgk_treatment, income_tax_treatment, stamp_tax_treatment)
         VALUES ('alloc1', 'batch1', 'p1', '2026-01', 'BASE_WAGE',
                 1000, 1100, 100, 'TAXABLE', 'TAXABLE', 'TAXABLE')",
        [],
    )?;
    Ok(())
}

#[test]
fn additive_migration_preserves_payroll_settings_and_retro_history() {
    let mut conn = Connection::open_in_memory().unwrap();
    seed_previous_schema(&mut conn).unwrap();
    get_migrations().to_latest(&mut conn).unwrap();
    let latest_version = conn
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .unwrap();
    assert!(
        latest_version >= COMPENSATION_POLICY_FLAGS_MIGRATION_VERSION as i64,
        "all migrations through the compensation-policy flags must apply"
    );
    for table in [
        "payroll_records",
        "institution_settings",
        "retro_adjustment_batches",
        "retro_adjustment_allocations",
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1, "{table} must be preserved");
    }
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&conn)
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn flags_migration_quarantines_old_rows_and_scoped_reads_skip_unrelated_incomplete_history() {
    let mut conn = Connection::open_in_memory().unwrap();
    get_migrations()
        .to_version(&mut conn, CALCULATION_METADATA_MIGRATION_VERSION as usize)
        .unwrap();
    let legacy_state = serde_json::to_string(&policy_state(dec!(100))).unwrap();
    conn.execute(
        "INSERT INTO compensation_policy_versions
            (id, effective_from, state_json, created_at, source, reason, supersedes_version_id)
         VALUES ('experimental-old', '2020-01-01', ?1, 't', 'test', 'pre-release', NULL)",
        params![legacy_state],
    )
    .unwrap();

    get_migrations().to_latest(&mut conn).unwrap();
    let missing_scope: Option<String> = conn
        .query_row(
            "SELECT boundary_scope FROM compensation_policy_versions WHERE id = 'experimental-old'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        missing_scope, None,
        "migration must not infer financial flags"
    );

    let selected_error =
        CompensationPolicyRepository::get_versions_for_period(&conn, "2020-01-01", "2020-01-31")
            .unwrap_err()
            .to_string();
    assert!(selected_error.contains("experimental-old"));
    assert!(selected_error.contains("karantinada"));
    assert!(CompensationPolicyRepository::get_all_versions(&conn).is_err());

    let current_state = serde_json::to_string(&policy_state(dec!(200))).unwrap();
    conn.execute(
        "INSERT INTO compensation_policy_versions
            (id, effective_from, state_json, created_at, source, reason,
             supersedes_version_id, boundary_scope, road_rounding_mode)
         VALUES ('current', '2026-01-01', ?1, 't', 'test', 'new baseline', NULL,
                 'FULL_EARNING', 'PER_COMPENSATION_SLICE')",
        params![current_state],
    )
    .unwrap();
    let scoped =
        CompensationPolicyRepository::get_versions_for_period(&conn, "2026-01-15", "2026-02-14")
            .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].id, "current");

    let missing_insert = conn.execute(
        "INSERT INTO compensation_policy_versions
            (id, effective_from, state_json, created_at, source, reason, supersedes_version_id)
         VALUES ('missing-flags', '2027-01-01', ?1, 't', 'test', 'must fail', NULL)",
        params![current_state],
    );
    assert!(
        missing_insert.is_err(),
        "new insert must require explicit flags"
    );
    assert!(CompensationPolicyRepository::insert_version(
        &conn,
        &version("correction", "2026-01-01", dec!(210), Some("current"))
    )
    .unwrap_err()
    .to_string()
    .contains("experimental-old"));
}

#[test]
fn fresh_database_has_empty_independent_timeline_and_period_deletion_keeps_policy() {
    let conn = create_in_memory_connection().unwrap();
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&conn)
            .unwrap()
            .len(),
        0
    );
    conn.execute(
        "INSERT INTO payroll_periods
            (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, created_at)
         VALUES ('2026-01', 2026, 1, '2026-01-15', '2026-02-14', 'Ocak', 't')",
        [],
    )
    .unwrap();
    CompensationPolicyRepository::insert_version(
        &conn,
        &version("base", "2026-01-15", dec!(100), None),
    )
    .unwrap();
    conn.execute("DELETE FROM payroll_periods WHERE id = '2026-01'", [])
        .unwrap();
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&conn)
            .unwrap()
            .len(),
        1
    );
    let foreign_keys: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_foreign_key_list('compensation_policy_versions') WHERE \"table\" = 'payroll_periods'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(foreign_keys, 0);
}

#[test]
fn correction_insert_read_and_period_projection_match_pure_resolver() {
    let conn = create_in_memory_connection().unwrap();
    for item in [
        version("older-unrelated", "2024-01-01", dec!(80), None),
        version("old", "2025-01-01", dec!(100), None),
        version("fix-1", "2025-01-01", dec!(110), Some("old")),
        version("fix-2", "2025-01-01", dec!(120), Some("fix-1")),
        version("change-1", "2026-01-20", dec!(130), None),
        version("change-2", "2026-02-01", dec!(140), None),
        version("future-unrelated", "2027-01-01", dec!(150), None),
    ] {
        CompensationPolicyRepository::insert_version(&conn, &item).unwrap();
    }
    let versions =
        CompensationPolicyRepository::get_versions_for_period(&conn, "2026-01-15", "2026-02-14")
            .unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["old", "fix-1", "fix-2", "change-1", "change-2"]
    );
    let resolved = resolve_compensation_segments("2026-01-15", "2026-02-14", &versions).unwrap();
    assert_eq!(
        resolved
            .iter()
            .map(|item| item.versionId.as_str())
            .collect::<Vec<_>>(),
        vec!["fix-2", "change-1", "change-2"]
    );
    assert_eq!(
        (resolved[0].startDate.as_str(), resolved[0].endDate.as_str()),
        ("2026-01-15", "2026-01-19")
    );
    assert_eq!(
        (resolved[1].startDate.as_str(), resolved[1].endDate.as_str()),
        ("2026-01-20", "2026-01-31")
    );
    assert_eq!(
        (resolved[2].startDate.as_str(), resolved[2].endDate.as_str()),
        ("2026-02-01", "2026-02-14")
    );

    let state_json: String = conn
        .query_row(
            "SELECT state_json FROM compensation_policy_versions WHERE id = 'old'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&state_json).unwrap();
    assert_eq!(value["gunlukTabanUcret"], "100");
    assert_eq!(value["gunlukYemek"], "10.12345");
}

#[test]
fn immutable_conflicts_and_failed_corrections_leave_old_history_unchanged() {
    let conn = create_in_memory_connection().unwrap();
    let baseline = version("base", "2026-01-15", dec!(100), None);
    CompensationPolicyRepository::insert_version(&conn, &baseline).unwrap();
    assert!(CompensationPolicyRepository::insert_version(
        &conn,
        &version("wrong", "2026-02-15", dec!(110), Some("base"))
    )
    .is_err());
    assert!(CompensationPolicyRepository::insert_version(&conn, &baseline).is_err());
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&conn).unwrap(),
        vec![baseline.clone()]
    );

    assert!(conn
        .execute(
            "UPDATE compensation_policy_versions SET reason = 'changed' WHERE id = 'base'",
            []
        )
        .is_err());
    assert!(conn
        .execute(
            "DELETE FROM compensation_policy_versions WHERE id = 'base'",
            []
        )
        .is_err());
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&conn).unwrap(),
        vec![baseline]
    );
}

#[test]
fn insert_or_replace_cannot_rewrite_baseline_or_correction_leaf() {
    let conn = create_in_memory_connection().unwrap();
    let chain = vec![
        version("base", "2026-01-15", dec!(100), None),
        version("fix-1", "2026-01-15", dec!(110), Some("base")),
        version("fix-2", "2026-01-15", dec!(120), Some("fix-1")),
    ];
    for item in &chain {
        CompensationPolicyRepository::insert_version(&conn, item).unwrap();
    }
    let recursive_triggers: i64 = conn
        .query_row("PRAGMA recursive_triggers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(recursive_triggers, 0);

    let base_json = serde_json::to_string(&chain[0].state).unwrap();
    let leaf_json = serde_json::to_string(&chain[2].state).unwrap();
    let changed_base_json = serde_json::to_string(&policy_state(dec!(999))).unwrap();
    let changed_leaf_json = serde_json::to_string(&policy_state(dec!(888))).unwrap();
    let attempts = [
        ("base", "2026-02-15", base_json.as_str(), None),
        ("base", "2026-01-15", changed_base_json.as_str(), None),
        ("fix-2", "2026-02-15", leaf_json.as_str(), Some("fix-1")),
        (
            "fix-2",
            "2026-01-15",
            changed_leaf_json.as_str(),
            Some("fix-1"),
        ),
    ];
    for (id, date, state_json, parent) in attempts {
        let result = conn.execute(
            "INSERT OR REPLACE INTO compensation_policy_versions
                (id, effective_from, state_json, created_at, source, reason,
                 supersedes_version_id, boundary_scope, road_rounding_mode)
             VALUES (?1, ?2, ?3, 'overwrite', 'overwrite', 'overwrite', ?4,
                     'FULL_EARNING', 'PER_COMPENSATION_SLICE')",
            params![id, date, state_json, parent],
        );
        assert!(result.is_err(), "replacement of {id} should fail");
        assert_eq!(
            CompensationPolicyRepository::get_all_versions(&conn).unwrap(),
            chain
        );
        let period_versions = CompensationPolicyRepository::get_versions_for_period(
            &conn,
            "2026-01-15",
            "2026-02-14",
        )
        .unwrap();
        let resolved =
            resolve_compensation_segments("2026-01-15", "2026-02-14", &period_versions).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].versionId, "fix-2");
        assert_eq!(resolved[0].state.gunlukTabanUcret, dec!(120));
    }
}

#[test]
fn period_scoped_read_skips_unrelated_corrupt_json_but_global_audit_rejects_it() {
    let conn = create_in_memory_connection().unwrap();
    for item in [
        version("old-valid", "2020-01-01", dec!(80), None),
        version("baseline", "2026-01-01", dec!(100), None),
        version("baseline-fix", "2026-01-01", dec!(110), Some("baseline")),
        version("period-change", "2026-02-01", dec!(120), None),
        version("future-valid", "2027-01-01", dec!(130), None),
    ] {
        CompensationPolicyRepository::insert_version(&conn, &item).unwrap();
    }
    insert_raw_policy(&conn, "old-corrupt", "2010-01-01", "{}", None).unwrap();
    insert_raw_policy(&conn, "future-corrupt", "2030-01-01", "{}", None).unwrap();

    let scoped =
        CompensationPolicyRepository::get_versions_for_period(&conn, "2026-01-15", "2026-02-14")
            .unwrap();
    assert_eq!(
        scoped
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["baseline", "baseline-fix", "period-change"]
    );
    assert_eq!(
        resolve_compensation_segments("2026-01-15", "2026-02-14", &scoped)
            .unwrap()
            .iter()
            .map(|item| item.versionId.as_str())
            .collect::<Vec<_>>(),
        vec!["baseline-fix", "period-change"]
    );
    assert!(CompensationPolicyRepository::get_all_versions(&conn).is_err());
}

#[test]
fn period_scoped_read_fails_closed_for_corrupt_selected_state_or_graph() {
    let corrupt_state = create_in_memory_connection().unwrap();
    insert_raw_policy(&corrupt_state, "selected-corrupt", "2026-01-01", "{}", None).unwrap();
    assert!(CompensationPolicyRepository::get_versions_for_period(
        &corrupt_state,
        "2026-01-15",
        "2026-02-14"
    )
    .is_err());

    let corrupt_graph = create_in_memory_connection().unwrap();
    corrupt_graph
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER compensation_policy_validate_insert;",
        )
        .unwrap();
    let state_json = serde_json::to_string(&policy_state(dec!(100))).unwrap();
    insert_raw_policy(
        &corrupt_graph,
        "cycle-a",
        "2026-01-01",
        &state_json,
        Some("cycle-b"),
    )
    .unwrap();
    insert_raw_policy(
        &corrupt_graph,
        "cycle-b",
        "2026-01-01",
        &state_json,
        Some("cycle-a"),
    )
    .unwrap();
    assert!(CompensationPolicyRepository::get_versions_for_period(
        &corrupt_graph,
        "2026-01-15",
        "2026-02-14"
    )
    .is_err());
}

#[test]
fn same_id_and_name_group_round_trips_in_the_repository() {
    let conn = create_in_memory_connection().unwrap();
    let mut policy = version("same-group-token", "2026-01-15", dec!(100), None);
    policy.state.isPrimiGruplari[0].id = "G1".into();
    policy.state.isPrimiGruplari[0].ad = "G1".into();
    CompensationPolicyRepository::insert_version(&conn, &policy).unwrap();

    let loaded =
        CompensationPolicyRepository::get_versions_for_period(&conn, "2026-01-15", "2026-02-14")
            .unwrap();
    assert_eq!(loaded, vec![policy.clone()]);
    let resolved = resolve_compensation_segments("2026-01-15", "2026-02-14", &loaded).unwrap();
    assert_eq!(resolved[0].state.isPrimiGruplari[0].id, "G1");
    assert_eq!(resolved[0].state.isPrimiGruplari[0].ad, "G1");
}

#[test]
fn a_composed_transaction_rolls_back_an_earlier_insert_when_later_conflict_fails() {
    let conn = create_in_memory_connection().unwrap();
    let result = with_transaction(&conn, |tx| {
        CompensationPolicyRepository::insert_version_in_transaction(
            tx,
            &version("first", "2026-01-15", dec!(100), None),
        )?;
        CompensationPolicyRepository::insert_version_in_transaction(
            tx,
            &version("second-root", "2026-01-15", dec!(200), None),
        )?;
        Ok(())
    });
    assert!(result.is_err());
    assert!(CompensationPolicyRepository::get_all_versions(&conn)
        .unwrap()
        .is_empty());
}

#[test]
fn strict_database_read_rejects_malformed_financial_json_and_constraints_reject_bad_links() {
    let conn = create_in_memory_connection().unwrap();
    conn.execute(
        "INSERT INTO compensation_policy_versions
            (id, effective_from, state_json, created_at, source, reason,
             supersedes_version_id, boundary_scope, road_rounding_mode)
         VALUES ('corrupt', '2026-01-15', ?1, 't', 'test', 'test', NULL,
                 'FULL_EARNING', 'PER_COMPENSATION_SLICE')",
        params![json!({"gunlukTabanUcret": 123}).to_string()],
    )
    .unwrap();
    assert!(CompensationPolicyRepository::get_all_versions(&conn).is_err());

    let second = create_in_memory_connection().unwrap();
    let result = second.execute(
        "INSERT INTO compensation_policy_versions
            (id, effective_from, state_json, created_at, source, reason,
             supersedes_version_id, boundary_scope, road_rounding_mode)
         VALUES ('dangling', '2026-01-15', '{}', 't', 'test', 'test', 'missing',
                 'FULL_EARNING', 'PER_COMPENSATION_SLICE')",
        [],
    );
    assert!(result.is_err());
}
