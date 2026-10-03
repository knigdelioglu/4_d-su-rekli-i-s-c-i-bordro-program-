fn main() {
    println!("cargo:rerun-if-env-changed=BORDRO_LICENSE_MODE");
    println!("cargo:rerun-if-env-changed=BORDRO_LICENSE_API_URL");
    tauri_build::build();
}
