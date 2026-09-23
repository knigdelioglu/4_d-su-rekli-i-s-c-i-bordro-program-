pub mod commands;
pub mod db;
pub mod domain;
pub mod repositories;
pub mod services;

use db::{create_connection, DbState};
use std::sync::{Arc, Mutex};

fn configure_builder<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: DbState,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_personnel_list,
            commands::save_personnel,
            commands::save_personnel_and_tax_opening,
            commands::delete_personnel,
            commands::get_tax_openings,
            commands::save_tax_opening,
            commands::get_periods,
            commands::save_period,
            commands::save_period_with_settings,
            commands::get_attendance_list,
            commands::save_attendance,
            commands::get_annual_payroll_parameters,
            commands::save_annual_payroll_parameters,
            commands::get_payroll_list,
            commands::get_payroll_notices,
            commands::calculate_payroll,
            commands::delete_payroll_accrual,
            commands::finalize_payroll,
            commands::evaluate_mutation_policy,
            commands::set_payroll_status,
            commands::get_institution_settings,
            commands::save_institution_settings,
            commands::get_app_setting,
            commands::set_app_setting,
            commands::check_legacy_migrated,
            commands::migrate_legacy_payload,
            commands::replace_backup_payload,
            commands::get_sick_leave_records,
            commands::save_sick_leave_record,
            commands::delete_sick_leave_record,
            commands::get_compensation_revisions,
            commands::get_compensation_revision_overrides,
            commands::get_retro_adjustment_batches,
            commands::get_retro_adjustment_allocations,
            commands::save_compensation_revision,
            commands::calculate_retro_preview,
            commands::save_retro_adjustment_batch,
            commands::create_retro_payment,
        ])
}

