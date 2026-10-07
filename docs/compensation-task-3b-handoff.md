# Görev 3B — Snapshot ve V6/V5 persistence handoff

## Requirement → owner → flow → success/failure → proof

| Gereksinim | Owner | Veri akışı | Başarı / hata davranışı | Kanıt |
|---|---|---|---|---|
| Bordro calculation metadata SQLite roundtrip | `db/migrations.rs`, `payroll_repo.rs` | `BordroKaydi.calculationMetadata` ↔ nullable JSON envelope | Eski satır `None`; typed envelope aynen döner; parent period validator reddederse write yok; FINALIZED değişmez | Migration upgrade ve repository roundtrip/rollback testleri |
| V6 JSON timeline/snapshot contract | `migration_service.rs`, `payrollPayload.ts`, `payrollPayloadSchema.ts`, `types/payroll.ts` | V6 DTO → exact Decimal sınırı → full-state/graph/snapshot validation | Missing collection, invalid state/graph/id/link/schema fail-closed; V5 → boş timeline, geçmiş ücretlerden tahmin yok | Native/browser V5/V6 fixtures ve invalid payload testleri |
| V2/V3/V5 compatibility thresholds | Native migration + browser payload codec | Source version → feature-specific compatibility branch | V2 required fields, V3 multi-accrual, V5 retro/payment/replay authority V6’da da etkin; yalnız V6 snapshot/timeline strict | V1–V5 compatibility fixtures |
| Canonical snapshot enum JSON tokens | Core serde models, native repository, WASM boundary, browser Decimal boundary | Typed enum ↔ frozen JSON literal | Nine tokens round-trip as canonical uppercase variant names; no wire-token rewrite | Core/WASM enum and browser Decimal boundary tests |
| Atomic restore ve immutable policy conflict | `migration_service.rs`, policy repository transaction API | Payload ön doğrulama → existing-policy conflict check → one DB transaction | Conflict/failure mevcut payroll, revisions, policies, tax ve retro state’i korur; `DELETE/REPLACE` immutable policy trigger’ını atlamaz | In-memory SQLite rollback, finalized/payment-link regression |
| Browser restore/revision/reload | `browserPayrollStore.ts`, `useBrowserPayrollPersistence.ts`, `confirmedBackupImport.ts`, `useBackupController.ts` | Parse/validate → IndexedDB CAS write/readback → authoritative payload adoption | CAS conflict kaydedilmemiş state’i korur; write/reload failure success bildirmez; restore sonrası fresh payload | Bun IDB/CAS/hook tests |

## Frozen API/schema kararları

- Bordro alanı `calculationMetadata?: PayrollCalculationMetadata`; metadata yoksa legacy kalır. `COMPENSATION_POLICY_ENGINE` + schema `1` + `compensation-engine-v1` değişmez.
- `payroll_core::validate_payroll_calculation_metadata(&BordroKaydi, &BordroDonemi)` snapshot ve parent-period için otoritedir. `NORMAL` `CALCULATED/FINALIZED` current-engine kaydı snapshotsız olamaz; legacy ve `DRAFT` istisnaları korunur.
- SQLite envelope alanı `calculation_metadata_json TEXT NULL` olacak; mevcut finansal kuruş kolonları değişmeyecek. JSON typed modeldeki Decimal alanları `rust_decimal::serde::str` / `str_option` ile saklanır.
- JSON backup sürümü `6`; native SQLite migration ve IndexedDB database version ayrı kalır. Özellik eşikleri sabitleri V2=2, multi-accrual=3, V5 authority=5, compensation timeline/snapshot=6 olarak kullanılacak; `CURRENT_BACKUP_VERSION` yalnız en yeni payload ve unsupported-future sınırını belirleyecek. Native V5, HEAD V5'in zorunlu tuttuğu retro/payment/replay alanlarını ister; ortak `exportedAt` ve `aktifDonemId` alanları native tarafta V6 eşiğindedir. Browser V5 codec'i bu iki alan için daha katı kalmıştır.
- V6 policy history tam `CompensationPolicyVersion[]`; tarihsel bordro segmenti var olan superseded sürüme bağlanabilir. Link doğrulaması ID varlığını ister, current active leaf eşitliği istemez.
- Restore’da immutable SQLite policy history silinmez. Mevcut policy kayıtları hedef payload’ın aynı ID/state/metadata kaydı olarak bulunmalı; hedeften eksik mevcut ID veya aynı ID için farklı içerik conflict olup restore’u reddeder. Hedefteki yeni sürümler transaction içinde topolojik sırayla eklenir.
- V1–V5 ve sürümsüz yedekler gerçek compensation timeline kaydı (dolu liste veya geçersiz non-null şekil) ya da non-null `calculationMetadata` taşıyamaz. Boş veya null timeline finansal veri taşımadığından uyumludur; V5 browser codec'i null'ı boş koleksiyona çevirir. V5 retro/payment/replay zorunlulukları korunur.
- V5 ve öncesi V6’ya çevrilebilir; timeline `[]` kalır, live setting/`zamAylari`/geçmiş bordrodan baseline üretilmez. V5 exact legacy replay ve retro/payment authority V6 içinde sürer. V6 current-engine snapshot’ları bu görevde legacy calculator ile replay edilmez.

