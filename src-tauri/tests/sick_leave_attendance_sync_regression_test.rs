use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, Personel, PersonelPuantaj, SickLeaveRecord};
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::Datelike;
use std::collections::HashMap;

fn test_person() -> Personel {
    Personel {
        id: "p-sync-test".into(),
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

fn period_dec_2025() -> BordroDonemi {
    BordroDonemi {
        id: "2025-12".into(),
        donemAdi: "Aralık 2025".into(),
        yil: 2025,
        ay: 12,
        baslangicTarihi: "2025-12-15".into(),
        bitisTarihi: "2026-01-14".into(),
        taxYear: 2026,
        taxMonth: 1,
    }
}

fn period_jan_2026() -> BordroDonemi {
    BordroDonemi {
        id: "2026-01".into(),
        donemAdi: "Ocak 2026".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        taxYear: 2026,
        taxMonth: 2,
    }
}

#[test]
fn sick_leave_syncs_puantaj_across_15_14_boundary_and_preserves_manual_changes() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    PersonnelRepository::save(&conn, &test_person())?;
    PeriodRepository::save(&conn, &period_dec_2025())?;
    PeriodRepository::save(&conn, &period_jan_2026())?;

    // 1. Initial attendance for 2025-12 with Ç and T
    let mut gunler_dec = HashMap::new();
    let mut curr = chrono::NaiveDate::from_ymd_opt(2025, 12, 15).unwrap();
    let end_dec = chrono::NaiveDate::from_ymd_opt(2026, 1, 14).unwrap();
    while curr <= end_dec {
        let code = if curr.weekday() == chrono::Weekday::Sat || curr.weekday() == chrono::Weekday::Sun {
            "T"
        } else {
            "Ç"
        };
        gunler_dec.insert(curr.format("%Y-%m-%d").to_string(), code.to_string());
        curr += chrono::Duration::days(1);
    }
    let att_dec = PersonelPuantaj {
        id: "p-sync-test_2025-12".into(),
        personelId: "p-sync-test".into(),
        donemId: "2025-12".into(),
        gunler: gunler_dec,
    };
    AttendanceRepository::save(&conn, &att_dec)?;

    // 2. Initial attendance for 2026-01 with Ç and T
    let mut gunler_jan = HashMap::new();
    let mut curr = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let end_jan = chrono::NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
    while curr <= end_jan {
        let code = if curr.weekday() == chrono::Weekday::Sat || curr.weekday() == chrono::Weekday::Sun {
            "T"
        } else {
            "Ç"
        };
        gunler_jan.insert(curr.format("%Y-%m-%d").to_string(), code.to_string());
        curr += chrono::Duration::days(1);
    }
    let att_jan = PersonelPuantaj {
        id: "p-sync-test_2026-01".into(),
        personelId: "p-sync-test".into(),
        donemId: "2026-01".into(),
        gunler: gunler_jan,
    };
    AttendanceRepository::save(&conn, &att_jan)?;

    // 3. Save a sick leave spanning across the 15-14 boundary: Jan 12 to Jan 17
    // Jan 12, 13, 14 fall in 2025-12 period.
    // Jan 15, 16, 17 fall in 2026-01 period.
    let sick = SickLeaveRecord {
        id: "sick-cross".into(),
        personnelId: "p-sync-test".into(),
        startDate: "2026-01-12".into(),
        endDate: "2026-01-17".into(),
        createdAt: None,
        updatedAt: None,
    };
    SickLeaveRepository::save_and_sync_attendance(&conn, &sick)?;

    // Verify 2025-12 attendance: Jan 12, 13, 14 must be 'R'
    let dec_updated = AttendanceRepository::get_by_personnel_and_period(&conn, "p-sync-test", "2025-12")?
        .expect("attendance exists");
    assert_eq!(dec_updated.gunler.get("2026-01-11").map(|s| s.as_str()), Some("T")); // Sunday before
    assert_eq!(dec_updated.gunler.get("2026-01-12").map(|s| s.as_str()), Some("R"));
    assert_eq!(dec_updated.gunler.get("2026-01-13").map(|s| s.as_str()), Some("R"));
    assert_eq!(dec_updated.gunler.get("2026-01-14").map(|s| s.as_str()), Some("R"));

    // Verify 2026-01 attendance: Jan 15, 16, 17 must be 'R'
    let jan_updated = AttendanceRepository::get_by_personnel_and_period(&conn, "p-sync-test", "2026-01")?
        .expect("attendance exists");
    assert_eq!(jan_updated.gunler.get("2026-01-15").map(|s| s.as_str()), Some("R"));
    assert_eq!(jan_updated.gunler.get("2026-01-16").map(|s| s.as_str()), Some("R"));
    assert_eq!(jan_updated.gunler.get("2026-01-17").map(|s| s.as_str()), Some("R"));
    assert_eq!(jan_updated.gunler.get("2026-01-18").map(|s| s.as_str()), Some("T")); // Sunday after

    // 4. Manual modification: user goes to puantaj and changes 2026-01-16 to 'Ç'
    let mut modified_jan = jan_updated.clone();
    modified_jan.gunler.insert("2026-01-16".into(), "Ç".into());
    AttendanceRepository::save(&conn, &modified_jan)?;

    // Verify modification saved
    let jan_check = AttendanceRepository::get_by_personnel_and_period(&conn, "p-sync-test", "2026-01")?
        .expect("attendance exists");
    assert_eq!(jan_check.gunler.get("2026-01-16").map(|s| s.as_str()), Some("Ç"));

    // 5. Delete the sick leave:
    // Dates that are still 'R' must be restored to default ('Ç' for weekdays, 'T' for weekends).
    // The manually modified date '2026-01-16' MUST NOT be overwritten ("kullanıcının sonradan değiştirdiği puantajı ezme").
    SickLeaveRepository::delete_and_sync_attendance(&conn, "sick-cross")?;

    let dec_after_delete = AttendanceRepository::get_by_personnel_and_period(&conn, "p-sync-test", "2025-12")?
        .expect("attendance exists");
    // Jan 12, 13, 14 were Monday, Tuesday, Wednesday -> restored to 'Ç'
    assert_eq!(dec_after_delete.gunler.get("2026-01-12").map(|s| s.as_str()), Some("Ç"));
    assert_eq!(dec_after_delete.gunler.get("2026-01-13").map(|s| s.as_str()), Some("Ç"));
    assert_eq!(dec_after_delete.gunler.get("2026-01-14").map(|s| s.as_str()), Some("Ç"));

    let jan_after_delete = AttendanceRepository::get_by_personnel_and_period(&conn, "p-sync-test", "2026-01")?
        .expect("attendance exists");
    // Jan 15 was Thursday, still 'R' -> restored to 'Ç'
    assert_eq!(jan_after_delete.gunler.get("2026-01-15").map(|s| s.as_str()), Some("Ç"));
    // Jan 16 was manually modified to 'Ç' -> MUST REMAIN 'Ç'!
    assert_eq!(jan_after_delete.gunler.get("2026-01-16").map(|s| s.as_str()), Some("Ç"));
    // Jan 17 was Saturday, still 'R' -> restored to 'T'
    assert_eq!(jan_after_delete.gunler.get("2026-01-17").map(|s| s.as_str()), Some("T"));

    Ok(())
}
