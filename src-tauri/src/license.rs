use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

const OFFLINE_GRACE_DAYS: i64 = 30;
const SERVER_CHECK_DAYS: i64 = 7;
const RETRY_HOURS: i64 = 6;
const CHECK_LOOP_SECONDS: u64 = 60;
const LICENSE_STATE_FILE: &str = "license-state.json";
const DEVICE_ID_FILE: &str = "license-device-id";
const PUBLIC_KEYS: &str = include_str!("../license-public-keys.json");
const KEYRING_SERVICE: &str = "4d-bordro-license";

fn secure_get(account: &str) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            KEYRING_SERVICE,
            "-a",
            account,
            "-w",
        ])
        .output()
        .map_err(|_| "macOS Keychain erişilebilir değil.".to_string())?;
    #[cfg(target_os = "linux")]
    let output = Command::new("secret-tool")
        .args(["lookup", "app", KEYRING_SERVICE, "account", account])
        .output()
        .map_err(|_| {
            "Linux Secret Service erişilebilir değil (secret-tool bulunamadı).".to_string()
        })?;
    #[cfg(target_os = "windows")]
    let output = windows_secure_get(account)?;
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err("Bu işletim sisteminde lisans güvenli deposu desteklenmiyor.".into());
    if output.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ));
    }
    #[cfg(target_os = "macos")]
    if String::from_utf8_lossy(&output.stderr).contains("could not be found") {
        return Ok(None);
    }
    #[cfg(target_os = "linux")]
    if output.status.code() == Some(1) && output.stderr.is_empty() {
        return Ok(None);
    }
    #[cfg(target_os = "windows")]
    if output.status.code() == Some(2) {
        return Ok(None);
    }
    Err("İşletim sistemi güvenli deposundan kayıt okunamadı.".into())
}

fn secure_set(account: &str, value: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYRING_SERVICE,
            "-a",
            account,
            "-w",
            value,
        ])
        .output()
        .map_err(|_| "macOS Keychain erişilebilir değil.".to_string())?;
    #[cfg(target_os = "linux")]
    let result = {
        use std::io::Write;
        let mut child = Command::new("secret-tool")
            .args([
                "store",
                "--label=4/D license protection",
                "app",
                KEYRING_SERVICE,
                "account",
                account,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| {
                "Linux Secret Service erişilebilir değil (secret-tool bulunamadı).".to_string()
            })?;
        child
            .stdin
            .take()
            .ok_or_else(|| "Linux Secret Service girdisi açılamadı.".to_string())?
            .write_all(value.as_bytes())
            .map_err(|_| "Linux Secret Service girdisi yazılamadı.".to_string())?;
        child
            .wait()
            .map_err(|_| "Linux Secret Service yanıt vermedi.".to_string())?
    };
    #[cfg(target_os = "windows")]
    let result = windows_secure_set(account, value)?;
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err("Bu işletim sisteminde lisans güvenli deposu desteklenmiyor.".into());
    if result.status.success() {
        Ok(())
    } else {
        Err("İşletim sistemi güvenli deposu güncellenemedi.".into())
    }
}

#[cfg(target_os = "windows")]
fn windows_secure_path(account: &str) -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "Windows yerel uygulama dizini bulunamadı.".to_string())?;
    let directory = PathBuf::from(base).join("4d-bordro-license");
    fs::create_dir_all(&directory)
        .map_err(|_| "Windows lisans güvenli deposu oluşturulamadı.".to_string())?;
    Ok(directory.join(format!(
        "{}.protected",
        hex(&Sha256::digest(account.as_bytes()))
    )))
}

#[cfg(target_os = "windows")]
fn windows_secure_command(
    account: &str,
    value: Option<&str>,
) -> Result<std::process::Output, String> {
    let path = windows_secure_path(account)?;
    let path = path.to_string_lossy().replace('\'', "''");
    let script = if let Some(value) = value {
        let value = value.replace('\'', "''");
        format!("& {{ Add-Type -AssemblyName System.Security;$b=[Text.Encoding]::UTF8.GetBytes('{value}');$e=[Security.Cryptography.ProtectedData]::Protect($b,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser);[IO.File]::WriteAllText('{path}',[Convert]::ToBase64String($e)) }}")
    } else {
        format!("& {{ if(!(Test-Path '{path}')){{exit 2}};Add-Type -AssemblyName System.Security;$e=[Convert]::FromBase64String([IO.File]::ReadAllText('{path}'));$b=[Security.Cryptography.ProtectedData]::Unprotect($e,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser);[Console]::Out.Write([Text.Encoding]::UTF8.GetString($b)) }}")
    };
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(script)
        .output()
        .map_err(|_| "Windows DPAPI güvenli deposu başlatılamadı.".into())
}

#[cfg(target_os = "windows")]
fn windows_secure_get(account: &str) -> Result<std::process::Output, String> {
    windows_secure_command(account, None)
}

#[cfg(target_os = "windows")]
fn windows_secure_set(account: &str, value: &str) -> Result<std::process::Output, String> {
    windows_secure_command(account, Some(value))
}

fn enforcement_required() -> bool {
    option_env!("BORDRO_LICENSE_MODE").unwrap_or("optional") == "required"
}

