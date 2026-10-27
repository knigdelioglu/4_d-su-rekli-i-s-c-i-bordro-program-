use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, Personel, PersonelPuantaj};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use chrono::{Datelike, Duration, NaiveDate};
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
        iban: "TR330006100519786457841326".into(),
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

fn period_for_range(id: &str, start: &str, end: &str) -> BordroDonemi {
    let start = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
    BordroDonemi {
        id: id.into(),
        yil: start.year(),
        ay: start.month() as i32,
        baslangicTarihi: start.format("%Y-%m-%d").to_string(),
        bitisTarihi: end.format("%Y-%m-%d").to_string(),
        donemAdi: id.into(),
        taxYear: end.year(),
        taxMonth: end.month() as i32,
    }
}

fn days_for_range(start: &str, end: &str) -> HashMap<String, String> {
    let mut date = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
    let mut days = HashMap::new();
    while date <= end {
        days.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    days
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
fn eksik_puantaj_tarihi_reject_ve_donem_gun_sayisi_tam_olmalidir() {
    let mut gunler = valid_days(31);
    gunler.remove("2026-08-14");

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-07", gunler),
        &period(),
    );

    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn tam_puantaj_28_29_30_31_gunluk_15_14_donemlerinde_kabul_edilir() {
    let cases = [
        ("2027-02", "2027-02-15", "2027-03-14", 28),
        ("2028-02", "2028-02-15", "2028-03-14", 29),
        ("2026-06", "2026-06-15", "2026-07-14", 30),
        ("2026-07", "2026-07-15", "2026-08-14", 31),
    ];

    for (id, start, end, expected_days) in cases {
        let period = period_for_range(id, start, end);
        let days = days_for_range(start, end);
        assert_eq!(days.len(), expected_days);
        AttendanceRepository::validate_attendance_for_period(&attendance(id, days), &period)
            .unwrap_or_else(|error| panic!("{id} tam puantajı reddedildi: {error}"));
    }
}

#[test]
fn kanonik_olmayan_tarih_anahtari_reject() {
    let mut gunler = valid_days(31);
    gunler.remove("2026-07-15");
    gunler.insert("2026-7-15".into(), "Ç".into());

    let result = AttendanceRepository::validate_attendance_for_period(
        &attendance("2026-07", gunler),
        &period(),
    );

    assert!(matches!(
        result,
        Err(DomainError::ValidationError(message)) if message.contains("kanonik olmalıdır")
    ));
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

#[test]
fn bulk_intent_preserves_latest_unselected_dates_and_report_codes() {
    let conn = create_in_memory_connection().unwrap();
    PeriodRepository::save(&conn, &period()).unwrap();
    PersonnelRepository::save(&conn, &person()).unwrap();
    let mut days = valid_days(31);
    days.insert("2026-07-17".into(), "İ".into());
    days.insert("2026-07-18".into(), "R".into());
    AttendanceRepository::save(&conn, &attendance(&period().id, days)).unwrap();
    AttendanceRepository::apply_bulk_intent(
        &conn,
        &period().id,
        &["p1".into()],
        &["2026-07-16".into(), "2026-07-18".into()],
        "T",
    )
    .unwrap();
    let saved = AttendanceRepository::get_by_personnel_and_period(&conn, "p1", &period().id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.gunler["2026-07-16"], "T");
    assert_eq!(saved.gunler["2026-07-17"], "İ");
    assert_eq!(saved.gunler["2026-07-18"], "R");
    assert!(AttendanceRepository::apply_bulk_intent(
        &conn,
        &period().id,
        &["p1".into(), "missing-person".into()],
        &["2026-07-16".into()],
        "G"
    )
    .is_err());
    assert_eq!(
        AttendanceRepository::get_by_personnel_and_period(&conn, "p1", &period().id)
            .unwrap()
            .unwrap()
            .gunler,
        saved.gunler
    );
}