## Başlangıç durumu ve sahiplik sınırı

- Çalışma ağacındaki Task 2/3A ve paralel Task 4 düzenlemeleri başlangıç state’idir; korunur. Engine, shadow hesaplama ve Task 4 test kapsamı değiştirilmedi; yalnız dört enum'un serde wire token düzeltmesi ve bu görevin snapshot fixture/regresyon testleri bu dar handoff'un içindedir.
- Repo kökünde fiziksel `AGENTS.md` bulunmadı; görev mesajındaki repo kuralları otoritedir. `production-engineering` skill uygulandı.
- Normal native dataset loader/App wiring 3B kapsamı dışındadır. Gerekli eksik bağlantı varsa burada App/loader genişletilmez; Task 5 handoff olarak işaretlenir.

## Uygulama ve doğrulama kaydı

### Tamamlanan 3B kapsamı

- Native SQLite migration 22, bordro metadata JSON kolonunu ekler; eski satırlar `NULL` kalır. Gelişmiş migration yollarında kolon onarımı ve tablo yeniden kurulumunda mevcut metadata'nın korunması da kapsanır.
- Native repository metadata'yı saklar/okur, parent-period validator'ını kayıt ve `FINALIZED` geçişinde uygular. Aynı `FINALIZED` değerine geçiş no-op kalır.
- Native ve browser V6 payload doğrulaması snapshot, timeline, policy-state ve bağlantıları doğrular. V5 exact legacy replay otoritesi V6 içinde korunur; V1–V5 payload'larına compensation metadata eklenmez. Eski sürümlerde dolu timeline veya non-null metadata sessizce atılmaz; null/boş timeline uyumludur.
- Native V6 export, veritabanındaki immutable policy timeline'ı ve tam Decimal metinlerini dışarı verir. Restore, mevcut immutable policy kayıtlarını silmeden transaction içinde uyum ve conflict kontrolü yapar.
- Browser persistence V5 uyarlamasını boş timeline ile yapar; CAS yazma, snapshot doğrulama ve yeniden yükleme hata davranışları fail-closed kalır.
- Paylaşılan `v6-current-engine-snapshot.json`, typed core testinden türetilmiş tam NORMAL snapshot ve policy timeline fixture'ıdır. Explicit tax opening, exact payment-event Decimal'ları, null/zero ve yüksek hassasiyetli policy Decimal'larını içerir; core, WASM serde, browser schema/Decimal parser ve native SQLite CALCULATED/FINALIZED save/export/restore sınırlarında kullanılır.
- Dört snapshot enum'unda `SCREAMING_SNAKE_CASE` rename kuralı, zaten büyük harfli Rust variant adlarını bozuyordu. Kural bu enum'lardan kaldırıldı; core, SQLite JSON, WASM ve browser sınırlarında `COMPENSATION_POLICY_ENGINE`, `COMPLETE`, `PARTIAL`, `EXPLICIT_OPENING`, `LEGACY_PERSONNEL_VALUE`, `IMPLICIT_ZERO`, `NONE`, `PAYMENT_EVENT`, `RETRO_OVERRIDE` doğrudan canonical JSON token'larıdır. Runtime token-rewrite adaptörleri kaldırıldı.

