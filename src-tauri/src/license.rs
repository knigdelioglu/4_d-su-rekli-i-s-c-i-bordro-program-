use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

const OFFLINE_GRACE_DAYS: i64 = 30;
const SERVER_CHECK_DAYS: i64 = 7;
const RETRY_HOURS: i64 = 6;
const CHECK_LOOP_SECONDS: u64 = 60;
const LICENSE_STATE_FILE: &str = "license-state.json";
const DEVICE_ID_FILE: &str = "license-device-id";
const PUBLIC_KEYS: &str = include_str!("../license-public-keys.json");

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

fn read_device_id(dir: &Path) -> Result<String, String> {
    let path = dir.join(DEVICE_ID_FILE);
    match fs::read_to_string(&path) {
        Ok(value) => {
            let value = value.trim();
            if Uuid::parse_str(value).is_ok() {
                return Ok(value.to_string());
            }
            return Err("Cihaz kimliği dosyası bozuk. Lisans desteğine başvurun.".into());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Cihaz kimliği dosyası okunamadı.".into()),
    }
    let id = Uuid::new_v4().to_string();
    write_private_file(&path, id.as_bytes())?;
    Ok(id)
}

fn device_hash(dir: &Path) -> Result<String, String> {
    let id = read_device_id(dir)?;
    Ok(hex(&Sha256::digest(id.as_bytes())))
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
    if claims.issued_at > now + Duration::minutes(5)
        || claims.last_online_at > now + Duration::minutes(5)
    {
        return Err("Cihaz saati lisans doğrulamasıyla uyuşmuyor.".into());
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
    let (status, license_id, expires_at, last_online_at, offline_until, validation_reason) =
        match claims {
            Ok(Some(claims)) => {
                let reason = claims_status(
                    &claims,
                    device_hash,
                    now,
                    state.last_seen_at,
                    state.server_denial.as_deref(),
                )
                .err();
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
    let body = response
        .json::<T>()
        .await
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
    claims_status(&claims, device_hash, now, state.last_seen_at, None)?;
    state.grant = Some(token);
    state.server_denial = None;
    state.activation_status = Some("active".into());
    state.activation_request_id = None;
    state.last_seen_at = Some(now);
    state.last_attempt_at = Some(now);
    write_state(dir, state)
}

pub fn get_status(dir: &Path) -> Result<LicenseStatus, String> {
    ensure_data_dir(dir)?;
    let device_hash = device_hash(dir)?;
    let state = read_state(dir)?;
    Ok(status_from_parts(
        &state,
        &device_hash,
        Utc::now(),
        enforcement_required(),
    ))
}

pub async fn activate(dir: &Path, license_key: &str) -> Result<LicenseStatus, String> {
    ensure_data_dir(dir)?;
    let device_hash = device_hash(dir)?;
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
    let mut state = read_state(dir)?;
    state.grant = None;
    state.server_denial = None;
    state.activation_status = Some(response.status);
    state.activation_request_id = Some(response.request_id);
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
    if state.grant.is_some() && state.server_denial.is_none() {
        remote_check(dir, &device_hash, &mut state).await?;
    } else if state.activation_request_id.is_some() {
        refresh_activation(dir, &device_hash, &mut state).await?;
    } else {
        return Err("Önce lisans anahtarınızı girip aktivasyon talebi oluşturun.".into());
    }
    Ok(status_from_parts(
        &state,
        &device_hash,
        Utc::now(),
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
    let token = state
        .grant
        .as_deref()
        .ok_or_else(|| format!("Lisans etkin değil. Yeni {operation} işlemi durduruldu."))?;
    let claims = verify_grant(token, &device_hash, &public_keys()?)?;
    let now = Utc::now();
    claims_status(
        &claims,
        &device_hash,
        now,
        state.last_seen_at,
        state.server_denial.as_deref(),
    )
    .map_err(|reason| {
        format!("Lisans uygun değil. Yeni {operation} işlemi durduruldu: {reason}")
    })?;
    state.last_seen_at = Some(now);
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
