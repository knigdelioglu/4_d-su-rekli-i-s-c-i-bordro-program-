use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{
    BordroDonemi, Personel, PersonelPuantaj, SickLeaveRecord,
};
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::Datelike;
use std::collections::HashMap;

fn person() -> Personel {
    Personel {
        id: "p-delete-atomicity".into(),
        tcNo: "22222222222".into(),
        ad: "Ahmet".into(),
        soyad: "Yılmaz".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "100".into(),
        iban: "TR00".into(),
        hizmetYili: 2,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn period(id: &str, year: i32, month: i32, start: &str, end: &str, tax_month: i32) -> BordroDonemi {
    BordroDonemi {
        id: id.into(),
        donemAdi: format!("{year}-{month:02}"),
        yil: year,
        ay: month,
        baslangicTarihi: start.into(),
        bitisTarihi: end.into(),
        taxYear: year,
        taxMonth: tax_month,
    }
}

fn attendance(period_id: &str, start: &str, end: &str) -> PersonelPuantaj {
    let mut gunler = HashMap::new();
    let mut date = chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
    let end = chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
    while date <= end {
        let code = if matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
            "T"
        } else {
            "Ç"
        };
        gunler.insert(date.format("%Y-%m-%d").to_string(), code.to_string());
        date += chrono::Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("p-delete-atomicity_{period_id}"),
        personelId: "p-delete-atomicity".into(),
        donemId: period_id.into(),
        gunler,
    }
}

#[test]
fn delete_keeps_report_and_both_periods_unchanged_if_later_attendance_write_fails(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    PersonnelRepository::save(&conn, &person())?;
    PeriodRepository::save(
        &conn,
        &period("2025-12", 2025, 12, "2025-12-15", "2026-01-14", 12),
    )?;
    PeriodRepository::save(
        &conn,
        &period("2026-01", 2026, 1, "2026-01-15", "2026-02-14", 1),
    )?;
    AttendanceRepository::save(&conn, &attendance("2025-12", "2025-12-15", "2026-01-14"))?;
    AttendanceRepository::save(&conn, &attendance("2026-01", "2026-01-15", "2026-02-14"))?;

    let sick_leave = SickLeaveRecord {
        id: "sick-delete-atomicity".into(),
        personnelId: "p-delete-atomicity".into(),
        startDate: "2026-01-12".into(),
        endDate: "2026-01-17".into(),
        createdAt: None,
        updatedAt: None,
    };
    SickLeaveRepository::save_and_sync_attendance(&conn, &sick_leave)?;

    // The first period is restored before this second-period write fails.
    conn.execute_batch(
        "CREATE TRIGGER fail_second_period_attendance_update
         BEFORE UPDATE ON attendance_records
         WHEN OLD.period_id = '2026-01'
         BEGIN
             SELECT RAISE(ABORT, 'simulated second-period write failure');
         END;",
    )?;

    assert!(
        SickLeaveRepository::delete_and_sync_attendance(&conn, "sick-delete-atomicity").is_err()
    );

    assert!(SickLeaveRepository::get_by_id(&conn, "sick-delete-atomicity")?.is_some());
    for period_id in ["2025-12", "2026-01"] {
        let saved = AttendanceRepository::get_by_personnel_and_period(
            &conn,
            "p-delete-atomicity",
            period_id,
        )?
        .expect("attendance remains stored");
        for date in [
            "2026-01-12",
            "2026-01-13",
            "2026-01-14",
            "2026-01-15",
            "2026-01-16",
            "2026-01-17",
        ] {
            if saved.gunler.contains_key(date) {
                assert_eq!(saved.gunler.get(date).map(String::as_str), Some("R"));
            }
        }
    }

    Ok(())
}