pub fn run() {
    let conn = create_connection(None).expect("Failed to initialize SQLite database");
    let state: DbState = Arc::new(Mutex::new(conn));

    configure_builder(tauri::Builder::default(), state)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod ipc_tests {
    use super::*;
    use crate::domain::models::{
        AnnualPayrollParameters, BordroDonemi, DonemselKurumDegerleri, Personel,
        PersonelKesintileri, PersonelPuantaj, PersonelTaxOpening, SickLeaveRecord,
    };
    use chrono::{Duration, NaiveDate};
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
    use tauri::webview::InvokeRequest;

    fn invoke<W: AsRef<tauri::Webview<MockRuntime>>>(
        webview: &W,
        command: &str,
        body: Value,
    ) -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            webview,
            InvokeRequest {
                cmd: command.to_string(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(any(windows, target_os = "android")) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .expect("valid Tauri origin"),
                body: body.into(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|response| {
            response
                .deserialize::<Value>()
                .expect("Tauri IPC response should be valid JSON")
        })
    }

    #[test]
    fn registered_tauri_ipc_commands_round_trip_storage_through_sqlite() {
        let connection = db::create_in_memory_connection()
            .expect("in-memory SQLite should initialize with migrations");
        let state: DbState = Arc::new(Mutex::new(connection));
        let app = configure_builder(mock_builder(), state)
            .build(mock_context(noop_assets()))
            .expect("mock Tauri app should build with the production command handler");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview should build");

        let period = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-15".into(),
            bitisTarihi: "2026-02-14".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 2,
        };
        let settings = DonemselKurumDegerleri {
            donemId: period.id.clone(),
            ..DonemselKurumDegerleri::default()
        };
        let minimum_wage_reference_period = BordroDonemi {
            id: "2025-12".into(),
            yil: 2025,
            ay: 12,
            baslangicTarihi: "2025-12-15".into(),
            bitisTarihi: "2026-01-14".into(),
            donemAdi: "Aralık 2025".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
        let minimum_wage_reference_settings = DonemselKurumDegerleri {
            donemId: minimum_wage_reference_period.id.clone(),
            ..DonemselKurumDegerleri::default()
        };

        assert_eq!(
            invoke(
                &webview,
                "save_period",
                json!({ "period": minimum_wage_reference_period })
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(
                &webview,
                "save_institution_settings",
                json!({ "settings": minimum_wage_reference_settings })
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(&webview, "save_period", json!({ "period": period.clone() })),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(
                &webview,
                "save_period_with_settings",
                json!({ "period": period.clone(), "settings": settings.clone() })
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(
                &webview,
                "save_institution_settings",
                json!({ "settings": settings })
            ),
            Ok(Value::Null)
        );

        let periods = invoke(&webview, "get_periods", json!({}))
            .expect("get_periods IPC command should succeed");
        assert_eq!(periods.as_array().map(Vec::len), Some(2));
        assert_eq!(periods[1]["id"], "2026-01");

        let institution_settings = invoke(&webview, "get_institution_settings", json!({}))
            .expect("get_institution_settings IPC command should succeed");
        assert_eq!(institution_settings["2026-01"]["donemId"], "2026-01");
        assert_eq!(institution_settings["2025-12"]["donemId"], "2025-12");

        assert_eq!(
            invoke(&webview, "get_app_setting", json!({ "key": "ipc-missing" })),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(
                &webview,
                "set_app_setting",
                json!({ "key": "ipc-test", "value": "saved" })
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(&webview, "get_app_setting", json!({ "key": "ipc-test" })),
            Ok(json!("saved"))
        );

        let person = Personel {
            id: "ipc-person".into(),
            tcNo: "10000000001".into(),
            ad: "IPC".into(),
            soyad: "Test".into(),
            grup: "1. Grup".into(),
            unvan: None,
            sgkSicilNo: "ipc-sgk".into(),
            iban: "TR000000000000000000000000".into(),
            hizmetYili: 1,
            aciklama: None,
            devirKumulatifGvMatrahi: None,
            devirKumulatifGvMatrahiYili: None,
            devirKumulatifGvMatrahiBaslangicAyi: None,
            devirKumulatifAsgariGvMatrahi: None,
            devirKumulatifAsgariGvMatrahiYili: None,
            kesintiler: Some(PersonelKesintileri::default()),
        };
        assert_eq!(
            invoke(
                &webview,
                "save_personnel",
                json!({ "personel": person.clone() })
            ),
            Ok(Value::Null)
        );
        let tax_opening = PersonelTaxOpening {
            id: "ipc-person-2026".into(),
            personnelId: person.id.clone(),
            year: 2026,
            gvCumulativeOpening: Some(rust_decimal::Decimal::from(1200)),
            effectiveFromPeriodId: Some("2026-01".into()),
            asgariGvCumulativeOpening: None,
            asgariGvEffectiveFromPeriodId: None,
            createdAt: None,
            updatedAt: None,
        };
        assert_eq!(
            invoke(
                &webview,
                "save_personnel_and_tax_opening",
                json!({ "personel": person.clone(), "taxOpening": tax_opening.clone() })
            ),
            Ok(Value::Null)
        );
        let personnel = invoke(&webview, "get_personnel_list", json!({}))
            .expect("get_personnel_list IPC command should succeed");
        assert_eq!(personnel.as_array().map(Vec::len), Some(1));
        assert_eq!(personnel[0]["id"], "ipc-person");
        let tax_openings = invoke(&webview, "get_tax_openings", json!({}))
            .expect("get_tax_openings IPC command should succeed");
        assert_eq!(tax_openings.as_array().map(Vec::len), Some(1));
        assert_eq!(tax_openings[0]["personnelId"], "ipc-person");
        assert_eq!(
            invoke(
                &webview,
                "save_tax_opening",
                json!({ "taxOpening": tax_opening })
            ),
            Ok(Value::Null)
        );

        let attendance_start = NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid start date");
        let attendance = PersonelPuantaj {
            id: "ipc-person_2026-01".into(),
            personelId: "ipc-person".into(),
            donemId: "2026-01".into(),
            gunler: (0..31)
                .map(|offset| {
                    (
                        (attendance_start + Duration::days(offset))
                            .format("%Y-%m-%d")
                            .to_string(),
                        "Ç".into(),
                    )
                })
                .collect::<HashMap<_, _>>(),
        };
        assert_eq!(
            invoke(
                &webview,
                "save_attendance",
                json!({ "attendance": attendance })
            ),
            Ok(Value::Null)
        );
        let attendance_list = invoke(&webview, "get_attendance_list", json!({}))
            .expect("get_attendance_list IPC command should succeed");
        assert_eq!(attendance_list.as_array().map(Vec::len), Some(1));
        assert_eq!(attendance_list[0]["personelId"], "ipc-person");

        let annual_parameters = AnnualPayrollParameters::default_for_2026();
        assert_eq!(
            invoke(
                &webview,
                "save_annual_payroll_parameters",
                json!({ "parameters": annual_parameters })
            ),
            Ok(Value::Null)
        );
        let annual_parameters = invoke(&webview, "get_annual_payroll_parameters", json!({}))
            .expect("get_annual_payroll_parameters IPC command should succeed");
        assert_eq!(annual_parameters.as_array().map(Vec::len), Some(1));
        assert_eq!(annual_parameters[0]["year"], 2026);

        let sick_leave = SickLeaveRecord {
            id: "ipc-sick-leave".into(),
            personnelId: "ipc-person".into(),
            startDate: "2026-01-20".into(),
            endDate: "2026-01-21".into(),
            createdAt: None,
            updatedAt: None,
        };
        assert_eq!(
            invoke(
                &webview,
                "save_sick_leave_record",
                json!({ "record": sick_leave })
            ),
            Ok(Value::Null)
        );
        let sick_leave_records = invoke(
            &webview,
            "get_sick_leave_records",
            json!({ "personnelId": "ipc-person" }),
        )
        .expect("get_sick_leave_records IPC command should succeed");
        assert_eq!(sick_leave_records.as_array().map(Vec::len), Some(1));
        assert_eq!(sick_leave_records[0]["id"], "ipc-sick-leave");
        assert_eq!(
            invoke(
                &webview,
                "delete_sick_leave_record",
                json!({ "id": "ipc-sick-leave" })
            ),
            Ok(Value::Null)
        );
        let sick_leave_records = invoke(
            &webview,
            "get_sick_leave_records",
            json!({ "personnelId": "ipc-person" }),
        )
        .expect("sick leave deletion should persist through SQLite");
        assert_eq!(sick_leave_records, json!([]));

        let finalized_transition = invoke(
            &webview,
            "set_payroll_status",
            json!({
                "personnelId": "ipc-person",
                "periodId": "2026-01",
                "status": "FINALIZED",
                "accrualId": null
            }),
        );
        let finalized_error = finalized_transition
            .expect_err("FINALIZED must only be reachable through finalize_payroll");
        assert!(
            format!("{finalized_error:?}").contains("FINALIZED"),
            "registered set_payroll_status command should reject direct finalization"
        );

        assert!(invoke(&webview, "check_legacy_migrated", json!({}))
            .expect("check_legacy_migrated IPC command should succeed")
            .is_boolean());

        assert!(invoke(
            &webview,
            "calculate_payroll",
            json!({
                "personnelId": "missing-person",
                "periodId": "missing-period",
                "manualIncome": null,
                "accrual": null
            })
        )
        .is_err());
        assert!(invoke(
            &webview,
            "finalize_payroll",
            json!({
                "personnelId": "missing-person",
                "periodId": "missing-period",
                "accrualId": null
            })
        )
        .is_err());
        let mutation_impact = invoke(
            &webview,
            "evaluate_mutation_policy",
            json!({ "mutation": { "kind": "ALL" } }),
        )
        .expect("evaluate_mutation_policy IPC command should succeed");
        assert!(mutation_impact.is_object());
        assert!(invoke(
            &webview,
            "delete_payroll_accrual",
            json!({
                "personnelId": "ipc-person",
                "periodId": "2026-01",
                "accrualId": "missing-accrual"
            })
        )
        .is_err());

        let revision = json!({
            "id": "ipc-revision",
            "reason": "COLLECTIVE_AGREEMENT",
            "title": "IPC revision",
            "effectiveFrom": "2026-01-15",
            "status": "DRAFT",
            "scope": "SELECTED_PERSONNEL",
            "personnelIds": ["ipc-person"]
        });
        assert_eq!(
            invoke(
                &webview,
                "save_compensation_revision",
                json!({ "revision": revision.clone(), "overrides": [] })
            ),
            Ok(Value::Null)
        );
        let retro_preview = invoke(
            &webview,
            "calculate_retro_preview",
            json!({
                "batchId": "ipc-preview",
                "revision": revision,
                "overrides": [],
                "personnelId": "ipc-person",
                "paymentDate": "2026-02-20",
                "calculatedAt": "2026-02-20T00:00:00Z",
                "description": "IPC preview"
            }),
        );
        assert!(
            retro_preview.is_ok(),
            "calculate_retro_preview IPC command should succeed: {retro_preview:?}"
        );

        let draft_batch = json!({
            "id": "ipc-draft-batch",
            "revisionId": "ipc-revision",
            "personnelId": "ipc-person",
            "paymentDate": "2026-02-20",
            "status": "DRAFT",
            "totalGrossDelta": "0"
        });
        assert!(invoke(
            &webview,
            "save_retro_adjustment_batch",
            json!({ "batch": draft_batch.clone(), "allocations": [] })
        )
        .is_err());
        assert!(invoke(
            &webview,
            "create_retro_payment",
            json!({
                "batch": draft_batch,
                "allocations": [],
                "paymentPeriodId": "2026-01",
                "sequence": 0
            })
        )
        .is_err());

        assert_eq!(
            invoke(
                &webview,
                "migrate_legacy_payload",
                json!({ "payloadJson": "{}" })
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(&webview, "check_legacy_migrated", json!({})),
            Ok(json!(true))
        );

        let calculated_payroll = invoke(
            &webview,
            "calculate_payroll",
            json!({
                "personnelId": "ipc-person",
                "periodId": "2026-01",
                "manualIncome": null,
                "accrual": null
            }),
        );
        assert!(
            calculated_payroll.is_ok(),
            "calculate_payroll IPC command should succeed: {calculated_payroll:?}"
        );
        let calculated_payroll = calculated_payroll.expect("checked above");
        assert_eq!(calculated_payroll["personelId"], "ipc-person");
        let accrual_id = calculated_payroll["accrualId"]
            .as_str()
            .expect("calculated payroll should expose its accrual id");
        assert_eq!(
            invoke(
                &webview,
                "delete_payroll_accrual",
                json!({
                    "personnelId": "ipc-person",
                    "periodId": "2026-01",
                    "accrualId": accrual_id
                })
            ),
            Ok(Value::Null)
        );
        assert!(invoke(
            &webview,
            "calculate_payroll",
            json!({
                "personnelId": "ipc-person",
                "periodId": "2026-01",
                "manualIncome": null,
                "accrual": null
            })
        )
        .is_ok());
        let finalized_payroll = invoke(
            &webview,
            "finalize_payroll",
            json!({
                "personnelId": "ipc-person",
                "periodId": "2026-01",
                "accrualId": null
            }),
        );
        assert!(
            finalized_payroll.is_ok(),
            "finalize_payroll IPC command should succeed: {finalized_payroll:?}"
        );
        assert_eq!(
            finalized_payroll.expect("checked above")["status"],
            "FINALIZED"
        );

        let disposable_person = Personel {
            id: "ipc-delete-person".into(),
            tcNo: "10000000002".into(),
            sgkSicilNo: "ipc-delete-sgk".into(),
            ..person.clone()
        };
        assert_eq!(
            invoke(
                &webview,
                "save_personnel",
                json!({ "personel": disposable_person })
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(
                &webview,
                "delete_personnel",
                json!({ "id": "ipc-delete-person" })
            ),
            Ok(Value::Null)
        );

        for (command, arguments) in [
            ("get_payroll_list", json!({})),
            ("get_payroll_notices", json!({ "periodId": "2026-01" })),
            ("get_compensation_revisions", json!({})),
            ("get_compensation_revision_overrides", json!({})),
            ("get_retro_adjustment_batches", json!({})),
            ("get_retro_adjustment_allocations", json!({})),
        ] {
            let result = invoke(&webview, command, arguments);
            assert!(
                result.is_ok(),
                "{command} IPC command should succeed: {result:?}"
            );
            let result = result.expect("checked above");
            assert!(
                result.is_array(),
                "{command} IPC command should return an array"
            );
        }

        let backup_state: DbState = Arc::new(Mutex::new(
            db::create_in_memory_connection().expect("isolated backup database should initialize"),
        ));
        let backup_app = configure_builder(mock_builder(), backup_state)
            .build(mock_context(noop_assets()))
            .expect("isolated backup mock app should build");
        let backup_webview =
            tauri::WebviewWindowBuilder::new(&backup_app, "backup-test", Default::default())
                .build()
                .expect("isolated backup mock webview should build");
        assert_eq!(
            invoke(
                &backup_webview,
                "replace_backup_payload",
                json!({
                    "payloadJson": r#"{
                        "backupVersion": 5,
                        "donemler": [],
                        "personeller": [],
                        "kurumDegerleriMap": {},
                        "puantajlar": [],
                        "bordrolar": [],
                        "taxOpenings": [],
                        "sickLeaveRecords": [],
                        "annualPayrollParameters": [],
                        "zamAylari": [],
                        "compensationRevisions": [],
                        "compensationRevisionOverrides": [],
                        "retroBatches": [],
                        "retroAllocations": []
                    }"#
                })
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            invoke(&backup_webview, "get_periods", json!({})),
            Ok(json!([]))
        );
    }
}