fn api_url() -> Option<&'static str> {
    option_env!("BORDRO_LICENSE_API_URL")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseGrantClaims {
    pub v: u8,
    pub aud: String,
    pub kid: String,
    pub license_id: String,
    pub device_hash: String,
    pub issued_at: DateTime<Utc>,
    pub last_online_at: DateTime<Utc>,
    pub offline_until: DateTime<Utc>,
    pub license_expires_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredLicenseState {
    #[serde(default)]
    machine_binding: Option<String>,
    #[serde(default)]
    activation_request_id: Option<String>,
    #[serde(default)]
    activation_status: Option<String>,
    #[serde(default)]
    grant: Option<String>,
    #[serde(default)]
    server_denial: Option<String>,
    #[serde(default)]
    last_seen_at: Option<DateTime<Utc>>,
    #[serde(default)]
    last_attempt_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    pub mode: String,
    pub state: String,
    pub can_calculate: bool,
    pub can_finalize: bool,
    pub device_id: String,
    pub license_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_online_at: Option<DateTime<Utc>>,
    pub offline_until: Option<DateTime<Utc>>,
    pub reason: Option<String>,
}

fn required_mode_configuration_error(required: bool) -> Option<String> {
    if !required {
        return None;
    }
    if api_url().is_none() {
        return Some("Zorunlu lisans modu için lisans sunucu adresi yapılandırılmamış.".into());
    }
    match public_keys() {
        Ok(keys) if !keys.is_empty() => None,
        _ => {
            Some("Zorunlu lisans modu için uygulamada Ed25519 açık anahtarı tanımlanmamış.".into())
        }
    }
}

#[derive(Clone)]
pub struct LicenseRuntime {
    data_dir: PathBuf,
}

impl LicenseRuntime {
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> Result<LicenseRuntime, String> {
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Yerel lisans dizinine erişilemedi.".to_string())?;
    Ok(LicenseRuntime { data_dir: path })
}

fn ensure_data_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(&path).map_err(|_| "Yerel lisans dizini oluşturulamadı.".to_string())?;
    Ok(())
}

fn device_hash(dir: &Path) -> Result<String, String> {
    // Keep the hash used by already-issued grants on upgraded installs. A secure
    // OS-keystore hardware binding below prevents the copied legacy UUID from
    // moving those grants to another machine.
    let legacy_id = dir.join(DEVICE_ID_FILE);
    if legacy_id.exists() {
        let value = fs::read_to_string(&legacy_id)
            .map_err(|_| "Cihaz kimliği dosyası okunamadı.".to_string())?;
        let value = value.trim();
        if Uuid::parse_str(value).is_ok() {
            return Ok(hex(&Sha256::digest(value.as_bytes())));
        }
        return Err("Cihaz kimliği dosyası bozuk. Lisans desteğine başvurun.".into());
    }
    let id = machine_identity()?;
    // Bind local secure-storage records to this OS installation and hardware identity.
    Ok(hex(&Sha256::digest(
        format!("4d-license-device-v2:{id}").as_bytes(),
    )))
}

fn secure_machine_binding() -> Result<Option<String>, String> {
    secure_get("machine-binding")
}

fn set_secure_machine_binding(value: &str) -> Result<(), String> {
    secure_set("machine-binding", value)
}

fn ensure_machine_binding(dir: &Path, state: &mut StoredLicenseState) -> Result<(), String> {
    let machine = hex(&Sha256::digest(machine_identity()?.as_bytes()));
    match secure_machine_binding()? {
        Some(saved) if saved != machine => Err(
            "Lisans kaydı başka bir cihaza kopyalanmış. Bu cihazda yeniden etkinleştirin.".into(),
        ),
        Some(_)
            if state
                .machine_binding
                .as_deref()
                .is_some_and(|saved| saved != machine) =>
        {
            Err(
                "Lisans kaydı başka bir cihaza kopyalanmış. Bu cihazda yeniden etkinleştirin."
                    .into(),
            )
        }
        Some(_) => {
            if state.server_denial.is_none() {
                state.server_denial = secure_server_denial()?;
            }
            if let Some(saved) = secure_last_seen()? {
                state.last_seen_at = Some(
                    state
                        .last_seen_at
                        .map(|value| value.max(saved))
                        .unwrap_or(saved),
                );
            }
            Ok(())
        }
        None if state
            .machine_binding
            .as_deref()
            .is_some_and(|saved| saved != machine) =>
        {
            Err(
                "Lisans kaydı başka bir cihaza kopyalanmış. Bu cihazda yeniden etkinleştirin."
                    .into(),
            )
        }
        None => {
            set_secure_machine_binding(&machine)?;
            state.machine_binding = Some(machine);
            if state.server_denial.is_none() {
                state.server_denial = secure_server_denial()?;
            } else if let Some(reason) = state.server_denial.as_deref() {
                secure_set("server-denial", reason)?;
            }
            if secure_last_seen()?.is_none() {
                if let Some(saved) = state.last_seen_at {
                    set_secure_last_seen(saved)?;
                }
            }
            if let Some(saved) = secure_last_seen()? {
                state.last_seen_at = Some(
                    state
                        .last_seen_at
                        .map(|value| value.max(saved))
                        .unwrap_or(saved),
                );
            }
            if secure_grant_issued_at()?.is_none() {
                if let Some(claims) = read_claims(state, &device_hash(dir)?)? {
                    set_secure_grant_issued_at(claims.issued_at)?;
                }
            }
            write_state(dir, state)
        }
    }
}

fn secure_server_denial() -> Result<Option<String>, String> {
    Ok(secure_get("server-denial")?.filter(|value| !value.is_empty()))
}

fn clear_secure_server_denial() -> Result<(), String> {
    secure_set("server-denial", "")
}

fn machine_identity() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .map_err(|_| "Bu Mac'in donanım kimliği okunamadı.".to_string())?;
        let text = String::from_utf8_lossy(&output.stdout);
        let value = text
            .lines()
            .find(|line| line.contains("IOPlatformUUID"))
            .and_then(|line| line.split('"').nth(3))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Bu Mac'in donanım kimliği bulunamadı.".to_string())?;
        return Ok(value.to_ascii_lowercase());
    }
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("reg")
            .args([
                "query",
                "HKLM\\SOFTWARE\\Microsoft\\Cryptography",
                "/v",
                "MachineGuid",
            ])
            .output()
            .map_err(|_| "Windows cihaz kimliği okunamadı.".to_string())?;
        let text = String::from_utf8_lossy(&output.stdout);
        let value = text
            .lines()
            .find(|line| line.contains("MachineGuid"))
            .and_then(|line| line.split_whitespace().last())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Windows cihaz kimliği bulunamadı.".to_string())?;
        return Ok(value.to_ascii_lowercase());
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(value) = fs::read_to_string("/sys/class/dmi/id/product_uuid") {
            if !value.trim().is_empty() {
                return Ok(value.trim().to_ascii_lowercase());
            }
        }
        return fs::read_to_string("/etc/machine-id")
            .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))
            .map(|value| value.trim().to_ascii_lowercase())
            .map_err(|_| "Linux cihaz kimliği okunamadı.".to_string());
    }
    #[allow(unreachable_code)]
    Err("Bu işletim sistemi için cihaz kimliği desteklenmiyor.".into())
}

