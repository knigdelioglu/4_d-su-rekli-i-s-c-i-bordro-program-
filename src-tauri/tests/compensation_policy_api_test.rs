use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{
    AnnualPayrollParameters, BordroDonemi, CompensationBoundaryScope, CompensationPolicyGroup,
    CompensationPolicyState, CompensationPolicyVersion, CompensationRevisionStatus,
    DonemselKurumDegerleri, PersonelPuantaj, PersonelTaxOpening, RetroAdjustmentBatch,
    RetroAllocation, RetroEarningCode, RetroSettlementStatus, RetroSgkTreatment, RetroTaxTreatment,
    RoadRoundingMode,
};
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::compensation_policy_repo::CompensationPolicyRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::retro_repo;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::repositories::tax_opening_repo::TaxOpeningRepository;
use bordro_programi_lib::services::compensation_policy_service::{
    CompensationPolicyApiError, CompensationPolicyLoadRequest, CompensationPolicyOperationResult,
    CompensationPolicyPrefill, CompensationPolicyPreviewRequest, CompensationPolicySaveRequest,
    CompensationPolicyService, CompensationPolicyWriteOperation, InitialBaselineConfirmation,
};
use bordro_programi_lib::services::payroll_service::PayrollService;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-01".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Ocak 2026".into(),
        taxYear: 2026,
        taxMonth: 2,
    }
}

fn scope() -> bordro_programi_lib::services::compensation_policy_service::CompensationPolicyReadScope
{
    bordro_programi_lib::services::compensation_policy_service::CompensationPolicyReadScope {
        institutionScope: "CURRENT_INSTITUTION".into(),
        periodId: "2026-01".into(),
        periodStartInclusive: "2026-01-15".into(),
        periodEndInclusive: "2026-02-14".into(),
        projectionDate: "2026-01-15".into(),
    }
}

fn policy_state(
    base: rust_decimal::Decimal,
    road: rust_decimal::Decimal,
) -> CompensationPolicyState {
    CompensationPolicyState {
        gunlukTabanUcret: base,
        gunlukYemek: dec!(10),
        birlestirilmisSosyalYardim: dec!(20),
        gunlukVasitaYol: road,
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        isPrimiYuzde: None,
        isPrimiGruplari: vec![CompensationPolicyGroup {
            id: "g1".into(),
            ad: "1. Grup".into(),
            oran: dec!(9),
            aktif: true,
        }],
        geceCalismaPrimiYuzde: dec!(0),
        geceCalismaTatiliPrimiYuzde: dec!(0),
        ekOdeme: None,
        digerGelirVarsayilan: Some(dec!(0)),
    }
}

