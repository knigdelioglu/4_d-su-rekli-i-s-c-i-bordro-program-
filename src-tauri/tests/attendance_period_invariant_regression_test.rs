use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, Personel, PersonelPuantaj};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use chrono::{Duration, NaiveDate};
use rusqlite::params;
use std::collections::HashMap;

fn period() -> BordroDonemi {
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

fn attendance(period_id: &str, gunler: HashMap<String, String>) -> PersonelPuantaj {
    PersonelPuantaj {
        id: format!("p1_{period_id}"),
        personelId: "p1".into(),
        donemId: period_id.into(),
        gunler,
    }
}

fn person() -> Personel {
    Personel {
        id: "p1".into(),
        tcNo: "11111111111".into(),
        ad: "Test".into(),
        soyad: "Personel".into(),
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
        kesintiler: None,
    }
}

fn valid_days(count: i64) -> HashMap<String, String> {
    let start = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
    (0..count)
        .map(|offset| {
            (
                (start + Duration::days(offset))
                    .format("%Y-%m-%d")
                    .to_string(),
                "Ç".to_string(),
            )
        })
        .collect()
}

#[test]
fn period_startindan_bir_gun_once_reject() {
    let mut gunler = HashMap::new();
    gunler.insert("2026-07-14".into(), "Ç".into());

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-07", gunler),
        &period(),
    );

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn period_bitiminden_bir_gun_sonra_reject() {
    let mut gunler = HashMap::new();
    gunler.insert("2026-08-15".into(), "Ç".into());

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-07", gunler),
        &period(),
    );

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn gecersiz_takvim_tarihi_reject() {
    let mut gunler = HashMap::new();
    gunler.insert("2026-07-32".into(), "Ç".into());

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-07", gunler),
        &period(),
    );

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn otuz_bir_gunluk_periodun_tum_gecerli_tarihleri_accept() {
    let gunler = valid_days(31);
    assert_eq!(gunler.len(), 31);

    AttendanceRepository::validate_attendance_for_period(&attendance("2026-07", gunler), &period())
        .expect("31 günlük gerçek dönem aralığı kabul edilmeli");
}

#[test]
fn otuz_ikinci_period_disi_tarih_reject() {
    let gunler = valid_days(32);
    assert_eq!(gunler.len(), 32);

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-07", gunler),
        &period(),
    );

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn bilinmeyen_puantaj_kodu_reject() {
    let mut gunler = HashMap::new();
    gunler.insert("2026-07-15".into(), "X".into());

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-07", gunler),
        &period(),
    );

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn attendance_period_id_eslesmezse_reject() {
    let mut gunler = HashMap::new();
    gunler.insert("2026-07-15".into(), "Ç".into());

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-08", gunler),
        &period(),
    );

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn repository_save_period_disi_tarihi_persist_etmez() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    PersonnelRepository::save(&conn, &person())?;
    PeriodRepository::save(&conn, &period())?;

    let mut gunler = HashMap::new();
    gunler.insert("2026-08-15".into(), "Ç".into());
    let result = AttendanceRepository::save(&conn, &attendance("2026-07", gunler));

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM attendance_records", [], |row| {
        row.get(0)
    })?;
    assert_eq!(count, 0);
    Ok(())
}

#[test]
fn raw_sql_ile_bozuk_kayit_okunsa_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    PersonnelRepository::save(&conn, &person())?;
    PeriodRepository::save(&conn, &period())?;

    conn.execute(
        "INSERT INTO attendance_records (id, personnel_id, period_id, attendance_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            "raw_bad",
            "p1",
            "2026-07",
            r#"{"2026-08-15":"Ç"}"#,
            "2026-08-16T00:00:00Z"
        ],
    )?;

    let result = AttendanceRepository::get_by_personnel_and_period(&conn, "p1", "2026-07");
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
    Ok(())
}

#[test]
fn payroll_service_bozuk_raw_puantajdan_ucret_uretmez() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    PersonnelRepository::save(&conn, &person())?;
    PeriodRepository::save(&conn, &period())?;

    conn.execute(
        "INSERT INTO attendance_records (id, personnel_id, period_id, attendance_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            "raw_bad_payroll",
            "p1",
            "2026-07",
            r#"{"2026-08-15":"Ç"}"#,
            "2026-08-16T00:00:00Z"
        ],
    )?;

    let result = PayrollService::calculate_payroll_for_personnel(&conn, "p1", "2026-07");
    assert!(matches!(result, Err(DomainError::ValidationError(_))));

    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM payroll_records", [], |row| row.get(0))?;
    assert_eq!(count, 0, "bozuk puantaj bordro üretmemeli");
    Ok(())
}
