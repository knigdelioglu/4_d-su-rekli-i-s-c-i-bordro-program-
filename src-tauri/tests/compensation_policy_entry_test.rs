use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::compensation_policy_repo::CompensationPolicyRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::retro_repo;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::repositories::tax_opening_repo::TaxOpeningRepository;
use bordro_programi_lib::services::compensation_policy_service::{
    CompensationPolicyOperationResult, CompensationPolicyPreviewRequest,
    CompensationPolicyReadScope, CompensationPolicySaveRequest, CompensationPolicyService,
    CompensationPolicyWriteOperation, InitialBaselineConfirmation,
};
use bordro_programi_lib::services::migration_service::MigrationService;
use bordro_programi_lib::services::payroll_service::PayrollService;
use bordro_programi_lib::services::period_service::PeriodService;
use chrono::{Datelike, Duration, NaiveDate};
use payroll_core::*;
use rust_decimal_macros::dec;
use serde_json::{json, Value};
use std::collections::HashMap;

fn period(id: &str, start: &str, end: &str, tax_year: i32, tax_month: i32) -> BordroDonemi {
    let date = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
    BordroDonemi {
        id: id.into(),
        yil: date.year(),
        ay: date.month() as i32,
        baslangicTarihi: start.into(),
        bitisTarihi: end.into(),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "10000000000".into(),
        ad: "Policy".into(),
        soyad: "Roundtrip".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "".into(),
        iban: "TR330006100519786457841326".into(),
        hizmetYili: 3,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn policy_state() -> CompensationPolicyState {
    CompensationPolicyState {
        gunlukTabanUcret: dec!(2000.125),
        gunlukYemek: dec!(250.25),
        birlestirilmisSosyalYardim: dec!(3000.50),
        gunlukVasitaYol: dec!(75.125),
        giyimYardimi: dec!(125),
        hizmetZammiBirimi: dec!(10.125),
        isPrimiYuzde: None,
        isPrimiGruplari: vec![CompensationPolicyGroup {
            id: "1. Grup".into(),
            ad: "Grup 1".into(),
            oran: dec!(9),
            aktif: true,
        }],
        geceCalismaPrimiYuzde: dec!(0),
        geceCalismaTatiliPrimiYuzde: dec!(0),
        ekOdeme: Some(dec!(0)),
        digerGelirVarsayilan: Some(dec!(0)),

        socialBenefitAccrualPolicies: None,
    }
}

fn attendance(personnel_id: &str, period: &BordroDonemi) -> PersonelPuantaj {
    let mut date = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut days = HashMap::new();
    while date <= end {
        days.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("{personnel_id}_{}", period.id),
        personelId: personnel_id.into(),
        donemId: period.id.clone(),
        gunler: days,
    }
}

fn assert_policy_roundtrip(period_id: &str, start: &str, end: &str, tax_year: i32, tax_month: i32) {
    let conn = create_in_memory_connection().unwrap();
    let personnel_id = format!("policy-{period_id}");
    let personnel = person(&personnel_id);
    let target = period(period_id, start, end, tax_year, tax_month);
    let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
    let previous_calendar_month = NaiveDate::from_ymd_opt(start_date.year(), start_date.month(), 1)
        .unwrap()
        - Duration::days(1);
    let previous_start = NaiveDate::from_ymd_opt(
        previous_calendar_month.year(),
        previous_calendar_month.month(),
        15,
    )
    .unwrap();
    let previous = period(
        &format!("{period_id}-previous"),
        &previous_start.format("%Y-%m-%d").to_string(),
        &(start_date - Duration::days(1))
            .format("%Y-%m-%d")
            .to_string(),
        tax_year,
        tax_month - 1,
    );

    PersonnelRepository::save(&conn, &personnel).unwrap();
    PeriodRepository::save(&conn, &previous).unwrap();
    PeriodRepository::save(&conn, &target).unwrap();
    SettingsRepository::save_institution_settings(
        &conn,
        &DonemselKurumDegerleri {
            donemId: target.id.clone(),
            ..DonemselKurumDegerleri::default()
        },
    )
    .unwrap();
    SettingsRepository::save_institution_settings(
        &conn,
        &DonemselKurumDegerleri {
            donemId: previous.id.clone(),
            ..DonemselKurumDegerleri::default()
        },
    )
    .unwrap();
    SettingsRepository::set_app_setting(&conn, "zam_aylari", "[]").unwrap();
    AttendanceRepository::save(&conn, &attendance(&personnel_id, &target)).unwrap();

    let mut annual = AnnualPayrollParameters::default_for_2026();
    annual.year = tax_year;
    // 2027 exercises schema/date coverage with synthetic annual parameters;
    // they are not represented as official statutory values.
    AnnualPayrollParametersRepository::save(&conn, &annual).unwrap();
    TaxOpeningRepository::save(
        &conn,
        &PersonelTaxOpening {
            id: format!("{personnel_id}_{tax_year}"),
            personnelId: personnel_id.clone(),
            year: tax_year,
            gvCumulativeOpening: Some(dec!(0)),
            effectiveFromPeriodId: Some(target.id.clone()),
            asgariGvCumulativeOpening: Some(dec!(0)),
            asgariGvEffectiveFromPeriodId: Some(target.id.clone()),
            createdAt: None,
            updatedAt: None,
        },
    )
    .unwrap();

    let earlier_supplemental = PayrollAccrualInput {
        accrualId: format!("supplemental-{period_id}"),
        accrualType: AccrualType::SUPPLEMENTAL,
        paymentDate: (NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap() - Duration::days(10))
            .format("%Y-%m-%d")
            .to_string(),
        sequence: 0,
        grossAmount: Some(dec!(100)),
        description: Some("Earlier finalized event".into()),
    };
    let earlier = PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        &personnel_id,
        &target.id,
        Some(&earlier_supplemental),
        None,
    )
    .unwrap();
    assert_eq!(earlier.status, BordroStatus::CALCULATED);
    let earlier = PayrollService::finalize_payroll_for_accrual(
        &conn,
        &personnel_id,
        &target.id,
        Some(&earlier_supplemental.accrualId),
    )
    .unwrap();
    assert_eq!(earlier.status, BordroStatus::FINALIZED);

    // The explicit path must not parse old compensation settings or the
    // global raise-month value after the finalized earlier event exists.
    conn.execute(
        "UPDATE institution_settings SET settings_json = '{broken' WHERE period_id = ?1",
        [&previous.id],
    )
    .unwrap();
    conn.execute(
        "UPDATE app_settings SET value = 'not-json' WHERE key = 'zam_aylari'",
        [],
    )
    .unwrap();
    CompensationPolicyRepository::insert_version(
        &conn,
        &CompensationPolicyVersion {
            id: format!("policy-{period_id}"),
            effectiveFrom: start.into(),
            state: policy_state(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PeriodTotal,
            createdAt: "2026-10-06T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Schema-2 native entry-path test".into(),
            supersedesVersionId: None,
        },
    )
    .unwrap();

    let snapshot = PayrollService::build_compensation_policy_calculation_snapshot_for(
        &conn,
        &personnel_id,
        &target.id,
    )
    .unwrap();
    assert!(snapshot.periods.iter().any(|item| item.id == previous.id));
    assert_eq!(snapshot.institutionSettings.len(), 1);
    assert!(snapshot.institutionSettings.contains_key(&target.id));
    assert_eq!(snapshot.compensationPolicyVersions.len(), 1);

    let request = PayrollCalculationRequest {
        personnelId: personnel_id.clone(),
        periodId: target.id.clone(),
        calculatedAt: "2026-10-06T00:00:00Z".into(),
        manualIncome: None,
        accrual: None,
        dataset: snapshot,
    };
    let oracle = calculate_payroll_with_compensation_policy_checked(&request).unwrap();
    let calculated =
        PayrollService::calculate_payroll_for_accrual_with_compensation_policy_checked(
            &conn,
            &personnel_id,
            &target.id,
            None,
            None,
        )
        .unwrap();
    assert_eq!(calculated.gelirToplam, oracle.gelirToplam);
    assert_eq!(calculated.kesintiToplam, oracle.kesintiToplam);
    assert_eq!(calculated.netOdeme, oracle.netOdeme);
    assert_eq!(calculated.sonrakiDevredenPek, oracle.sonrakiDevredenPek);
    let saved = PayrollRepository::get_all(&conn).unwrap();
    assert_eq!(saved.len(), 2);
    let current_saved = saved
        .iter()
        .find(|row| row.accrualType == AccrualType::NORMAL)
        .unwrap();
    let metadata = current_saved.calculationMetadata.as_ref().unwrap();
    assert_eq!(
        metadata.provenance,
        PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE
    );
    let calculation_snapshot = metadata.snapshot.as_ref().unwrap();
    assert_eq!(calculation_snapshot.snapshotSchemaVersion, 2);
    assert_eq!(
        serde_json::to_value(calculation_snapshot).unwrap(),
        serde_json::to_value(
            calculated
                .calculationMetadata
                .as_ref()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
        )
        .unwrap()
    );

    // Retro preview intentionally loads the complete historical graph. Restore
    // inputs corrupted above for the scoped Normal calculation before building
    // that separate lifecycle request.
    let restored_previous_settings = DonemselKurumDegerleri {
        donemId: previous.id.clone(),
        ..DonemselKurumDegerleri::default()
    };
    conn.execute(
        "UPDATE institution_settings SET settings_json = ?1 WHERE period_id = ?2",
        rusqlite::params![
            serde_json::to_string(&restored_previous_settings).unwrap(),
            previous.id
        ],
    )
    .unwrap();
    conn.execute(
        "UPDATE app_settings SET value = '[]' WHERE key = 'zam_aylari'",
        [],
    )
    .unwrap();

    let retro_revision = CompensationRevision {
        id: format!("native-lifecycle-revision-{period_id}"),
        reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
        title: "Native schema-2 lifecycle".into(),
        effectiveFrom: start.into(),
        effectiveTo: None,
        decisionDate: Some(start.into()),
        signedAt: Some(start.into()),
        description: Some("Calculated from persisted schema-2 source".into()),
        status: CompensationRevisionStatus::DRAFT,
        scope: CompensationRevisionScope::SELECTED_PERSONNEL,
        personnelIds: vec![personnel_id.clone()],
        personnelGroup: None,
        createdAt: Some("2026-10-06T00:00:00Z".into()),
        updatedAt: None,
    };
    let retro_override = CompensationRevisionOverride {
        id: format!("native-lifecycle-override-{period_id}"),
        revisionId: retro_revision.id.clone(),
        parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
        value: dec!(2001.13),
        personnelId: Some(personnel_id.clone()),
    };
    retro_repo::save_revision_with_overrides(
        &conn,
        &retro_revision,
        std::slice::from_ref(&retro_override),
    )
    .unwrap();
    let retro_dataset = PayrollService::build_dataset_snapshot(&conn).unwrap();
    let batch_id = format!("native-lifecycle-batch-{period_id}");
    let retro = RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: batch_id.clone(),
        revision: retro_revision,
        overrides: vec![retro_override],
        personnelId: personnel_id.clone(),
        paymentDate: end.into(),
        calculatedAt: "2026-10-06T00:00:00Z".into(),
        description: Some("Native lifecycle payment".into()),
        dataset: retro_dataset,
    })
    .expect("native persisted schema-2 payroll should produce a retro preview");
    assert_eq!(retro.batch.status, CompensationRevisionStatus::CALCULATED);
    assert!(retro.batch.totalGrossDelta > rust_decimal::Decimal::ZERO);
    assert!(retro
        .allocations
        .iter()
        .any(|item| item.deltaAmount > rust_decimal::Decimal::ZERO));
    let retro_payment = PayrollService::create_retro_payment(
        &conn,
        &retro.batch,
        &retro.allocations,
        &target.id,
        1,
    )
    .expect("native calculated retro should persist a real payment event");
    assert_eq!(retro_payment.status, BordroStatus::CALCULATED);
    assert_eq!(retro_payment.accrualId, batch_id);

    let mut corrected_state = policy_state();
    corrected_state.gunlukTabanUcret += dec!(1);
    let correction = CompensationPolicyWriteOperation::SameDateCorrection {
        effectiveFrom: start.into(),
        state: corrected_state,
        roadRoundingMode: RoadRoundingMode::PeriodTotal,
        reason: "Native dependency-scope regression".into(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        changeIntent: "FULL_STATE_CHANGE".into(),
        supersedesVersionId: format!("policy-{period_id}"),
    };
    let scope =
        bordro_programi_lib::services::compensation_policy_service::CompensationPolicyReadScope {
            institutionScope: "CURRENT_INSTITUTION".into(),
            periodId: target.id.clone(),
            periodStartInclusive: target.baslangicTarihi.clone(),
            periodEndInclusive: target.bitisTarihi.clone(),
            projectionDate: target.baslangicTarihi.clone(),
        };
    let preview = CompensationPolicyService::preview(
        &conn,
        &CompensationPolicyPreviewRequest {
            scope: scope.clone(),
            operation: correction.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("earlier finalized supplemental must not block the later policy root");
    };
    assert_eq!(preview.retroEffects.len(), 1);
    assert_eq!(preview.retroEffects[0].batchId, retro.batch.id);
    assert_eq!(
        preview.retroEffects[0].action,
        RetroMutationAction::StaleEligible
    );
    assert!(preview.retroEffects[0].blockers.is_empty());
    let affected_accruals = preview
        .dependencyClosureEffects
        .iter()
        .map(|effect| effect.affectedPayroll.accrualId.as_str())
        .collect::<Vec<_>>();
    assert!(affected_accruals.contains(&current_saved.accrualId.as_str()));
    assert!(affected_accruals.contains(&retro_payment.accrualId.as_str()));
    let receipt = CompensationPolicyService::save(
        &conn,
        &CompensationPolicySaveRequest {
            scope,
            operation: correction,
            previewToken: preview.previewToken,
            initialBaselineConfirmation: None,
        },
    );
    let CompensationPolicyOperationResult::Success { value: receipt, .. } = receipt else {
        panic!("unchanged native dependency token should save");
    };
    let invalidated_accruals = receipt
        .invalidatedPayrolls
        .iter()
        .map(|item| item.payroll.accrualId.as_str())
        .collect::<Vec<_>>();
    assert!(invalidated_accruals.contains(&current_saved.accrualId.as_str()));
    assert!(invalidated_accruals.contains(&retro_payment.accrualId.as_str()));
    assert_eq!(receipt.invalidatedRetroBatches.len(), 1);
    assert_eq!(receipt.invalidatedRetroBatches[0].batchId, retro.batch.id);
    assert_eq!(receipt.invalidatedRetroPaymentEvents.len(), 1);
    assert_eq!(
        receipt.invalidatedRetroPaymentEvents[0].payroll.payrollId,
        retro_payment.id
    );
    let after_policy_change = PayrollRepository::get_all(&conn).unwrap();
    assert_eq!(
        after_policy_change
            .iter()
            .find(|row| row.accrualId == earlier_supplemental.accrualId)
            .unwrap()
            .status,
        BordroStatus::FINALIZED
    );
    assert_eq!(
        after_policy_change
            .iter()
            .find(|row| row.accrualType == AccrualType::NORMAL)
            .unwrap()
            .status,
        BordroStatus::STALE
    );
    assert_eq!(
        after_policy_change
            .iter()
            .find(|row| row.id == retro_payment.id)
            .unwrap()
            .status,
        BordroStatus::STALE
    );

    let updated_request = PayrollCalculationRequest {
        dataset: PayrollService::build_compensation_policy_calculation_snapshot_for(
            &conn,
            &personnel_id,
            &target.id,
        )
        .unwrap(),
        ..request
    };
    let updated_oracle =
        calculate_payroll_with_compensation_policy_checked(&updated_request).unwrap();
    let recalculated =
        PayrollService::calculate_payroll_for_accrual_with_compensation_policy_checked(
            &conn,
            &personnel_id,
            &target.id,
            None,
            None,
        )
        .unwrap();
    assert_eq!(recalculated.gelirToplam, updated_oracle.gelirToplam);
    assert_eq!(recalculated.netOdeme, updated_oracle.netOdeme);

    let later_start = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap() + Duration::days(1);
    let later_end = later_start
        .checked_add_months(chrono::Months::new(1))
        .unwrap()
        - Duration::days(1);
    let later_tax_year = if tax_month == 12 {
        tax_year + 1
    } else {
        tax_year
    };
    let later_tax_month = if tax_month == 12 { 1 } else { tax_month + 1 };
    let later = period(
        &format!("{period_id}-next"),
        &later_start.format("%Y-%m-%d").to_string(),
        &later_end.format("%Y-%m-%d").to_string(),
        later_tax_year,
        later_tax_month,
    );
    PeriodRepository::save(&conn, &later).unwrap();
    SettingsRepository::save_institution_settings(
        &conn,
        &DonemselKurumDegerleri {
            donemId: later.id.clone(),
            ..DonemselKurumDegerleri::default()
        },
    )
    .unwrap();
    AttendanceRepository::save(&conn, &attendance(&personnel_id, &later)).unwrap();

    let stale_event =
        PayrollService::calculate_payroll_for_accrual_with_compensation_policy_checked(
            &conn,
            &personnel_id,
            &later.id,
            None,
            None,
        )
        .expect_err("downstream replay must block while the invalidated retro event is unresolved");
    assert!(stale_event.to_string().contains("STALE"));

    let replay_dataset = PayrollService::build_dataset_snapshot(&conn).unwrap();
    let replay_revision = replay_dataset
        .compensationRevisions
        .iter()
        .find(|revision| revision.id == retro.batch.revisionId)
        .cloned()
        .unwrap();
    let replay_overrides = replay_dataset
        .compensationRevisionOverrides
        .iter()
        .filter(|override_item| override_item.revisionId == replay_revision.id)
        .cloned()
        .collect();
    let replay = RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: retro.batch.id.clone(),
        revision: replay_revision,
        overrides: replay_overrides,
        personnelId: personnel_id.clone(),
        paymentDate: retro.batch.paymentDate.clone(),
        calculatedAt: "2026-10-06T00:00:00Z".into(),
        description: retro.batch.description.clone(),
        dataset: replay_dataset,
    })
    .expect("the invalidated retro batch should replay against the corrected payroll");
    let refreshed_payment = PayrollService::create_retro_payment(
        &conn,
        &replay.batch,
        &replay.allocations,
        &target.id,
        1,
    )
    .expect("canonical replay should refresh the stale payment event");
    assert_eq!(refreshed_payment.status, BordroStatus::CALCULATED);

    let downstream =
        PayrollService::calculate_payroll_for_accrual_with_compensation_policy_checked(
            &conn,
            &personnel_id,
            &later.id,
            None,
            None,
        )
        .unwrap();
    assert_eq!(downstream.status, BordroStatus::CALCULATED);

    let before_failure = PayrollRepository::get_all(&conn).unwrap();
    let source_before_failure = before_failure
        .iter()
        .find(|row| row.accrualType == AccrualType::NORMAL && row.donemId == target.id)
        .unwrap();
    let downstream_before_failure = before_failure
        .iter()
        .find(|row| row.accrualType == AccrualType::NORMAL && row.donemId == later.id)
        .unwrap();
    conn.execute_batch(
        "CREATE TEMP TRIGGER fail_compensation_finalization_invalidation
         BEFORE UPDATE OF status ON payroll_records
         WHEN NEW.status = 'STALE'
         BEGIN
             SELECT RAISE(ABORT, 'forced compensation finalization invalidation failure');
         END;",
    )
    .unwrap();
    let failed_finalize =
        PayrollService::finalize_payroll_for_personnel(&conn, &personnel_id, &target.id)
            .expect_err("a downstream invalidation write failure must abort finalization");
    assert!(failed_finalize
        .to_string()
        .contains("forced compensation finalization invalidation failure"));
    let after_failure = PayrollRepository::get_all(&conn).unwrap();
    let source_after_failure = after_failure
        .iter()
        .find(|row| row.accrualType == AccrualType::NORMAL && row.donemId == target.id)
        .unwrap();
    let downstream_after_failure = after_failure
        .iter()
        .find(|row| row.accrualType == AccrualType::NORMAL && row.donemId == later.id)
        .unwrap();
    assert_eq!(source_after_failure.status, BordroStatus::CALCULATED);
    assert_eq!(downstream_after_failure.status, BordroStatus::CALCULATED);
    assert_eq!(
        serde_json::to_value(source_after_failure).unwrap(),
        serde_json::to_value(source_before_failure).unwrap(),
        "the failed finalization must roll back the source-row write"
    );
    assert_eq!(
        serde_json::to_value(downstream_after_failure).unwrap(),
        serde_json::to_value(downstream_before_failure).unwrap(),
        "the failed finalization must roll back partial downstream invalidation"
    );
    conn.execute_batch("DROP TRIGGER fail_compensation_finalization_invalidation;")
        .unwrap();

    let finalized =
        PayrollService::finalize_payroll_for_personnel(&conn, &personnel_id, &target.id).unwrap();
    assert_eq!(finalized.status, BordroStatus::FINALIZED);
    assert_eq!(finalized.gelirToplam, updated_oracle.gelirToplam);
    assert_eq!(finalized.kesintiToplam, updated_oracle.kesintiToplam);
    assert_eq!(finalized.netOdeme, updated_oracle.netOdeme);
    assert_eq!(
        finalized.sonrakiDevredenPek,
        updated_oracle.sonrakiDevredenPek
    );
    assert_eq!(
        PayrollRepository::get_all(&conn)
            .unwrap()
            .into_iter()
            .find(|row| row.accrualType == AccrualType::NORMAL)
            .unwrap()
            .calculationMetadata
            .as_ref()
            .unwrap()
            .snapshot
            .as_ref()
            .unwrap()
            .snapshotSchemaVersion,
        2
    );
}

