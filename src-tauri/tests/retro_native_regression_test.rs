use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::retro_repo::{
    get_batches, save_batch, save_revision_with_overrides,
};
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use chrono::{Datelike, Duration, NaiveDate};
use payroll_core::PayrollMutation;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn personnel() -> Personel {
    Personel {
        id: "retro-atomic-person".into(),
        tcNo: "10000000001".into(),
        ad: "Retro".into(),
        soyad: "Atomic".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: String::new(),
        iban: String::new(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-03".into(),
        yil: 2026,
        ay: 3,
        baslangicTarihi: "2026-03-15".into(),
        bitisTarihi: "2026-04-14".into(),
        donemAdi: "Mart 2026".into(),
        taxYear: 2026,
        taxMonth: 4,
    }
}

fn revision() -> CompensationRevision {
    CompensationRevision {
        id: "revision-atomic".into(),
        reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
        title: "Atomic retro test".into(),
        effectiveFrom: "2026-03-15".into(),
        effectiveTo: None,
        decisionDate: None,
        signedAt: None,
        description: None,
        status: CompensationRevisionStatus::DRAFT,
        scope: CompensationRevisionScope::SELECTED_PERSONNEL,
        personnelIds: vec!["retro-atomic-person".into()],
        personnelGroup: None,
        createdAt: None,
        updatedAt: None,
    }
}

fn batch() -> RetroAdjustmentBatch {
    RetroAdjustmentBatch {
        id: "batch-atomic".into(),
        revisionId: "revision-atomic".into(),
        personnelId: "retro-atomic-person".into(),
        paymentDate: "2026-06-20".into(),
        status: CompensationRevisionStatus::CALCULATED,
        settlementStatus: RetroSettlementStatus::UNSETTLED,
        totalGrossDelta: dec!(10),
        payableSettlementAmount: dec!(10),
        offsetSettlementAmount: dec!(0),
        recoveredAmount: dec!(0),
        recoverableAmount: dec!(0),
        outstandingReceivable: dec!(0),
        description: Some("Atomic test".into()),
        createdAt: Some("2026-06-20T00:00:00Z".into()),
        calculatedAt: Some("2026-06-20T00:00:00Z".into()),
        finalizedAt: None,
    }
}

fn allocation() -> RetroAllocation {
    RetroAllocation {
        id: "allocation-atomic".into(),
        batchId: "batch-atomic".into(),
        personnelId: "retro-atomic-person".into(),
        sourcePeriodId: "2026-03".into(),
        earningCode: RetroEarningCode::BASE_WAGE,
        originalRecognizedAmount: dec!(0),
        previousAuthoritativeRetroAmount: dec!(0),
        targetAmount: dec!(10),
        deltaAmount: dec!(10),
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
        payableSettlementAmount: dec!(10),
        offsetSettlementAmount: dec!(0),
        recoverableAmount: dec!(0),
        metadata: None,
    }
}

fn retro_period(id: &str, tax_month: i32) -> BordroDonemi {
    let start = NaiveDate::parse_from_str(&format!("{id}-15"), "%Y-%m-%d").unwrap();
    let (end_year, end_month) = if start.month() == 12 {
        (start.year() + 1, 1)
    } else {
        (start.year(), start.month() + 1)
    };
    let end = NaiveDate::from_ymd_opt(end_year, end_month, 14).unwrap();
    BordroDonemi {
        id: id.into(),
        yil: start.year(),
        ay: start.month() as i32,
        baslangicTarihi: start.format("%Y-%m-%d").to_string(),
        bitisTarihi: end.format("%Y-%m-%d").to_string(),
        donemAdi: id.into(),
        taxYear: 2026,
        taxMonth: tax_month,
    }
}

fn complete_attendance(period: &BordroDonemi, personnel_id: &str) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut date = start;
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("{}_{}", personnel_id, period.id),
        personelId: personnel_id.into(),
        donemId: period.id.clone(),
        gunler,
    }
}

fn setup_full_retro_database() -> (
    rusqlite::Connection,
    BordroDonemi,
    BordroDonemi,
    CompensationRevision,
) {
    let conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let person = personnel();
    let source_period = retro_period("2026-03", 4);
    let payment_period = retro_period("2026-06", 6);
    // The production preflight requires a contiguous minimum-wage GV
    // reference chain. Keep this fixture representative of a real June
    // settlement instead of weakening the calculation just for the test.
    let reference_periods = [
        retro_period("2025-12", 1),
        retro_period("2026-01", 2),
        retro_period("2026-02", 3),
        retro_period("2026-04", 5),
    ];
    PersonnelRepository::save(&conn, &person).expect("personel kaydedilmeli");
    let mut all_periods = reference_periods.to_vec();
    all_periods.push(source_period.clone());
    all_periods.push(payment_period.clone());
    all_periods.sort_by(|left, right| left.baslangicTarihi.cmp(&right.baslangicTarihi));
    for current_period in &all_periods {
        PeriodRepository::save(&conn, current_period).expect("dönem kaydedilmeli");
        SettingsRepository::save_institution_settings(
            &conn,
            &DonemselKurumDegerleri {
                donemId: current_period.id.clone(),
                ..DonemselKurumDegerleri::default()
            },
        )
        .expect("dönem ayarları kaydedilmeli");
        AttendanceRepository::save(&conn, &complete_attendance(current_period, &person.id))
            .expect("puantaj kaydedilmeli");
    }
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())
        .expect("yıllık parametre kaydedilmeli");
    let revision = CompensationRevision {
        id: "revision-native-canonical".into(),
        reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
        title: "Native canonical retro".into(),
        effectiveFrom: "2026-03-15".into(),
        effectiveTo: None,
        decisionDate: Some("2026-06-10".into()),
        signedAt: Some("2026-06-10".into()),
        description: Some("Native retro regression".into()),
        status: CompensationRevisionStatus::DRAFT,
        scope: CompensationRevisionScope::SELECTED_PERSONNEL,
        personnelIds: vec![person.id.clone()],
        personnelGroup: None,
        createdAt: Some("2026-06-10T00:00:00Z".into()),
        updatedAt: None,
    };
    save_revision_with_overrides(
        &conn,
        &revision,
        &[CompensationRevisionOverride {
            id: "override-native-canonical".into(),
            revisionId: revision.id.clone(),
            parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
            value: dec!(10000),
            personnelId: None,
        }],
    )
    .expect("revision kaydedilmeli");
    PayrollService::calculate_payroll_for_personnel(&conn, &person.id, &source_period.id)
        .expect("source normal bordro hesaplanmalı");
    // BUG-RETRO-001: every closed source period inside the (open-ended)
    // revision window needs an authoritative NORMAL baseline. 2026-04 used
    // to have attendance only and was silently recognized as 0 TL.
    PayrollService::calculate_payroll_for_personnel(&conn, &person.id, "2026-04")
        .expect("2026-04 normal bordro hesaplanmalı");
    (conn, source_period, payment_period, revision)
}

