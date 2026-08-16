use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::{
    SettingsRepository, ZAM_AYLARI_SETTING_KEY,
};
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111111".into(),
        ad: "Rapor".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "12345".into(),
        iban: "TR00".into(),
        hizmetYili: 0,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn july_period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-07".into(),
        yil: 2026,
        ay: 7,
        baslangicTarihi: "2026-07-15".into(),
        bitisTarihi: "2026-08-14".into(),
        donemAdi: "Temmuz 2026".into(),
        taxYear: 2026,
        taxMonth: 8,
    }
}

fn settings(period_id: &str, daily_wage: Decimal) -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: period_id.into(),
        gunlukTabanUcret: daily_wage,
        gunlukYemek: dec!(100),
        gunlukVasitaYol: dec!(50),
        birlestirilmisSosyalYardim: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        ..Default::default()
    }
}

fn save_annual_parameters(conn: &rusqlite::Connection) -> Result<(), Box<dyn std::error::Error>> {
    AnnualPayrollParametersRepository::save(conn, &AnnualPayrollParameters::default_for_2026())?;
    Ok(())
}

fn attendance_28_work_2_sick(personnel_id: &str) -> PersonelPuantaj {
    let mut gunler = HashMap::new();
    for day in 15..=31 {
        gunler.insert(format!("2026-07-{day:02}"), "Ç".to_string());
    }
    for day in 1..=11 {
        gunler.insert(format!("2026-08-{day:02}"), "Ç".to_string());
    }
    gunler.insert("2026-08-12".into(), "R".into());
    gunler.insert("2026-08-13".into(), "R".into());

    PersonelPuantaj {
        id: format!("{personnel_id}_2026-07"),
        personelId: personnel_id.into(),
        donemId: "2026-07".into(),
        gunler,
    }
}

fn sick_record(id: &str, personnel_id: &str, start: &str, end: &str) -> SickLeaveRecord {
    SickLeaveRecord {
        id: id.into(),
        personnelId: personnel_id.into(),
        startDate: start.into(),
        endDate: end.into(),
        createdAt: None,
        updatedAt: None,
    }
}

#[test]
fn paid_sick_days_restore_base_wage_but_not_meal_road_or_work_premium(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = person("paid-sick-basic");
    let period = july_period();

    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &period)?;
    SettingsRepository::save_institution_settings(&conn, &settings(&period.id, dec!(1000)))?;
    save_annual_parameters(&conn)?;
    AttendanceRepository::save(&conn, &attendance_28_work_2_sick(&p.id))?;
    SickLeaveRepository::save(
        &conn,
        &sick_record("paid-sick-basic-1", &p.id, "2026-08-12", "2026-08-13"),
    )?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &period.id)?;

    assert_eq!(payroll.raporluGun, Some(2));
    assert_eq!(payroll.odenenRaporluGun, Some(2));
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(30000)));
    assert_eq!(payroll.gelirler.yemek, Some(dec!(2800)));
    assert_eq!(payroll.gelirler.vasitaYol, Some(dec!(1400)));
    assert_eq!(payroll.gelirler.isPrimi, Some(dec!(2520)));
    assert_eq!(
        payroll.pekDetay.as_ref().map(|d| d.fiiliYemekGunu),
        Some(28)
    );

    Ok(())
}

#[test]
fn sixth_sick_episode_does_not_restore_base_wage() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = person("paid-sick-sixth");
    let period = july_period();

    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &period)?;
    SettingsRepository::save_institution_settings(&conn, &settings(&period.id, dec!(1000)))?;
    save_annual_parameters(&conn)?;
    AttendanceRepository::save(&conn, &attendance_28_work_2_sick(&p.id))?;

    for (idx, month) in [1, 2, 3, 4, 5].into_iter().enumerate() {
        SickLeaveRepository::save(
            &conn,
            &sick_record(
                &format!("paid-sick-sixth-prior-{idx}"),
                &p.id,
                &format!("2026-{month:02}-01"),
                &format!("2026-{month:02}-02"),
            ),
        )?;
    }
    SickLeaveRepository::save(
        &conn,
        &sick_record("paid-sick-sixth-current", &p.id, "2026-08-12", "2026-08-13"),
    )?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &period.id)?;

    assert_eq!(payroll.raporluGun, Some(2));
    assert_eq!(payroll.odenenRaporluGun, Some(0));
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(28000)));
    assert_eq!(payroll.gelirler.yemek, Some(dec!(2800)));
    assert_eq!(payroll.gelirler.vasitaYol, Some(dec!(1400)));

    Ok(())
}

