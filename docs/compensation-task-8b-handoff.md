# Görev 8B — normal runtime switch ve legacy cleanup

## Owner / çağrı haritası

Bu görev `payroll-core` normal hesap modeli ve release module wiring; native
payroll servis/loader/command/repository sınırı; WASM ve TypeScript hesap
adapter'ı; App tekli/batch/preflight akışı; aktif dönem ayarı ve restore DTO
sınırlarının tek sahibidir. Önceki 1–8A çalışma ağacı korunur. Ana migration
contract ve task haritası orchestrator sahipliğindedir.

| Akış | Mevcut giriş ve veri yolu | Bu görevde hedef |
|---|---|---|
| Normal core hesap | `calculate_payroll_checked` → legacy `calculate_payroll` / `calculate_payroll_with_index`; `resolve_raise_transition` `zamAylari` ve period settings ile geçmiş dönem tahmini yapar. | Checked/default NORMAL girişi policy resolver + schema-2 snapshot üretimine geçer; legacy formül ve fixture oracle'ı yalnız dev/test bölümünde kalır. |
| Native tekli hesap/finalize | Tauri `calculate_payroll` → `PayrollService::calculate_payroll_for_accrual_checked`; default preflight `validate_payroll_request_for_accrual`; finalize `finalize_payroll_for_accrual`. Service'in policy'li varyantı ayrı opt-in'dir. | Aynı normal policy loader hem preflight hem calculate tarafından kullanılır; finalize saved provenance/snapshot replay'i seçer. Supplemental/TEDIYE/TIS/RETRO kendi event girdileriyle çalışır. |
| Browser/WASM ve App | `PayrollEngine.calculatePayroll` → `calculate_payroll_json`; `PayrollEngine.validatePayroll`; App/hook tekli ve batch yolları bu default adapter'ı çağırır. Ayrı policy API'si vardı. | Default WASM ve UI yolu policy resolver'ına gider; native ve browser baseline eksikliğinde hesap/preflight fail-closed olur, UI policy başlangıç-state ekranını açar. |
| Dataset ve storage | Native period loader `institutionSettings`, `zamAylari`; TS dataset aynı alanları taşır. Period settings ve eski restore okuyucuları compensation parasal alanları içerir. | Aktif dataset'te NORMAL compensation yalnız scoped policy timeline/snapshot'tır. Eski JSON alanları yalnız legacy DTO/import adapter'da okunur; statutory, kesinti/yemek uygulama tercihleri ve Tediye/TİS korunur. |
| Retro / historical | Schema-2 replay saved inputs ile ortak engine'i kullanır; snapshot'sız legacy replay ayrı adapter'dır. | Retro runtime eski normal inference motoruna bağlanmaz; exact persisted source adapter yoksa `LEGACY_REPLAY_UNAVAILABLE` ile fail-closed olur. |

### Sürüm otoritesi ve özellik otoritesi

- Backup sürümü, hangi wire alanlarının okunup korunacağını ve hangi migration
  doğrulamasının uygulanacağını belirler; tek başına bordronun yeni policy
  engine'iyle üretildiğini kanıtlamaz.
- V5'in kayıtlı hesap/GV/PEK ve retro snapshot'ları tarihsel otoritedir ve
  policy timeline olmadan da atomik biçimde restore edilip aynen korunabilir.
  V5 restore yeni policy sürümü üretmez. Yeni NORMAL hesap için kullanıcı açık
  baseline kaydedene kadar native/browser preflight fail-closed kalır.
- Tam schema-2 replay yalnız ilgili bordro satırında
  `calculationMetadata.provenance = COMPENSATION_POLICY_ENGINE` ve snapshot
  bulunduğunda zorunludur. V6 top-level timeline ve bu satır-özellik sınırı
  ayrı doğrulanır; metadata'sız tarihsel V6/V5 satırları V6 replay kuralıyla
  geriye dönük yeniden yorumlanmaz.
- Legacy kaynak verisi mevcutsa V5 import adapter'ı eski kayıtları kendi
  wire/source anlamıyla doğrular. Dated raise/transport bilgisi exact legacy
  kaynak hesabı gerektiren bir retro replay'i desteklemiyorsa yalnız o replay
  `LEGACY_REPLAY_UNAVAILABLE` olur; backup'ın genel restore/read hakkı düşmez.

### Gereksinim → kanıt

- Default NORMAL API, native/browser loader, preflight ve tekli/batch UI aynı
  resolver'a gider; 2026-09–2027-08, 15–14 sınırları, V5/V6 ve finalize
  acceptance'i bu gerçek girişlerden tekrar geçer.
- Policy baseline yoksa hiçbir hesap/restore tahminle state yazmaz; native ve
  Chromium integration testleri ledger'ın değişmediğini ve UI yönlendirmesini
  doğrular.
- Release graph'ta legacy raise/transport inference yoktur. Eski parity
  implementation yalnız bağımsız dev/test oracle'dır; N≥3 goldens ve per-income
  rounding ile 1–2 segment tam bordro parity testleri default route'u denetler.
- Aktif period settings, generated contract, V1–V6 restore, invalidation ve
  immutable FINALIZED/snapshot semantiği regression ve freshness/package
  kontrolleriyle doğrulanır.

## Uygulama ve doğrulama sonuçları

- V5 backup restore tam arşiv kaynağını kullanıyor. Asgari ücret GV referans
  verisi bulunmadığında yalnız etkilenen tarihsel satır replay dışında kalıyor;
  bu dar istisna dışındaki doğrulama ve replay hataları aktarılıyor. V6 replay,
  schema-2 provenance/snapshot taşıyan yeni-engine satırları için zorunlu.
- Aktif DTO'dan eski `zamAylari` alanı çıkarıldı; eski global ayar yalnız V5
  uyumluluk/import yolunda okunuyor. Dönem ücret politikası açık policy
  sürümünden geliyor; eksik policy baseline'ı eski yevmiye verisiyle sessizce
  doldurulmuyor. V5/V6 arşiv ve timeline davranışı için parity testleri eklendi.
- Rust doğrulamaları başarılı: `cargo test -p payroll-core --lib --no-fail-fast`
  (76 test), `cargo test -p payroll-core --tests --no-fail-fast`,
  `cargo test -p payroll-wasm --no-fail-fast` (2 test), migration parity
  entegrasyon hedefi (31 test) ve ilgili native entegrasyon hedefleri geçti.
  Tüm native entegrasyon hedefleri `--no-run` ile derlendi. Fixture düzeltmesinden
  sonra `cargo test --workspace --no-fail-fast` bir kez başarıyla tamamlandı.
- WASM doğrulaması başarılı: `bun run wasm:build`, `bun run wasm:test` (9 test)
  ve `bun run verify:wasm-package` / `bun run verify:wasm-freshness` geçti.
  Payroll contract ve production graph doğrulamaları ile
  `cargo fmt --all -- --check` de başarılı.
- İlgili web testleri geçti: raise transition ve browser store testleri (77),
  Chromium compensation-policy API testleri (7). `bun run build` üretim derlemesi
  başarılı; Vite 500 kB üzeri JavaScript chunk uyarısı verdi.
- `bun run lint` başarısız: TypeScript derlemesi, testlerde kullanılan matcher
  tiplerinde eksik assertion metotları için çok sayıda hata veriyor; ayrıca
  `src/components/PuantajGrid.tsx:826` satırında typesız fonksiyon çağrısına
  generic veriliyor. Bu hatalar bu doğrulama turunda giderilmedi.
