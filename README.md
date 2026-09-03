# 4/D Sürekli İşçi Bordro Programı

React + TypeScript arayüzü, Tauri/Rust servisleri ve SQLite yerel veritabanı kullanan 4/D sürekli işçi bordro uygulamasıdır. Üretim bordro hesabı, native Tauri ve bağımsız tarayıcı çalışma zamanları tarafından paylaşılan tek Rust `payroll-core` motorunda yapılır.

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
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
bun run wasm:build
bun run wasm:test
bun run build
bun run web:build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

`bun run dev`, `bun run build` ve `bun run web:build` öncesinde tarayıcı motoru için
`wasm-pack` ve `wasm32-unknown-unknown` hedefi gerekir. `wasm:build` komutu
üretilen glue kodunu ve `.wasm` dosyasını `src/wasm/pkg/` altında oluşturur;
bu çıktı kaynak kontrolüne alınmaz.

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

## Tarayıcı çalışma zamanı

Tauri dışı kullanımda bordro hesaplama WASM üzerinden, veri kaydı ise yalnızca
kullanıcının tarayıcısında yapılır. Uygulama önce IndexedDB'deki sürüm kontrollü
snapshot'ı kullanır; eski `localStorage` yedeği ilk başarılı okumada IndexedDB'ye
taşınır ve kurtarma amacıyla silinmez. Sunucu, API veya ortak backend yoktur.

Tarayıcı hesaplaması da native akışla aynı 15–14 dönem, kümülatif GV, PEK,
raporlu gün, STALE ve FINALIZED kurallarını uygular. Kaynak veri değişikliği
hesaplanmış açık bordroları STALE yapar; kesinleştirilmiş tarihçeyi etkileyen
değişiklikler görünür bir hata ile reddedilir. Tarayıcıda JSON yedek/geri
yükleme, puantaj ve liste Excel çıktıları, ücret pusulası Excel/PDF çıktıları
ve yazdırma akışı cihaz üzerinde çalışır. SQLite'a bağlı native toplu rapor
sorgusu veya sunucu tabanlı bir çıktı servisi yoktur.

Native notice servisi repository-backed ayrıntılı bilgilendirme üretebilir; tarayıcı
WASM notice katmanı ortak hesaplamayı etkileyen eksik veri, STALE ve rapor/puantaj
uyumsuzluklarını görünür ve bloklayıcı şekilde raporlar.

## Veri güvenliği

JSON yedekleri sürüm 2 sözleşmesiyle dönem, personel, kurum ayarları, puantaj, bordro, vergi açılışları, hastalık raporu kayıtları ve yıllık bordro parametrelerini birlikte içerir. Native içe aktarma ve örnek veri sıfırlama, mevcut domain verisini tek SQLite transaction içinde silip yükler; tarayıcı akışı aynı sözleşmeyi IndexedDB snapshot'ı olarak uygular. Kesinleştirilen (`FINALIZED`) bordrolar yeniden hesaplanamaz veya geriye alınamaz.

Vergi açılışında authoritative kaynak `personnel_tax_opening` tablosudur. Personel formundaki devir alanları yalnızca bu tabloda aynı vergi yılı için ayrı açılış bulunmadığında geriye dönük uyumluluk fallback'idir; iki kaynak çakışırsa tablo kaydı önceliklidir.