fn baseline() -> CompensationPolicyWriteOperation {
    CompensationPolicyWriteOperation::InitialBaseline {
        effectiveFrom: "2026-01-15".into(),
        state: policy_state(dec!(100.12345), dec!(5.125)),
        roadRoundingMode: RoadRoundingMode::PeriodTotal,
        reason: "Onaylı başlangıç tarifesi".into(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
    }
}

fn policy_change() -> CompensationPolicyWriteOperation {
    CompensationPolicyWriteOperation::NewEffectiveDate {
        effectiveFrom: "2026-02-01".into(),
        state: policy_state(dec!(110), dec!(5.125)),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        changeIntent: "FULL_STATE_CHANGE".into(),
        roadRoundingMode: RoadRoundingMode::PeriodTotal,
        reason: "Retro policy mutation fixture".into(),
    }
}

fn seed_retro_graph(conn: &rusqlite::Connection, source_status: &str) {
    PeriodRepository::save(conn, &period()).unwrap();
    conn.execute(
        "INSERT INTO personnel
            (id, tc_no, ad, soyad, grup, sgk_sicil_no, iban, hizmet_yili, created_at, updated_at)
         VALUES ('retro-person', '10000000000', 'Retro', 'Worker', '1. Grup', '', '', 3, 't', 't')",
        [],
    )
    .unwrap();
    let mut settings = DonemselKurumDegerleri::default();
    settings.donemId = "2026-01".into();
    SettingsRepository::save_institution_settings(conn, &settings).unwrap();
    let start = chrono::NaiveDate::parse_from_str(&period().baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = chrono::NaiveDate::parse_from_str(&period().bitisTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    let mut date = start;
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += chrono::Duration::days(1);
    }
    AttendanceRepository::save(
        conn,
        &PersonelPuantaj {
            id: "retro-person_2026-01".into(),
            personelId: "retro-person".into(),
            donemId: "2026-01".into(),
            gunler,
        },
    )
    .unwrap();
    CompensationPolicyRepository::insert_version(
        conn,
        &CompensationPolicyVersion {
            id: "retro-policy-baseline".into(),
            effectiveFrom: "2026-01-15".into(),
            state: policy_state(dec!(100.12345), dec!(5.125)),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PeriodTotal,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Canonical source payroll baseline".into(),
            supersedesVersionId: None,
        },
    )
    .unwrap();
    AnnualPayrollParametersRepository::save(conn, &AnnualPayrollParameters::default_for_2026())
        .unwrap();
    TaxOpeningRepository::save(
        conn,
        &PersonelTaxOpening {
            id: "retro-person-2026".into(),
            personnelId: "retro-person".into(),
            year: 2026,
            gvCumulativeOpening: Some(dec!(0)),
            effectiveFromPeriodId: Some("2026-01".into()),
            asgariGvCumulativeOpening: Some(dec!(0)),
            asgariGvEffectiveFromPeriodId: Some("2026-01".into()),
            createdAt: None,
            updatedAt: None,
        },
    )
    .unwrap();
    PayrollService::calculate_payroll_for_personnel(conn, "retro-person", "2026-01")
        .expect("canonical source NORMAL payroll should calculate");
    conn.execute(
        "UPDATE payroll_records SET status = ?1 WHERE id = 'retro-person_2026-01'",
        [source_status],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO compensation_revisions
            (id, reason, title, effective_from, status, scope, personnel_ids_json)
         VALUES ('retro-revision', 'OTHER', 'Retro fixture', '2026-01-15', 'CALCULATED', 'ALL_PERSONNEL', '[]')",
        [],
    )
    .unwrap();
    let batch = RetroAdjustmentBatch {
        id: "retro-batch".into(),
        revisionId: "retro-revision".into(),
        personnelId: "retro-person".into(),
        paymentDate: "2026-02-10".into(),
        status: CompensationRevisionStatus::CALCULATED,
        settlementStatus: RetroSettlementStatus::UNSETTLED,
        totalGrossDelta: dec!(100),
        payableSettlementAmount: dec!(100),
        offsetSettlementAmount: dec!(0),
        recoveredAmount: dec!(0),
        recoverableAmount: dec!(0),
        outstandingReceivable: dec!(0),
        description: Some("Retro fixture".into()),
        createdAt: Some("2026-10-06T00:00:00Z".into()),
        calculatedAt: Some("2026-10-06T00:00:00Z".into()),
        finalizedAt: None,
    };
    let allocation = RetroAllocation {
        id: "retro-allocation".into(),
        batchId: batch.id.clone(),
        personnelId: batch.personnelId.clone(),
        sourcePeriodId: "2026-01".into(),
        earningCode: RetroEarningCode::BASE_WAGE,
        originalRecognizedAmount: dec!(0),
        previousAuthoritativeRetroAmount: dec!(0),
        targetAmount: dec!(100),
        deltaAmount: dec!(100),
        sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
        incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
        stampTaxTreatment: RetroTaxTreatment::TAXABLE,
        originalPek: dec!(0),
        retroPekDelta: dec!(0),
        adjustedPek: dec!(0),
        workerSgkDelta: dec!(0),
        workerUnemploymentDelta: dec!(0),
        employerSgkDelta: dec!(0),
        employerUnemploymentDelta: dec!(0),
        originalEmployerLowerBound: dec!(0),
        targetEmployerLowerBound: dec!(0),
        employerLowerBoundDelta: dec!(0),
        employerLowerBoundPremiumDelta: dec!(0),
        originalSourceCarry: None,
        targetSourceCarry: None,
        payableSettlementAmount: dec!(100),
        offsetSettlementAmount: dec!(0),
        recoverableAmount: dec!(0),
        metadata: None,
    };
    retro_repo::save_batch(conn, &batch, &[allocation]).unwrap();
    conn.execute(
        "INSERT INTO payroll_records (
            id, personnel_id, period_id, accrual_id, accrual_type, payment_date, sequence,
            gross_total, sgk_base, gv_base, previous_cumulative_gv, new_cumulative_gv,
            income_tax, stamp_tax, total_deductions, net_payment, status,
            puantaj_summary_json, calculated_at, updated_at
         ) VALUES (
            'retro-payment-event', 'retro-person', '2026-01', 'retro-batch',
            'RETRO_ADJUSTMENT', '2026-02-10', 1, 10000, 10000, 8500, 0, 8500,
            1000, 75, 1500, 8500, 'CALCULATED', '{}', 't', 't'
         )",
        [],
    )
    .unwrap();
}

#[test]
fn native_api_loads_previews_and_atomically_appends_exact_baseline() {
    let conn = create_in_memory_connection().unwrap();
    PeriodRepository::save(&conn, &period()).unwrap();

    let loaded = CompensationPolicyService::load(
        &conn,
        &CompensationPolicyLoadRequest {
            scope: scope(),
            nonAuthoritativePrefill: None,
        },
    );
    let CompensationPolicyOperationResult::Success { value, .. } = loaded else {
        panic!("empty timeline should load as a confirmed-baseline state");
    };
    assert!(value.initialBaselineRequired);
    assert_eq!(value.timeline.resolution, "MISSING_START_POLICY");
    assert!(matches!(
        value.nonAuthoritativePrefill,
        CompensationPolicyPrefill::Unavailable { reason } if reason == "NO_LEGACY_SETTINGS"
    ));

    let operation = baseline();
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: operation.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("valid first baseline should preview");
    };
    assert_eq!(preview.directCompensationEffects.len(), 1);
    assert_eq!(preview.directCompensationEffects[0].changedFields.len(), 12);
    assert!(preview.dependencyClosureEffects.is_empty());
    assert!(preview.previewToken.policyTimelineVersionIds.is_empty());

    let saved = CompensationPolicyService::save(
        &conn,
        &CompensationPolicySaveRequest {
            scope: scope(),
            operation,
            previewToken: preview.previewToken,
            initialBaselineConfirmation: Some(InitialBaselineConfirmation {
                confirmed: true,
                effectiveFrom: "2026-01-15".into(),
            }),
        },
    );
    let CompensationPolicyOperationResult::Success { value: receipt, .. } = saved else {
        panic!("confirmed baseline should commit");
    };
    assert!(receipt.authoritativeReloadRequired);
    assert!(receipt.invalidatedPayrolls.is_empty());
    assert_eq!(receipt.version.state.gunlukTabanUcret, dec!(100.12345));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM compensation_policy_versions",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn native_road_only_accepts_equal_decimal_values_with_different_scales() {
    let conn = create_in_memory_connection().unwrap();
    PeriodRepository::save(&conn, &period()).unwrap();
    let initial = baseline();
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: initial.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("baseline should preview before ROAD_ONLY scale check");
    };
    let saved = CompensationPolicyService::save(
        &conn,
        &CompensationPolicySaveRequest {
            scope: scope(),
            operation: initial,
            previewToken: preview.previewToken,
            initialBaselineConfirmation: Some(InitialBaselineConfirmation {
                confirmed: true,
                effectiveFrom: "2026-01-15".into(),
            }),
        },
    );
    assert!(matches!(
        saved,
        CompensationPolicyOperationResult::Success { .. }
    ));

    let road_only = CompensationPolicyWriteOperation::NewEffectiveDate {
        effectiveFrom: "2026-02-01".into(),
        state: policy_state(dec!(100.123450000), dec!(8.75)),
        boundaryScope: CompensationBoundaryScope::RoadOnly,
        changeIntent: "ROAD_AMOUNT_ONLY".into(),
        roadRoundingMode: RoadRoundingMode::PeriodTotal,
        reason: "Yol tutarı değişikliği".into(),
    };
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: road_only,
        },
    );
    let CompensationPolicyOperationResult::Success { value, .. } = preview else {
        panic!("ROAD_ONLY must compare Decimal values, not their textual scales: {preview:?}");
    };
    assert_eq!(
        value.directCompensationEffects[0].changedFields,
        vec!["gunlukVasitaYol"]
    );
}

