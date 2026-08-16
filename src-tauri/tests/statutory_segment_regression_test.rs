use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{
    AnnualPayrollParameters, BordroDonemi, DonemselKurumDegerleri, Personel, PersonelPuantaj,
    StatutoryParameterSegment,
};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::payroll_service::{
    resolve_statutory_snapshot_for_period, PayrollService,
};
use chrono::{Duration, NaiveDate};
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2025-12".into(),
        yil: 2025,
        ay: 12,
        baslangicTarihi: "2025-12-15".into(),
        bitisTarihi: "2026-01-14".into(),
        donemAdi: "Aralık 2025".into(),
        taxYear: 2026,
        taxMonth: 1,
    }
}

fn full_attendance(period: &BordroDonemi) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    let mut date = start;
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".to_string());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: "p-segment_2025-12".into(),
        personelId: "p-segment".into(),
        donemId: period.id.clone(),
        gunler,
    }
}

fn base_settings() -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: "2025-12".into(),
        gunlukAsgariUcret: Some(dec!(100)),
        pekTavanKatsayisi: Some(dec!(10)),
        gunlukYemekIstisnasiSGK: Some(dec!(20)),
        gunlukYemekIstisnasiGV: Some(dec!(30)),
        statutoryParameterSegments: None,
        ..Default::default()
    }
}

fn segment(effective_from: &str) -> StatutoryParameterSegment {
    StatutoryParameterSegment {
        effectiveFrom: effective_from.into(),
        gunlukAsgariUcret: None,
        pekTavanKatsayisi: None,
        gunlukYemekIstisnasiSGK: None,
        gunlukYemekIstisnasiGV: None,
    }
}

#[test]
fn no_segment_keeps_single_baseline_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &base_settings())?;
    assert_eq!(snapshot.sgkPrimGunSayisi, 30);
    assert_eq!(snapshot.pekAltSinir, dec!(3000));
    assert_eq!(snapshot.pekUstSinir, dec!(30000));
    assert_eq!(snapshot.segments.len(), 1);
    Ok(())
}

#[test]
fn january_first_change_splits_public_15_14_pek_limits_as_16_plus_14(
) -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2026-01-01");
    change.gunlukAsgariUcret = Some(dec!(200));
    change.pekTavanKatsayisi = Some(dec!(9));
    change.gunlukYemekIstisnasiSGK = Some(dec!(25));
    settings.statutoryParameterSegments = Some(vec![change]);

    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.segments.len(), 2);
    assert_eq!(snapshot.segments[0].sgkPrimGunSayisi, 16);
    assert_eq!(snapshot.segments[1].sgkPrimGunSayisi, 14);
    assert_eq!(snapshot.pekAltSinir, dec!(4400));
    assert_eq!(snapshot.pekUstSinir, dec!(41200));
    // Meal exemptions follow real worked calendar dates, not virtual SGK days.
    assert_eq!(snapshot.sgkYemekIstisnasiToplam, dec!(690));
    // GV meal exemption did not change: 31 actual worked dates × 30.
    assert_eq!(snapshot.gvYemekIstisnasiToplam, dec!(930));
    assert_eq!(snapshot.gvReferansGunlukAsgariUcret, dec!(200));
    Ok(())
}

#[test]
fn segment_on_first_day_replaces_baseline_for_whole_period(
) -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2025-12-15");
    change.gunlukAsgariUcret = Some(dec!(200));
    settings.statutoryParameterSegments = Some(vec![change]);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.segments.len(), 1);
    assert_eq!(snapshot.pekAltSinir, dec!(6000));
    Ok(())
}

#[test]
fn segment_on_last_day_affects_only_last_sgk_day() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2026-01-14");
    change.gunlukAsgariUcret = Some(dec!(200));
    settings.statutoryParameterSegments = Some(vec![change]);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.segments[0].sgkPrimGunSayisi, 29);
    assert_eq!(snapshot.segments[1].sgkPrimGunSayisi, 1);
    assert_eq!(snapshot.pekAltSinir, dec!(3100));
    Ok(())
}

#[test]
fn sgk_and_gv_meal_exemptions_change_independently() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2026-01-01");
    change.gunlukYemekIstisnasiSGK = Some(dec!(40));
    settings.statutoryParameterSegments = Some(vec![change]);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.sgkYemekIstisnasiToplam, dec!(900));
    assert_eq!(snapshot.gvYemekIstisnasiToplam, dec!(930));
    Ok(())
}

#[test]
fn out_of_period_segment_rejected_on_settings_save() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = period();
    PeriodRepository::save(&conn, &p)?;
    let mut settings = base_settings();
    let mut bad = segment("2025-12-14");
    bad.gunlukAsgariUcret = Some(dec!(150));
    settings.statutoryParameterSegments = Some(vec![bad]);
    assert!(matches!(
        SettingsRepository::save_institution_settings(&conn, &settings),
        Err(DomainError::ValidationError(_))
    ));
    Ok(())
}

#[test]
fn duplicate_or_unsorted_segment_dates_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = period();
    PeriodRepository::save(&conn, &p)?;
    let mut settings = base_settings();
    let mut a = segment("2026-01-05");
    a.gunlukAsgariUcret = Some(dec!(150));
    let mut b = segment("2026-01-01");
    b.gunlukAsgariUcret = Some(dec!(160));
    settings.statutoryParameterSegments = Some(vec![a, b]);
    assert!(matches!(
        SettingsRepository::save_institution_settings(&conn, &settings),
        Err(DomainError::ValidationError(_))
    ));
    Ok(())
}

fn person() -> Personel {
    Personel {
        id: "p-segment".into(),
        tcNo: "22222222222".into(),
        ad: "Segment".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "2".into(),
        iban: "TR00".into(),
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

#[test]
fn payroll_persists_resolved_statutory_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = period();
    PeriodRepository::save(&conn, &p)?;
    PersonnelRepository::save(&conn, &person())?;
    AttendanceRepository::save(&conn, &full_attendance(&p))?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;

    let mut settings = base_settings();
    let mut change = segment("2026-01-01");
    change.gunlukAsgariUcret = Some(dec!(200));
    settings.statutoryParameterSegments = Some(vec![change]);
    SettingsRepository::save_institution_settings(&conn, &settings)?;

    let calculated = PayrollService::calculate_payroll_for_personnel(&conn, "p-segment", &p.id)?;
    assert_eq!(
        calculated
            .statutorySnapshot
            .as_ref()
            .expect("calculated snapshot")
            .pekAltSinir,
        dec!(4400)
    );
    let reloaded = PayrollRepository::get_all(&conn)?;
    let persisted = reloaded
        .iter()
        .find(|payroll| payroll.id == calculated.id)
        .and_then(|payroll| payroll.statutorySnapshot.as_ref())
        .expect("persisted statutory snapshot");
    assert_eq!(persisted.pekAltSinir, dec!(4400));
    assert_eq!(persisted.segments.len(), 2);
    Ok(())
}
