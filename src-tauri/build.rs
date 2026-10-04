use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::VerifyingKey;

fn main() {
    println!("cargo:rerun-if-env-changed=BORDRO_LICENSE_MODE");
    println!("cargo:rerun-if-env-changed=BORDRO_LICENSE_API_URL");
    println!("cargo:rerun-if-changed=license-public-keys.json");
    if std::env::var("BORDRO_LICENSE_MODE").as_deref() == Ok("required") {
        let keys: serde_json::Value =
            serde_json::from_str(include_str!("license-public-keys.json"))
                .expect("BORDRO_LICENSE_MODE=required needs valid license-public-keys.json");
        let valid = keys.as_object().is_some_and(|keys| {
            !keys.is_empty()
                && keys.iter().all(|(kid, value)| {
                    !kid.trim().is_empty()
                        && value.as_str().is_some_and(|encoded| {
                            let Ok(bytes) = URL_SAFE_NO_PAD.decode(encoded) else {
                                return false;
                            };
                            let Ok(key_bytes): Result<[u8; 32], _> = bytes.try_into() else {
                                return false;
                            };
                            VerifyingKey::from_bytes(&key_bytes).is_ok()
                        })
                })
        });
        assert!(valid, "BORDRO_LICENSE_MODE=required needs at least one valid Ed25519 public key in license-public-keys.json");
    }
    tauri_build::build();
}