#[test]
fn native_retro_payment_rolls_back_batch_when_core_calculation_fails() {
    let conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let person = personnel();
    PersonnelRepository::save(&conn, &person).expect("personel kaydedilmeli");
    PeriodRepository::save(&conn, &period()).expect("dönem kaydedilmeli");
    save_revision_with_overrides(&conn, &revision(), &[]).expect("revision kaydedilmeli");

    let result =
        PayrollService::create_retro_payment(&conn, &batch(), &[allocation()], "2026-03", 0);

    assert!(
        result.is_err(),
        "eksik tarihsel kurum ayarı hesaplamayı durdurmalı"
    );
    assert!(
        get_batches(&conn)
            .expect("retro batch listesi okunmalı")
            .is_empty(),
        "başarısız hesaplama batch'i transaction dışına sızdırmamalı"
    );
}

#[test]
fn native_retro_repo_allows_multiple_active_batches_for_same_revision_and_personnel() {
    let conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let person = personnel();
    PersonnelRepository::save(&conn, &person).expect("personel kaydedilmeli");
    PeriodRepository::save(&conn, &period()).expect("dönem kaydedilmeli");
    save_revision_with_overrides(&conn, &revision(), &[]).expect("revision kaydedilmeli");
    save_batch(&conn, &batch(), &[allocation()]).expect("ilk retro batch kaydedilmeli");

    let mut duplicate_batch = batch();
    duplicate_batch.id = "batch-duplicate".into();
    let mut duplicate_allocation = allocation();
    duplicate_allocation.id = "allocation-duplicate".into();
    duplicate_allocation.batchId = duplicate_batch.id.clone();

    save_batch(&conn, &duplicate_batch, &[duplicate_allocation])
        .expect("aynı revision için ikinci authoritative düzeltme kaydedilebilmeli");
    assert_eq!(get_batches(&conn).expect("batch listesi okunmalı").len(), 2);
}

#[test]
fn native_retro_batch_save_cannot_rebind_an_existing_batch_id() {
    let conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let person = personnel();
    PersonnelRepository::save(&conn, &person).expect("personel kaydedilmeli");
    PeriodRepository::save(&conn, &period()).expect("dönem kaydedilmeli");
    save_revision_with_overrides(&conn, &revision(), &[]).expect("ilk revision kaydedilmeli");
    save_batch(&conn, &batch(), &[allocation()]).expect("ilk retro batch kaydedilmeli");

    let mut other_revision = revision();
    other_revision.id = "revision-other".into();
    save_revision_with_overrides(&conn, &other_revision, &[])
        .expect("ikinci revision kaydedilmeli");

    let mut forged = batch();
    forged.revisionId = other_revision.id;
    let error = save_batch(&conn, &forged, &[allocation()])
        .expect_err("batch primary id'si başka revision'a bağlanamamalı");
    assert!(error.to_string().contains("primary id"));

    let saved = get_batches(&conn).expect("batch listesi okunmalı");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].revisionId, revision().id);
}

#[test]
fn native_source_mutation_marks_retro_batch_stale() {
    let conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let person = personnel();
    let source_period = period();
    PersonnelRepository::save(&conn, &person).expect("personel kaydedilmeli");
    PeriodRepository::save(&conn, &source_period).expect("dönem kaydedilmeli");
    save_revision_with_overrides(&conn, &revision(), &[]).expect("revision kaydedilmeli");
    save_batch(&conn, &batch(), &[allocation()]).expect("retro batch kaydedilmeli");

    let mutation = PayrollMutation::PersonPeriod {
        personnelId: person.id.clone(),
        periodId: source_period.id.clone(),
    };
    let impact = PayrollInvalidationRepository::assert_mutation_allowed(&conn, &mutation)
        .expect("kaynak mutation engellenmemeli");
    assert_eq!(impact.affectedRetroBatches, vec!["batch-atomic"]);
    PayrollInvalidationRepository::apply_impact(&conn, &impact)
        .expect("retro batch stale yapılmalı");

    assert_eq!(
        get_batches(&conn).expect("batch listesi okunmalı")[0].status,
        CompensationRevisionStatus::STALE
    );
}

#[test]
fn native_positive_retro_batch_cannot_be_saved_without_payment_event() {
    let (conn, _source_period, _payment_period, revision) = setup_full_retro_database();
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let result =
        payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
            batchId: "batch-without-payment".into(),
            revision,
            overrides: vec![CompensationRevisionOverride {
                id: "override-native-canonical".into(),
                revisionId: "revision-native-canonical".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(10000),
                personnelId: None,
            }],
            personnelId: "retro-atomic-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Positive batch without payment".into()),
            dataset,
        })
        .expect("canonical preview hesaplanmalı");

    let error =
        PayrollService::save_retro_adjustment_batch(&conn, &result.batch, &result.allocations)
            .expect_err("pozitif batch payment event olmadan saklanmamalı");
    assert!(
        error.to_string().to_lowercase().contains("payment event"),
        "unexpected save error: {error}"
    );
    assert!(get_batches(&conn)
        .expect("batch listesi okunmalı")
        .is_empty());
}

#[test]
fn native_deleting_unfinalized_retro_payment_stales_its_ledger() {
    let (conn, _source_period, payment_period, revision) = setup_full_retro_database();
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let result =
        payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
            batchId: "batch-delete-retro".into(),
            revision,
            overrides: vec![CompensationRevisionOverride {
                id: "override-native-canonical".into(),
                revisionId: "revision-native-canonical".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(10000),
                personnelId: None,
            }],
            personnelId: "retro-atomic-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Delete retro".into()),
            dataset,
        })
        .expect("canonical preview hesaplanmalı");
    PayrollService::create_retro_payment(
        &conn,
        &result.batch,
        &result.allocations,
        &payment_period.id,
        0,
    )
    .expect("retro payment event oluşturulmalı");

    PayrollRepository::delete_accrual(
        &conn,
        "retro-atomic-person",
        &payment_period.id,
        "batch-delete-retro",
    )
    .expect("retro payment event silinebilmeli");
    assert_eq!(
        get_batches(&conn).expect("batch listesi okunmalı")[0].status,
        CompensationRevisionStatus::STALE
    );
}

#[test]
fn native_retro_payment_replay_refreshes_stale_event_without_duplicate() {
    let (conn, _source_period, payment_period, revision) = setup_full_retro_database();
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let result =
        payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
            batchId: "batch-replay-stale".into(),
            revision,
            overrides: vec![CompensationRevisionOverride {
                id: "override-native-canonical".into(),
                revisionId: "revision-native-canonical".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(10000),
                personnelId: None,
            }],
            personnelId: "retro-atomic-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Replay stale retro event".into()),
            dataset,
        })
        .expect("canonical preview hesaplanmalı");
    PayrollService::create_retro_payment(
        &conn,
        &result.batch,
        &result.allocations,
        &payment_period.id,
        0,
    )
    .expect("ilk retro payment event oluşturulmalı");

    let mut stale_payment = PayrollRepository::get_all(&conn)
        .expect("bordrolar okunmalı")
        .into_iter()
        .find(|payroll| payroll.accrualId == result.batch.id)
        .expect("payment event bulunmalı");
    stale_payment.status = BordroStatus::STALE;
    PayrollRepository::save_legacy_in_transaction(&conn, &stale_payment)
        .expect("payment event stale duruma alınmalı");

    let replayed = PayrollService::create_retro_payment(
        &conn,
        &result.batch,
        &result.allocations,
        &payment_period.id,
        0,
    )
    .expect("aynı retro payment event canonical olarak replay edilebilmeli");

    assert_eq!(replayed.accrualId, result.batch.id);
    assert_eq!(replayed.status, BordroStatus::CALCULATED);
    assert_eq!(
        PayrollRepository::get_all(&conn)
            .expect("bordrolar okunmalı")
            .iter()
            .filter(|payroll| payroll.accrualId == result.batch.id)
            .count(),
        1,
        "replay ikinci bir payment event oluşturmamalı"
    );
    assert_eq!(
        get_batches(&conn).expect("retro batch listesi okunmalı")[0].status,
        CompensationRevisionStatus::CALCULATED
    );
}

