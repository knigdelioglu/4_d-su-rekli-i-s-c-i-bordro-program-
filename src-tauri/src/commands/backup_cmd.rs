use crate::db::DbState;
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::compensation_policy_repo::CompensationPolicyRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::SettingsRepository;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use rusqlite::Connection;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

pub fn export_authoritative_sqlite_backup(conn: &Connection) -> Result<String, String> {
    let periods = PeriodRepository::get_all(conn)
        .map_err(|error| format!("Dönemler okunamadı: {error}"))?;
    let active_period_id = SettingsRepository::get_app_setting(conn, "active_period_id")
        .map_err(|error| format!("Aktif dönem okunamadı: {error}"))?
        .unwrap_or_default();
    let personnel = PersonnelRepository::get_all(conn)
        .map_err(|error| format!("Personeller okunamadı: {error}"))?;
    let institution_settings = SettingsRepository::get_all_institution_settings(conn)
        .map_err(|error| format!("Kurum ayarları okunamadı: {error}"))?;
    let attendances = AttendanceRepository::get_all(conn)
        .map_err(|error| format!("Puantajlar okunamadı: {error}"))?;
    let payrolls = PayrollRepository::get_all(conn)
        .map_err(|error| format!("Bordrolar okunamadı: {error}"))?;
    let tax_openings = TaxOpeningRepository::get_all(conn)
        .map_err(|error| format!("Kümülatif vergi matrahları okunamadı: {error}"))?;
    let sick_leave_records = SickLeaveRepository::get_all(conn)
        .map_err(|error| format!("Rapor kayıtları okunamadı: {error}"))?;
    let annual_payroll_parameters = AnnualPayrollParametersRepository::get_all(conn)
        .map_err(|error| format!("Yıllık parametreler okunamadı: {error}"))?;
    let compensation_revisions = crate::repositories::retro_repo::get_revisions(conn)
        .map_err(|error| format!("Retro revizyonları okunamadı: {error}"))?;
    let compensation_revision_overrides = crate::repositories::retro_repo::get_overrides(conn)
        .map_err(|error| format!("Retro revizyon istisnaları okunamadı: {error}"))?;
    let retro_batches = crate::repositories::retro_repo::get_batches(conn)
        .map_err(|error| format!("Retro paketleri okunamadı: {error}"))?;
    let retro_allocations = crate::repositories::retro_repo::get_allocations(conn)
        .map_err(|error| format!("Retro dağılımları okunamadı: {error}"))?;
    let compensation_policy_versions = CompensationPolicyRepository::get_all_versions(conn)
        .map_err(|error| format!("Ücret politikası sürümleri okunamadı: {error}"))?;
    let is_primi_gruplari: Vec<payroll_core::IsPrimiGrupItem> = match SettingsRepository::get_app_setting(conn, "is_primi_gruplari")
        .map_err(|error| format!("İş primi grupları okunamadı: {error}"))? {
        Some(val) => serde_json::from_str(&val).unwrap_or_default(),
        None => compensation_policy_versions
            .iter()
            .max_by(|a, b| a.effectiveFrom.cmp(&b.effectiveFrom).then_with(|| a.createdAt.cmp(&b.createdAt)))
            .map(|v| v.state.isPrimiGruplari.iter().map(|g| payroll_core::IsPrimiGrupItem {
                id: g.id.clone(),
                ad: g.ad.clone(),
                oran: g.oran,
                aktif: g.aktif,
            }).collect())
            .unwrap_or_default(),
    };
    let exported_at = chrono::Utc::now().to_rfc3339();

    let payload = serde_json::json!({
        "backupVersion": 6,
        "exportedAt": exported_at,
        "donemler": periods,
        "aktifDonemId": active_period_id,
        "personeller": personnel,
        "kurumDegerleriMap": institution_settings,
        "puantajlar": attendances,
        "bordrolar": payrolls,
        "taxOpenings": tax_openings,
        "sickLeaveRecords": sick_leave_records,
        "annualPayrollParameters": annual_payroll_parameters,
        "compensationRevisions": compensation_revisions,
        "compensationRevisionOverrides": compensation_revision_overrides,
        "retroBatches": retro_batches,
        "retroAllocations": retro_allocations,
        "compensationPolicyVersions": compensation_policy_versions,
        "isPrimiGruplari": is_primi_gruplari,
    });

    serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("V6 yedek JSON'u üretilemedi: {error}"))
}

