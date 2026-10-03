use crate::license::{self, LicenseRuntime, LicenseStatus};
use tauri::State;

#[tauri::command]
pub fn get_license_status(runtime: State<'_, LicenseRuntime>) -> Result<LicenseStatus, String> {
    license::get_status(runtime.data_dir())
}

#[tauri::command]
pub async fn activate_license(
    runtime: State<'_, LicenseRuntime>,
    license_key: String,
) -> Result<LicenseStatus, String> {
    let data_dir = runtime.data_dir().to_path_buf();
    license::activate(&data_dir, &license_key).await
}

#[tauri::command]
pub async fn refresh_license_status(
    runtime: State<'_, LicenseRuntime>,
) -> Result<LicenseStatus, String> {
    let data_dir = runtime.data_dir().to_path_buf();
    license::refresh_status(&data_dir).await
}