#[test]
fn native_stale_batch_and_payment_recover_in_place_and_remain_idempotent() {
    let (conn, _source_period, payment_period, revision) = setup_full_retro_database();
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let initial =
        payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
            batchId: "batch-stale-pair-recovery".into(),
            revision: revision.clone(),
            overrides: dataset
                .compensationRevisionOverrides
                .iter()
                .filter(|item| item.revisionId == revision.id)
                .cloned()
                .collect(),
            personnelId: "retro-atomic-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Stale pair replay".into()),
            dataset,
        })
        .expect("initial preview");
    PayrollService::create_retro_payment(
        &conn,
        &initial.batch,
        &initial.allocations,
        &payment_period.id,
        0,
    )
    .expect("initial payment");

    conn.execute(
        "UPDATE retro_adjustment_batches SET status = 'STALE' WHERE id = ?1",
        [&initial.batch.id],
    )
    .expect("batch stale");
    PayrollService::set_payroll_status_for_accrual(
        &conn,
        "retro-atomic-person",
        &payment_period.id,
        Some(&initial.batch.id),
        BordroStatus::STALE,
    )
    .expect("payment stale");

    let fresh_dataset = PayrollService::build_dataset_snapshot(&conn).expect("fresh dataset");
    let recovered =
        payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
            batchId: initial.batch.id.clone(),
            revision,
            overrides: fresh_dataset
                .compensationRevisionOverrides
                .iter()
                .filter(|item| item.revisionId == "revision-native-canonical")
                .cloned()
                .collect(),
            personnelId: "retro-atomic-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Stale pair replay".into()),
            dataset: fresh_dataset,
        })
        .expect("stale pair recalculation");
    assert!(recovered.batch.payableSettlementAmount > Decimal::ZERO);
    for _ in 0..2 {
        let replayed = PayrollService::create_retro_payment(
            &conn,
            &recovered.batch,
            &recovered.allocations,
            &payment_period.id,
            0,
        )
        .expect("stale batch/payment same-ID replay");
        assert_eq!(replayed.status, BordroStatus::CALCULATED);
    }
    let persisted = PayrollService::build_dataset_snapshot(&conn).expect("persisted state");
    assert_eq!(
        persisted
            .retroBatches
            .iter()
            .filter(|item| item.id == initial.batch.id)
            .count(),
        1,
        "same logical batch retains its identity"
    );
    assert_eq!(
        persisted
            .payrolls
            .iter()
            .filter(|item| item.accrualId == initial.batch.id)
            .count(),
        1,
        "recovery never creates a duplicate payment event"
    );
    assert_eq!(
        persisted.retroBatches[0].status,
        CompensationRevisionStatus::CALCULATED
    );

    let next_period = retro_period("2026-07", 7);
    PeriodRepository::save(&conn, &next_period).expect("sonraki dönem kaydedilmeli");
    SettingsRepository::save_institution_settings(
        &conn,
        &DonemselKurumDegerleri {
            donemId: next_period.id.clone(),
            ..DonemselKurumDegerleri::default()
        },
    )
    .expect("sonraki dönem ayarı kaydedilmeli");
    AttendanceRepository::save(
        &conn,
        &complete_attendance(&next_period, "retro-atomic-person"),
    )
    .expect("sonraki dönem puantajı kaydedilmeli");
    let next_payroll = PayrollService::calculate_payroll_for_personnel(
        &conn,
        "retro-atomic-person",
        &next_period.id,
    )
    .expect("recovery sonrası sonraki payment-event/PEK zinciri çözülmeli");
    assert_eq!(next_payroll.status, BordroStatus::CALCULATED);
}

#[test]
fn native_retro_payment_rejects_forged_preview_before_any_write() {
    let (conn, source_period, payment_period, revision) = setup_full_retro_database();
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let result =
        payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
            batchId: "batch-native-forged".into(),
            revision,
            overrides: vec![CompensationRevisionOverride {
                id: "override-native-canonical".into(),
                revisionId: "revision-native-canonical".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(10000),
                personnelId: None,
            }],
            personnelId: "retro-atomic-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Forged preview".into()),
            dataset,
        })
        .expect("canonical preview hesaplanmalı");
    assert!(!result.allocations.is_empty());
    let mut forged_allocations = result.allocations.clone();
    forged_allocations[0].deltaAmount += dec!(1);
    let error = PayrollService::create_retro_payment(
        &conn,
        &result.batch,
        &forged_allocations,
        &payment_period.id,
        999,
    )
    .expect_err("native boundary forged allocation'ı reddetmeli");
    assert!(
        error.to_string().contains("eşleşmiyor"),
        "beklenen stale/forged preview hatası, alınan: {error}"
    );
    assert!(get_batches(&conn)
        .expect("batch listesi okunmalı")
        .is_empty());
    assert_eq!(
        bordro_programi_lib::repositories::payroll_repo::PayrollRepository::get_all(&conn)
            .expect("bordrolar okunmalı")
            .len(),
        2,
        "yalnız source normal payroll kalmalı (2026-03 + 2026-04 NORMAL)"
    );
    let _ = source_period;
}

#[test]
fn native_negative_retro_batch_is_persisted_as_overpayment_without_payment_event() {
    let (conn, _source_period, _payment_period, mut revision) = setup_full_retro_database();
    revision.id = "revision-native-negative".into();
    revision.title = "Native negative retro".into();
    revision.effectiveFrom = "2026-03-15".into();
    save_revision_with_overrides(
        &conn,
        &revision,
        &[CompensationRevisionOverride {
            id: "override-native-negative".into(),
            revisionId: revision.id.clone(),
            parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
            value: dec!(1),
            personnelId: None,
        }],
    )
    .expect("negative revision kaydedilmeli");
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let result =
        payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
            batchId: "batch-native-overpayment".into(),
            revision,
            overrides: vec![CompensationRevisionOverride {
                id: "override-native-negative".into(),
                revisionId: "revision-native-negative".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: dec!(1),
                personnelId: None,
            }],
            personnelId: "retro-atomic-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Overpayment".into()),
            dataset,
        })
        .expect("negative preview hesaplanmalı");
    assert!(result.batch.totalGrossDelta < Decimal::ZERO);
    PayrollService::save_retro_adjustment_batch(&conn, &result.batch, &result.allocations)
        .expect("negative batch ödeme olmadan saklanmalı");
    let saved = get_batches(&conn).expect("batch listesi okunmalı");
    assert_eq!(saved.len(), 1);
    assert_eq!(
        saved[0].settlementStatus,
        RetroSettlementStatus::OVERPAYMENT
    );
    assert!(
        bordro_programi_lib::repositories::payroll_repo::PayrollRepository::get_all(&conn)
            .expect("bordrolar okunmalı")
            .iter()
            .all(|record| record.accrualType != AccrualType::RETRO_ADJUSTMENT)
    );
}