#[test]
fn policy_save_stales_eligible_retro_batch_and_linked_payment_event_atomically() {
    let conn = create_in_memory_connection().unwrap();
    seed_retro_graph(&conn, "CALCULATED");
    let operation = policy_change();
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: operation.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("eligible unsettled retro history should preview with complete state tokens: {preview:?}");
    };
    assert_eq!(preview.retroEffects.len(), 1);
    assert_eq!(
        preview.retroEffects[0].action,
        payroll_core::RetroMutationAction::StaleEligible
    );
    assert!(preview.retroEffects[0].blockers.is_empty());
    assert_eq!(preview.previewToken.retroDependencyStates.len(), 1);
    assert_eq!(
        preview.previewToken.retroDependencyStates[0].linkedPaymentEventIds,
        vec!["retro-payment-event"]
    );
    assert_eq!(
        preview.previewToken.retroDependencyStates[0].persistedState["sourceAttendances"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let saved = CompensationPolicyService::save(
        &conn,
        &CompensationPolicySaveRequest {
            scope: scope(),
            operation,
            previewToken: preview.previewToken,
            initialBaselineConfirmation: None,
        },
    );
    let CompensationPolicyOperationResult::Success { value: receipt, .. } = saved else {
        panic!("eligible retro mutation should commit as one policy transaction");
    };
    assert_eq!(receipt.invalidatedRetroBatches.len(), 1);
    assert_eq!(receipt.invalidatedRetroBatches[0].batchId, "retro-batch");
    assert_eq!(receipt.invalidatedRetroBatches[0].fromStatus, "CALCULATED");
    assert_eq!(receipt.invalidatedRetroBatches[0].toStatus, "STALE");
    assert_eq!(receipt.invalidatedRetroPaymentEvents.len(), 1);
    assert_eq!(
        receipt.invalidatedRetroPaymentEvents[0].payroll.payrollId,
        "retro-payment-event"
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM retro_adjustment_batches WHERE id = 'retro-batch'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "STALE"
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM payroll_records WHERE id = 'retro-payment-event'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "STALE"
    );
}

#[test]
fn finalized_source_returns_a_named_retro_blocker_without_appending_policy() {
    let conn = create_in_memory_connection().unwrap();
    seed_retro_graph(&conn, "FINALIZED");
    let result = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: policy_change(),
        },
    );
    assert!(matches!(
        result,
        CompensationPolicyOperationResult::Failure {
            error: CompensationPolicyApiError::RetroBlocker { blockers, .. },
            ..
        } if blockers.iter().any(|effect| {
            effect.batchId == "retro-batch"
                && effect.blockers.contains(&payroll_core::RetroMutationBlockerReason::FinalizedSource)
        })
    ));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM compensation_policy_versions",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn retro_state_change_after_preview_conflicts_and_rolls_back_policy_append() {
    let conn = create_in_memory_connection().unwrap();
    seed_retro_graph(&conn, "CALCULATED");
    let operation = policy_change();
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: operation.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("retro token preview should succeed: {preview:?}");
    };
    conn.execute(
        "UPDATE retro_adjustment_allocations SET metadata = 'concurrent-edit' WHERE id = 'retro-allocation'",
        [],
    )
    .unwrap();
    let result = CompensationPolicyService::save(
        &conn,
        &CompensationPolicySaveRequest {
            scope: scope(),
            operation,
            previewToken: preview.previewToken,
            initialBaselineConfirmation: None,
        },
    );
    assert!(matches!(
        result,
        CompensationPolicyOperationResult::Failure {
            error: CompensationPolicyApiError::Conflict { .. },
            ..
        }
    ));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM compensation_policy_versions",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM retro_adjustment_batches WHERE id = 'retro-batch'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "CALCULATED"
    );
}

