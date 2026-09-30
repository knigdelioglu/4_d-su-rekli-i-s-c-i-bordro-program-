use std::fs;
use std::path::Path;
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
    use super::{validate_excel_path, write_backup_file, write_excel_file, write_pdf_file};
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