#[test]
fn native_payroll_persistence_rejects_payment_date_tax_month_split_brain() {
    let (conn, _source_period, _payment_period, _revision) = setup_full_retro_database();
    let mut records = PayrollRepository::get_all(&conn).expect("bordrolar okunmalı");
    let original_date = records[0].paymentDate.clone();
    records[0].paymentDate = "2026-06-20".into();
    let error = PayrollRepository::save(&conn, &records[0])
        .expect_err("ödeme tarihi vergi ayı ile eşleşmeyen kayıt yazılmamalı");
    assert!(error.to_string().contains("eşleşmiyor"));
    assert_eq!(
        PayrollRepository::get_all(&conn)
            .expect("bordrolar okunmalı")
            .first()
            .expect("source bordro")
            .paymentDate,
        original_date
    );
}

#[test]
fn native_payroll_save_cannot_reuse_another_record_primary_id() {
    let (conn, _source_period, _payment_period, _revision) = setup_full_retro_database();
    let source = PayrollRepository::get_all(&conn)
        .expect("bordrolar okunmalı")
        .into_iter()
        .next()
        .expect("source bordro bulunmalı");

    let mut forged = source.clone();
    forged.accrualId = "forged-accrual-id".into();
    forged.id = source.id.clone();
    forged.sequence = 1;

    let error = PayrollRepository::save(&conn, &forged)
        .expect_err("başka tahakkukun primary id'si yeniden kullanılamamalı");
    assert!(error.to_string().contains("primary id"));

    let after = PayrollRepository::get_all(&conn).expect("bordrolar okunmalı");
    assert_eq!(after.len(), 2, "2026-03 + 2026-04 NORMAL; forged kayıt yazılmamalı");
    assert!(after.iter().any(|payroll| payroll.accrualId == source.accrualId));
    assert!(after.iter().all(|payroll| payroll.accrualId != "forged-accrual-id"));
}

#[test]
fn native_retro_payment_totals_must_reconcile_with_income_and_deductions() {
    let (conn, _source_period, _payment_period, _revision) = setup_full_retro_database();
    let source = PayrollRepository::get_all(&conn)
        .expect("bordrolar okunmalı")
        .into_iter()
        .next()
        .expect("source bordro bulunmalı");

    let mut forged = source.clone();
    forged.id = "retro-total-check".into();
    forged.accrualId = "retro-total-check".into();
    forged.accrualType = AccrualType::RETRO_ADJUSTMENT;
    forged.sequence = 1;
    forged.netOdeme += dec!(1);

    let error = PayrollRepository::save_in_transaction(&conn, &forged)
        .expect_err("retro payment net toplamı kalemlerden bağımsız değiştirilememeli");
    assert!(error.to_string().contains("finansal toplamları"));
    assert_eq!(
        PayrollRepository::get_all(&conn)
            .expect("bordrolar okunmalı")
            .len(),
        2,
        "reddedilen retro event veritabanına yazılmamalı (2026-03 + 2026-04 NORMAL)"
    );

    forged.netOdeme = (forged.gelirToplam - forged.kesintiToplam).round_dp(2);
    PayrollRepository::save_in_transaction(&conn, &forged)
        .expect("kalemlerle eşleşen retro toplamları yazılmalı");
    assert_eq!(
        PayrollRepository::get_all(&conn)
            .expect("bordrolar okunmalı")
            .len(),
        3
    );
}

fn native_preview(
    conn: &rusqlite::Connection,
    batch_id: &str,
    revision: &CompensationRevision,
    payment_date: &str,
) -> payroll_core::Result<payroll_core::RetroCalculationResult> {
    let dataset = PayrollService::build_dataset_snapshot(conn).expect("dataset okunmalı");
    payroll_core::RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
        batchId: batch_id.into(),
        revision: revision.clone(),
        overrides: dataset
            .compensationRevisionOverrides
            .iter()
            .filter(|item| item.revisionId == revision.id)
            .cloned()
            .collect(),
        personnelId: "retro-atomic-person".into(),
        paymentDate: payment_date.into(),
        calculatedAt: "2026-06-20T00:00:00Z".into(),
        description: Some("BUG-RETRO regression".into()),
        dataset,
    })
}

#[test]
fn native_bug_retro_001_missing_normal_source_period_is_rejected_not_zero() {
    let (conn, _source_period, _payment_period, revision) = setup_full_retro_database();
    conn.execute(
        "DELETE FROM payroll_records WHERE period_id = '2026-04'",
        [],
    )
    .expect("2026-04 NORMAL silinmeli");
    let error = native_preview(&conn, "batch-missing-normal", &revision, "2026-06-20")
        .expect_err("NORMAL bordrosu olmayan dönem 0 TL tanınmış sayılmamalı");
    assert!(error.to_string().contains("2026-04"), "{error}");
    assert!(error.to_string().contains("NORMAL bordro yok"), "{error}");

    // A bounded revision that ends before 2026-04 does not need that period.
    let mut bounded = revision.clone();
    bounded.effectiveTo = Some("2026-04-14".into());
    save_revision_with_overrides(
        &conn,
        &bounded,
        &[CompensationRevisionOverride {
            id: "override-native-canonical".into(),
            revisionId: bounded.id.clone(),
            parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
            value: dec!(10000),
            personnelId: None,
        }],
    )
    .expect("bounded revision kaydedilmeli");
    let result = native_preview(&conn, "batch-bounded", &bounded, "2026-06-20")
        .expect("effectiveTo öncesi kaynak hesaplanmalı");
    assert_eq!(
        result
            .periods
            .iter()
            .map(|period| period.sourcePeriodId.as_str())
            .collect::<Vec<_>>(),
        vec!["2026-03"]
    );
}

