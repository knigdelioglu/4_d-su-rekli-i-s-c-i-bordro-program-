use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, Personel, PersonelPuantaj, SickLeaveRecord};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::services::sick_leave_service::SickLeaveService;

fn person() -> Personel {
    Personel {
        id: "p-sick-overlap".into(),
        tcNo: "11111111111".into(),
        ad: "Rapor".into(),
        soyad: "Test".into(),
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

fn leave(id: &str, start: &str, end: &str) -> SickLeaveRecord {
    SickLeaveRecord {
        id: id.into(),
        personnelId: "p-sick-overlap".into(),
        startDate: start.into(),
        endDate: end.into(),
        createdAt: None,
        updatedAt: None,
    }
}

fn setup() -> Result<rusqlite::Connection, Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    PersonnelRepository::save(&conn, &person())?;
    Ok(conn)
}

fn assert_validation<T>(result: Result<T, DomainError>) {
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn partial_overlap_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-01-01", "2026-01-03"))?;
    assert_validation(SickLeaveRepository::save(
        &conn,
        &leave("b", "2026-01-02", "2026-01-04"),
    ));
    Ok(())
}

#[test]
fn touching_same_day_is_overlap_and_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-01-01", "2026-01-03"))?;
    assert_validation(SickLeaveRepository::save(
        &conn,
        &leave("b", "2026-01-03", "2026-01-05"),
    ));
    Ok(())
}

#[test]
fn adjacency_next_day_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-01-01", "2026-01-03"))?;
    SickLeaveRepository::save(&conn, &leave("b", "2026-01-04", "2026-01-06"))?;
    assert_eq!(
        SickLeaveRepository::get_by_personnel(&conn, "p-sick-overlap")?.len(),
        2
    );
    Ok(())
}

#[test]
fn exact_duplicate_different_id_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-02-01", "2026-02-03"))?;
    assert_validation(SickLeaveRepository::save(
        &conn,
        &leave("b", "2026-02-01", "2026-02-03"),
    ));
    Ok(())
}

#[test]
fn updating_same_record_excludes_itself_from_overlap_check(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-03-01", "2026-03-10"))?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-03-03", "2026-03-05"))?;
    let saved = SickLeaveRepository::get_by_id(&conn, "a")?.expect("record exists");
    assert_eq!(saved.startDate, "2026-03-03");
    assert_eq!(saved.endDate, "2026-03-05");
    Ok(())
}

#[test]
fn first_five_episode_rule_and_sixth_episode_behavior_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    for i in 0..6u32 {
        let day = 1 + i * 4;
        let start = format!("2026-04-{day:02}");
        let end = format!("2026-04-{:02}", day + 1);
        SickLeaveRepository::save(&conn, &leave(&format!("e{i}"), &start, &end))?;
    }

    // İlk dört rapor 15 Nisan'dan önce kotayı tüketir. Gerçek 15–14 bordro
    // döneminde 5. raporun (17–18 Nisan) iki günü ödenir; 6. raporun
    // (21–22 Nisan) günleri yıllık ilk-beş-rapor kuralı nedeniyle ödenmez.
    let period = BordroDonemi {
        id: "2026-04".into(),
        yil: 2026,
        ay: 4,
        baslangicTarihi: "2026-04-15".into(),
        bitisTarihi: "2026-05-14".into(),
        donemAdi: "Nisan 2026".into(),
        taxYear: 2026,
        taxMonth: 5,
    };
    let paid =
        SickLeaveService::calculate_paid_sick_dates_for_period(&conn, "p-sick-overlap", &period)?;
    let paid_text = paid
        .iter()
        .map(|date| date.format("%Y-%m-%d").to_string())
        .collect::<Vec<_>>();
    assert_eq!(paid_text, vec!["2026-04-17", "2026-04-18"]);
    Ok(())
}

#[test]
fn one_episode_may_cross_calendar_year_without_being_split_or_rejected(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("cross", "2026-12-31", "2027-01-02"))?;
    let stored = SickLeaveRepository::get_by_id(&conn, "cross")?.expect("record exists");
    assert_eq!(stored.startDate, "2026-12-31");
    assert_eq!(stored.endDate, "2027-01-02");
    Ok(())
}

#[test]
fn overlapping_atomic_save_leaves_record_and_attendance_unchanged(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    let period = BordroDonemi {
        id: "2027-03".into(),
        yil: 2027,
        ay: 3,
        baslangicTarihi: "2027-03-15".into(),
        bitisTarihi: "2027-04-14".into(),
        donemAdi: "Mart 2027".into(),
        taxYear: 2027,
        taxMonth: 4,
    };
    PeriodRepository::save(&conn, &period)?;
    let mut days = std::collections::HashMap::new();
    days.insert("2027-03-19".into(), "Ç".into());
    days.insert("2027-03-20".into(), "R".into());
    days.insert("2027-03-21".into(), "T".into());
    let attendance = PersonelPuantaj {
        id: "p-sick-overlap_2027-03".into(),
        personelId: "p-sick-overlap".into(),
        donemId: period.id.clone(),
        gunler: days,
    };
    AttendanceRepository::save(&conn, &attendance)?;
    SickLeaveRepository::save_and_sync_attendance(&conn, &leave("a", "2027-03-20", "2027-03-20"))?;

    let before = AttendanceRepository::get_by_personnel_and_period(
        &conn,
        "p-sick-overlap",
        &period.id,
    )?
    .expect("attendance exists");
    assert_validation(SickLeaveRepository::save_and_sync_attendance(
        &conn,
        &leave("b", "2027-03-19", "2027-03-21"),
    ));

    let after = AttendanceRepository::get_by_personnel_and_period(
        &conn,
        "p-sick-overlap",
        &period.id,
    )?
    .expect("attendance remains");
    assert_eq!(after.gunler, before.gunler);
    assert_eq!(SickLeaveRepository::get_by_personnel(&conn, "p-sick-overlap")?.len(), 1);
    assert!(SickLeaveRepository::get_by_id(&conn, "b")?.is_none());
    Ok(())
}
