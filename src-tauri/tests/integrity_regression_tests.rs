use bordro_programi_lib::db::connection::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::{DomainError, Result};
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::services::migration_service::MigrationService;
use bordro_programi_lib::services::payroll_service::PayrollService;
use rust_decimal_macros::dec;
use serde_json::json;
use std::collections::HashMap;

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: format!("10000000{:03}", id.len()),
        ad: "Integrity".into(),
        soyad: "Test".into(),
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

fn period(
    id: &str,
    year: i32,
    month: i32,
    start: &str,
    end: &str,
    tax_year: i32,
    tax_month: i32,
) -> BordroDonemi {
    BordroDonemi {
        id: id.into(),
        yil: year,
        ay: month,
        baslangicTarihi: start.into(),
        bitisTarihi: end.into(),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

fn settings(period_id: &str) -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: period_id.into(),
        ..DonemselKurumDegerleri::default()
    }
}

#[test]
fn year_boundary_pek_uses_old_and_new_daily_limits() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let personnel = person("year-boundary");
    PersonnelRepository::save(&conn, &personnel)?;

    let previous_period = period("2025-11", 2025, 11, "2025-11-15", "2025-12-14", 2025, 12);
    let current_period = period("2025-12", 2025, 12, "2025-12-15", "2026-01-14", 2026, 1);
    PeriodRepository::save(&conn, &previous_period)?;
    PeriodRepository::save(&conn, &current_period)?;

    let mut previous_settings = settings(&previous_period.id);
    previous_settings.gunlukAsgariUcret = Some(dec!(866.85));
    previous_settings.pekTavanKatsayisi = Some(dec!(7.5));
    previous_settings.gunlukYemekIstisnasiSGK = Some(dec!(158));
    SettingsRepository::save_institution_settings(&conn, &previous_settings)?;

    let mut current_settings = settings(&current_period.id);
    current_settings.gunlukAsgariUcret = Some(dec!(1101));
    current_settings.pekTavanKatsayisi = Some(dec!(9));
    current_settings.gunlukYemekIstisnasiSGK = Some(dec!(158));
    SettingsRepository::save_institution_settings(&conn, &current_settings)?;

    let mut days = HashMap::new();
    for day in 15..=31 {
        days.insert(format!("2025-12-{day:02}"), "Ç".into());
    }
    for day in 1..=14 {
        days.insert(format!("2026-01-{day:02}"), "Ç".into());
    }
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: "year-boundary_2025-12".into(),
            personelId: personnel.id.clone(),
            donemId: current_period.id.clone(),
            gunler: days,
        },
    )?;

    let payroll =
        PayrollService::calculate_payroll_for_personnel(&conn, &personnel.id, &current_period.id)?;
    let pek = payroll.pekDetay.expect("PEK detayı üretilmeli");

    assert_eq!(pek.pekAltSinir, dec!(29283.60));
    assert_eq!(pek.pekUstSinir, dec!(242748.08));
    Ok(())
}

#[test]
fn negative_net_payroll_is_rejected() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let mut personnel = person("negative-net");
    personnel.kesintiler = Some(PersonelKesintileri {
        sendikaUyesi: Some(false),
        sabitSendikaAidati: None,
        besUyesi: Some(false),
        oksOraniYuzde: None,
        sabitBesTutar: None,
        icraTutar: None,
        kisiBorcuTutar: None,
        dogumAskerlikBorclanmasiTutar: None,
        hayatSaglikSigortasiTutar: None,
        digerKesintiTutar: Some(dec!(1000000)),
    });
    PersonnelRepository::save(&conn, &personnel)?;

    let payroll_period = period("2026-05", 2026, 5, "2026-05-15", "2026-06-14", 2026, 6);
    PeriodRepository::save(&conn, &payroll_period)?;
    SettingsRepository::save_institution_settings(&conn, &settings(&payroll_period.id))?;
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: "negative-net_2026-05".into(),
            personelId: personnel.id.clone(),
            donemId: payroll_period.id.clone(),
            gunler: HashMap::from([("2026-05-15".into(), "Ç".into())]),
        },
    )?;

    assert!(matches!(
        PayrollService::calculate_payroll_for_personnel(
            &conn,
            &personnel.id,
            &payroll_period.id
        ),
        Err(DomainError::ValidationError(message)) if message.contains("negatif")
    ));
    Ok(())
}