fn secure_last_seen() -> Result<Option<DateTime<Utc>>, String> {
    match secure_get("last-seen")? {
        Some(value) => DateTime::parse_from_rfc3339(&value)
            .map(|date| Some(date.with_timezone(&Utc)))
            .map_err(|_| "Lisans güvenli deposundaki zaman kaydı bozuk.".to_string()),
        None => Ok(None),
    }
}

fn set_secure_last_seen(value: DateTime<Utc>) -> Result<(), String> {
    secure_set("last-seen", &value.to_rfc3339())
}

fn secure_grant_issued_at() -> Result<Option<DateTime<Utc>>, String> {
    match secure_get("grant-issued-at")? {
        Some(value) => DateTime::parse_from_rfc3339(&value)
            .map(|date| Some(date.with_timezone(&Utc)))
            .map_err(|_| "Lisans güvenli deposundaki izin kaydı bozuk.".to_string()),
        None => Ok(None),
    }
}

fn is_fresh_grant(issued_at: DateTime<Utc>, protected_issued_at: Option<DateTime<Utc>>) -> bool {
    protected_issued_at.map_or(true, |previous| issued_at > previous)
}

fn grant_replay_error(
    issued_at: DateTime<Utc>,
    protected_issued_at: Option<DateTime<Utc>>,
) -> Option<String> {
    protected_issued_at
        .filter(|previous| issued_at < *previous)
        .map(|_| "Yerel kayıttaki lisans izni eski; çevrimiçi yenileme gerekli.".into())
}

fn set_secure_grant_issued_at(value: DateTime<Utc>) -> Result<(), String> {
    secure_set("grant-issued-at", &value.to_rfc3339())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join(LICENSE_STATE_FILE)
}

fn read_state(dir: &Path) -> Result<StoredLicenseState, String> {
    let path = state_path(dir);
    match fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "Yerel lisans kaydı bozuk.".to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(StoredLicenseState::default())
        }
        Err(_) => Err("Yerel lisans kaydı okunamadı.".into()),
    }
}

fn write_state(dir: &Path, state: &StoredLicenseState) -> Result<(), String> {
    let bytes = serde_json::to_vec(state).map_err(|_| "Lisans kaydı hazırlanamadı.".to_string())?;
    write_private_file(&state_path(dir), &bytes)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    fs::write(&temp, bytes).map_err(|_| "Yerel lisans kaydı yazılamadı.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))
            .map_err(|_| "Yerel lisans kaydı izinleri ayarlanamadı.".to_string())?;
    }
    fs::rename(&temp, path).map_err(|_| "Yerel lisans kaydı güncellenemedi.".to_string())
}

fn public_keys() -> Result<HashMap<String, String>, String> {
    serde_json::from_str(PUBLIC_KEYS)
        .map_err(|_| "Uygulama lisans açık anahtar listesi bozuk.".to_string())
}

