use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::compensation_policy_repo::CompensationPolicyRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::repositories::tax_opening_repo::TaxOpeningRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
use chrono::{Duration, NaiveDate};
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
        iban: "TR330006100519786457841326".into(),
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

fn policy_version(
    id: &str,
    effective_from: &str,
    daily_wage: Decimal,
    supersedes: Option<&str>,
) -> CompensationPolicyVersion {
    policy_version_with_rates(
        id,
        effective_from,
        daily_wage,
        dec!(100),
        dec!(50),
        supersedes,
    )
}

fn policy_version_with_rates(
    id: &str,
    effective_from: &str,
    daily_wage: Decimal,
    daily_meal: Decimal,
    daily_road: Decimal,
    supersedes: Option<&str>,
) -> CompensationPolicyVersion {
    CompensationPolicyVersion {
        id: id.into(),
        effectiveFrom: effective_from.into(),
        state: CompensationPolicyState {
            gunlukTabanUcret: daily_wage,
            gunlukYemek: daily_meal,
            birlestirilmisSosyalYardim: dec!(0),
            gunlukVasitaYol: daily_road,
            giyimYardimi: dec!(0),
            hizmetZammiBirimi: dec!(0),
            isPrimiYuzde: None,
            isPrimiGruplari: vec![CompensationPolicyGroup {
                id: "1. Grup".into(),
                ad: "1. Grup".into(),
                oran: dec!(9),
                aktif: true,
            }],
            geceCalismaPrimiYuzde: dec!(0),
            geceCalismaTatiliPrimiYuzde: dec!(0),
            ekOdeme: Some(dec!(0)),
            digerGelirVarsayilan: Some(dec!(0)),
        },
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
        createdAt: "2026-01-01T00:00:00Z".into(),
        source: "TEST_FIXTURE".into(),
        reason: format!("{id} test policy"),
        supersedesVersionId: supersedes.map(str::to_owned),
    }
}

fn save_policy_baseline(
    conn: &rusqlite::Connection,
    effective_from: &str,
    daily_wage: Decimal,
) -> Result<(), Box<dyn std::error::Error>> {
    CompensationPolicyRepository::insert_version(
        conn,
        &policy_version("paid-sick-baseline", effective_from, daily_wage, None),
    )?;
    Ok(())
}

fn save_annual_parameters(conn: &rusqlite::Connection) -> Result<(), Box<dyn std::error::Error>> {
    AnnualPayrollParametersRepository::save(conn, &AnnualPayrollParameters::default_for_2026())?;
    Ok(())
}

fn save_tax_opening(
    conn: &rusqlite::Connection,
    personnel_id: &str,
    period: &BordroDonemi,
) -> Result<(), Box<dyn std::error::Error>> {
    TaxOpeningRepository::save(
        conn,
        &PersonelTaxOpening {
            id: format!("{personnel_id}_{}", period.taxYear),
            personnelId: personnel_id.into(),
            year: period.taxYear,
            gvCumulativeOpening: Some(dec!(0)),
            effectiveFromPeriodId: Some(period.id.clone()),
            asgariGvCumulativeOpening: Some(dec!(0)),
            asgariGvEffectiveFromPeriodId: Some(period.id.clone()),
            createdAt: None,
            updatedAt: None,
        },
    )?;
    Ok(())
}

fn attendance_period(
    personnel_id: &str,
    period: &BordroDonemi,
    default_code: &str,
) -> PersonelPuantaj {
    let mut gunler = HashMap::new();
    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    for offset in 0..=(end - start).num_days() {
        let date = start + Duration::days(offset);
        gunler.insert(
            date.format("%Y-%m-%d").to_string(),
            default_code.to_string(),
        );
    }
    PersonelPuantaj {
        id: format!("{personnel_id}_{}", period.id),
        personelId: personnel_id.into(),
        donemId: period.id.clone(),
        gunler,
    }
}