#[test]
fn duplicate_work_or_tax_period_is_rejected() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let first = period("2026-05", 2026, 5, "2026-05-15", "2026-06-14", 2026, 6);
    PeriodRepository::save(&conn, &first)?;

    let duplicate_work = period("other-id", 2026, 5, "2026-05-15", "2026-06-14", 2026, 7);
    assert!(matches!(
        PeriodRepository::save(&conn, &duplicate_work),
        Err(DomainError::ValidationError(_))
    ));

    let duplicate_tax = period("2026-06", 2026, 6, "2026-06-15", "2026-07-14", 2026, 6);
    assert!(matches!(
        PeriodRepository::save(&conn, &duplicate_tax),
        Err(DomainError::ValidationError(_))
    ));
    Ok(())
}

#[test]
fn overlapping_sick_leave_is_rejected() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let personnel = person("sick-overlap");
    PersonnelRepository::save(&conn, &personnel)?;

    SickLeaveRepository::save(
        &conn,
        &SickLeaveRecord {
            id: "sick-1".into(),
            personnelId: personnel.id.clone(),
            startDate: "2026-06-01".into(),
            endDate: "2026-06-05".into(),
            createdAt: None,
            updatedAt: None,
        },
    )?;

    let overlapping = SickLeaveRecord {
        id: "sick-2".into(),
        personnelId: personnel.id.clone(),
        startDate: "2026-06-05".into(),
        endDate: "2026-06-07".into(),
        createdAt: None,
        updatedAt: None,
    };
    assert!(matches!(
        SickLeaveRepository::save(&conn, &overlapping),
        Err(DomainError::ValidationError(_))
    ));
    Ok(())
}

#[test]
fn invalid_backup_payroll_snapshot_rolls_back_replace() -> Result<()> {
    let mut conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let sentinel = person("sentinel");
    PersonnelRepository::save(&conn, &sentinel)?;

    let imported_person = person("backup-person");
    let imported_period = period("2026-05", 2026, 5, "2026-05-15", "2026-06-14", 2026, 6);
    let bad_payroll = BordroKaydi {
        id: "backup-person_2026-05".into(),
        personelId: imported_person.id.clone(),
        donemId: imported_period.id.clone(),
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri {
            tabanBrutAylik: Some(dec!(100)),
            ..GelirKalemleri::default()
        },
        gelirToplam: dec!(999),
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: dec!(0),
        netOdeme: dec!(999),
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: "2026-06-01T00:00:00Z".into(),
        sonGuncellemeTarihi: "2026-06-01T00:00:00Z".into(),
        notlar: None,
        oncekiKumulatifGvMatrahi: None,
        oncekiKumulatifAsgariGvMatrahi: None,
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: None,
        pekDetay: None,
        isPrimiDetay: None,
        gvDetay: None,
        odenenRaporluGun: None,
        raporluGun: Some(0),
    };

    let payload = json!({
        "backupVersion": 2,
        "donemler": [imported_period],
        "aktifDonemId": "2026-05",
        "personeller": [imported_person],
        "kurumDegerleriMap": {},
        "puantajlar": [],
        "bordrolar": [bad_payroll],
        "taxOpenings": [],
        "sickLeaveRecords": [],
        "annualPayrollParameters": [AnnualPayrollParameters::default_for_2026()],
        "zamAylari": []
    })
    .to_string();

    assert!(MigrationService::replace_backup_data(&mut conn, &payload).is_err());
    assert!(PersonnelRepository::get_by_id(&conn, &sentinel.id)?.is_some());
    assert!(PersonnelRepository::get_by_id(&conn, "backup-person")?.is_none());
    Ok(())
}