pub fn verify_grant(
    token: &str,
    expected_device_hash: &str,
    keys: &HashMap<String, String>,
) -> Result<LicenseGrantClaims, String> {
    let mut parts = token.split('.');
    if parts.next() != Some("v1") {
        return Err("Lisans izni sürümü desteklenmiyor.".into());
    }
    let payload_encoded = parts
        .next()
        .ok_or_else(|| "Lisans izni biçimi geçersiz.".to_string())?;
    let signature_encoded = parts
        .next()
        .ok_or_else(|| "Lisans izni biçimi geçersiz.".to_string())?;
    if parts.next().is_some() {
        return Err("Lisans izni biçimi geçersiz.".into());
    }
    let payload = URL_SAFE_NO_PAD
        .decode(payload_encoded)
        .map_err(|_| "Lisans izni bozuk.".to_string())?;
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(signature_encoded)
        .map_err(|_| "Lisans imzası bozuk.".to_string())?;
    let claims: LicenseGrantClaims =
        serde_json::from_slice(&payload).map_err(|_| "Lisans izni okunamadı.".to_string())?;
    if claims.v != 1 || claims.aud != "4d-bordro-desktop" || claims.status != "active" {
        return Err("Lisans izni bu uygulama için geçerli değil.".into());
    }
    if claims.device_hash != expected_device_hash {
        return Err("Lisans izni bu cihaz için düzenlenmemiş.".into());
    }
    let encoded_key = keys
        .get(&claims.kid)
        .ok_or_else(|| "Lisans imza anahtarı uygulamada tanınmıyor.".to_string())?;
    let key_bytes = URL_SAFE_NO_PAD
        .decode(encoded_key)
        .map_err(|_| "Lisans açık anahtarı bozuk.".to_string())?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| "Lisans açık anahtarı geçersiz.".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&key_array)
        .map_err(|_| "Lisans açık anahtarı geçersiz.".to_string())?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| "Lisans imzası biçimi geçersiz.".to_string())?;
    verifying_key
        .verify(&payload, &signature)
        .map_err(|_| "Lisans imzası doğrulanamadı.".to_string())?;
    Ok(claims)
}

fn claims_status(
    claims: &LicenseGrantClaims,
    device_hash: &str,
    now: DateTime<Utc>,
    last_seen_at: Option<DateTime<Utc>>,
    server_denial: Option<&str>,
) -> Result<(), String> {
    if claims.device_hash != device_hash {
        return Err("Lisans bu cihaz için düzenlenmemiş.".into());
    }
    if let Some(last_seen) = last_seen_at {
        if now < last_seen {
            return Err(
                "Sistem saati geriye alınmış görünüyor. İnternete bağlanıp lisansı doğrulayın."
                    .into(),
            );
        }
    }
    if let Some(reason) = server_denial {
        return Err(format!("Lisans sunucu tarafından reddedildi ({reason})."));
    }
    if now > claims.license_expires_at || now > claims.offline_until {
        return Err(
            "Lisansın çevrimdışı kullanım süresi doldu. İnternete bağlanıp lisansı yenileyin."
                .into(),
        );
    }
    if claims.offline_until > claims.last_online_at + Duration::days(OFFLINE_GRACE_DAYS) {
        return Err("Lisans çevrimdışı süresi sunucu sınırını aşıyor.".into());
    }
    Ok(())
}

fn read_claims(
    state: &StoredLicenseState,
    device_hash: &str,
) -> Result<Option<LicenseGrantClaims>, String> {
    let Some(token) = state.grant.as_deref() else {
        return Ok(None);
    };
    Ok(Some(verify_grant(token, device_hash, &public_keys()?)?))
}

fn status_from_parts(
    state: &StoredLicenseState,
    device_hash: &str,
    now: DateTime<Utc>,
    required: bool,
) -> LicenseStatus {
    let short_device_id = device_hash.chars().take(12).collect();
    if let Some(reason) = required_mode_configuration_error(required) {
        return LicenseStatus {
            mode: "required".into(),
            state: "configurationError".into(),
            can_calculate: false,
            can_finalize: false,
            device_id: short_device_id,
            license_id: None,
            expires_at: None,
            last_online_at: None,
            offline_until: None,
            reason: Some(reason),
        };
    }
    let claims = read_claims(state, device_hash);
    let protected_issued_at = if state.grant.is_some() {
        secure_grant_issued_at()
    } else {
        Ok(None)
    };
    let (status, license_id, expires_at, last_online_at, offline_until, validation_reason) =
        match claims {
            Ok(Some(claims)) => {
                let reason = match protected_issued_at.as_ref() {
                    Err(error) => Some(error.clone()),
                    Ok(anchor) => grant_replay_error(claims.issued_at, *anchor).or_else(|| {
                        claims_status(
                            &claims,
                            device_hash,
                            now,
                            state.last_seen_at,
                            state.server_denial.as_deref(),
                        )
                        .err()
                    }),
                };
                let state_name = if state.server_denial.is_some() {
                    "denied"
                } else if reason
                    .as_deref()
                    .is_some_and(|message| message.contains("süresi doldu"))
                {
                    "expired"
                } else if reason.is_some() {
                    "invalid"
                } else {
                    "active"
                };
                (
                    state_name,
                    Some(claims.license_id),
                    Some(claims.license_expires_at),
                    Some(claims.last_online_at),
                    Some(claims.offline_until),
                    reason,
                )
            }
            Ok(None) => {
                let state_name = match state.activation_status.as_deref() {
                    Some("pending") | Some("approving") => "pending",
                    Some("denied") | Some("released") => "denied",
                    Some("approved") => "approved",
                    _ => "notActivated",
                };
                (state_name, None, None, None, None, None)
            }
            Err(reason) => ("invalid", None, None, None, None, Some(reason)),
        };
    let can_use = !required || status == "active";
    LicenseStatus {
        mode: if required { "required" } else { "optional" }.into(),
        state: status.into(),
        can_calculate: can_use,
        can_finalize: can_use,
        device_id: short_device_id,
        license_id,
        expires_at,
        last_online_at,
        offline_until,
        reason: validation_reason,
    }
}