#[test]
fn late_retro_batch_failure_rolls_back_version_payrolls_batch_and_allocations() {
    let conn = create_in_memory_connection().unwrap();
    seed_retro_graph(&conn, "CALCULATED");
    let operation = policy_change();
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: operation.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("late-failure retro graph should preview: {preview:?}");
    };
    conn.execute_batch(
        "CREATE TRIGGER inject_retro_stale_failure
         AFTER UPDATE OF status ON retro_adjustment_batches
         WHEN NEW.id = 'retro-batch' AND NEW.status = 'STALE'
         BEGIN SELECT RAISE(ABORT, 'injected failure after retro stale update'); END;",
    )
    .unwrap();

    let result = CompensationPolicyService::save(
        &conn,
        &CompensationPolicySaveRequest {
            scope: scope(),
            operation,
            previewToken: preview.previewToken,
            initialBaselineConfirmation: None,
        },
    );
    assert!(matches!(
        result,
        CompensationPolicyOperationResult::Failure { .. }
    ));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM compensation_policy_versions",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM payroll_records WHERE id = 'retro-person_2026-01'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "CALCULATED"
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM payroll_records WHERE id = 'retro-payment-event'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "CALCULATED"
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM retro_adjustment_batches WHERE id = 'retro-batch'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "CALCULATED"
    );
    let allocation: (String, Option<String>) = conn
        .query_row(
            "SELECT CAST(target_amount AS TEXT), metadata FROM retro_adjustment_allocations WHERE id = 'retro-allocation'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(allocation, ("10000".into(), None));
}