#[test]
fn native_policy_calculation_and_schema2_finalization_roundtrip_uses_scoped_inputs() {
    assert_policy_roundtrip("2026-09", "2026-09-15", "2026-10-14", 2026, 10);
}

#[test]
fn synthetic_2027_fifteenth_to_fourteenth_periods_exercise_policy_path() {
    assert_policy_roundtrip("2027-03", "2027-03-15", "2027-04-14", 2027, 4);
    assert_policy_roundtrip("2027-09", "2027-09-15", "2027-10-14", 2027, 10);
}

fn acceptance_period(year: i32, month: u32) -> BordroDonemi {
    let start = NaiveDate::from_ymd_opt(year, month, 15).unwrap();
    let (end_year, end_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    period(
        &format!("{year}-{month:02}"),
        &start.format("%Y-%m-%d").to_string(),
        &NaiveDate::from_ymd_opt(end_year, end_month, 14)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string(),
        year,
        month as i32,
    )
}

fn acceptance_periods() -> Vec<BordroDonemi> {
    (2026..=2027)
        .flat_map(|year| {
            let months: Vec<u32> = if year == 2026 {
                (9..=12).collect()
            } else {
                (1..=8).collect()
            };
            months
                .into_iter()
                .map(move |month| acceptance_period(year, month))
        })
        .collect()
}

fn acceptance_settings(period: &BordroDonemi) -> DonemselKurumDegerleri {
    // Native settings validation requires positive legacy compensation values.
    // Keep its normal prefill values; the checked policy calculation must use
    // the separately persisted immutable timeline recorded in each snapshot.
    let mut settings = DonemselKurumDegerleri {
        donemId: period.id.clone(),
        ..DonemselKurumDegerleri::default()
    };
    settings.transportRateChanges = None;
    settings.gunlukAsgariUcret = Some(if period.taxYear == 2026 {
        dec!(1000)
    } else {
        dec!(1200)
    });
    settings.pekTavanKatsayisi = Some(dec!(7.5));
    settings.gunlukYemekIstisnasiSGK = Some(dec!(100));
    settings.gunlukYemekIstisnasiGV = Some(dec!(100));
    settings.statutoryParameterSnapshot = None;
    settings.statutoryParameterSegments = Some(Vec::new());
    settings
}

fn acceptance_policy_scope(period: &BordroDonemi) -> CompensationPolicyReadScope {
    CompensationPolicyReadScope {
        institutionScope: "CURRENT_INSTITUTION".into(),
        periodId: period.id.clone(),
        periodStartInclusive: period.baslangicTarihi.clone(),
        periodEndInclusive: period.bitisTarihi.clone(),
        projectionDate: period.baslangicTarihi.clone(),
    }
}

fn save_policy_through_api(
    conn: &rusqlite::Connection,
    scope: CompensationPolicyReadScope,
    operation: CompensationPolicyWriteOperation,
    baseline_confirmation: bool,
) -> CompensationPolicyVersion {
    let effective_from = match &operation {
        CompensationPolicyWriteOperation::InitialBaseline { effectiveFrom, .. }
        | CompensationPolicyWriteOperation::NewEffectiveDate { effectiveFrom, .. }
        | CompensationPolicyWriteOperation::SameDateCorrection { effectiveFrom, .. } => {
            effectiveFrom.clone()
        }
    };
    let preview = CompensationPolicyService::preview(
        conn,
        &CompensationPolicyPreviewRequest {
            scope: scope.clone(),
            operation: operation.clone(),
        },
    );
    let CompensationPolicyOperationResult::Success { value: preview, .. } = preview else {
        panic!("acceptance policy preview failed: {preview:?}");
    };
    let initial_baseline_confirmation =
        baseline_confirmation.then_some(InitialBaselineConfirmation {
            confirmed: true,
            effectiveFrom: effective_from,
        });
    let saved = CompensationPolicyService::save(
        conn,
        &CompensationPolicySaveRequest {
            scope,
            operation,
            previewToken: preview.previewToken,
            initialBaselineConfirmation: initial_baseline_confirmation,
        },
    );
    let CompensationPolicyOperationResult::Success { value: receipt, .. } = saved else {
        panic!("acceptance policy save failed: {saved:?}");
    };
    receipt.version
}

fn acceptance_backup_payload(conn: &rusqlite::Connection, active_period_id: &str) -> Value {
    let dataset = PayrollService::build_dataset_snapshot(conn).unwrap();
    let mut payload = serde_json::to_value(dataset).unwrap();
    let object = payload.as_object_mut().unwrap();
    for (dataset_key, backup_key) in [
        ("personnel", "personeller"),
        ("periods", "donemler"),
        ("institutionSettings", "kurumDegerleriMap"),
        ("attendances", "puantajlar"),
        ("payrolls", "bordrolar"),
    ] {
        let value = object.remove(dataset_key).unwrap();
        object.insert(backup_key.into(), value);
    }
    object.insert("backupVersion".into(), json!(6));
    object.insert("exportedAt".into(), json!("2026-10-06T00:00:00Z"));
    object.insert("aktifDonemId".into(), json!(active_period_id));
    object.insert("zamAylari".into(), json!([]));
    object.insert(
        "compensationPolicyVersions".into(),
        serde_json::to_value(CompensationPolicyRepository::get_all_versions(conn).unwrap())
            .unwrap(),
    );
    payload
}

type RawPolicyTimelineRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
);

