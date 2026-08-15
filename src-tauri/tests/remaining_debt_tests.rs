use bordro_programi_lib::db::connection::create_in_memory_connection;
use bordro_programi_lib::db::migrations::initialize_db;
use bordro_programi_lib::domain::calculations::calculate_total_tax_for_cumulative_matrah_with_brackets;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::{DomainError, Result};
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::services::migration_service::MigrationService;
use bordro_programi_lib::services::payroll_service::PayrollService;
use bordro_programi_lib::services::period_service::PeriodService;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: format!("100000000{}", id.len()),
        ad: "Test".into(),
        soyad: "Personel".into(),
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

fn period(id: &str, tax_year: i32) -> BordroDonemi {
    BordroDonemi {
        id: id.into(),
        yil: 2026,
        ay: 5,
        baslangicTarihi: "2026-05-15".into(),
        bitisTarihi: "2026-06-14".into(),
        donemAdi: "Mayıs 2026".into(),
        taxYear: tax_year,
        taxMonth: 6,
    }
}

fn settings(period_id: &str) -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: period_id.into(),
        ..DonemselKurumDegerleri::default()
    }
}

fn attendance(personnel_id: &str, period_id: &str, code: &str) -> PersonelPuantaj {
    let mut gunler = HashMap::new();
    gunler.insert("2026-05-15".into(), code.into());
    PersonelPuantaj {
        id: format!("{personnel_id}_{period_id}"),
        personelId: personnel_id.into(),
        donemId: period_id.into(),
        gunler,
    }
}

#[test]
fn open_ended_tax_bracket_is_sqlite_safe_and_unbounded() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let parameters = AnnualPayrollParametersRepository::get_by_year(&conn, 2026)?
        .expect("fresh database must contain the default annual tariff");
    let last = parameters.gelirVergisiDilimleri.last().unwrap();
    assert_eq!(last.limit, Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT));

    let tax = calculate_total_tax_for_cumulative_matrah_with_brackets(
        Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT) + dec!(1),
        &parameters.gelirVergisiDilimleri,
    );
    assert!(tax > Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT) * dec!(0.39));
    Ok(())
}

#[test]
fn devir_migration_with_existing_columns_is_idempotent() -> Result<()> {
    let mut conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let mut saved_person = person("migration-person");
    saved_person.devirKumulatifGvMatrahi = Some(dec!(1234.56));
    saved_person.devirKumulatifGvMatrahiYili = Some(2026);
    PersonnelRepository::save(&conn, &saved_person)?;

    conn.execute("ALTER TABLE payroll_records DROP COLUMN notlar", [])
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    conn.pragma_update(None, "user_version", 5u32)
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    initialize_db(&mut conn).map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    let restored = PersonnelRepository::get_by_id(&conn, "migration-person")?
        .expect("migration must preserve existing personnel data");
    assert_eq!(restored.devirKumulatifGvMatrahi, Some(dec!(1234.56)));
    assert!(
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('payroll_records') WHERE name = 'notlar'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
            == 1
    );

    conn.pragma_update(None, "user_version", 6u32)
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    initialize_db(&mut conn).map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    Ok(())
}

#[test]
fn v1_import_fills_missing_annual_parameters_per_tax_year() -> Result<()> {
    let mut conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let payload = r#"{
        "backupVersion": 1,
        "donemler": [{
            "id": "2026-05-v1", "yil": 2026, "ay": 5,
            "baslangicTarihi": "2026-05-15", "bitisTarihi": "2026-06-14",
            "donemAdi": "Mayıs 2026", "taxYear": 2027, "taxMonth": 1
        }]
    }"#;
    MigrationService::migrate_legacy_data(&mut conn, payload)?;

    let imported = AnnualPayrollParametersRepository::get_by_year(&conn, 2027)?
        .expect("V1 import must create a tariff for every imported tax year");
    assert_eq!(
        imported.gelirVergisiDilimleri.last().unwrap().limit,
        Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT)
    );
    Ok(())
}

#[test]
fn backend_rejects_invalid_dates_codes_and_unsafe_tax_limit() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    let invalid_period = BordroDonemi {
        bitisTarihi: "2026-05-14".into(),
        ..period("invalid-period", 2026)
    };
    assert!(matches!(
        PeriodRepository::save(&conn, &invalid_period),
        Err(DomainError::ValidationError(_))
    ));

    assert!(matches!(
        AttendanceRepository::save(&conn, &attendance("p", "2026-05", "X")),
        Err(DomainError::ValidationError(_))
    ));

    let invalid_sick = SickLeaveRecord {
        id: "sick-invalid".into(),
        personnelId: "p".into(),
        startDate: "2026-06-02".into(),
        endDate: "2026-06-01".into(),
        createdAt: None,
        updatedAt: None,
    };
    assert!(matches!(
        SickLeaveRepository::save(&conn, &invalid_sick),
        Err(DomainError::ValidationError(_))
    ));

    let mut unsafe_parameters = AnnualPayrollParameters::default_for_2026();
    unsafe_parameters
        .gelirVergisiDilimleri
        .last_mut()
        .unwrap()
        .limit = Decimal::MAX;
    assert!(matches!(
        AnnualPayrollParametersRepository::save(&conn, &unsafe_parameters),
        Err(DomainError::ValidationError(_))
    ));
    Ok(())
}