#[test]
fn save_rechecks_retro_batch_status_settlement_and_source_after_preview() {
    for mutation in ["status", "settlement", "source"] {
        let conn = create_in_memory_connection().unwrap();
        seed_retro_graph(&conn, "CALCULATED");
        let operation = policy_change();
        let preview = CompensationPolicyService::preview(
            &conn,
            &CompensationPolicyPreviewRequest {
                scope: scope(),
                operation: operation.clone(),
            },
        );
        let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
            panic!("retro graph should preview before {mutation} changes: {preview:?}");
        };
        match mutation {
            "status" => {
                conn.execute(
                    "UPDATE retro_adjustment_batches SET status = 'STALE' WHERE id = 'retro-batch'",
                    [],
                )
                .unwrap();
            }
            "settlement" => {
                conn.execute(
                    "UPDATE retro_adjustment_batches SET settlement_status = 'PAID' WHERE id = 'retro-batch'",
                    [],
                )
                .unwrap();
            }
            "source" => {
                conn.execute(
                    "UPDATE payroll_records SET status = 'FINALIZED' WHERE id = 'retro-person_2026-01'",
                    [],
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let result = CompensationPolicyService::save(
            &conn,
            &CompensationPolicySaveRequest {
                scope: scope(),
                operation,
                previewToken: preview.previewToken,
                initialBaselineConfirmation: None,
            },
        );
        match mutation {
            "status" => assert!(matches!(
                result,
                CompensationPolicyOperationResult::Failure {
                    error: CompensationPolicyApiError::Conflict { .. },
                    ..
                }
            )),
            "settlement" => assert!(matches!(
                result,
                CompensationPolicyOperationResult::Failure {
                    error: CompensationPolicyApiError::RetroBlocker { blockers, .. },
                    ..
                } if blockers.iter().any(|effect| {
                    effect.blockers.contains(&payroll_core::RetroMutationBlockerReason::SettledBatch)
                })
            )),
            "source" => assert!(matches!(
                result,
                CompensationPolicyOperationResult::Failure {
                    error: CompensationPolicyApiError::RetroBlocker { blockers, .. },
                    ..
                } if blockers.iter().any(|effect| {
                    effect.blockers.contains(&payroll_core::RetroMutationBlockerReason::FinalizedSource)
                })
            )),
            _ => unreachable!(),
        }
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM compensation_policy_versions",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }
}

#[test]
fn stale_native_preview_token_rolls_back_without_appending() {
    let conn = create_in_memory_connection().unwrap();
    PeriodRepository::save(&conn, &period()).unwrap();
    let operation = baseline();
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope(),
            operation: operation.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("baseline should preview");
    };

    let mut stale_token = preview.previewToken;
    stale_token.targetState.sameDateActiveLeafVersionId = Some("newer-version".into());
    let saved = CompensationPolicyService::save(
        &conn,
        &CompensationPolicySaveRequest {
            scope: scope(),
            operation,
            previewToken: stale_token,
            initialBaselineConfirmation: Some(InitialBaselineConfirmation {
                confirmed: true,
                effectiveFrom: "2026-01-15".into(),
            }),
        },
    );
    assert!(matches!(
        saved,
        CompensationPolicyOperationResult::Failure {
            error: bordro_programi_lib::services::compensation_policy_service::CompensationPolicyApiError::Conflict { .. },
            ..
        }
    ));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM compensation_policy_versions",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn native_load_exposes_current_settings_as_exact_string_prefill_only() {
    let conn = create_in_memory_connection().unwrap();
    PeriodRepository::save(&conn, &period()).unwrap();
    let mut settings = DonemselKurumDegerleri::default();
    settings.donemId = "2026-01".into();
    settings.gunlukTabanUcret = dec!(1234.56789);
    SettingsRepository::save_institution_settings(&conn, &settings).unwrap();

    let loaded = CompensationPolicyService::load(
        &conn,
        &CompensationPolicyLoadRequest {
            scope: scope(),
            nonAuthoritativePrefill: None,
        },
    );
    let CompensationPolicyOperationResult::Success { value, .. } = loaded else {
        panic!("existing settings should be available as an advisory prefill");
    };
    let CompensationPolicyPrefill::Available {
        authority,
        source,
        sourcePeriodId,
        state,
    } = value.nonAuthoritativePrefill
    else {
        panic!("active-period settings should be exposed as prefill-only data");
    };
    assert_eq!(authority, "PREFILL_ONLY");
    assert_eq!(source, "LEGACY_INSTITUTION_SETTINGS");
    assert_eq!(sourcePeriodId, "2026-01");
    assert_eq!(state.gunlukTabanUcret, Some("1234.56789".into()));
}
