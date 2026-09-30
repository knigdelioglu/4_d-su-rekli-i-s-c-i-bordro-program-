/// Opens the operating system print dialog for the current Tauri webview.
#[tauri::command]
pub async fn print_current_webview<R: tauri::Runtime>(
    webview_window: tauri::WebviewWindow<R>,
) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        webview_window.print().map_err(|error| error.to_string())?;
        Ok(true)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = webview_window;
        Ok(false)
    }
}