#[cfg(test)]
fn attach_native_policy_timeline(conn: &Connection, payload_json: &str) -> Result<String, String> {
    let mut payload: serde_json::Value = serde_json::from_str(payload_json)
        .map_err(|error| format!("Yedek payload JSON olarak okunamadı: {error}"))?;
    if payload.get("backupVersion").and_then(serde_json::Value::as_u64) != Some(6) {
        return Err("Native yedek dışa aktarımı V6 payload gerektirir.".into());
    }
    let versions = CompensationPolicyRepository::get_all_versions(conn)
        .map_err(|error| format!("Policy timeline yedek için okunamadı: {error}"))?;
    payload["compensationPolicyVersions"] = serde_json::to_value(versions)
        .map_err(|error| format!("Policy timeline yedek JSON'una çevrilemedi: {error}"))?;
    serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("V6 yedek JSON'u üretilemedi: {error}"))
}
use tauri_plugin_dialog::DialogExt;

fn write_backup_file(path: &Path, payload_json: &str) -> Result<(), String> {
    write_backup_file_with(path, payload_json.as_bytes(), |file, bytes| {
        file.write_all(bytes)
    })
}

fn write_backup_file_with(
    path: &Path,
    payload: &[u8],
    write_payload: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
) -> Result<(), String> {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| "Yedek dosyası yazılamadı: dosya adı geçersiz.".to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..100_u32 {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{}.{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            timestamp,
            counter + u64::from(attempt),
        ));
        let mut temp_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Yedek dosyası yazılamadı: {error}")),
        };

        let write_result =
            write_payload(&mut temp_file, payload).and_then(|()| temp_file.sync_all());
        drop(temp_file);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("Yedek dosyası yazılamadı: {error}"));
        }

        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("Yedek dosyası yazılamadı: {error}"));
        }
        return Ok(());
    }

    Err("Yedek dosyası yazılamadı: geçici dosya adı oluşturulamadı.".into())
}

fn write_excel_file(path: &Path, excel_bytes: &[u8]) -> Result<(), String> {
    fs::write(path, excel_bytes).map_err(|error| format!("Excel dosyası yazılamadı: {error}"))
}

fn validate_excel_path(path: &Path) -> Result<(), String> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xlsx"))
    {
        Ok(())
    } else {
        Err("Excel dosyası .xlsx uzantısıyla kaydedilmeli.".to_string())
    }
}

fn write_pdf_file(path: &Path, pdf_bytes: &[u8]) -> Result<(), String> {
    fs::write(path, pdf_bytes).map_err(|error| format!("PDF dosyası yazılamadı: {error}"))
}

#[tauri::command]
pub async fn export_backup<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    db: State<'_, DbState>,
    _payload_json: String,
    file_name: String,
) -> Result<bool, String> {
    let Some(file_path) = window
        .dialog()
        .file()
        .set_title("Bordro yedeğini kaydet")
        .set_file_name(file_name)
        .add_filter("JSON yedeği", &["json"])
        .blocking_save_file()
    else {
        return Ok(false);
    };

    let path = file_path
        .into_path()
        .map_err(|error| format!("Seçilen yedek yolu kullanılamadı: {error}"))?;
    let payload_json = {
        let conn = db
            .lock()
            .map_err(|error| format!("SQLite bağlantı kilidi alınamadı: {error}"))?;
        export_authoritative_sqlite_backup(&conn)?
    };
    write_backup_file(&path, &payload_json)?;
    Ok(true)
}