fn set_attendance(attendance: &mut PersonelPuantaj, date: &str, code: &str) {
    attendance.gunler.insert(date.into(), code.into());
}

fn attendance_28_work_2_sick(personnel_id: &str) -> PersonelPuantaj {
    let period = july_period();
    let mut attendance = attendance_period(personnel_id, &period, "R");
    for day in 15..=31 {
        set_attendance(&mut attendance, &format!("2026-07-{day:02}"), "Ç");
    }
    for day in 1..=11 {
        set_attendance(&mut attendance, &format!("2026-08-{day:02}"), "Ç");
    }
    set_attendance(&mut attendance, "2026-08-12", "R");
    set_attendance(&mut attendance, "2026-08-13", "R");
    set_attendance(&mut attendance, "2026-08-14", "T");
    attendance
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
    save_policy_baseline(&conn, &period.baslangicTarihi, dec!(1000))?;
    save_annual_parameters(&conn)?;
    save_tax_opening(&conn, &p.id, &period)?;
    AttendanceRepository::save(&conn, &attendance_28_work_2_sick(&p.id))?;
    SickLeaveRepository::save(
        &conn,
        &sick_record("paid-sick-basic-1", &p.id, "2026-08-12", "2026-08-13"),
    )?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &period.id)?;

    assert_eq!(payroll.raporluGun, Some(2));
    assert_eq!(payroll.odenenRaporluGun, Some(2));
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(31000)));
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
    save_policy_baseline(&conn, &period.baslangicTarihi, dec!(1000))?;
    save_annual_parameters(&conn)?;
    save_tax_opening(&conn, &p.id, &period)?;
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
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(29000)));
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
    save_policy_baseline(&conn, &period.baslangicTarihi, dec!(1000))?;

    let mut attendance = attendance_period(&p.id, &period, "R");
    set_attendance(&mut attendance, "2026-08-12", "Ç");
    set_attendance(&mut attendance, "2026-08-13", "R");
    AttendanceRepository::save(&conn, &attendance)?;
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
    CompensationPolicyRepository::insert_version(
        &conn,
        &policy_version("paid-sick-baseline", "2026-02-15", dec!(1000), None),
    )?;
    CompensationPolicyRepository::insert_version(
        &conn,
        &policy_version_with_rates(
            "paid-sick-april-rate",
            "2026-04-01",
            dec!(2000),
            dec!(200),
            dec!(50),
            None,
        ),
    )?;
    save_annual_parameters(&conn)?;
    save_tax_opening(&conn, &p.id, &active)?;

    let mut attendance = attendance_period(&p.id, &active, "Ç");
    for date in [
        "2026-03-15",
        "2026-03-22",
        "2026-03-29",
        "2026-04-05",
        "2026-04-12",
    ] {
        set_attendance(&mut attendance, date, "T");
    }
    set_attendance(&mut attendance, "2026-03-31", "R");
    set_attendance(&mut attendance, "2026-04-01", "R");
    set_attendance(&mut attendance, "2026-04-02", "Ç");
    AttendanceRepository::save(&conn, &attendance)?;
    SickLeaveRepository::save(
        &conn,
        &sick_record("paid-sick-raise-1", &p.id, "2026-03-31", "2026-04-02"),
    )?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &active.id)?;

    // Before 1 April: 17 entitlement days × 1,000. From 1 April:
    // 14 entitlement days × 2,000. One paid sick day falls on each side.
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(45000)));
    assert_eq!(payroll.odenenRaporluGun, Some(2));
    // 13 worked days before the raise and 11 after it receive meal, road,
    // and work-premium treatment. The two paid R days receive base wage only.
    assert_eq!(payroll.gelirler.yemek, Some(dec!(3500)));
    assert_eq!(payroll.gelirler.vasitaYol, Some(dec!(1200)));
    assert_eq!(payroll.gelirler.isPrimi, Some(dec!(3150)));

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