#[test]
fn missing_legal_setting_fails_payroll_instead_of_defaulting() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let personnel = person("missing-legal");
    PersonnelRepository::save(&conn, &personnel)?;
    let payroll_period = period("2026-05", 2026);
    PeriodRepository::save(&conn, &payroll_period)?;
    let mut incomplete = settings("2026-05");
    incomplete.sgkIsciOraniYuzde = None;
    SettingsRepository::save_institution_settings(&conn, &incomplete)?;
    AttendanceRepository::save(&conn, &attendance("missing-legal", "2026-05", "Ç"))?;

    let result = PayrollService::calculate_payroll_for_personnel(&conn, "missing-legal", "2026-05");
    assert!(
        matches!(result, Err(DomainError::ValidationError(message)) if message.contains("sgkIsciOraniYuzde"))
    );
    Ok(())
}

#[test]
fn period_and_settings_are_atomic_when_settings_write_fails() -> Result<()> {
    let mut conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    conn.execute_batch(
        "CREATE TRIGGER fail_institution_settings
         BEFORE INSERT ON institution_settings
         BEGIN SELECT RAISE(ABORT, 'test settings failure'); END;",
    )
    .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    let result = PeriodService::save_period_with_settings(
        &mut conn,
        &period("atomic-period", 2026),
        &settings("atomic-period"),
    );
    assert!(result.is_err());
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM payroll_periods WHERE id = 'atomic-period'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0, "failed settings write must roll back the period");
    Ok(())
}

#[test]
fn common_raise_month_splits_15_14_daily_income() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let personnel = person("raise-person");
    PersonnelRepository::save(&conn, &personnel)?;

    let previous_period = BordroDonemi {
        id: "2026-02".into(),
        yil: 2026,
        ay: 2,
        baslangicTarihi: "2026-02-15".into(),
        bitisTarihi: "2026-03-14".into(),
        donemAdi: "Şubat 2026".into(),
        taxYear: 2026,
        taxMonth: 3,
    };
    let current_period = BordroDonemi {
        id: "2026-03".into(),
        yil: 2026,
        ay: 3,
        baslangicTarihi: "2026-03-15".into(),
        bitisTarihi: "2026-04-14".into(),
        donemAdi: "Mart 2026".into(),
        taxYear: 2026,
        taxMonth: 4,
    };
    PeriodRepository::save(&conn, &previous_period)?;
    PeriodRepository::save(&conn, &current_period)?;

    let mut previous_settings = settings(&previous_period.id);
    previous_settings.gunlukTabanUcret = dec!(1000);
    previous_settings.gunlukYemek = dec!(10);
    previous_settings.gunlukVasitaYol = dec!(5);
    SettingsRepository::save_institution_settings(&conn, &previous_settings)?;

    let mut current_settings = settings(&current_period.id);
    current_settings.gunlukTabanUcret = dec!(1200);
    current_settings.gunlukYemek = dec!(20);
    current_settings.gunlukVasitaYol = dec!(7);
    SettingsRepository::save_institution_settings(&conn, &current_settings)?;

    let mut gunler = HashMap::new();
    for day in 15..=31 {
        gunler.insert(format!("2026-03-{day:02}"), "Ç".into());
    }
    for day in 1..=14 {
        gunler.insert(format!("2026-04-{day:02}"), "Ç".into());
    }
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: "raise-person_2026-03".into(),
            personelId: personnel.id.clone(),
            donemId: current_period.id.clone(),
            gunler,
        },
    )?;
    SettingsRepository::set_app_setting(&conn, "zam_aylari", "[4]")?;

    let payroll =
        PayrollService::calculate_payroll_for_personnel(&conn, &personnel.id, &current_period.id)?;

    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(33800.00)));
    assert_eq!(payroll.gelirler.yemek, Some(dec!(450.00)));
    assert_eq!(payroll.gelirler.vasitaYol, Some(dec!(183.00)));
    assert_eq!(payroll.gelirler.isPrimi, Some(dec!(3042.00)));
    assert_eq!(payroll.isPrimiDetay.unwrap().hakGunu, 31);
    Ok(())
}
