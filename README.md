# 4/D Sürekli İşçi Bordro Programı

React + TypeScript arayüzü, Tauri/Rust servisleri ve SQLite yerel veritabanı kullanan 4/D sürekli işçi bordro uygulamasıdır. Üretim bordro hesabı Rust tarafındaki tek hesap motorunda yapılır; tarayıcı modu kayıtları saklama, içe/dışa aktarma ve inceleme için kullanılabilir.

## Gereksinimler

- Bun
- Rust toolchain ve Cargo
- Tauri geliştirme bağımlılıkları

## Geliştirme ve doğrulama

```bash
bun install
bun run dev
bun test
bun run lint
bun run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Rust test döngüsünü hızlandırmak için bağımlılık crate'leri `dev` ve `test`
profillerinde 3. seviye optimizasyonla derlenir; uygulama kodu hızlı debug
derlemesinde kalır. Küçük değişikliklerde `bun run check:rust` veya doğrudan
`cargo check --manifest-path src-tauri/Cargo.toml` kullanın.

Paralel test çalıştırma için bir kez `cargo install cargo-nextest --locked`
kurduktan sonra `bun run test:rust` otomatik olarak nextest'i kullanır.
`cargo-nextest` kurulu değilse aynı komut standart `cargo test`e düşer;
`bun run test:rust:nextest` ise nextest'i zorunlu kılan doğrudan komuttur.

macOS'ta LLVM `lld` kuruluysa hızlı linker yapılandırması açıkça etkinleştirilebilir:

```bash
cargo --config src-tauri/.cargo/config.fast.toml nextest run --manifest-path src-tauri/Cargo.toml
```

Bu yapılandırma varsayılan `config.toml` olarak etkin değildir; böylece `lld`
kurulu olmayan geliştirici ve CI makinelerinde test akışı kırılmaz.

macOS güvenlik taraması derleme döngüsünü yavaşlatıyorsa yalnızca proje
önbelleği olan `src-tauri/target/` dizinini kurumunuzun güvenlik politikasına
uygun biçimde tarama dışı bırakmayı değerlendirin.

Tauri geliştirme akışı için `bun tauri dev` kullanılabilir. SQLite verisi kullanıcı uygulama dizinindeki `4d_bordro_data/bordro.sqlite` dosyasında tutulur.

## Veri güvenliği

JSON yedekleri sürüm 2 sözleşmesiyle dönem, personel, kurum ayarları, puantaj, bordro, vergi açılışları, hastalık raporu kayıtları ve yıllık bordro parametrelerini birlikte içerir. İçe aktarma ve örnek veri sıfırlama, mevcut domain verisini tek SQLite transaction içinde silip yükler. Kesinleştirilen (`FINALIZED`) bordrolar yeniden hesaplanamaz veya geriye alınamaz.

Vergi açılışında authoritative kaynak `personnel_tax_opening` tablosudur. Personel formundaki devir alanları yalnızca bu tabloda aynı vergi yılı için ayrı açılış bulunmadığında geriye dönük uyumluluk fallback'idir; iki kaynak çakışırsa tablo kaydı önceliklidir.
