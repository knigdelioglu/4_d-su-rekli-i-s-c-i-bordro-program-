use crate::db::DbState;
use crate::repositories::compensation_policy_repo::CompensationPolicyRepository;
use rusqlite::Connection;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tauri::State;

fn attach_native_policy_timeline(conn: &Connection, payload_json: &str) -> Result<String, String> {
    let mut payload: Value = serde_json::from_str(payload_json)
        .map_err(|error| format!("Yedek payload JSON olarak okunamadı: {error}"))?;
    if payload.get("backupVersion").and_then(Value::as_u64) != Some(6) {
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
    fs::write(path, payload_json).map_err(|error| format!("Yedek dosyası yazılamadı: {error}"))
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
    payload_json: String,
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
        attach_native_policy_timeline(&conn, &payload_json)?
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
        attach_native_policy_timeline, validate_excel_path, write_backup_file, write_excel_file,
        write_pdf_file,
    };
    use crate::db::create_in_memory_connection;
    use crate::domain::CompensationPolicyVersion;
    use crate::repositories::compensation_policy_repo::CompensationPolicyRepository;
    use serde_json::{json, Value};
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

        write_backup_file(&path, payload).expect("backup file should be written");
        let saved = fs::read_to_string(&path).expect("written backup should be readable");
        let _ = fs::remove_file(&path);

        assert_eq!(saved, payload);
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
}
