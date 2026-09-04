# 4/D Bordro

React + TypeScript arayüzü, Tauri/Rust servisleri ve SQLite yerel veritabanı kullanan 4/D sürekli işçi bordro uygulamasıdır. Üretim bordro hesabı, native Tauri ve bağımsız tarayıcı çalışma zamanları tarafından paylaşılan tek Rust `payroll-core` motorunda yapılır.

Tarayıcı production dağıtımı Netlify üzerinde statiktir:

```text
Netlify → HTML / JS / CSS / WASM → Browser CPU
                           └────→ IndexedDB
```

Netlify yalnız static hosting/CDN görevi görür. Netlify Functions, payroll
server'ı ve remote payroll backend yoktur.

## Gereksinimler

- Bun 1.3.14
- Rust toolchain ve Cargo
- `wasm-pack` 0.15.0
- Tauri geliştirme bağımlılıkları

Proje Rust toolchain'ini `rust-toolchain.toml` ile 1.97.1'e ve browser WASM
üretim aracını CI ile 0.15.0'a sabitler. Böylece kaynak değişmediğinde
üretilen `src/wasm/pkg/` çıktısının toolchain sürüm farkları yüzünden drift
etmesi önlenir.

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
bun run web:verify
bun run verify:privacy
bun run verify:wasm-package
bun run web:build
bunx playwright install chromium
bun run test:e2e:typecheck
bun run test:e2e
bun run test:netlify-smoke
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

`bun run build` yalnız Vite static build'idir; Netlify build aşamasında Rust
server veya bordro hesabı çalışmaz. `wasm:build` komutu üretilen glue kodunu ve
`.wasm` dosyasını `src/wasm/pkg/` altında oluşturur. Bu generated çıktı
repository içinde tutulur; Netlify static build sırasında Rust toolchain
gerektirmemesi için deploy'un parçasıdır. CI, `payroll-core` kaynağından yeniden
üretilen WASM ile tracked `src/wasm/pkg` çıktısının aynı olduğunu doğrular.

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

Tauri dışı kullanımda gerçek zincir `React → PayrollEngine → Rust/WASM →
payroll-core` şeklindedir; bordro hesaplaması browser CPU üzerinde yapılır.
Veri kaydı yalnızca kullanıcının tarayıcısındaki IndexedDB sürüm kontrollü
atomic snapshot'ına yapılır. IndexedDB yoksa uygulama görünür hata verir ve
payroll verisi başka bir browser storage'a yazılmaz. Eski `localStorage` yedeği
yalnız migration read olarak, IndexedDB kullanılabilirken doğrulanıp taşınır;
kurtarma amacıyla hemen silinmez. Küçük UI tercihleri (ör. aktif sekme)
localStorage'da tutulabilir. Sunucu, API veya ortak backend yoktur.

Personel, bordro, puantaj, dönem ve kurum verileri Netlify'a veya başka bir
sunucuya gönderilmez.

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

Tarayıcıdaki authoritative finansal değerler Decimal string olarak korunur:
`Rust Decimal ↔ WASM JSON DTO ↔ browser domain/storage DTO ↔ IndexedDB`.
`PayrollUiModel` yalnız sunum ve kullanıcı girişi için kullanılır; `Number`,
`parseFloat` ve benzeri dönüşümler authoritative hesaplama veya persistence
yoluna geri sokulmaz. `bun run test:netlify-smoke` gerçek Netlify local
runtime'ında header, CSP, WASM yüklenmesi, IndexedDB ve hesaplama akışını
doğrular.

## Veri güvenliği

JSON yedekleri sürüm 2 sözleşmesiyle dönem, personel, kurum ayarları, puantaj, bordro, vergi açılışları, hastalık raporu kayıtları ve yıllık bordro parametrelerini birlikte içerir. Native içe aktarma ve örnek veri sıfırlama, mevcut domain verisini tek SQLite transaction içinde silip yükler; tarayıcı akışı aynı sözleşmeyi IndexedDB snapshot'ı olarak uygular. Kesinleştirilen (`FINALIZED`) bordrolar yeniden hesaplanamaz veya geriye alınamaz.

Tarayıcıdaki mevcut IndexedDB snapshot'ı JSON parse ve V2 sürüm kontrolünden
sonra `payrollPayloadSchema` içindeki tam domain şemasıyla doğrulanır; yalnız bu
adım geçerse exact Decimal kontrolünden geçip authoritative state'e alınır.
Bilinen alanların zorunluluk ve tip kontrolleri fail-closed'dur; mevcut V2
nesnesindeki bilinmeyen ekstra alanlar ileriye dönük uyumluluk için reddedilmez
ve korunur. Legacy JSON ise yalnız açık canonicalization adımından sonra aynı
tam şema kontrolünü geçerse IndexedDB'ye yazılır. Bu yapısal kontrol Rust'taki
bordro/mevzuat iş kurallarının yerine geçmez.

Tarayıcı snapshot bütünlüğü ayrıca native SQLite'ın persistence seviyesindeki
UNIQUE ve foreign-key invariants kurallarını mümkün olduğunca mirror eder
(personel TC, kişi+dönem kayıtları, vergi yılı/ayı ve kurum ayarı referansları).
Bunlar bordro business rule'ları değil, bozuk snapshot'ın authoritative state'e
girmesini önleyen storage kontrolleridir.

Vergi açılışında authoritative kaynak `personnel_tax_opening` tablosudur. Personel formundaki devir alanları yalnızca bu tabloda aynı vergi yılı için ayrı açılış bulunmadığında geriye dönük uyumluluk fallback'idir; iki kaynak çakışırsa tablo kaydı önceliklidir.
