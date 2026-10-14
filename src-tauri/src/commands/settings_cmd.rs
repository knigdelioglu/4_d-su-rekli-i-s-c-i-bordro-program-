use crate::db::DbState;
use crate::domain::{DomainError, Result};
use crate::repositories::settings_repo::SettingsRepository;
use rusqlite::Connection;
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
pub fn get_institution_settings(
    db: State<'_, DbState>,
) -> Result<HashMap<String, payroll_core::PayrollPeriodInstitutionSettings>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::get_all_active_institution_settings(&conn)
}

#[tauri::command]
pub fn save_institution_settings(
    db: State<'_, DbState>,
    settings: payroll_core::PayrollPeriodInstitutionSettings,
) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::save_active_institution_settings(&conn, &settings)
}

#[tauri::command]
pub fn get_app_setting(db: State<'_, DbState>, key: String) -> Result<Option<String>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::get_app_setting(&conn, &key)
}

#[tauri::command]
pub fn set_app_setting(db: State<'_, DbState>, key: String, value: String) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;

    set_app_setting_with_connection(&conn, &key, &value)
}

pub fn set_app_setting_with_connection(conn: &Connection, key: &str, value: &str) -> Result<()> {
    if key == "is_primi_gruplari" {
        let groups: Vec<payroll_core::IsPrimiGrupItem> =
            serde_json::from_str(value).map_err(|error| {
                DomainError::InvalidData(format!(
                    "İş primi grupları ayarı geçersiz JSON içeriyor: {error}"
                ))
            })?;
        return crate::services::compensation_policy_service::CompensationPolicyService::
            save_work_premium_groups(conn, &groups);
    }

    SettingsRepository::set_app_setting(conn, key, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_in_memory_connection;
    use crate::domain::models::{
        BordroDonemi, CompensationBoundaryScope, CompensationPolicyGroup, CompensationPolicyState,
        CompensationPolicyVersion, RoadRoundingMode,
    };
    use crate::repositories::compensation_policy_repo::CompensationPolicyRepository;
    use crate::repositories::period_repo::PeriodRepository;
    use crate::repositories::settings_repo::SettingsRepository;

    fn state() -> CompensationPolicyState {
        CompensationPolicyState {
            gunlukTabanUcret: "2000".parse().unwrap(),
            gunlukYemek: "250".parse().unwrap(),
            birlestirilmisSosyalYardim: "3000".parse().unwrap(),
            gunlukVasitaYol: "75".parse().unwrap(),
            giyimYardimi: "125".parse().unwrap(),
            hizmetZammiBirimi: "10".parse().unwrap(),
            isPrimiYuzde: None,
            isPrimiGruplari: vec![CompensationPolicyGroup {
                id: "1. Grup".into(),
                ad: "Grup 1".into(),
                oran: "9".parse().unwrap(),
                aktif: true,
            }],
            geceCalismaPrimiYuzde: "0".parse().unwrap(),
            geceCalismaTatiliPrimiYuzde: "0".parse().unwrap(),
            ekOdeme: Some("0".parse().unwrap()),
            digerGelirVarsayilan: Some("0".parse().unwrap()),

            socialBenefitAccrualPolicies: None,
        }
    }

    #[test]
    fn work_premium_group_setting_appends_an_immutable_policy_version() {
        let conn = create_in_memory_connection().unwrap();
        PeriodRepository::save(
            &conn,
            &BordroDonemi {
                id: "2026-01".into(),
                yil: 2026,
                ay: 1,
                baslangicTarihi: "2026-01-15".into(),
                bitisTarihi: "2026-02-14".into(),
                donemAdi: "Ocak 2026".into(),
                taxYear: 2026,
                taxMonth: 2,
            },
        )
        .unwrap();
        SettingsRepository::set_app_setting(&conn, "active_period_id", "2026-01").unwrap();
        let original = CompensationPolicyVersion {
            id: "work-premium-baseline".into(),
            effectiveFrom: "2026-01-15".into(),
            state: state(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PeriodTotal,
            createdAt: "2026-01-15T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Test baseline".into(),
            supersedesVersionId: None,
        };
        CompensationPolicyRepository::insert_version(&conn, &original).unwrap();
        let groups = r#"[{"id":"1. Grup","ad":"Grup 1","oran":"11","aktif":true}]"#;

        let result = set_app_setting_with_connection(&conn, "is_primi_gruplari", groups);
        assert!(
            result.is_ok(),
            "group save should not attempt an immutable UPDATE: {result:?}"
        );

        let versions = CompensationPolicyRepository::get_all_versions(&conn).unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(
            versions[0], original,
            "the historical version must remain byte-for-byte typed-identical"
        );
        assert_eq!(versions[1].effectiveFrom, original.effectiveFrom);
        assert_eq!(
            versions[1].supersedesVersionId.as_deref(),
            Some(original.id.as_str())
        );
        assert_eq!(versions[1].state.isPrimiGruplari[0].oran.to_string(), "11");
        assert_eq!(
            SettingsRepository::get_app_setting(&conn, "is_primi_gruplari")
                .unwrap()
                .as_deref(),
            Some(groups)
        );
    }

    #[test]
    fn work_premium_group_setting_rolls_back_policy_when_mirror_update_fails() {
        let conn = create_in_memory_connection().unwrap();
        PeriodRepository::save(
            &conn,
            &BordroDonemi {
                id: "2026-01".into(),
                yil: 2026,
                ay: 1,
                baslangicTarihi: "2026-01-15".into(),
                bitisTarihi: "2026-02-14".into(),
                donemAdi: "Ocak 2026".into(),
                taxYear: 2026,
                taxMonth: 2,
            },
        )
        .unwrap();
        SettingsRepository::set_app_setting(&conn, "active_period_id", "2026-01").unwrap();
        let original_groups = r#"[{"id":"1. Grup","ad":"Grup 1","oran":"9","aktif":true}]"#;
        SettingsRepository::set_app_setting(&conn, "is_primi_gruplari", original_groups).unwrap();
        let original = CompensationPolicyVersion {
            id: "work-premium-rollback-baseline".into(),
            effectiveFrom: "2026-01-15".into(),
            state: state(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PeriodTotal,
            createdAt: "2026-01-15T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Test baseline".into(),
            supersedesVersionId: None,
        };
        CompensationPolicyRepository::insert_version(&conn, &original).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_work_premium_mirror_update
             BEFORE UPDATE OF value ON app_settings
             WHEN OLD.key = 'is_primi_gruplari'
             BEGIN
                 SELECT RAISE(ABORT, 'synthetic premium mirror failure');
             END;",
        )
        .unwrap();

        let result = set_app_setting_with_connection(
            &conn,
            "is_primi_gruplari",
            r#"[{"id":"1. Grup","ad":"Grup 1","oran":"11","aktif":true}]"#,
        );
        assert!(result.is_err());
        assert_eq!(
            CompensationPolicyRepository::get_all_versions(&conn).unwrap(),
            vec![original]
        );
        assert_eq!(
            SettingsRepository::get_app_setting(&conn, "is_primi_gruplari")
                .unwrap()
                .as_deref(),
            Some(original_groups)
        );
    }
}
