use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::BordroDonemi;
use bordro_programi_lib::repositories::{
    period_repo::PeriodRepository, settings_repo::SettingsRepository,
};
use bordro_programi_lib::services::period_service::PeriodService;

#[test]
fn first_period_can_be_created_without_income_parameters() {
    let conn = create_in_memory_connection().unwrap();
    let period = BordroDonemi {
        id: "2026-01".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Ocak 2026".into(),
        taxYear: 2026,
        taxMonth: 2,
    };
    PeriodService::save_period(&conn, &period).unwrap();
    let periods = PeriodRepository::get_all(&conn).unwrap();
    assert_eq!(periods.len(), 1);
    assert_eq!(periods[0].id, period.id);
    assert!(SettingsRepository::get_all_institution_settings(&conn)
        .unwrap()
        .is_empty());
}