/// BUG-RETRO-002 (Tests F/G/H/I + settlement regression): a real source
/// mutation leaves batch STALE/UNSETTLED and the payment event STALE; the
/// UI recovery path (canonical replay with the same batch id followed by
/// create_retro_payment) must restore CALCULATED/UNSETTLED + CALCULATED in
/// the database, unblock the next NORMAL payroll, create no duplicate and
/// keep the amounts; the settlement flow must still finalize it once.
#[test]
fn native_bug_retro_002_stale_unsettled_recovery_restores_db_state_and_unblocks_normal() {
    let (conn, source_period, payment_period, revision) = setup_full_retro_database();
    let initial = native_preview(&conn, "batch-bug-retro-002", &revision, "2026-06-20")
        .expect("initial preview");
    let initial_event = PayrollService::create_retro_payment(
        &conn,
        &initial.batch,
        &initial.allocations,
        &payment_period.id,
        0,
    )
    .expect("initial retro payment");

    // Real source mutation (same as an attendance change on the source).
    let impact = PayrollInvalidationRepository::assert_mutation_allowed(
        &conn,
        &PayrollMutation::PersonPeriod {
            personnelId: "retro-atomic-person".into(),
            periodId: source_period.id.clone(),
        },
    )
    .expect("source mutation allowed");
    PayrollInvalidationRepository::apply_impact(&conn, &impact).expect("impact applied");
    let stale = PayrollService::build_dataset_snapshot(&conn).expect("dataset");
    let stale_batch = stale
        .retroBatches
        .iter()
        .find(|batch| batch.id == initial.batch.id)
        .expect("batch");
    assert_eq!(stale_batch.status, CompensationRevisionStatus::STALE);
    assert_eq!(stale_batch.settlementStatus, RetroSettlementStatus::UNSETTLED);
    let stale_event = stale
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == initial.batch.id)
        .expect("event");
    assert_eq!(stale_event.status, BordroStatus::STALE);

    // While stale, the next NORMAL payroll is blocked by the stale event.
    let next_period = retro_period("2026-07", 7);
    PeriodRepository::save(&conn, &next_period).expect("sonraki dönem");
    SettingsRepository::save_institution_settings(
        &conn,
        &DonemselKurumDegerleri {
            donemId: next_period.id.clone(),
            ..DonemselKurumDegerleri::default()
        },
    )
    .expect("sonraki dönem ayarı");
    AttendanceRepository::save(
        &conn,
        &complete_attendance(&next_period, "retro-atomic-person"),
    )
    .expect("sonraki dönem puantajı");

    // Source NORMALs are recalculated first (UI: Bordro Hesaplama).
    for period_id in [source_period.id.as_str(), "2026-04"] {
        let status = PayrollService::build_dataset_snapshot(&conn)
            .expect("dataset")
            .payrolls
            .iter()
            .find(|payroll| payroll.donemId == period_id && payroll.accrualType == AccrualType::NORMAL)
            .map(|payroll| payroll.status);
        if status != Some(BordroStatus::CALCULATED) {
            PayrollService::calculate_payroll_for_personnel(&conn, "retro-atomic-person", period_id)
                .expect("source normal recalculated");
        }
    }

    // Recovery: identical to GeriyeDonukFarklar.handleReplayPayment.
    let recovered = native_preview(&conn, &initial.batch.id, &revision, "2026-06-20")
        .expect("stale recovery preview");
    assert_eq!(recovered.batch.totalGrossDelta, initial.batch.totalGrossDelta);
    assert_eq!(
        recovered.batch.payableSettlementAmount,
        initial.batch.payableSettlementAmount
    );
    for _ in 0..2 {
        let replayed = PayrollService::create_retro_payment(
            &conn,
            &recovered.batch,
            &recovered.allocations,
            &payment_period.id,
            0,
        )
        .expect("recovery");
        assert_eq!(replayed.status, BordroStatus::CALCULATED);
        assert_eq!(replayed.netOdeme, initial_event.netOdeme, "net korunmalı");
        assert_eq!(replayed.gelirToplam, initial_event.gelirToplam, "brüt korunmalı");
    }

    let persisted = PayrollService::build_dataset_snapshot(&conn).expect("persisted");
    let batches = persisted
        .retroBatches
        .iter()
        .filter(|batch| batch.revisionId == revision.id)
        .collect::<Vec<_>>();
    assert_eq!(batches.len(), 1, "duplicate batch yok");
    assert_eq!(batches[0].status, CompensationRevisionStatus::CALCULATED);
    assert_eq!(batches[0].settlementStatus, RetroSettlementStatus::UNSETTLED);
    let events = persisted
        .payrolls
        .iter()
        .filter(|payroll| payroll.accrualType == AccrualType::RETRO_ADJUSTMENT)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 1, "duplicate payment event yok");
    assert_eq!(events[0].status, BordroStatus::CALCULATED);
    let allocation_count = persisted
        .retroAllocations
        .iter()
        .filter(|allocation| allocation.batchId == initial.batch.id)
        .count();
    assert_eq!(allocation_count, initial.allocations.len(), "duplicate allocation yok");

    // Test G: the next NORMAL payroll is no longer blocked.
    let next = PayrollService::calculate_payroll_for_personnel(
        &conn,
        "retro-atomic-person",
        &next_period.id,
    )
    .expect("recovery sonrası normal bordro hesaplanmalı");
    assert_eq!(next.status, BordroStatus::CALCULATED);

    // Settlement regression: CALCULATED/UNSETTLED -> FINALIZED/PAID once.
    // Production rule: earlier payment events are finalized first.
    // Finalizing a source NORMAL invalidates later CALCULATED events (existing
    // production policy), so the chain is refreshed in order before the
    // retro settlement - exactly the UI workflow.
    for period_id in [source_period.id.as_str(), "2026-04"] {
        let stale_normal = PayrollService::build_dataset_snapshot(&conn)
            .expect("dataset")
            .payrolls
            .iter()
            .any(|payroll| payroll.donemId == period_id && payroll.status == BordroStatus::STALE);
        if stale_normal {
            PayrollService::calculate_payroll_for_personnel(&conn, "retro-atomic-person", period_id)
                .expect("source NORMAL refresh");
        }
        PayrollService::finalize_payroll_for_personnel(&conn, "retro-atomic-person", period_id)
            .expect("source NORMAL finalize");
    }
    let retro_stale = PayrollService::build_dataset_snapshot(&conn)
        .expect("dataset")
        .payrolls
        .iter()
        .any(|payroll| payroll.accrualId == initial.batch.id && payroll.status == BordroStatus::STALE);
    if retro_stale {
        let again = native_preview(&conn, &initial.batch.id, &revision, "2026-06-20")
            .expect("recovery after source finalize");
        assert_eq!(again.batch.totalGrossDelta, initial.batch.totalGrossDelta);
        PayrollService::create_retro_payment(&conn, &again.batch, &again.allocations, &payment_period.id, 0)
            .expect("recovery after source finalize");
    }
    let before = PayrollService::build_dataset_snapshot(&conn).expect("before finalize");
    let event_before = before
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == initial.batch.id)
        .cloned()
        .expect("event");
    let finalized = PayrollService::finalize_payroll_for_accrual(
        &conn,
        "retro-atomic-person",
        &payment_period.id,
        Some(&initial.batch.id),
    )
    .expect("retro settlement finalize");
    assert_eq!(finalized.status, BordroStatus::FINALIZED);
    assert_eq!(finalized.netOdeme, event_before.netOdeme);
    assert_eq!(finalized.gelirToplam, event_before.gelirToplam);
    let after = PayrollService::build_dataset_snapshot(&conn).expect("after finalize");
    let batch = after
        .retroBatches
        .iter()
        .find(|batch| batch.id == initial.batch.id)
        .expect("batch");
    assert_eq!(batch.status, CompensationRevisionStatus::FINALIZED);
    assert_eq!(batch.settlementStatus, RetroSettlementStatus::PAID);
    assert!(batch.finalizedAt.is_some());
    assert_eq!(
        after
            .retroAllocations
            .iter()
            .filter(|allocation| allocation.batchId == initial.batch.id)
            .count(),
        allocation_count,
        "settlement allocation değiştirmemeli"
    );
    assert!(
        PayrollService::create_retro_payment(
            &conn,
            &recovered.batch,
            &recovered.allocations,
            &payment_period.id,
            0,
        )
        .is_err(),
        "FINALIZED retro ikinci kez settle/rewrite edilemez"
    );
}


