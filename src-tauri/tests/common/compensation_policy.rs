use bordro_programi_lib::domain::models::{
    BordroDonemi, CompensationBoundaryScope, CompensationPolicyGroup, CompensationPolicyState,
    CompensationPolicyVersion, DonemselKurumDegerleri, RoadRoundingMode,
};
use bordro_programi_lib::repositories::compensation_policy_repo::CompensationPolicyRepository;
use rusqlite::Connection;

pub fn state_from_settings(settings: &DonemselKurumDegerleri) -> CompensationPolicyState {
    CompensationPolicyState {
        gunlukTabanUcret: settings.gunlukTabanUcret,
        gunlukYemek: settings.gunlukYemek,
        birlestirilmisSosyalYardim: settings.birlestirilmisSosyalYardim,
        gunlukVasitaYol: settings.gunlukVasitaYol,
        giyimYardimi: settings.giyimYardimi,
        hizmetZammiBirimi: settings.hizmetZammiBirimi,
        isPrimiYuzde: settings.isPrimiYuzde,
        isPrimiGruplari: settings
            .isPrimiGruplari
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|group| CompensationPolicyGroup {
                id: group.id,
                ad: group.ad,
                oran: group.oran,
                aktif: group.aktif,
            })
            .collect(),
        geceCalismaPrimiYuzde: settings.geceCalismaPrimiYuzde.unwrap_or_default(),
        geceCalismaTatiliPrimiYuzde: settings.geceCalismaTatiliPrimiYuzde.unwrap_or_default(),
        ekOdeme: settings.ekOdeme,
        digerGelirVarsayilan: settings.digerGelirVarsayilan,
    }
}

pub fn seed_period_baseline(
    conn: &Connection,
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> Result<(), bordro_programi_lib::domain::DomainError> {
    if !CompensationPolicyRepository::get_versions_for_period(
        conn,
        &period.baslangicTarihi,
        &period.bitisTarihi,
    )?
    .is_empty()
    {
        return Ok(());
    }
    CompensationPolicyRepository::insert_version(
        conn,
        &CompensationPolicyVersion {
            id: format!("test-policy-{}", period.id),
            effectiveFrom: period.baslangicTarihi.clone(),
            state: state_from_settings(settings),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Explicit policy baseline for payroll fixture".into(),
            supersedesVersionId: None,
        },
    )
}

pub fn seed_period_policy_version(
    conn: &Connection,
    period: &BordroDonemi,
    version: &CompensationPolicyVersion,
) -> Result<(), bordro_programi_lib::domain::DomainError> {
    if !CompensationPolicyRepository::get_versions_for_period(
        conn,
        &period.baslangicTarihi,
        &period.bitisTarihi,
    )?
    .is_empty()
    {
        return Ok(());
    }
    CompensationPolicyRepository::insert_version(conn, version)
}
