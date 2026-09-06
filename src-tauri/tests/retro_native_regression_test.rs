use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
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
    AnnualPayrollParametersRepository::save(
        &conn,
        &AnnualPayrollParameters::default_for_2026(),
    )
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
    let result = payroll_core::RetroEntitlementEngine::calculate(
        &payroll_core::RetroCalculationRequest {
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
        },
    )
    .expect("canonical preview hesaplanmalı");

    let error = PayrollService::save_retro_adjustment_batch(&conn, &result.batch, &result.allocations)
        .expect_err("pozitif batch payment event olmadan saklanmamalı");
    assert!(error.to_string().contains("payment event"));
    assert!(get_batches(&conn).expect("batch listesi okunmalı").is_empty());
}

#[test]
fn native_deleting_unfinalized_retro_payment_stales_its_ledger() {
    let (conn, _source_period, payment_period, revision) = setup_full_retro_database();
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let result = payroll_core::RetroEntitlementEngine::calculate(
        &payroll_core::RetroCalculationRequest {
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
        },
    )
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
fn native_retro_payment_rejects_forged_preview_before_any_write() {
    let (conn, source_period, payment_period, revision) = setup_full_retro_database();
    let dataset = PayrollService::build_dataset_snapshot(&conn).expect("dataset okunmalı");
    let result = payroll_core::RetroEntitlementEngine::calculate(
        &payroll_core::RetroCalculationRequest {
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
        },
    )
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
    assert!(get_batches(&conn).expect("batch listesi okunmalı").is_empty());
    assert_eq!(
        bordro_programi_lib::repositories::payroll_repo::PayrollRepository::get_all(&conn)
            .expect("bordrolar okunmalı")
            .len(),
        1,
        "yalnız source normal payroll kalmalı"
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
    let result = payroll_core::RetroEntitlementEngine::calculate(
        &payroll_core::RetroCalculationRequest {
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
        },
    )
    .expect("negative preview hesaplanmalı");
    assert!(result.batch.totalGrossDelta < Decimal::ZERO);
    PayrollService::save_retro_adjustment_batch(&conn, &result.batch, &result.allocations)
        .expect("negative batch ödeme olmadan saklanmalı");
    let saved = get_batches(&conn).expect("batch listesi okunmalı");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].settlementStatus, RetroSettlementStatus::OVERPAYMENT);
    assert!(bordro_programi_lib::repositories::payroll_repo::PayrollRepository::get_all(&conn)
        .expect("bordrolar okunmalı")
        .iter()
        .all(|record| record.accrualType != AccrualType::RETRO_ADJUSTMENT));
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
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].accrualId, source.accrualId);
}