#[tauri::command]
pub async fn export_excel<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    excel_bytes: Vec<u8>,
    file_name: String,
) -> Result<bool, String> {
    let Some(file_path) = window
        .dialog()
        .file()
        .set_title("Excel dosyasını kaydet")
        .set_file_name(file_name)
        .blocking_save_file()
    else {
        return Ok(false);
    };

    let path = file_path
        .into_path()
        .map_err(|error| format!("Seçilen Excel yolu kullanılamadı: {error}"))?;
    validate_excel_path(&path)?;
    write_excel_file(&path, &excel_bytes)?;
    Ok(true)
}

#[tauri::command]
pub async fn export_pdf<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    pdf_bytes: Vec<u8>,
    file_name: String,
) -> Result<bool, String> {
    let Some(file_path) = window
        .dialog()
        .file()
        .set_title("PDF dosyasını kaydet")
        .set_file_name(file_name)
        .add_filter("PDF belgesi", &["pdf"])
        .blocking_save_file()
    else {
        return Ok(false);
    };

    let path = file_path
        .into_path()
        .map_err(|error| format!("Seçilen PDF yolu kullanılamadı: {error}"))?;
    write_pdf_file(&path, &pdf_bytes)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{
        attach_native_policy_timeline, validate_excel_path, write_backup_file,
        write_backup_file_with, write_excel_file, write_pdf_file,
    };
    use crate::db::create_in_memory_connection;
    use crate::domain::CompensationPolicyVersion;
    use crate::repositories::compensation_policy_repo::CompensationPolicyRepository;
    use serde_json::{json, Value};
    use std::io::Write;
    use std::{fs, path::Path, time::SystemTime};

    #[test]
    fn writes_atomic_reference_backup_without_changing_its_json_or_decimal_text() {
        let payload = include_str!("../../tests/fixtures/session3-atomic-reference.json");
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bordro-backup-export-{}-{unique}.json",
            std::process::id()
        ));

        fs::write(&path, b"previous-good-backup").expect("previous destination should be written");
        write_backup_file(&path, payload).expect("backup file should be written");
        let saved = fs::read_to_string(&path).expect("written backup should be readable");
        let _ = fs::remove_file(&path);

        assert_eq!(saved, payload);
    }

    #[test]
    fn failed_atomic_backup_write_preserves_existing_file_and_cleans_temporary_file() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "bordro-backup-atomic-failure-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("test directory should be created");
        let path = directory.join("existing.json");
        fs::write(&path, b"previous-good-backup").expect("existing backup should be written");

        let error = write_backup_file_with(&path, b"replacement", |file, _| {
            file.write_all(b"partial")?;
            Err(std::io::Error::other("injected disk write failure"))
        })
        .expect_err("injected write should fail");

        assert!(error.contains("injected disk write failure"));
        assert_eq!(
            fs::read(&path).expect("existing backup must remain readable"),
            b"previous-good-backup"
        );
        assert_eq!(
            fs::read_dir(&directory)
                .expect("temporary directory should be readable")
                .count(),
            1,
            "failed temporary backup should be removed"
        );
        fs::remove_dir_all(directory).expect("test directory should be removed");
    }

    #[test]
    fn native_v6_export_reads_the_exact_immutable_policy_timeline() {
        let conn = create_in_memory_connection().expect("in-memory schema should migrate");
        let version: CompensationPolicyVersion = serde_json::from_value(json!({
            "id": "policy-export",
            "effectiveFrom": "2026-09-15",
            "state": {
                "gunlukTabanUcret": "1234.567890123",
                "gunlukYemek": "0",
                "birlestirilmisSosyalYardim": "0",
                "gunlukVasitaYol": "0",
                "giyimYardimi": "0",
                "hizmetZammiBirimi": "0",
                "isPrimiYuzde": null,
                "isPrimiGruplari": [{"id":"g","ad":"Grup","oran":"0","aktif":false}],
                "geceCalismaPrimiYuzde": "0",
                "geceCalismaTatiliPrimiYuzde": "0",
                "ekOdeme": null,
                "digerGelirVarsayilan": "0"
            },
            "boundaryScope": "FULL_EARNING",
            "roadRoundingMode": "PER_COMPENSATION_SLICE",
            "createdAt": "2026-09-01T00:00:00Z",
            "source": "user-confirmed",
            "reason": "initial baseline",
            "supersedesVersionId": null
        }))
        .expect("exact policy model should deserialize");
        CompensationPolicyRepository::insert_version(&conn, &version)
            .expect("immutable policy should persist");

        let exported = attach_native_policy_timeline(
            &conn,
            r#"{"backupVersion":6,"exportedAt":"2026-10-06T00:00:00Z","bordrolar":[]}"#,
        )
        .expect("native V6 export should add the DB timeline");
        let payload: Value = serde_json::from_str(&exported).expect("export should be JSON");
        assert_eq!(
            payload["compensationPolicyVersions"][0]["id"],
            "policy-export"
        );
        assert_eq!(
            payload["compensationPolicyVersions"][0]["state"]["gunlukTabanUcret"],
            "1234.567890123"
        );
        assert!(payload.get("zamAylari").is_none());
        assert!(attach_native_policy_timeline(&conn, r#"{"backupVersion":5}"#).is_err());
    }

    #[test]
    fn reports_file_write_failure() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bordro-backup-missing-{}-{unique}",
            std::process::id()
        ));

        let error = write_backup_file(&path.join("backup.json"), "{}")
            .expect_err("writing into a missing directory should fail");
        assert!(error.starts_with("Yedek dosyası yazılamadı:"));
    }

    #[test]
    fn writes_excel_bytes_without_modifying_the_workbook() {
        let bytes = b"PK\x03\x04workbook";
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bordro-excel-export-{}-{unique}.xlsx",
            std::process::id()
        ));

        write_excel_file(&path, bytes).expect("Excel bytes should be written");
        let saved = fs::read(&path).expect("written Excel file should be readable");
        let _ = fs::remove_file(&path);

        assert_eq!(saved, bytes);
    }

    #[test]
    fn accepts_xlsx_paths_without_a_macos_extension_filter() {
        assert!(validate_excel_path(Path::new("Bordro_2027-02_Ayse_Kaya_NORMAL.xlsx")).is_ok());
        assert!(validate_excel_path(Path::new("Bordro_2027-02_Ayse_Kaya_NORMAL.XLSX")).is_ok());
        assert!(validate_excel_path(Path::new("Bordro_2027-02_Ayse_Kaya_NORMAL.csv")).is_err());
        assert!(validate_excel_path(Path::new("Bordro_2027-02_Ayse_Kaya_NORMAL")).is_err());
    }

    #[test]
    fn reports_excel_file_write_failure() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bordro-excel-missing-{}-{unique}",
            std::process::id()
        ));

        let error = write_excel_file(&path.join("workbook.xlsx"), b"PK")
            .expect_err("writing into a missing directory should fail");
        assert!(error.starts_with("Excel dosyası yazılamadı:"));
    }

    #[test]
    fn writes_pdf_bytes_without_modifying_the_document() {
        let bytes = b"%PDF-1.4\npdf document";
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bordro-pdf-export-{}-{unique}.pdf",
            std::process::id()
        ));

        write_pdf_file(&path, bytes).expect("PDF bytes should be written");
        let saved = fs::read(&path).expect("written PDF should be readable");
        let _ = fs::remove_file(&path);

        assert_eq!(saved, bytes);
    }

    #[test]
    fn reports_pdf_file_write_failure() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bordro-pdf-missing-{}-{unique}",
            std::process::id()
        ));

        let error = write_pdf_file(&path.join("document.pdf"), b"%PDF")
            .expect_err("writing into a missing directory should fail");
        assert!(error.starts_with("PDF dosyası yazılamadı:"));
    }

    #[test]
    fn native_backup_export_is_produced_solely_from_authoritative_sqlite_snapshot() {
        let source = create_in_memory_connection().expect("in-memory source DB should migrate");

        let period = crate::domain::BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-15".into(),
            bitisTarihi: "2026-02-14".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
        crate::repositories::period_repo::PeriodRepository::save(&source, &period)
            .expect("period should persist");
        crate::repositories::settings_repo::SettingsRepository::set_app_setting(
            &source,
            "active_period_id",
            "2026-01",
        )
        .expect("active period setting should persist");

        let person = crate::domain::Personel {
            id: "person-native-backup".into(),
            tcNo: "11111111110".into(),
            ad: "Ahmet".into(),
            soyad: "Kaya".into(),
            grup: "1. Grup".into(),
            unvan: None,
            sgkSicilNo: "12345".into(),
            iban: "TR000000000000000000000000".into(),
            hizmetYili: 3,
            aciklama: None,
            devirKumulatifGvMatrahi: None,
            devirKumulatifGvMatrahiYili: None,
            devirKumulatifGvMatrahiBaslangicAyi: None,
            devirKumulatifAsgariGvMatrahi: None,
            devirKumulatifAsgariGvMatrahiYili: None,
            kesintiler: None,
        };
        crate::repositories::personnel_repo::PersonnelRepository::save(&source, &person)
            .expect("person should persist");

        let version: CompensationPolicyVersion = serde_json::from_value(json!({
            "id": "policy-native-export",
            "effectiveFrom": "2026-01-01",
            "state": {
                "gunlukTabanUcret": "1500",
                "gunlukYemek": "100",
                "birlestirilmisSosyalYardim": "500",
                "gunlukVasitaYol": "50",
                "giyimYardimi": "0",
                "hizmetZammiBirimi": "0",
                "isPrimiYuzde": null,
                "isPrimiGruplari": [{"id":"g1","ad":"Grup 1","oran":"10","aktif":true}],
                "geceCalismaPrimiYuzde": "0",
                "geceCalismaTatiliPrimiYuzde": "0",
                "ekOdeme": null,
                "digerGelirVarsayilan": "0"
            },
            "boundaryScope": "FULL_EARNING",
            "roadRoundingMode": "PER_COMPENSATION_SLICE",
            "createdAt": "2026-01-01T00:00:00Z",
            "source": "user-confirmed",
            "reason": "initial baseline",
            "supersedesVersionId": null
        }))
        .expect("policy model deserializes");
        CompensationPolicyRepository::insert_version(&source, &version)
            .expect("policy version should persist");

        crate::repositories::settings_repo::SettingsRepository::set_app_setting(
            &source,
            "is_primi_gruplari",
            r#"[{"id":"g1","ad":"Grup 1","oran":"10","aktif":true}]"#,
        )
        .expect("work premium groups setting should persist");

        let exported_json = super::export_authoritative_sqlite_backup(&source)
            .expect("authoritative export should succeed");

        let exported_value: Value = serde_json::from_str(&exported_json)
            .expect("exported JSON should be valid");

        assert_eq!(exported_value["backupVersion"], 6);
        assert_eq!(exported_value["aktifDonemId"], "2026-01");
        assert_eq!(exported_value["personeller"][0]["id"], "person-native-backup");
        assert_eq!(exported_value["compensationPolicyVersions"][0]["id"], "policy-native-export");
        assert_eq!(exported_value["isPrimiGruplari"][0]["id"], "g1");

        // Verify that the exported authoritative payload round-trips into a fresh database
        let mut target = create_in_memory_connection().expect("in-memory target DB should migrate");
        crate::services::migration_service::MigrationService::replace_backup_data(
            &mut target,
            &exported_json,
        )
        .expect("exported authoritative backup should restore cleanly into target DB");

        assert_eq!(
            crate::repositories::personnel_repo::PersonnelRepository::get_by_id(
                &target,
                "person-native-backup",
            )
            .unwrap()
            .unwrap()
            .ad,
            "Ahmet"
        );
        assert_eq!(
            CompensationPolicyRepository::get_all_versions(&target).unwrap()[0].id,
            "policy-native-export"
        );
    }
}
