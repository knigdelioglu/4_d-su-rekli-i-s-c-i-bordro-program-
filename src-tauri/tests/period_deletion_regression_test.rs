use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::{
    attendance_repo::AttendanceRepository, payroll_repo::PayrollRepository,
    period_repo::PeriodRepository, personnel_repo::PersonnelRepository,
    settings_repo::SettingsRepository, tax_opening_repo::TaxOpeningRepository,
};
use bordro_programi_lib::services::{
    payroll_service::PayrollService, period_service::PeriodService,
};
use chrono::{Duration, NaiveDate};
use rust_decimal_macros::dec;
use std::collections::HashMap;
fn period(id: &str, year: i32, month: i32, tax_year: i32, tax_month: i32) -> BordroDonemi {
    let (end_year, end_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    BordroDonemi {
        id: id.into(),
        yil: year,
        ay: month,
        baslangicTarihi: format!("{year}-{month:02}-15"),
        bitisTarihi: format!("{end_year}-{end_month:02}-14"),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111111".into(),
        ad: "Dependency".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "1".into(),
        iban: "TR00".into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri::default()),
    }
}

fn settings(period_id: &str, daily_minimum: rust_decimal::Decimal) -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: period_id.into(),
        gunlukTabanUcret: dec!(1000),
        gunlukYemek: dec!(0),
        birlestirilmisSosyalYardim: dec!(0),
        gunlukVasitaYol: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        isPrimiGruplari: Some(vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(0),
            aktif: true,
        }]),
        ekOdeme: Some(dec!(0)),
        digerGelirVarsayilan: Some(dec!(0)),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        damgaVergisiOraniBinde: Some(dec!(7.59)),
        sendikaAidatiYuzde: Some(dec!(0)),
        besOraniYuzde: Some(dec!(3)),
        gunlukYemekIstisnasiSGK: Some(dec!(0)),
        gunlukYemekIstisnasiGV: Some(dec!(0)),
        gunlukAsgariUcret: Some(daily_minimum),
        pekTavanKatsayisi: Some(dec!(3)),
        sgkIsverenOraniYuzde: Some(dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        ..DonemselKurumDegerleri::default()
    }
}

fn complete_attendance(personnel_id: &str, p: &BordroDonemi, code: &str) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&p.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&p.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    let mut date = start;
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), code.to_string());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("{}_{}", personnel_id, p.id),
        personelId: personnel_id.into(),
        donemId: p.id.clone(),
        gunler,
    }
}

fn setup() -> rusqlite::Connection {
    let conn = create_in_memory_connection().unwrap();
    let p = person("p1");
    PersonnelRepository::save(&conn, &p).unwrap();
    for month in [1, 2] {
        let d = period(&format!("2026-{month:02}"), 2026, month, 2026, month);
        PeriodRepository::save(&conn, &d).unwrap();
        SettingsRepository::save_institution_settings(&conn, &settings(&d.id, dec!(1000))).unwrap();
        AttendanceRepository::save(&conn, &complete_attendance(&p.id, &d, "Ç")).unwrap();
    }
    conn
}