fn raw_policy_timeline(conn: &rusqlite::Connection) -> Vec<RawPolicyTimelineRow> {
    let mut statement = conn
        .prepare(
            "SELECT id, effective_from, state_json, created_at, source, reason,
                    supersedes_version_id, boundary_scope, road_rounding_mode
             FROM compensation_policy_versions ORDER BY effective_from, id",
        )
        .unwrap();
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn add_acceptance_period_data(
    conn: &rusqlite::Connection,
    personnel_id: &str,
    period: &BordroDonemi,
) {
    PeriodRepository::save(conn, period).unwrap();
    SettingsRepository::save_institution_settings(conn, &acceptance_settings(period)).unwrap();
    AttendanceRepository::save(conn, &attendance(personnel_id, period)).unwrap();
}

fn assert_saved_segment_versions(payroll: &BordroKaydi, expected: &[&str]) {
    let snapshot = payroll
        .calculationMetadata
        .as_ref()
        .unwrap()
        .snapshot
        .as_ref()
        .unwrap();
    assert_eq!(snapshot.snapshotSchemaVersion, 2);
    let actual = snapshot
        .compensation
        .segments
        .iter()
        .map(|segment| segment.versionId.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn native_2026_2027_acceptance_roundtrips_v6_v5_and_policy_timeline() {
    let mut conn = create_in_memory_connection().unwrap();
    let personnel_id = "native-acceptance-worker";
    PersonnelRepository::save(&conn, &person(personnel_id)).unwrap();
    let periods = acceptance_periods();
    for period in &periods {
        add_acceptance_period_data(&conn, personnel_id, period);
    }
    SettingsRepository::set_app_setting(&conn, "zam_aylari", "[]").unwrap();
    SettingsRepository::set_app_setting(&conn, "active_period_id", "2026-09").unwrap();

    let mut annual_2026 = AnnualPayrollParameters::default_for_2026();
    annual_2026.year = 2026;
    AnnualPayrollParametersRepository::save(&conn, &annual_2026).unwrap();
    let mut annual_2027 = annual_2026.clone();
    annual_2027.year = 2027;
    // Synthetic future test input; this is not represented as an official tariff.
    annual_2027.sigortaGvYillikBrutAsgariUcretTavani = Some(dec!(180000));
    AnnualPayrollParametersRepository::save(&conn, &annual_2027).unwrap();
    for (year, period_id, normal, minimum) in [
        (2026, "2026-09", dec!(1234.56), dec!(456.78)),
        (2027, "2027-01", dec!(2345.67), dec!(567.89)),
    ] {
        TaxOpeningRepository::save(
            &conn,
            &PersonelTaxOpening {
                id: format!("{personnel_id}_{year}"),
                personnelId: personnel_id.into(),
                year,
                gvCumulativeOpening: Some(normal),
                effectiveFromPeriodId: Some(period_id.into()),
                asgariGvCumulativeOpening: Some(minimum),
                asgariGvEffectiveFromPeriodId: Some(period_id.into()),
                createdAt: None,
                updatedAt: None,
            },
        )
        .unwrap();
    }

    // Capture a real pre-policy dataset as a V5 restore fixture. It contains
    // no invented compensation history and no schema-2 payroll records.
    let mut v5_payload = acceptance_backup_payload(&conn, "2026-09");
    v5_payload["backupVersion"] = json!(5);
    v5_payload
        .as_object_mut()
        .unwrap()
        .remove("compensationPolicyVersions");

    let baseline_state = policy_state();
    let baseline = save_policy_through_api(
        &conn,
        acceptance_policy_scope(&periods[0]),
        CompensationPolicyWriteOperation::InitialBaseline {
            effectiveFrom: "2026-09-15".into(),
            state: baseline_state.clone(),
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            reason: "Synthetic history-free acceptance baseline".into(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
        },
        true,
    );
    let mut march_state = baseline_state;
    march_state.gunlukTabanUcret += dec!(15);
    march_state.gunlukYemek += dec!(2);
    let march = save_policy_through_api(
        &conn,
        acceptance_policy_scope(&periods[5]), // 2027-02: 01.03 is inside this period.
        CompensationPolicyWriteOperation::NewEffectiveDate {
            effectiveFrom: "2027-03-01".into(),
            state: march_state.clone(),
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            reason: "Synthetic 2027-03-01 state change".into(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            changeIntent: "FULL_STATE_CHANGE".into(),
        },
        false,
    );
    let mut september_state = march_state;
    september_state.gunlukVasitaYol += dec!(3);
    let september = save_policy_through_api(
        &conn,
        acceptance_policy_scope(&periods[11]), // 2027-08: 01.09 is inside this period.
        CompensationPolicyWriteOperation::NewEffectiveDate {
            effectiveFrom: "2027-09-01".into(),
            state: september_state,
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            reason: "Synthetic 2027-09-01 road state change".into(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            changeIntent: "FULL_STATE_CHANGE".into(),
        },
        false,
    );
    assert_eq!(
        [
            baseline.effectiveFrom.as_str(),
            march.effectiveFrom.as_str(),
            september.effectiveFrom.as_str(),
        ],
        ["2026-09-15", "2027-03-01", "2027-09-01"]
    );
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&conn)
            .unwrap()
            .len(),
        3
    );
    let mut saved_payrolls = Vec::new();
    for period in &periods {
        let accrual = PayrollAccrualInput {
            accrualId: format!("{personnel_id}-{}", period.id),
            accrualType: AccrualType::NORMAL,
            paymentDate: format!("{}-25", period.id),
            sequence: 0,
            grossAmount: None,
            description: Some("Native 2026–2027 history-free acceptance".into()),
        };
        let calculated = PayrollService::calculate_payroll_for_accrual_checked(
            &conn,
            personnel_id,
            &period.id,
            Some(&accrual),
            None,
        )
        .unwrap_or_else(|error| panic!("{} calculate/persist failed: {error}", period.id));
        assert_eq!(calculated.status, BordroStatus::CALCULATED);
        let finalized = PayrollService::finalize_payroll_for_accrual(
            &conn,
            personnel_id,
            &period.id,
            Some(&accrual.accrualId),
        )
        .unwrap_or_else(|error| panic!("{} finalize failed: {error}", period.id));
        assert_eq!(finalized.status, BordroStatus::FINALIZED);
        saved_payrolls.push(finalized);
    }
    assert_eq!(saved_payrolls.len(), 12);
    assert_eq!(saved_payrolls[0].donemId, "2026-09");
    assert_eq!(saved_payrolls[11].donemId, "2027-08");
    assert_eq!(periods.len(), 12);
    assert_saved_segment_versions(&saved_payrolls[5], &[&baseline.id, &march.id]);
    assert_saved_segment_versions(&saved_payrolls[11], &[&march.id, &september.id]);
    let march_boundary_segments = &saved_payrolls[5]
        .calculationMetadata
        .as_ref()
        .unwrap()
        .snapshot
        .as_ref()
        .unwrap()
        .compensation
        .segments;
    assert_eq!(march_boundary_segments[0].startDate, "2027-02-15");
    assert_eq!(march_boundary_segments[0].endDate, "2027-02-28");
    assert_eq!(march_boundary_segments[0].versionId, baseline.id);
    assert_eq!(march_boundary_segments[0].state, policy_state());
    assert_eq!(march_boundary_segments[1].startDate, "2027-03-01");
    assert_eq!(march_boundary_segments[1].endDate, "2027-03-14");
    assert_eq!(march_boundary_segments[1].versionId, march.id);
    assert_eq!(
        march_boundary_segments[1].state.gunlukTabanUcret,
        dec!(2015.125)
    );
    let september_boundary_segments = &saved_payrolls[11]
        .calculationMetadata
        .as_ref()
        .unwrap()
        .snapshot
        .as_ref()
        .unwrap()
        .compensation
        .segments;
    assert_eq!(september_boundary_segments[0].startDate, "2027-08-15");
    assert_eq!(september_boundary_segments[0].endDate, "2027-08-31");
    assert_eq!(september_boundary_segments[0].versionId, march.id);
    assert_eq!(september_boundary_segments[1].startDate, "2027-09-01");
    assert_eq!(september_boundary_segments[1].endDate, "2027-09-14");
    assert_eq!(september_boundary_segments[1].versionId, september.id);
    assert_eq!(
        september_boundary_segments[1].state.gunlukVasitaYol,
        dec!(78.125)
    );
    let whole_dataset = PayrollService::build_dataset_snapshot(&conn).unwrap();
    assert_eq!(whole_dataset.periods.len(), 12);
    assert!(whole_dataset
        .periods
        .iter()
        .all(|period| period.id.as_str() >= "2026-09"));
    assert!(whole_dataset.payrolls.iter().all(|payroll| {
        payroll.accrualType == AccrualType::NORMAL
            && payroll.status == BordroStatus::FINALIZED
            && payroll.calculationMetadata.is_some()
    }));
    let first_inputs = serde_json::to_value(
        saved_payrolls[0]
            .calculationMetadata
            .as_ref()
            .unwrap()
            .snapshot
            .as_ref()
            .unwrap()
            .usedInputs
            .clone(),
    )
    .unwrap();
    assert_eq!(
        first_inputs["taxHistory"]["normalGvOpening"]["amount"],
        json!("1234.56")
    );
    let january_2027_inputs = serde_json::to_value(
        saved_payrolls[4]
            .calculationMetadata
            .as_ref()
            .unwrap()
            .snapshot
            .as_ref()
            .unwrap()
            .usedInputs
            .clone(),
    )
    .unwrap();
    assert_eq!(
        january_2027_inputs["annualParameters"]["sigortaGvYillikBrutAsgariUcretTavani"],
        json!("180000")
    );
    for payroll in &saved_payrolls {
        let parent_period = whole_dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
            .unwrap();
        validate_payroll_calculation_metadata_with_context(
            payroll,
            parent_period,
            &whole_dataset.payrolls,
            &whole_dataset.periods,
        )
        .unwrap_or_else(|error| {
            panic!(
                "{} canonical final replay failed: {error}",
                parent_period.id
            )
        });
    }

    // Delete a later independent, empty period through the real native service
    // and compare the exact SQLite policy rows before and after.
    let deletable = acceptance_period(2028, 1);
    PeriodRepository::save(&conn, &deletable).unwrap();
    let timeline_before_delete = raw_policy_timeline(&conn);
    PeriodService::delete_period(&mut conn, &deletable.id).unwrap();
    assert_eq!(raw_policy_timeline(&conn), timeline_before_delete);
    assert!(PeriodRepository::get_by_id(&conn, &deletable.id)
        .unwrap()
        .is_none());

    // Export the repository-backed V6 shape and restore into a fresh database,
    // then load authoritative inputs back through native repositories.
    let v6_payload = acceptance_backup_payload(&conn, "2027-08");
    assert_eq!(v6_payload["backupVersion"], json!(6));
    assert_eq!(v6_payload["bordrolar"].as_array().unwrap().len(), 12);
    let mut restored = create_in_memory_connection().unwrap();
    MigrationService::replace_backup_data(&mut restored, &v6_payload.to_string()).unwrap();
    let restored_dataset = PayrollService::build_dataset_snapshot(&restored).unwrap();
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&restored).unwrap(),
        CompensationPolicyRepository::get_all_versions(&conn).unwrap()
    );
    assert_eq!(restored_dataset.payrolls.len(), 12);
    assert!(restored_dataset
        .payrolls
        .iter()
        .all(|payroll| payroll.status == BordroStatus::FINALIZED));
    for payroll in &restored_dataset.payrolls {
        let parent_period = restored_dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
            .unwrap();
        validate_payroll_calculation_metadata_with_context(
            payroll,
            parent_period,
            &restored_dataset.payrolls,
            &restored_dataset.periods,
        )
        .unwrap_or_else(|error| panic!("restored {} replay failed: {error}", parent_period.id));
    }

    // A fresh period after restore proves native calculation reloads the
    // restored immutable timeline and persisted 2027 data.
    let after_restore = acceptance_period(2027, 9);
    add_acceptance_period_data(&restored, personnel_id, &after_restore);
    let post_restore_normal = PayrollService::calculate_payroll_for_accrual_checked(
        &restored,
        personnel_id,
        &after_restore.id,
        None,
        None,
    )
    .unwrap();
    assert_eq!(post_restore_normal.status, BordroStatus::CALCULATED);
    assert_saved_segment_versions(&post_restore_normal, &[&september.id]);

    let revision = CompensationRevision {
        id: "native-post-restore-retro-revision".into(),
        reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
        title: "Post-restore synthetic entitlement revision".into(),
        effectiveFrom: "2027-08-15".into(),
        effectiveTo: None,
        decisionDate: Some("2027-09-20".into()),
        signedAt: Some("2027-09-20".into()),
        description: Some("Replay from the restored V6 source ledger".into()),
        status: CompensationRevisionStatus::DRAFT,
        scope: CompensationRevisionScope::SELECTED_PERSONNEL,
        personnelIds: vec![personnel_id.into()],
        personnelGroup: None,
        createdAt: Some("2026-10-06T00:00:00Z".into()),
        updatedAt: None,
    };
    let retro_override = CompensationRevisionOverride {
        id: "native-post-restore-retro-wage".into(),
        revisionId: revision.id.clone(),
        parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
        value: CompensationPolicyRepository::get_all_versions(&restored)
            .unwrap()
            .iter()
            .find(|version| version.effectiveFrom == "2027-09-01")
            .unwrap()
            .state
            .gunlukTabanUcret
            .round_dp(2)
            + dec!(1),
        personnelId: Some(personnel_id.into()),
    };
    retro_repo::save_revision_with_overrides(
        &restored,
        &revision,
        std::slice::from_ref(&retro_override),
    )
    .unwrap();
    let retro = RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: "native-post-restore-retro-batch".into(),
        revision,
        overrides: vec![retro_override],
        personnelId: personnel_id.into(),
        paymentDate: "2027-09-25".into(),
        calculatedAt: "2026-10-06T00:00:00Z".into(),
        description: Some("Post-restore retro calculation".into()),
        dataset: PayrollService::build_dataset_snapshot(&restored).unwrap(),
    })
    .expect("fresh native dataset should calculate retro entitlement");
    assert_eq!(retro.batch.status, CompensationRevisionStatus::CALCULATED);
    assert!(retro.batch.payableSettlementAmount > rust_decimal::Decimal::ZERO);
    assert!(!retro.allocations.is_empty());

    // V5 restore carries no V6 compensation timeline. Policy calculation must
    // fail without mutating the restored V5 database, then pass after the same
    // real API baseline flow is completed.
    let mut v5_restored = create_in_memory_connection().unwrap();
    MigrationService::replace_backup_data(&mut v5_restored, &v5_payload.to_string()).unwrap();
    assert!(CompensationPolicyRepository::get_all_versions(&v5_restored)
        .unwrap()
        .is_empty());
    let first = &periods[0];
    let before_baseline_error = PayrollService::calculate_payroll_for_accrual_checked(
        &v5_restored,
        personnel_id,
        &first.id,
        None,
        None,
    )
    .expect_err("V5-restored policy calculation must fail without an explicit baseline");
    assert!(
        before_baseline_error
            .to_string()
            .to_lowercase()
            .contains("ücret politikası tanımlı değil"),
        "missing-baseline failure should be explicit: {before_baseline_error}"
    );
    assert!(PayrollRepository::get_all(&v5_restored).unwrap().is_empty());
    let restored_baseline = save_policy_through_api(
        &v5_restored,
        acceptance_policy_scope(first),
        CompensationPolicyWriteOperation::InitialBaseline {
            effectiveFrom: "2026-09-15".into(),
            state: policy_state(),
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            reason: "Baseline after V5 restore".into(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
        },
        true,
    );
    assert_eq!(restored_baseline.effectiveFrom, "2026-09-15");
    let after_baseline = PayrollService::calculate_payroll_for_accrual_checked(
        &v5_restored,
        personnel_id,
        &first.id,
        None,
        None,
    )
    .expect("policy calculation should succeed after the V5 baseline API save");
    assert_eq!(after_baseline.status, BordroStatus::CALCULATED);
    assert_saved_segment_versions(&after_baseline, &[&restored_baseline.id]);
}
