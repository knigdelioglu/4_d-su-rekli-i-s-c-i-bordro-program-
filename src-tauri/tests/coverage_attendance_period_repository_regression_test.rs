use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, Personel, PersonelPuantaj};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use chrono::{Duration, NaiveDate};
use rusqlite::params;
use std::collections::HashMap;

fn period(id: &str, start: &str, end: &str, tax_month: i32) -> BordroDonemi {
    BordroDonemi {
        id: id.into(),
        yil: start[..4].parse().unwrap(),
        ay: start[5..7].parse().unwrap(),
        baslangicTarihi: start.into(),
        bitisTarihi: end.into(),
        donemAdi: id.into(),
        taxYear: 2026,
        taxMonth: tax_month,
    }
}

fn attendance(id: &str, days: HashMap<String, String>) -> PersonelPuantaj {
    PersonelPuantaj {
        id: "a".into(),
        personelId: "p".into(),
        donemId: id.into(),
        gunler: days,
    }
}

fn person() -> Personel {
    Personel {
        id: "p".into(),
        tcNo: "11111111111".into(),
        ad: "Test".into(),
        soyad: "Person".into(),
        grup: "1".into(),
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

fn full_days(start: &str, end: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut date = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
    while date <= end {
        result.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    result
}

#[test]
fn attendance_requires_every_15_14_calendar_day() {
    let p = period("2028-02", "2028-02-15", "2028-03-14", 3);
    let mut days = full_days(&p.baslangicTarihi, &p.bitisTarihi);
    days.remove("2028-02-29");
    let error = AttendanceRepository::validate_attendance_for_period(&attendance(&p.id, days), &p)
        .expect_err("missing leap-day attendance must be rejected");
    assert!(
        matches!(error, DomainError::ValidationError(message) if message.contains("2028-02-29"))
    );
}

#[test]
fn attendance_rejects_noncanonical_date_key_even_when_parseable() {
    let p = period("2026-07", "2026-07-15", "2026-08-14", 8);
    let mut days = full_days(&p.baslangicTarihi, &p.bitisTarihi);
    let value = days.remove("2026-07-15").unwrap();
    days.insert("2026-7-15".into(), value);
    let error = AttendanceRepository::validate_attendance_for_period(&attendance(&p.id, days), &p)
        .expect_err("noncanonical date must be rejected");
    assert!(matches!(error, DomainError::ValidationError(message) if message.contains("kanonik")));
}

#[test]
fn period_tax_chronology_rejects_nonadvancing_tax_month() {
    let conn = create_in_memory_connection().unwrap();
    let previous = period("2026-01", "2026-01-15", "2026-02-14", 2);
    PeriodRepository::save(&conn, &previous).unwrap();
    let next = period("2026-02", "2026-02-15", "2026-03-14", 2);
    let error = PeriodRepository::save(&conn, &next)
        .expect_err("tax month must advance with the 15-14 work period");
    assert!(
        matches!(error, DomainError::ValidationError(message) if message.contains("kronolojisi"))
    );
}

#[test]
fn period_tax_chronology_accepts_year_boundary() {
    let conn = create_in_memory_connection().unwrap();
    let previous = period("2026-12", "2026-12-15", "2027-01-14", 1);
    PeriodRepository::save(&conn, &previous).unwrap();
    let next = BordroDonemi {
        taxYear: 2027,
        taxMonth: 2,
        ..period("2027-01", "2027-01-15", "2027-02-14", 2)
    };
    PeriodRepository::save(&conn, &next).unwrap();
    assert_eq!(PeriodRepository::get_all(&conn).unwrap().len(), 2);
}

#[test]
fn attendance_repository_round_trips_and_returns_none_for_unknown_person_period() {
    let conn = create_in_memory_connection().unwrap();
    let p = period("2026-07", "2026-07-15", "2026-08-14", 8);
    PersonnelRepository::save(&conn, &person()).unwrap();
    PeriodRepository::save(&conn, &p).unwrap();
    let days = full_days(&p.baslangicTarihi, &p.bitisTarihi);
    let record = attendance(&p.id, days.clone());
    AttendanceRepository::save(&conn, &record).unwrap();
    let loaded = AttendanceRepository::get_by_personnel_and_period(&conn, "p", &p.id)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.gunler, days);
    assert!(
        AttendanceRepository::get_by_personnel_and_period(&conn, "missing", &p.id)
            .unwrap()
            .is_none()
    );
    assert_eq!(AttendanceRepository::get_all(&conn).unwrap().len(), 1);
}

#[test]
fn attendance_repository_fails_closed_for_malformed_json_and_unknown_period() {
    let conn = create_in_memory_connection().unwrap();
    PersonnelRepository::save(&conn, &person()).unwrap();
    let p = period("2026-07", "2026-07-15", "2026-08-14", 8);
    PeriodRepository::save(&conn, &p).unwrap();
    conn.execute(
        "INSERT INTO attendance_records (id, personnel_id, period_id, attendance_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params!["bad-json", "p", &p.id, "{", "2026-01-01T00:00:00Z"],
    ).unwrap();
    assert!(matches!(
        AttendanceRepository::get_all(&conn),
        Err(DomainError::InvalidData(_))
    ));
    conn.execute("DELETE FROM attendance_records", []).unwrap();
}

#[test]
fn attendance_save_many_rejects_empty_and_duplicate_keys() {
    let conn = create_in_memory_connection().unwrap();
    assert!(matches!(
        AttendanceRepository::save_many(&conn, &[]),
        Err(DomainError::ValidationError(_))
    ));
    let p = period("2026-07", "2026-07-15", "2026-08-14", 8);
    let days = full_days(&p.baslangicTarihi, &p.bitisTarihi);
    let a = attendance(&p.id, days);
    assert!(
        matches!(AttendanceRepository::save_many(&conn, &[a.clone(), a]), Err(DomainError::ValidationError(message)) if message.contains("birden fazla"))
    );
}

#[test]
fn period_tax_month_overlap_checks_start_or_end_month() {
    let valid_start = period("2026-07", "2026-07-15", "2026-08-14", 7);
    PeriodRepository::validate_tax_month_overlap(&valid_start).unwrap();
    let invalid = period("2026-07", "2026-07-15", "2026-08-14", 9);
    assert!(PeriodRepository::validate_tax_month_overlap(&invalid).is_err());
}