#[test]
fn delete_finalized_period_cascades_and_invalidates_later_finalized_payroll() {
    let mut conn = setup();
    for id in ["2026-01", "2026-02"] {
        PayrollService::calculate_payroll_for_personnel(&conn, "p1", id).unwrap();
        PayrollService::finalize_payroll_for_personnel(&conn, "p1", id).unwrap();
    }
    SettingsRepository::set_app_setting(&conn, "active_period_id", "2026-01").unwrap();
    conn.execute("INSERT INTO personnel_tax_opening (id, personnel_id, year, gv_cumulative_opening, effective_from_period_id, created_at, updated_at) VALUES ('opening', 'p1', 2026, 0, '2026-01', '', '')", []).unwrap();
    PeriodService::delete_period(&mut conn, "2026-01").unwrap();
    assert!(PeriodRepository::get_by_id(&conn, "2026-01")
        .unwrap()
        .is_none());
    let payrolls = PayrollRepository::get_all(&conn).unwrap();
    assert_eq!(payrolls.len(), 1);
    assert_eq!(payrolls[0].donemId, "2026-02");
    assert_eq!(payrolls[0].status, BordroStatus::STALE);
    for table in ["attendance_records", "institution_settings"] {
        let count: i64 = conn
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE period_id = '2026-01'"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
    let count: i64 = conn
        .query_row("SELECT count(*) FROM personnel_tax_opening", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        SettingsRepository::get_app_setting(&conn, "active_period_id")
            .unwrap()
            .as_deref(),
        Some("2026-02")
    );
    PeriodService::delete_period(&mut conn, "2026-02").unwrap();
    assert!(PeriodRepository::get_all(&conn).unwrap().is_empty());
    assert_eq!(
        SettingsRepository::get_app_setting(&conn, "active_period_id")
            .unwrap()
            .as_deref(),
        Some("")
    );
    let count: i64 = conn
        .query_row("SELECT count(*) FROM personnel", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn deletion_failure_rolls_back_cascades_and_active_selection() {
    let mut conn = setup();
    PayrollService::calculate_payroll_for_personnel(&conn, "p1", "2026-01").unwrap();
    SettingsRepository::set_app_setting(&conn, "active_period_id", "2026-01").unwrap();
    conn.execute_batch("CREATE TRIGGER reject_period_delete BEFORE DELETE ON payroll_periods BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
    assert!(PeriodService::delete_period(&mut conn, "2026-01").is_err());
    assert_eq!(PeriodRepository::get_all(&conn).unwrap().len(), 2);
    assert_eq!(
        PayrollRepository::get_all(&conn).unwrap()[0].status,
        BordroStatus::CALCULATED
    );
    assert_eq!(
        SettingsRepository::get_app_setting(&conn, "active_period_id")
            .unwrap()
            .as_deref(),
        Some("2026-01")
    );
    assert!(PeriodService::delete_period(&mut conn, "missing").is_err());
}

#[test]
fn deletion_clears_only_the_tax_opening_component_linked_to_the_period() {
    let mut conn = setup();
    let march = period("2026-03", 2026, 3, 2026, 4);
    PeriodRepository::save(&conn, &march).unwrap();
    TaxOpeningRepository::save(
        &conn,
        &PersonelTaxOpening {
            id: "opening-cross-period".into(),
            personnelId: "p1".into(),
            year: 2026,
            gvCumulativeOpening: Some(dec!(120000)),
            effectiveFromPeriodId: Some("2026-01".into()),
            asgariGvCumulativeOpening: Some(dec!(60000)),
            asgariGvEffectiveFromPeriodId: Some("2026-03".into()),
            createdAt: None,
            updatedAt: None,
        },
    )
    .unwrap();

    PeriodService::delete_period(&mut conn, "2026-01").unwrap();
    let opening = TaxOpeningRepository::get_by_personnel_and_year(&conn, "p1", 2026)
        .unwrap()
        .expect("March asgari GV opening korunmalı");
    assert_eq!(opening.gvCumulativeOpening, None);
    assert_eq!(opening.effectiveFromPeriodId, None);
    assert_eq!(opening.asgariGvCumulativeOpening, Some(dec!(60000)));
    assert_eq!(
        opening.asgariGvEffectiveFromPeriodId.as_deref(),
        Some("2026-03")
    );

    PeriodService::delete_period(&mut conn, "2026-03").unwrap();
    assert!(
        TaxOpeningRepository::get_by_personnel_and_year(&conn, "p1", 2026)
            .unwrap()
            .is_none()
    );
}

#[test]
fn deleting_active_period_selects_the_earliest_remaining_period_by_calendar_order() {
    let mut conn = setup();
    let earlier = period("2025-12", 2025, 12, 2025, 12);
    PeriodRepository::save(&conn, &earlier).unwrap();
    SettingsRepository::set_app_setting(&conn, "active_period_id", "2026-01").unwrap();

    PeriodService::delete_period(&mut conn, "2026-01").unwrap();

    assert_eq!(
        SettingsRepository::get_app_setting(&conn, "active_period_id")
            .unwrap()
            .as_deref(),
        Some("2025-12")
    );
}

fn revision() -> CompensationRevision {
    CompensationRevision {
        id: "revision-atomic".into(),
        reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
        title: "Atomic retro test".into(),
        effectiveFrom: "2026-01-15".into(),
        effectiveTo: None,
        decisionDate: None,
        signedAt: None,
        description: None,
        status: CompensationRevisionStatus::DRAFT,
        scope: CompensationRevisionScope::SELECTED_PERSONNEL,
        personnelIds: vec!["p1".into()],
        personnelGroup: None,
        createdAt: None,
        updatedAt: None,
    }
}

fn batch() -> RetroAdjustmentBatch {
    RetroAdjustmentBatch {
        id: "batch-atomic".into(),
        revisionId: "revision-atomic".into(),
        personnelId: "p1".into(),
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
        personnelId: "p1".into(),
        sourcePeriodId: "2026-01".into(),
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

#[test]
fn deletion_removes_retro_batches_and_allocations_before_restricted_foreign_key() {
    use bordro_programi_lib::repositories::retro_repo::{
        get_allocations, get_batches, save_batch, save_revision_with_overrides,
    };
    let mut conn = setup();
    save_revision_with_overrides(&conn, &revision(), &[]).unwrap();
    save_batch(&conn, &batch(), &[allocation()]).unwrap();
    assert_eq!(get_allocations(&conn).unwrap().len(), 1);
    PeriodService::delete_period(&mut conn, "2026-01").unwrap();
    assert!(get_batches(&conn).unwrap().is_empty());
    assert!(get_allocations(&conn).unwrap().is_empty());
    assert_eq!(PeriodRepository::get_all(&conn).unwrap().len(), 1);
    let mut check = conn.prepare("PRAGMA foreign_key_check").unwrap();
    assert!(!check.query([]).unwrap().next().unwrap().is_some());
}
