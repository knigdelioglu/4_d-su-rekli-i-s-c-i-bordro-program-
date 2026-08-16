use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, Personel, SickLeaveRecord};
use bordro_programi_lib::domain::DomainError;
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
    let period = BordroDonemi {
        id: "2026-all".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-01".into(),
        bitisTarihi: "2026-12-31".into(),
        donemAdi: "2026".into(),
        taxYear: 2026,
        taxMonth: 12,
    };
    let paid =
        SickLeaveService::calculate_paid_sick_dates_for_period(&conn, "p-sick-overlap", &period)?;
    assert_eq!(paid.len(), 10);
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