fn api_endpoint(path: &str) -> Result<String, String> {
    let base = api_url()
        .ok_or_else(|| "Lisans sunucu adresi uygulamada yapılandırılmamış.".to_string())?;
    let parsed =
        reqwest::Url::parse(base).map_err(|_| "Lisans sunucu adresi geçersiz.".to_string())?;
    if parsed.scheme() != "https" && !cfg!(debug_assertions) {
        return Err("Yayın derlemesinde lisans sunucu adresi HTTPS olmalıdır.".into());
    }
    Ok(format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    ))
}

fn server_error_message(status: u16, bytes: &[u8]) -> String {
    let payload: serde_json::Value = serde_json::from_slice(bytes).unwrap_or_default();
    let detail = payload
        .get("message")
        .or_else(|| payload.get("reason"))
        .or_else(|| payload.get("error"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Lisans isteği reddedildi.");
    match detail {
        "revoked" => "Lisans sunucu tarafından iptal edilmiş.".into(),
        "expired" => "Lisansın geçerlilik süresi dolmuş.".into(),
        _ => format!("Lisans sunucusu isteği reddetti ({status}): {detail}"),
    }
}

async fn post_json<T: for<'de> Deserialize<'de>>(
    path: &str,
    value: &impl Serialize,
) -> Result<(u16, T), String> {
    let endpoint = api_endpoint(path)?;
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|_| "Lisans ağı başlatılamadı.".to_string())?
        .post(endpoint)
        .json(value)
        .send()
        .await
        .map_err(|_| {
            "Lisans sunucusuna ulaşılamadı. Mevcut çevrimdışı izin korunuyor.".to_string()
        })?;
    let status = response.status().as_u16();
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "Lisans sunucusunun yanıtı okunamadı.".to_string())?;
    if !(200..300).contains(&status) {
        if let Ok(body) = serde_json::from_slice::<T>(&bytes) {
            return Ok((status, body));
        }
        return Err(server_error_message(status, &bytes));
    }
    let body = serde_json::from_slice::<T>(&bytes)
        .map_err(|_| "Lisans sunucusunun yanıtı okunamadı.".to_string())?;
    Ok((status, body))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivationRequest<'a> {
    license_key: &'a str,
    device_hash: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusRequest<'a> {
    request_id: &'a str,
    device_hash: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckRequest<'a> {
    license_id: &'a str,
    device_hash: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivationResponse {
    request_id: String,
    status: String,
}

#[derive(Deserialize)]
struct GrantResponse {
    decision: String,
    #[serde(default)]
    grant: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

fn store_server_denial(
    dir: &Path,
    state: &mut StoredLicenseState,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<(), String> {
    secure_set("server-denial", reason)?;
    state.server_denial = Some(reason.to_string());
    state.activation_status = Some("denied".into());
    state.last_attempt_at = Some(now);
    write_state(dir, state)
}

fn apply_grant(
    dir: &Path,
    state: &mut StoredLicenseState,
    token: String,
    device_hash: &str,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let claims = verify_grant(&token, device_hash, &public_keys()?)?;
    let already_installed = state.grant.as_deref() == Some(token.as_str());
    if !already_installed && !is_fresh_grant(claims.issued_at, secure_grant_issued_at()?) {
        return Err(
            "Sunucudan gelen izin eski bir izin kaydıyla eşleşiyor. Yeni doğrulama isteyin.".into(),
        );
    }
    // Signed server time is the recovery authority after a local clock jump.
    let trusted_now = now.max(claims.last_online_at);
    let last_seen = if already_installed {
        secure_last_seen()?
            .into_iter()
            .chain(state.last_seen_at)
            .max()
    } else {
        Some(claims.last_online_at)
    };
    claims_status(&claims, device_hash, trusted_now, last_seen, None)?;
    if !already_installed {
        set_secure_grant_issued_at(claims.issued_at)?;
    }
    state.grant = Some(token);
    state.server_denial = None;
    state.activation_status = Some("active".into());
    state.activation_request_id = None;
    state.last_seen_at = Some(trusted_now);
    set_secure_last_seen(trusted_now)?;
    state.last_attempt_at = Some(now);
    write_state(dir, state)?;
    clear_secure_server_denial()
}

pub fn get_status(dir: &Path) -> Result<LicenseStatus, String> {
    ensure_data_dir(dir)?;
    let device_hash = device_hash(dir)?;
    let mut state = read_state(dir)?;
    ensure_machine_binding(dir, &mut state)?;
    if let Some(secure_seen) = secure_last_seen()? {
        state.last_seen_at = Some(
            state
                .last_seen_at
                .map(|saved| saved.max(secure_seen))
                .unwrap_or(secure_seen),
        );
    }
    let claims = read_claims(&state, &device_hash).ok().flatten();
    let now = Utc::now().max(
        claims
            .as_ref()
            .map(|claims| claims.last_online_at)
            .unwrap_or(DateTime::<Utc>::MIN_UTC),
    );
    Ok(status_from_parts(
        &state,
        &device_hash,
        now,
        enforcement_required(),
    ))
}

pub async fn activate(dir: &Path, license_key: &str) -> Result<LicenseStatus, String> {
    ensure_data_dir(dir)?;
    let device_hash = device_hash(dir)?;
    let mut state = read_state(dir)?;
    ensure_machine_binding(dir, &mut state)?;
    let key = license_key.trim();
    if key.is_empty() || key.len() > 160 {
        return Err("Lisans anahtarı geçerli biçimde değil.".into());
    }
    let body = ActivationRequest {
        license_key: key,
        device_hash: &device_hash,
    };
    let (status, response) = post_json::<ActivationResponse>("activate", &body).await?;
    if status != 200 && status != 202 {
        return Err("Lisans anahtarı reddedildi veya lisans süresi doldu.".into());
    }
    let protected_issued_at = secure_grant_issued_at()?;
    let existing_grant_is_usable = read_claims(&state, &device_hash)
        .ok()
        .flatten()
        .is_some_and(|claims| {
            grant_replay_error(claims.issued_at, protected_issued_at).is_none()
                && claims_status(
                    &claims,
                    &device_hash,
                    Utc::now().max(claims.last_online_at),
                    state.last_seen_at,
                    state.server_denial.as_deref(),
                )
                .is_ok()
        });
    let already_active = response.status == "approved" && existing_grant_is_usable;
    state.activation_status = Some(response.status);
    if already_active {
        state.activation_request_id = None;
    } else {
        state.activation_request_id = Some(response.request_id);
    }
    state.last_attempt_at = Some(Utc::now());
    write_state(dir, &state)?;
    Ok(status_from_parts(
        &state,
        &device_hash,
        Utc::now(),
        enforcement_required(),
    ))
}

async fn refresh_activation(
    dir: &Path,
    device_hash: &str,
    state: &mut StoredLicenseState,
) -> Result<(), String> {
    let request_id = state
        .activation_request_id
        .clone()
        .ok_or_else(|| "Bekleyen lisans talebi bulunamadı.".to_string())?;
    let body = StatusRequest {
        request_id: &request_id,
        device_hash,
    };
    let (status, response) = post_json::<GrantResponse>("activation-status", &body).await?;
    let now = Utc::now();
    match response.decision.as_str() {
        "pending" => {
            state.activation_status = Some("pending".into());
            state.last_attempt_at = Some(now);
            write_state(dir, state)
        }
        "authorized" if (200..300).contains(&status) => {
            let grant = response
                .grant
                .ok_or_else(|| "Sunucudan imzalı lisans izni gelmedi.".to_string())?;
            let latest = read_state(dir)?;
            if latest.activation_request_id.as_deref() != Some(request_id.as_str()) {
                return Err("Lisans talebi bu sırada değişti. Durumu yeniden okuyun.".into());
            }
            *state = latest;
            apply_grant(dir, state, grant, device_hash, now)
        }
        "denied" | "authorized" => store_server_denial(
            dir,
            state,
            response.reason.as_deref().unwrap_or("denied"),
            now,
        ),
        _ => Err("Lisans sunucusunun yanıtı tanınmıyor.".into()),
    }
}

async fn remote_check(
    dir: &Path,
    device_hash: &str,
    state: &mut StoredLicenseState,
) -> Result<(), String> {
    let expected_grant = state
        .grant
        .clone()
        .ok_or_else(|| "Etkin lisans bulunamadı.".to_string())?;
    let claims =
        read_claims(state, device_hash)?.ok_or_else(|| "Etkin lisans bulunamadı.".to_string())?;
    let body = CheckRequest {
        license_id: &claims.license_id,
        device_hash,
    };
    let (status, response) = post_json::<GrantResponse>("check", &body).await?;
    let now = Utc::now();
    let latest = read_state(dir)?;
    if latest.grant.as_deref() != Some(expected_grant.as_str()) {
        return Err("Lisans bu sırada değişti. Durumu yeniden okuyun.".into());
    }
    *state = latest;
    match response.decision.as_str() {
        "authorized" if (200..300).contains(&status) => {
            let grant = response
                .grant
                .ok_or_else(|| "Sunucudan imzalı lisans izni gelmedi.".to_string())?;
            apply_grant(dir, state, grant, device_hash, now)
        }
        "denied" | "authorized" => store_server_denial(
            dir,
            state,
            response.reason.as_deref().unwrap_or("denied"),
            now,
        ),
        _ => Err("Lisans sunucusunun yanıtı tanınmıyor.".into()),
    }
}

pub async fn refresh_status(dir: &Path) -> Result<LicenseStatus, String> {
    ensure_data_dir(dir)?;
    let device_hash = device_hash(dir)?;
    let mut state = read_state(dir)?;
    ensure_machine_binding(dir, &mut state)?;
    if state.grant.is_some() && state.server_denial.is_none() {
        remote_check(dir, &device_hash, &mut state).await?;
    } else if state.grant.is_some() && state.server_denial.is_some() {
        // A denied grant is checked online again so an administrator can restore access.
        remote_check(dir, &device_hash, &mut state).await?;
    } else if state.activation_request_id.is_some() {
        refresh_activation(dir, &device_hash, &mut state).await?;
    } else {
        return Err("Önce lisans anahtarınızı girip aktivasyon talebi oluşturun.".into());
    }
    let claims = read_claims(&state, &device_hash).ok().flatten();
    let now = Utc::now().max(
        claims
            .as_ref()
            .map(|claims| claims.last_online_at)
            .unwrap_or(DateTime::<Utc>::MIN_UTC),
    );
    Ok(status_from_parts(
        &state,
        &device_hash,
        now,
        enforcement_required(),
    ))
}

pub fn require_allowed(dir: &Path, operation: &str) -> Result<(), String> {
    if !enforcement_required() {
        return Ok(());
    }
    ensure_data_dir(dir)?;
    let device_hash = device_hash(dir)?;
    let mut state = read_state(dir)?;
    ensure_machine_binding(dir, &mut state).map_err(|reason| {
        format!("Lisans uygun değil. Yeni {operation} işlemi durduruldu: {reason}")
    })?;
    let token = state
        .grant
        .as_deref()
        .ok_or_else(|| format!("Lisans etkin değil. Yeni {operation} işlemi durduruldu."))?;
    let claims = verify_grant(token, &device_hash, &public_keys()?)?;
    let protected_issued_at = secure_grant_issued_at()?;
    if let Some(reason) = grant_replay_error(claims.issued_at, protected_issued_at) {
        return Err(format!(
            "Lisans uygun değil. Yeni {operation} işlemi durduruldu: {reason}"
        ));
    }
    let now = Utc::now().max(claims.last_online_at);
    let secure_seen = secure_last_seen()?;
    let protected_seen = secure_seen.into_iter().chain(state.last_seen_at).max();
    claims_status(
        &claims,
        &device_hash,
        now,
        protected_seen,
        state.server_denial.as_deref(),
    )
    .map_err(|reason| {
        format!("Lisans uygun değil. Yeni {operation} işlemi durduruldu: {reason}")
    })?;
    state.last_seen_at = Some(now);
    set_secure_last_seen(now).map_err(|reason| {
        format!("Lisans uygun değil. Yeni {operation} işlemi durduruldu: {reason}")
    })?;
    write_state(dir, &state)
}

pub fn start_background_checker(data_dir: PathBuf) {
    if !enforcement_required() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            if ensure_data_dir(&data_dir).is_ok() {
                if let Ok(device_hash) = device_hash(&data_dir) {
                    if let Ok(mut state) = read_state(&data_dir) {
                        let now = Utc::now();
                        let due = state
                            .grant
                            .as_ref()
                            .filter(|_| state.server_denial.is_none())
                            .and_then(|_| {
                                read_claims(&state, &device_hash)
                                    .ok()
                                    .flatten()
                                    .map(|claims| {
                                        now - claims.last_online_at
                                            >= Duration::days(SERVER_CHECK_DAYS)
                                            && state
                                                .last_attempt_at
                                                .map(|at| now - at >= Duration::hours(RETRY_HOURS))
                                                .unwrap_or(true)
                                    })
                            })
                            .unwrap_or(false);
                        if due {
                            if remote_check(&data_dir, &device_hash, &mut state)
                                .await
                                .is_err()
                            {
                                record_transient_failure(&mut state, Utc::now());
                                let _ = write_state(&data_dir, &state);
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(CHECK_LOOP_SECONDS)).await;
        }
    });
}

pub fn should_attempt_check(
    required: bool,
    last_online: DateTime<Utc>,
    last_attempt: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    required
        && now - last_online >= Duration::days(SERVER_CHECK_DAYS)
        && last_attempt
            .map(|at| now - at >= Duration::hours(RETRY_HOURS))
            .unwrap_or(true)
}

fn record_transient_failure(state: &mut StoredLicenseState, now: DateTime<Utc>) {
    state.last_attempt_at = Some(now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use chrono::TimeZone;
    use ed25519_dalek::{Signer, SigningKey};

    fn test_claims() -> LicenseGrantClaims {
        let last_online = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        LicenseGrantClaims {
            v: 1,
            aud: "4d-bordro-desktop".into(),
            kid: "test-key".into(),
            license_id: "test-license".into(),
            device_hash: "a".repeat(64),
            issued_at: last_online,
            last_online_at: last_online,
            offline_until: last_online + Duration::days(30),
            license_expires_at: last_online + Duration::days(365),
            status: "active".into(),
        }
    }

    fn sign(claims: &LicenseGrantClaims, key: &SigningKey) -> String {
        let payload = serde_json::to_vec(claims).unwrap();
        let signature = key.sign(&payload);
        format!(
            "v1.{}.{}",
            URL_SAFE_NO_PAD.encode(payload),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        )
    }

    #[test]
    fn valid_and_invalid_ed25519_signatures_are_distinguished() {
        let signing = SigningKey::from_bytes(&[19; 32]);
        let public_key = URL_SAFE_NO_PAD.encode(signing.verifying_key().to_bytes());
        let keys = HashMap::from([("test-key".into(), public_key)]);
        let token = sign(&test_claims(), &signing);
        assert!(verify_grant(&token, &"a".repeat(64), &keys).is_ok());
        let mut token_parts = token.split('.').map(str::to_string).collect::<Vec<_>>();
        let signature = token_parts.last_mut().unwrap();
        signature.replace_range(0..1, if signature.starts_with('A') { "B" } else { "A" });
        let tampered = token_parts.join(".");
        assert!(verify_grant(&tampered, &"a".repeat(64), &keys).is_err());
    }

    #[test]
    fn grant_is_bound_to_expected_device() {
        let signing = SigningKey::from_bytes(&[20; 32]);
        let keys = HashMap::from([(
            "test-key".into(),
            URL_SAFE_NO_PAD.encode(signing.verifying_key().to_bytes()),
        )]);
        let token = sign(&test_claims(), &signing);
        assert!(verify_grant(&token, &"b".repeat(64), &keys)
            .unwrap_err()
            .contains("bu cihaz"));
    }

    #[test]
    fn seven_day_check_and_thirty_day_offline_limit_are_enforced() {
        let claims = test_claims();
        assert!(claims_status(
            &claims,
            &claims.device_hash,
            claims.last_online_at + Duration::days(29),
            None,
            None
        )
        .is_ok());
        assert!(claims_status(
            &claims,
            &claims.device_hash,
            claims.last_online_at + Duration::days(31),
            None,
            None
        )
        .is_err());
        assert!(should_attempt_check(
            true,
            claims.last_online_at,
            None,
            claims.last_online_at + Duration::days(7)
        ));
        assert!(!should_attempt_check(
            false,
            claims.last_online_at,
            None,
            claims.last_online_at + Duration::days(8)
        ));
        assert!(!should_attempt_check(
            true,
            claims.last_online_at,
            Some(claims.last_online_at + Duration::days(7)),
            claims.last_online_at + Duration::days(7)
        ));
    }

    #[test]
    fn explicit_server_denial_blocks_a_still_unexpired_offline_grant() {
        let claims = test_claims();
        assert!(claims_status(
            &claims,
            &claims.device_hash,
            claims.last_online_at + Duration::days(2),
            None,
            Some("revoked")
        )
        .is_err());
    }

    #[test]
    fn transient_network_failure_keeps_the_cached_grant_and_does_not_revoke() {
        let now = test_claims().last_online_at + Duration::days(8);
        let mut state = StoredLicenseState {
            grant: Some("signed-offline-grant".into()),
            activation_status: Some("active".into()),
            ..StoredLicenseState::default()
        };
        record_transient_failure(&mut state, now);
        assert_eq!(state.grant.as_deref(), Some("signed-offline-grant"));
        assert_eq!(state.server_denial, None);
        assert_eq!(state.last_attempt_at, Some(now));
    }

    #[test]
    fn clock_rollback_is_detected_from_the_last_locally_observed_time() {
        let claims = test_claims();
        let now = claims.last_online_at + Duration::days(2);
        assert!(claims_status(
            &claims,
            &claims.device_hash,
            now,
            Some(now + Duration::hours(1)),
            None,
        )
        .is_err());
    }

    #[test]
    fn a_new_signed_server_time_can_recover_a_forward_clock_jump() {
        let mut claims = test_claims();
        claims.issued_at += Duration::days(2);
        claims.last_online_at = claims.issued_at;
        claims.offline_until = claims.last_online_at + Duration::days(30);
        let erroneous_local_now = claims.last_online_at + Duration::days(10);
        assert!(claims_status(
            &claims,
            &claims.device_hash,
            erroneous_local_now,
            None,
            None
        )
        .is_ok());
        assert!(claims_status(
            &claims,
            &claims.device_hash,
            claims.last_online_at,
            None,
            None
        )
        .is_ok());
    }

    #[test]
    fn protected_server_issue_time_rejects_replayed_grants() {
        let claims = test_claims();
        assert!(!is_fresh_grant(claims.issued_at, Some(claims.issued_at)));
        assert!(is_fresh_grant(
            claims.issued_at + Duration::seconds(1),
            Some(claims.issued_at)
        ));
        assert!(is_fresh_grant(claims.issued_at, None));
        assert!(grant_replay_error(
            claims.issued_at - Duration::seconds(1),
            Some(claims.issued_at)
        )
        .is_some());
        assert!(grant_replay_error(claims.issued_at, Some(claims.issued_at)).is_none());
    }

    #[test]
    fn http_error_messages_preserve_server_decisions_and_explain_status() {
        assert_eq!(
            server_error_message(
                404,
                r#"{"error":"invalid_license_key","message":"Lisans anahtarı geçersiz."}"#
                    .as_bytes()
            ),
            "Lisans sunucusu isteği reddetti (404): Lisans anahtarı geçersiz."
        );
        assert!(
            server_error_message(403, br#"{"decision":"denied","reason":"revoked"}"#)
                .contains("iptal")
        );
        assert!(
            server_error_message(403, br#"{"decision":"denied","reason":"expired"}"#)
                .contains("dolmuş")
        );
    }

    #[test]
    fn approved_activation_has_an_explicit_refreshable_local_state() {
        let state = StoredLicenseState {
            activation_status: Some("approved".into()),
            ..StoredLicenseState::default()
        };
        let status = status_from_parts(&state, &"a".repeat(64), Utc::now(), false);
        assert_eq!(status.state, "approved");
    }

    #[test]
    fn optional_policy_does_not_require_an_activation() {
        let state = StoredLicenseState::default();
        let status = status_from_parts(&state, &"a".repeat(64), Utc::now(), false);
        assert_eq!(status.mode, "optional");
        assert!(status.can_calculate && status.can_finalize);
        assert!(!should_attempt_check(
            false,
            Utc::now() - Duration::days(10),
            None,
            Utc::now()
        ));
    }
}