// ---------------------------------------------------------------------------
// P1 (BUG-RETRO-002 follow-up): zero-difference recovery must retire the
// linked STALE payment event instead of leaving it behind with its old
// gross/net, where it contradicted the 0 TL batch and blocked the next
// NORMAL payroll's tax chain ("Önceki vergi zincirinde DRAFT/STALE bordro var").
// ---------------------------------------------------------------------------

const P1_PERSON: &str = "retro-atomic-person";

fn p1_save_next_period(conn: &rusqlite::Connection) -> BordroDonemi {
    let next_period = retro_period("2026-07", 7);
    PeriodRepository::save(conn, &next_period).expect("sonraki dönem");
    SettingsRepository::save_institution_settings(
        conn,
        &DonemselKurumDegerleri {
            donemId: next_period.id.clone(),
            ..DonemselKurumDegerleri::default()
        },
    )
    .expect("sonraki dönem ayarı");
    AttendanceRepository::save(conn, &complete_attendance(&next_period, P1_PERSON))
        .expect("sonraki dönem puantajı");
    next_period
}

fn p1_refresh_source_normals(conn: &rusqlite::Connection, source_period: &BordroDonemi) {
    for period_id in [source_period.id.as_str(), "2026-04"] {
        let status = PayrollService::build_dataset_snapshot(conn)
            .expect("dataset")
            .payrolls
            .iter()
            .find(|payroll| {
                payroll.donemId == period_id && payroll.accrualType == AccrualType::NORMAL
            })
            .map(|payroll| payroll.status);
        if status != Some(BordroStatus::CALCULATED) {
            PayrollService::calculate_payroll_for_personnel(conn, P1_PERSON, period_id)
                .expect("source normal recalculated");
        }
    }
}

/// Makes the persisted revision resolve to "no difference": the target
/// entitlement equals what was already recognized.
fn p1_neutralize_revision(conn: &rusqlite::Connection, revision: &CompensationRevision) {
    save_revision_with_overrides(conn, revision, &[]).expect("revision farksız kaydedilmeli");
}

/// Live state reproduced: batch STALE/UNSETTLED, linked event STALE with a
/// positive gross/net, allocations present, source NORMALs authoritative.
fn p1_stale_positive_retro(
    batch_id: &str,
) -> (
    rusqlite::Connection,
    BordroDonemi,
    BordroDonemi,
    CompensationRevision,
    BordroKaydi,
    usize,
) {
    let (conn, source_period, payment_period, revision) = setup_full_retro_database();
    let initial = native_preview(&conn, batch_id, &revision, "2026-06-20").expect("preview");
    assert!(initial.batch.payableSettlementAmount > Decimal::ZERO);
    let event = PayrollService::create_retro_payment(
        &conn,
        &initial.batch,
        &initial.allocations,
        &payment_period.id,
        0,
    )
    .expect("initial retro payment");
    let impact = PayrollInvalidationRepository::assert_mutation_allowed(
        &conn,
        &PayrollMutation::PersonPeriod {
            personnelId: P1_PERSON.into(),
            periodId: source_period.id.clone(),
        },
    )
    .expect("source mutation allowed");
    PayrollInvalidationRepository::apply_impact(&conn, &impact).expect("impact applied");
    p1_refresh_source_normals(&conn, &source_period);
    let snapshot = PayrollService::build_dataset_snapshot(&conn).expect("dataset");
    let batch = snapshot
        .retroBatches
        .iter()
        .find(|batch| batch.id == batch_id)
        .expect("batch");
    assert_eq!(batch.status, CompensationRevisionStatus::STALE);
    assert_eq!(batch.settlementStatus, RetroSettlementStatus::UNSETTLED);
    let stale_event = snapshot
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == batch_id)
        .expect("event");
    assert_eq!(stale_event.status, BordroStatus::STALE);
    assert!(stale_event.gelirToplam > Decimal::ZERO && stale_event.netOdeme > Decimal::ZERO);
    let allocation_count = snapshot
        .retroAllocations
        .iter()
        .filter(|allocation| allocation.batchId == batch_id)
        .count();
    assert!(allocation_count > 0, "başlangıçta allocation mevcut");
    (conn, source_period, payment_period, revision, event, allocation_count)
}

/// Same branch as GeriyeDonukFarklar.handleReplayPayment for payable <= 0.
fn p1_zero_recovery(
    conn: &rusqlite::Connection,
    batch_id: &str,
    revision: &CompensationRevision,
) -> payroll_core::Result<()> {
    let replay = native_preview(conn, batch_id, revision, "2026-06-20").expect("replay");
    assert_eq!(replay.batch.totalGrossDelta, Decimal::ZERO, "gerçek fark 0 TL");
    assert_eq!(replay.batch.payableSettlementAmount, Decimal::ZERO);
    PayrollService::save_retro_adjustment_batch(conn, &replay.batch, &replay.allocations)
}

fn p1_assert_zero_ledger_without_event(conn: &rusqlite::Connection, batch_id: &str) {
    let snapshot = PayrollService::build_dataset_snapshot(conn).expect("dataset");
    let batches = snapshot
        .retroBatches
        .iter()
        .filter(|batch| batch.id == batch_id)
        .collect::<Vec<_>>();
    assert_eq!(batches.len(), 1, "duplicate batch yok");
    assert_eq!(batches[0].status, CompensationRevisionStatus::CALCULATED);
    assert_eq!(batches[0].settlementStatus, RetroSettlementStatus::UNSETTLED);
    assert_eq!(batches[0].totalGrossDelta, Decimal::ZERO);
    assert_eq!(batches[0].payableSettlementAmount, Decimal::ZERO);
    assert!(
        snapshot
            .retroAllocations
            .iter()
            .filter(|allocation| allocation.batchId == batch_id)
            .all(|allocation| allocation.deltaAmount == Decimal::ZERO
                && allocation.payableSettlementAmount == Decimal::ZERO),
        "allocation'lar eski pozitif farkı taşımamalı"
    );
    assert!(
        snapshot
            .payrolls
            .iter()
            .all(|payroll| payroll.accrualId != batch_id && payroll.id != batch_id),
        "0 TL ledger'a bağlı payment event kalmamalı (STALE/CALCULATED/DRAFT)"
    );
    let raw_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM payroll_records WHERE accrual_id = ?1 OR id = ?1",
            [batch_id],
            |row| row.get(0),
        )
        .expect("raw count");
    assert_eq!(raw_rows, 0, "DB'de eski event satırı yaşamamalı");
    let stale_retro: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM payroll_records
             WHERE accrual_type = 'RETRO_ADJUSTMENT' AND status IN ('DRAFT', 'STALE')",
            [],
            |row| row.get(0),
        )
        .expect("stale retro count");
    assert_eq!(stale_retro, 0, "aktif STALE retro payment event kalmamalı");
}

