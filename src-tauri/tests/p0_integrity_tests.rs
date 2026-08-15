use bordro_programi_lib::db::connection::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::{DomainError, Result};
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "10000000001".into(),
        ad: "P0".into(),
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

fn settings(period_id: &str) -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: period_id.into(),
        ..DonemselKurumDegerleri::default()
    }
}

#[test]
fn attendance_rejects_invalid_and_out_of_period_dates() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let period = BordroDonemi {
        id: "2026-05".into(),
        yil: 2026,
        ay: 5,
        baslangicTarihi: "2026-05-15".into(),
        bitisTarihi: "2026-06-14".into(),
        donemAdi: "Mayıs 2026".into(),
        taxYear: 2026,
        taxMonth: 6,
    };
    PeriodRepository::save(&conn, &period)?;

    let invalid_date = PersonelPuantaj {
        id: "invalid-date".into(),
        personelId: "p".into(),
        donemId: period.id.clone(),
        gunler: HashMap::from([("2026-05-99".into(), "Ç".into())]),
    };
    assert!(matches!(
        AttendanceRepository::save(&conn, &invalid_date),
        Err(DomainError::ValidationError(_))
    ));

    let outside_period = PersonelPuantaj {
        id: "outside-period".into(),
        personelId: "p".into(),
        donemId: period.id.clone(),
        gunler: HashMap::from([("2026-06-15".into(), "Ç".into())]),
    };
    assert!(matches!(
        AttendanceRepository::save(&conn, &outside_period),
        Err(DomainError::ValidationError(_))
    ));

    Ok(())
}

#[test]
fn april_2026_meal_exemption_is_split_at_17_april() -> Result<()> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let personnel = person("meal-transition");
    PersonnelRepository::save(&conn, &personnel)?;

    let previous_period = BordroDonemi {
        id: "2026-03".into(),
        yil: 2026,
        ay: 3,
        baslangicTarihi: "2026-03-15".into(),
        bitisTarihi: "2026-04-14".into(),
        donemAdi: "Mart 2026".into(),
        taxYear: 2026,
        taxMonth: 4,
    };
    let current_period = BordroDonemi {
        id: "2026-04".into(),
        yil: 2026,
        ay: 4,
        baslangicTarihi: "2026-04-15".into(),
        bitisTarihi: "2026-05-14".into(),
        donemAdi: "Nisan 2026".into(),
        taxYear: 2026,
        taxMonth: 5,
    };
    PeriodRepository::save(&conn, &previous_period)?;
    PeriodRepository::save(&conn, &current_period)?;

    let mut previous_settings = settings(&previous_period.id);
    previous_settings.gunlukYemek = dec!(300.75);
    previous_settings.gunlukYemekIstisnasiSGK = Some(dec!(158.00));
    SettingsRepository::save_institution_settings(&conn, &previous_settings)?;

    let mut current_settings = settings(&current_period.id);
    current_settings.gunlukYemek = dec!(300.75);
    current_settings.gunlukYemekIstisnasiSGK = Some(dec!(300.00));
    SettingsRepository::save_institution_settings(&conn, &current_settings)?;

    let attendance = PersonelPuantaj {
        id: "meal-transition_2026-04".into(),
        personelId: personnel.id.clone(),
        donemId: current_period.id.clone(),
        gunler: HashMap::from([
            ("2026-04-15".into(), "Ç".into()),
            ("2026-04-16".into(), "Ç".into()),
            ("2026-04-17".into(), "Ç".into()),
            ("2026-04-18".into(), "Ç".into()),
        ]),
    };
    AttendanceRepository::save(&conn, &attendance)?;

    let payroll = PayrollService::calculate_payroll_for_personnel(
        &conn,
        &personnel.id,
        &current_period.id,
    )?;
    let pek = payroll.pekDetay.expect("PEK detayı hesaplanmalı");

    assert_eq!(pek.fiiliYemekGunu, 4);
    assert_eq!(pek.yemekIstisnasiTutar, dec!(916.00));
    Ok(())
}