#[test]
fn payable_sick_date_must_match_r_code_in_attendance() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = person("paid-sick-conflict");
    let period = july_period();

    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &period)?;

    let mut gunler = HashMap::new();
    gunler.insert("2026-08-12".into(), "Ç".into());
    gunler.insert("2026-08-13".into(), "R".into());
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: "paid-sick-conflict_2026-07".into(),
            personelId: p.id.clone(),
            donemId: period.id.clone(),
            gunler,
        },
    )?;
    SickLeaveRepository::save(
        &conn,
        &sick_record("paid-sick-conflict-1", &p.id, "2026-08-12", "2026-08-13"),
    )?;

    let result = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &period.id);
    match result {
        Err(DomainError::InvalidData(message)) => {
            assert!(message.contains("2026-08-12"));
            assert!(message.contains("puantajda 'Ç'"));
        }
        other => panic!("expected attendance/sick-leave conflict, got {other:?}"),
    }

    Ok(())
}

#[test]
fn paid_sick_wage_uses_correct_rate_on_each_side_of_raise_date(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = person("paid-sick-raise");

    let previous = BordroDonemi {
        id: "2026-02".into(),
        yil: 2026,
        ay: 2,
        baslangicTarihi: "2026-02-15".into(),
        bitisTarihi: "2026-03-14".into(),
        donemAdi: "Şubat 2026".into(),
        taxYear: 2026,
        taxMonth: 3,
    };
    let active = BordroDonemi {
        id: "2026-03".into(),
        yil: 2026,
        ay: 3,
        baslangicTarihi: "2026-03-15".into(),
        bitisTarihi: "2026-04-14".into(),
        donemAdi: "Mart 2026".into(),
        taxYear: 2026,
        taxMonth: 4,
    };

    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &previous)?;
    PeriodRepository::save(&conn, &active)?;
    SettingsRepository::save_institution_settings(&conn, &settings(&previous.id, dec!(1000)))?;
    SettingsRepository::save_institution_settings(&conn, &settings(&active.id, dec!(2000)))?;
    SettingsRepository::set_app_setting(&conn, ZAM_AYLARI_SETTING_KEY, "[4]")?;
    save_annual_parameters(&conn)?;

    let mut gunler = HashMap::new();
    gunler.insert("2026-03-30".into(), "Ç".into());
    gunler.insert("2026-03-31".into(), "R".into());
    gunler.insert("2026-04-01".into(), "R".into());
    gunler.insert("2026-04-02".into(), "Ç".into());
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: "paid-sick-raise_2026-03".into(),
            personelId: p.id.clone(),
            donemId: active.id.clone(),
            gunler,
        },
    )?;
    SickLeaveRepository::save(
        &conn,
        &sick_record("paid-sick-raise-1", &p.id, "2026-03-31", "2026-04-02"),
    )?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &active.id)?;

    // Before 1 April: 1 worked + 1 institution-paid sick day = 2 x 1,000.
    // From 1 April: 1 institution-paid sick + 1 worked day = 2 x 2,000.
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(6000)));
    assert_eq!(payroll.odenenRaporluGun, Some(2));
    assert_eq!(payroll.gelirler.yemek, Some(dec!(200)));
    assert_eq!(payroll.gelirler.vasitaYol, Some(dec!(100)));
    assert_eq!(payroll.gelirler.isPrimi, Some(dec!(270)));

    Ok(())
}

#[test]
fn duplicate_sick_record_does_not_consume_episode_quota_or_double_pay() {
    let period = july_period();
    let records = vec![
        sick_record("dup-1", "p", "2026-01-01", "2026-01-02"),
        sick_record("dup-2", "p", "2026-01-01", "2026-01-02"),
        sick_record("episode-2", "p", "2026-02-01", "2026-02-02"),
        sick_record("episode-3", "p", "2026-03-01", "2026-03-02"),
        sick_record("episode-4", "p", "2026-04-01", "2026-04-02"),
        sick_record("episode-5", "p", "2026-08-12", "2026-08-13"),
    ];

    let dates = SickLeaveService::calculate_paid_sick_dates_from_records(&records, &period);
    assert_eq!(dates.len(), 2);
    assert_eq!(dates[0].to_string(), "2026-08-12");
    assert_eq!(dates[1].to_string(), "2026-08-13");
}