/// Test 1 + Test 2 + Test 4: the exact live scenario.
#[test]
fn native_p1_zero_difference_recovery_retires_stale_event_and_unblocks_normal() {
    let batch_id = "retro-p1-zero-difference";
    let (conn, _source_period, _payment_period, revision, _event, _allocations) =
        p1_stale_positive_retro(batch_id);
    let next_period = p1_save_next_period(&conn);

    // Precondition (the live blocker): the stale retro event blocks NORMAL.
    let blocked = PayrollService::calculate_payroll_for_personnel(&conn, P1_PERSON, &next_period.id)
        .expect_err("stale retro event önceki vergi zincirini bloke etmeli");
    assert!(blocked.to_string().contains("STALE"), "{blocked}");

    p1_neutralize_revision(&conn, &revision);
    p1_zero_recovery(&conn, batch_id, &revision).expect("zero-difference recovery");
    p1_assert_zero_ledger_without_event(&conn, batch_id);

    // Test 4: the same recovery again is idempotent.
    p1_zero_recovery(&conn, batch_id, &revision).expect("ikinci zero-difference recovery");
    p1_assert_zero_ledger_without_event(&conn, batch_id);
    let allocations_after_second = PayrollService::build_dataset_snapshot(&conn)
        .expect("dataset")
        .retroAllocations
        .iter()
        .filter(|allocation| allocation.batchId == batch_id)
        .count();
    p1_zero_recovery(&conn, batch_id, &revision).expect("üçüncü zero-difference recovery");
    assert_eq!(
        PayrollService::build_dataset_snapshot(&conn)
            .expect("dataset")
            .retroAllocations
            .iter()
            .filter(|allocation| allocation.batchId == batch_id)
            .count(),
        allocations_after_second,
        "allocation çoğalmamalı"
    );

    // Test 2 (critical): the next NORMAL payroll is no longer blocked.
    let next = PayrollService::calculate_payroll_for_personnel(&conn, P1_PERSON, &next_period.id)
        .expect("zero-difference recovery sonrası NORMAL bordro hesaplanmalı");
    assert_eq!(next.status, BordroStatus::CALCULATED);
    assert_eq!(next.accrualType, AccrualType::NORMAL);
    // The production checked boundary agrees.
    PayrollService::calculate_payroll_for_accrual_checked(
        &conn,
        P1_PERSON,
        &next_period.id,
        None,
        None,
    )
    .expect("checked NORMAL hesaplama da bloke olmamalı");

    // A repeated recovery after NORMAL exists still creates nothing.
    p1_zero_recovery(&conn, batch_id, &revision).expect("NORMAL sonrası tekrar recovery");
    p1_assert_zero_ledger_without_event(&conn, batch_id);
}

/// Scenario C: the event was already deleted; zero recovery creates none.
#[test]
fn native_p1_zero_difference_recovery_after_deleted_event_creates_no_event() {
    let batch_id = "retro-p1-deleted-event";
    let (conn, _source_period, payment_period, revision, _event, _allocations) =
        p1_stale_positive_retro(batch_id);
    PayrollRepository::delete_accrual(&conn, P1_PERSON, &payment_period.id, batch_id)
        .expect("stale retro event silinebilmeli");
    p1_neutralize_revision(&conn, &revision);
    p1_zero_recovery(&conn, batch_id, &revision).expect("zero-difference recovery");
    p1_assert_zero_ledger_without_event(&conn, batch_id);
}

/// Scenario D: a still-CALCULATED (unfinalized) event whose ledger now
/// replays to zero is no longer a payable obligation and is retired too.
#[test]
fn native_p1_zero_difference_recovery_retires_unfinalized_calculated_event() {
    let batch_id = "retro-p1-calculated-event";
    let (conn, _source_period, payment_period, revision) = setup_full_retro_database();
    let initial = native_preview(&conn, batch_id, &revision, "2026-06-20").expect("preview");
    PayrollService::create_retro_payment(
        &conn,
        &initial.batch,
        &initial.allocations,
        &payment_period.id,
        0,
    )
    .expect("initial retro payment");
    p1_neutralize_revision(&conn, &revision);
    // Revision edits stale the event; force the unfinalized CALCULATED state
    // this scenario is about (e.g. a legacy/partially recovered database).
    p1_force_event_status(&conn, batch_id, "CALCULATED");
    p1_zero_recovery(&conn, batch_id, &revision).expect("zero-difference recovery");
    p1_assert_zero_ledger_without_event(&conn, batch_id);
}

fn p1_force_event_status(conn: &rusqlite::Connection, batch_id: &str, status: &str) {
    let changed = conn
        .execute(
            "UPDATE payroll_records SET status = ?1 WHERE accrual_id = ?2",
            [status, batch_id],
        )
        .expect("event status");
    assert_eq!(changed, 1, "fixture: tek bağlı event");
}

/// Overpayment / offset ledgers keep the existing explicit-delete rule for a
/// live (DRAFT/CALCULATED) event: only the zero-difference path retires it.
#[test]
fn native_p1_overpayment_save_still_requires_explicit_delete_of_calculated_event() {
    let batch_id = "retro-p1-overpayment-guard";
    let (conn, _source_period, payment_period, mut revision) = setup_full_retro_database();
    let initial = native_preview(&conn, batch_id, &revision, "2026-06-20").expect("preview");
    PayrollService::create_retro_payment(
        &conn,
        &initial.batch,
        &initial.allocations,
        &payment_period.id,
        0,
    )
    .expect("initial retro payment");
    revision.title = "Native canonical retro (negative)".into();
    save_revision_with_overrides(
        &conn,
        &revision,
        &[CompensationRevisionOverride {
            id: "override-native-canonical".into(),
            revisionId: revision.id.clone(),
            parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
            value: dec!(100),
            personnelId: None,
        }],
    )
    .expect("negatif revision");
    let negative = native_preview(&conn, batch_id, &revision, "2026-06-20").expect("preview");
    assert!(negative.batch.totalGrossDelta < Decimal::ZERO);
    // Revision edits stale the linked event; force the "still live" state
    // this guard is about.
    p1_force_event_status(&conn, batch_id, "CALCULATED");
    let error = PayrollService::save_retro_adjustment_batch(
        &conn,
        &negative.batch,
        &negative.allocations,
    )
    .expect_err("CALCULATED event overpayment save ile sessizce silinmemeli");
    assert!(error.to_string().contains("payment event silinmeden"), "{error}");
    let event = PayrollService::build_dataset_snapshot(&conn)
        .expect("dataset")
        .payrolls
        .into_iter()
        .find(|payroll| payroll.accrualId == batch_id)
        .expect("event korunmalı");
    assert_eq!(event.status, BordroStatus::CALCULATED);
}