### Doğrulama sonuçları

| Komut | Sonuç |
|---|---|
| `bun test src/services/storage/browserPayrollStore.test.ts src/hooks/useBackupController.test.ts src/services/payrollEngine/decimalBoundary.test.ts` | Başarılı: 89 test, 0 hata. |
| `cargo test --manifest-path crates/payroll-core/Cargo.toml --test payroll_snapshot` | Başarılı: 19 test; fixture equality ve dokuz canonical enum token roundtrip'i dahil. |
| `cargo test --manifest-path crates/payroll-wasm/Cargo.toml` | Başarılı: 2 unit test; canonical enum'lar ve shared full snapshot boundary dahil. |
| `cargo test --manifest-path src-tauri/Cargo.toml --test migration_legacy_serde_parity_test --test compensation_policy_repo_test` | Başarılı: 38 test (28 parity/migration + 10 immutable policy repository). |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib ipc_tests::registered_tauri_ipc_commands_round_trip_storage_through_sqlite` | Başarılı: 1 test; HEAD V5 fixture ve V5 alan eşikleri korunuyor. |
| `bun run verify:production-graph` | Başarılı. |
| `rustfmt --check --edition 2021` (ilgili 6 Rust dosyası) ve `git diff --check` | Başarılı. |
| `bunx tsc --noEmit --pretty false` | Başarısız: repo genelinde 66 mevcut matcher tipi tanısı ve `PuantajGrid.tsx` generic çağrı tanısı var. Değişen production TS dosyalarında tanı yok; değişen browser testindeki tanılar eski `rejects`/`toBeUndefined` matcher satırlarında, yeni testlerde değil. |
| `bun run verify:wasm-freshness` | Başarısız: committed source hash `de05c7b5e44ba7a327eff625d2d39dd9247eb6edfc2af8cbeb1aea8874c4d328`, current Rust/WASM hash `4ea924a09a93ad33414e169f6d2090a9522b2fbd442bc6d0701b901e75c11b2f`. Generated `src/wasm/pkg/*` kullanıcı talimatıyla yeniden üretilmedi; package freshness, Task4 konsolidasyonundan sonra doğrulanmalı. |

### Açık doğrulama sınırları ve sahip handoff'ları

- Typed full-snapshot fixture core, WASM serde, browser schema/Decimal ve native SQLite CALCULATED/FINALIZED structural roundtrip'lerinde doğrulandı. Mevcut core engine iki production yolunda hâlâ `calculationMetadata: None` döndürüyor; fixture gerçek engine metadata üretimi veya canonical financial replay kanıtı değildir. Engine üretimi Task4, exact replay Task8 kapsamındadır.
- Native V5 `exportedAt`/`aktifDonemId` alanlarını HEAD semantiğine uygun biçimde zorunlu tutmaz; Browser V5 codec'i bu alanlarda daha katıdır. Bu bilinen adapter farkı korunmuştur.
- `verify:wasm-freshness` committed source hash ile current Rust/WASM hash'inin farklı olduğunu doğruladı. WASM package bu görevde değiştirilmedi; Task4 paket konsolidasyonundan sonra yeni binding üretilip freshness testi tekrar çalıştırılmalıdır.
- Tüm repository TypeScript komutu yeşil değildir. Mevcut matcher tanımları ve `PuantajGrid.tsx` generic hatası kapsam dışındadır; değişen production TS dosyalarında tanı yoktur.

### Kalan Task 5/8 handoff

- Task 5: native normal dataset loader, typed policy timeline’ı yükleyip authoritative runtime dataset’e bağlamalı; backup restore sonrası `loadData` yeni payload’ı getirmelidir.
- Task 8: V6 current-engine snapshot için canonical exact payroll replay; 3B’de yapılmış sayılmaz.