/// Test 5: a FINALIZED/PAID retro is never retired or rewritten by the
/// zero-difference recovery path, and the failure leaves no partial state.
#[test]
fn native_p1_zero_difference_recovery_never_retires_finalized_paid_retro() {
    let batch_id = "retro-p1-finalized";
    let (conn, source_period, payment_period, revision) = setup_full_retro_database();
    let initial = native_preview(&conn, batch_id, &revision, "2026-06-20").expect("preview");
    PayrollService::create_retro_payment(
        &conn,
        &initial.batch,
        &initial.allocations,
        &payment_period.id,
        0,
    )
    .expect("initial retro payment");
    for period_id in [source_period.id.as_str(), "2026-04"] {
        let stale_normal = PayrollService::build_dataset_snapshot(&conn)
            .expect("dataset")
            .payrolls
            .iter()
            .any(|payroll| payroll.donemId == period_id && payroll.status == BordroStatus::STALE);
        if stale_normal {
            PayrollService::calculate_payroll_for_personnel(&conn, P1_PERSON, period_id)
                .expect("source NORMAL refresh");
        }
        PayrollService::finalize_payroll_for_personnel(&conn, P1_PERSON, period_id)
            .expect("source NORMAL finalize");
    }
    let retro_stale = PayrollService::build_dataset_snapshot(&conn)
        .expect("dataset")
        .payrolls
        .iter()
        .any(|payroll| payroll.accrualId == batch_id && payroll.status == BordroStatus::STALE);
    if retro_stale {
        let again = native_preview(&conn, batch_id, &revision, "2026-06-20").expect("replay");
        PayrollService::create_retro_payment(
            &conn,
            &again.batch,
            &again.allocations,
            &payment_period.id,
            0,
        )
        .expect("retro refresh");
    }
    let finalized = PayrollService::finalize_payroll_for_accrual(
        &conn,
        P1_PERSON,
        &payment_period.id,
        Some(batch_id),
    )
    .expect("retro settlement finalize");
    assert_eq!(finalized.status, BordroStatus::FINALIZED);
    let before = PayrollService::build_dataset_snapshot(&conn).expect("before");
    let batch_before = before
        .retroBatches
        .iter()
        .find(|batch| batch.id == batch_id)
        .cloned()
        .expect("batch");
    assert_eq!(batch_before.status, CompensationRevisionStatus::FINALIZED);
    assert_eq!(batch_before.settlementStatus, RetroSettlementStatus::PAID);
    let allocations_before = before
        .retroAllocations
        .iter()
        .filter(|allocation| allocation.batchId == batch_id)
        .cloned()
        .collect::<Vec<_>>();

    // A forged zero-difference ledger for the same batch id.
    let mut forged = batch_before.clone();
    forged.status = CompensationRevisionStatus::CALCULATED;
    forged.settlementStatus = RetroSettlementStatus::UNSETTLED;
    forged.totalGrossDelta = Decimal::ZERO;
    forged.payableSettlementAmount = Decimal::ZERO;
    forged.finalizedAt = None;
    assert!(
        PayrollService::save_retro_adjustment_batch(&conn, &forged, &[]).is_err(),
        "FINALIZED/PAID retro zero-difference save ile değiştirilemez"
    );

    let after = PayrollService::build_dataset_snapshot(&conn).expect("after");
    let event_after = after
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == batch_id)
        .expect("FINALIZED event silinmemeli");
    assert_eq!(event_after.status, BordroStatus::FINALIZED);
    assert_eq!(event_after.netOdeme, finalized.netOdeme);
    assert_eq!(event_after.gelirToplam, finalized.gelirToplam);
    let batch_after = after
        .retroBatches
        .iter()
        .find(|batch| batch.id == batch_id)
        .expect("batch");
    assert_eq!(batch_after.status, CompensationRevisionStatus::FINALIZED);
    assert_eq!(batch_after.settlementStatus, RetroSettlementStatus::PAID);
    assert_eq!(batch_after.totalGrossDelta, batch_before.totalGrossDelta);
    assert_eq!(
        after
            .retroAllocations
            .iter()
            .filter(|allocation| allocation.batchId == batch_id)
            .cloned()
            .collect::<Vec<_>>(),
        allocations_before
    );
}

/// Atomicity: the event retirement and the batch/allocation rewrite are one
/// transaction. A failure in the batch upsert (after the event row has
/// already been deleted inside the transaction) must roll everything back.
#[test]
fn native_p1_zero_difference_recovery_is_atomic_when_batch_write_fails() {
    let batch_id = "retro-p1-atomic";
    let (conn, _source_period, _payment_period, revision, event, allocation_count) =
        p1_stale_positive_retro(batch_id);
    p1_neutralize_revision(&conn, &revision);
    conn.execute_batch(
        "CREATE TEMP TRIGGER p1_force_batch_write_failure
         BEFORE UPDATE ON retro_adjustment_batches
         BEGIN SELECT RAISE(ABORT, 'p1 forced batch write failure'); END;",
    )
    .expect("test trigger");
    let error = p1_zero_recovery(&conn, batch_id, &revision)
        .expect_err("batch yazımı başarısızsa recovery başarısız olmalı");
    assert!(error.to_string().contains("p1 forced batch write failure"), "{error}");
    conn.execute_batch("DROP TRIGGER p1_force_batch_write_failure;")
        .expect("drop trigger");

    let snapshot = PayrollService::build_dataset_snapshot(&conn).expect("dataset");
    let batch = snapshot
        .retroBatches
        .iter()
        .find(|batch| batch.id == batch_id)
        .expect("batch");
    assert_eq!(batch.status, CompensationRevisionStatus::STALE, "batch değişmemeli");
    assert!(batch.totalGrossDelta > Decimal::ZERO);
    assert_eq!(
        snapshot
            .retroAllocations
            .iter()
            .filter(|allocation| allocation.batchId == batch_id)
            .count(),
        allocation_count,
        "allocation'lar değişmemeli"
    );
    let stale_event = snapshot
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == batch_id)
        .expect("event rollback ile geri gelmeli");
    assert_eq!(stale_event.status, BordroStatus::STALE);
    assert_eq!(stale_event.netOdeme, event.netOdeme);

    // After the transient failure the recovery itself succeeds.
    p1_zero_recovery(&conn, batch_id, &revision).expect("recovery");
    p1_assert_zero_ledger_without_event(&conn, batch_id);
}

/// Databases already left in the inconsistent live state by the old code
/// (batch CALCULATED/UNSETTLED 0 TL, linked event STALE with old gross/net,
/// e.g. retro-9f210d2c-...) are repaired by running the same recovery again.
#[test]
fn native_p1_legacy_zero_ledger_with_stale_event_is_repaired_by_recovery() {
    let batch_id = "retro-p1-legacy-split-brain";
    let (conn, _source_period, _payment_period, revision, event, _allocations) =
        p1_stale_positive_retro(batch_id);
    let next_period = p1_save_next_period(&conn);
    p1_neutralize_revision(&conn, &revision);
    // Old behavior: ledger rewritten, event left untouched.
    let replay = native_preview(&conn, batch_id, &revision, "2026-06-20").expect("replay");
    save_batch(&conn, &replay.batch, &replay.allocations).expect("legacy ledger save");
    let legacy = PayrollService::build_dataset_snapshot(&conn).expect("dataset");
    let legacy_batch = legacy
        .retroBatches
        .iter()
        .find(|batch| batch.id == batch_id)
        .expect("batch");
    assert_eq!(legacy_batch.status, CompensationRevisionStatus::CALCULATED);
    assert_eq!(legacy_batch.payableSettlementAmount, Decimal::ZERO);
    let legacy_event = legacy
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == batch_id)
        .expect("legacy event");
    assert_eq!(legacy_event.status, BordroStatus::STALE);
    assert_eq!(legacy_event.netOdeme, event.netOdeme);
    assert!(
        PayrollService::calculate_payroll_for_personnel(&conn, P1_PERSON, &next_period.id)
            .is_err(),
        "legacy split-brain NORMAL'ı bloke ediyordu"
    );

    p1_zero_recovery(&conn, batch_id, &revision).expect("legacy repair");
    p1_assert_zero_ledger_without_event(&conn, batch_id);
    PayrollService::calculate_payroll_for_personnel(&conn, P1_PERSON, &next_period.id)
        .expect("onarım sonrası NORMAL hesaplanmalı");
}
