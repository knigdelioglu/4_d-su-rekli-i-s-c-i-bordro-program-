# Bordro Test Güvence Durumu

Bu belge release kanıtının kısa manifestidir. Coverage ve mutation sayıları
komut çıktısından/CI artifact'larından alınır. Mutation baseline artık gerçek
8-shard CI artifact'ına dayanır; `pendingEvidence` yalnız bağımsız golden kanıtı
henüz tamamlanmamış fixture'ları belirtir.

## Son doğrulama kimliği

Bu CI run ID'leri aşağıdaki commit için tarihsel kanıttır; çalışma ağacı
doğrulamaları ayrıca listelenmiştir.

- Tarih: `2026-09-19`
- Verification identity: ölçülen kod commit'i
  `c9d0e9b838fbf2c6ae1a9fc1f1b07eaba11ea906`
- Normal CI / verify: `35439156749` — PASS
- Full mutation workflow: `35439833175` — PASS

## 2026-09-23 çalışma ağacı doğrulaması

- `bun run test:ci`: **207 geçti**; TypeScript coverage ratchet **16/16 metrikte PASS**.
- `bun run lint`: TypeScript typecheck ve production graph doğrulaması PASS.
- Golden corpus testi: **3 geçti**; 33/33 fixture ve evidence bağlantısı exact
  replay ile doğrulandı.
- Tauri mock-runtime IPC testi: **1 geçti**; 38/38 registered komut üretim
  handler'ı üzerinden çağrılıyor. Kritik storage akışları ayrıca SQLite
  round-trip ve state değişimiyle doğrulanıyor.
- Gerçek Linux Tauri WebView/IPC smoke testi CI'a eklendi; ilk CI koşusuyla
  doğrulanacak. Mevcut macOS çalışma ortamında native driver çalıştırılamadı.
- `PROPTEST_RNG_SEED=2026091001 bun run test:rust:coverage`: payroll-core ve
  Tauri test paketleri PASS; Rust coverage ratchet **28 dosya / 84 metrikte PASS**.
- `cargo fmt --all -- --check`, `git diff --check` ve mutation enforcement
  kontrolü PASS. Sentetik mutation gerilemesi beklenen şekilde blocking reddedildi.

## Golden corpus

- Fixture sayısı: **33**
- Kapsam: `G001`–`G033`, 2026 yasal parametre/sürüm sözleşmesi
- Blocking test: `cargo test -p payroll-core --test golden_payroll_corpus`
- Bağımsız evidence: **33/33 verified**; her fixture'ın request girdilerinden
  ayrı aritmetik hesabı `evidence/Gxxx.md` çalışma kâğıdına bağlıdır.
- Evidence formatı: `crates/payroll-core/tests/golden/evidence/Gxxx.md`;
  loader JSON Schema'yı, kritik bağlantının varlığını ve zorunlu hesap
  bölümlerini kontrol eder. Her fixture ayrıca `source.verificationStatus` ile
  `verified` veya `pendingEvidence` olarak açıkça işaretlenir.

## 2026 statutory parameter reference

- Bağımsız fixture: `crates/payroll-core/tests/statutory/2026.json`.
- `AnnualPayrollParameters::default_for_2026()` yıllık GV dilimleri ve yıllık
  brüt asgari ücret tavanı ile bu fixture'a exact Decimal karşılaştırılır.
- Günlük/aylık asgari ücret, işçi/işveren SGK ve işsizlik oranları, PEK tavan
  katsayısı, damga vergisi, GV yemek istisnası ve aylık GV referans matrahı da
  period alanları olarak karşılaştırılır.
- SGK yemek istisnası için 2026 4/D uygulama sözleşmesi `300.00` TL/gün
  değerini `verified` olarak taşır. Tarihsel veya uygulama kapsamı dışındaki
  değerler bu sözleşmenin parçası değildir.

## Generated WASM kanıtı

- `scripts/build-wasm.mjs`, workspace ve Cargo registry yollarını
  `CARGO_ENCODED_RUSTFLAGS` ile canonical metadata yollarına remap eder;
  boşluk içeren runner/workspace yolları tek flag olarak korunur ve Cargo
  `--locked`, non-incremental build kullanır.
- CI, `Cargo.lock` içindeki `wasm-bindgen` sürümüyle eşleşen Cargo-built
  `wasm-bindgen-cli 0.2.127` kurar. Böylece `wasm-pack`'in Linux'ta prebuilt
  CLI, macOS'ta Cargo-built CLI seçmesinden kaynaklanan code-section drift'i
  ortadan kaldırılır.
- CI canonical runner'da `src/wasm/pkg` için exact `git diff --exit-code`
  freshness kontrolü yapar. Type, code, data, import/export ve tüm custom
  section değişiklikleri ile beklenmeyen generated dosya değişiklikleri
  blocking failure'dır; binary drift normalizasyonla gizlenmez.
- Bu canonical freshness kontrolü Linux/macOS arasında generated `.wasm`
  byte-equality release şartı değildir. Bilinen code/data layout farkı bu turda
  P2 risk olarak korunur. Package allowlist, source freshness, WASM adapter ve
  browser E2E kontrolleri PASS durumundadır.

## Property ve oracle

- Property integration testleri: **19** (`crates/payroll-core/tests/property_tests.rs`)
- Property alanları: normal bordro finansal korunum, GV, PEK/carry,
  payment-event, retro determinism/conservation, validation ve Decimal.
- Test-only oracle modülleri: `tax_oracle`, `pek_oracle`, `exemptions_oracle`,
  `deductions_oracle`, `normal_payroll_oracle`.
- Oracle production hesap helper'larını import etmez; differential testler
  property suite içinde çalışır.

## Mutation evidence (Round 1 baseline)

- Araç: `cargo-mutants 27.1.0`
- Kritik kapsam: `calculations.rs`, `gv_exemption.rs`, `payroll_engine.rs`,
  `policies.rs`, `retro.rs`, `validation.rs`.
- Aday mutant sayısı: **2.147** (liste baseline'ı).
- Full run: `35013775841` — PASS; ölçülen commit
  `34198be740843e8ae22ae459e1a4263e7618cf82`. Sekiz shard'ın tamamı
  (`0..7`) ve summary job `104540908933` PASS'tır. Aggregate artifact
  `payroll-core-mutation-summary` içindeki gerçek sonuç:

  | Sayaç | Değer |
  |---|---:|
  | Total | 2.147 |
  | Caught | 1.349 |
  | Missed | 494 |
  | Timeout | 0 |
  | Unviable | 304 |
  | Meaningful | 1.843 |
  | Score | %73,20 |

## Payroll Engine Quality Round 2

- Başlangıç payroll-engine sonucu: **781 total, 408 caught, 241 missed,
  0 timeout, 132 unviable, 649 meaningful, %62,87**.
- Eklenen testler: `crates/payroll-core/tests/payroll_engine_quality_round2.rs`;
  gün/hakediş ve SGK gün ağırlığı, period/finalization boundary, accrual/retro
  carry, aynı-ay PEK, insurance GV ve earning classification public API
  regression/table-driven/property kapsamı.
- Targeted payroll-engine ölçümü: **781 total, 526 caught, 123 missed,
  0 timeout, 132 unviable, 649 meaningful, %81,05**.
- Full 8-shard aggregate (`35439833175`, summary job `105891611784`):
  **2.147 total, 1.483 caught, 360 missed, 0 timeout, 304 unviable,
  1.843 meaningful, %80,47**.
- Full aggregate payroll-engine sonucu: **781 total, 533 caught, 116 missed,
  0 timeout, 132 unviable, 649 meaningful, %82,13**.
- Full run verify, coverage, shard `0..7` ve `mutation-summary` PASS'tır.
- `docs/payroll-mutation-baseline.json` yeni full aggregate ile güncellendi;
  önceki 494 overall survivor listesi `previousBaseline` altında korunur.
- Payroll-engine final triage: `REAL_TEST_GAP 0`, `EQUIVALENT 0`,
  `UNREACHABLE 0`, `NEEDS_REVIEW 116`. Bu turda production payroll bug'ı
  bulunmadı ve production hesaplama kodu değiştirilmedi.

- Score exact olarak `caught / (total - unviable)` formülüyle hesaplanır:
  `1.349 / 1.843`. Summary, duplicate/missing shard, invalid JSON veya
  denominator uyuşmazlığında fail-closed davranır.
- Altyapı hataları (baseline, shard, JSON veya eksik artifact) blocking'dir.
  Full scheduled/manual aggregate artık
  `scripts/check-cargo-mutants-baseline.mjs --enforce` çalıştırır; score
  düşüşü, `missed`/`timeout` artışı veya yeni missed mutant aggregate'i
  başarısız kılar. Mevcut reviewed survivor'lar kabul edilen baseline olarak
  korunur ve tek başlarına engel oluşturmaz.
- Her shard'ın canonical yolu `target/cargo-mutants/outcomes.json`'dır;
  cargo-mutants 27.1.0'un tool-native `target/mutants.out` klasörü CI'da tek
  seferde bu dizine taşınır. Survivor triage kaydı
  `docs/payroll-mutation-survivor-review.md` içindedir.
- İlk yeni koşuya göre 31 missed mutant caught olmuştur; policies.rs
  missed 16'dan 5'e, retro.rs missed 161'den 141'e inmiştir.
- Yerel smoke kanıtı (`gv_exemption.rs`): 7 toplam, 6 caught, 0 missed,
  0 timeout, 1 unviable; anlamlı skor **%100**.
- Unviable mutant'lar sessizce exclude edilmez; araç çıktısında ayrı raporlanır.

## Rust coverage baseline ve ratchet

Komut:

```text
PROPTEST_RNG_SEED=2026091001 cargo llvm-cov --package payroll-core --package bordro-programi --all-features --json --summary-only
node scripts/check-rust-coverage.mjs <coverage.json> docs/payroll-coverage-baseline.json
```

| payroll-core kritik modül | Lines | Functions | Regions |
|---|---:|---:|---:|
| `calculations.rs` | 91.86% | 92.54% | 95.91% |
| `gv_exemption.rs` | 100.00% | 100.00% | 100.00% |
| `payroll_engine.rs` | 84.23% | 69.41% | 88.36% |
| `policies.rs` | 91.88% | 87.67% | 94.11% |
| `retro.rs` | 88.51% | 77.65% | 90.46% |
| `validation.rs` | 76.63% | 73.08% | 87.80% |

Ratchet dosya bazında lines/functions/regions metriklerinin exact
`covered/count` oranını baseline'ın altına indirmeye izin vermez; dosya/metric
eksikliği, duplicate path ve NaN/undefined değerler fail'dir. Baseline 6
`payroll-core` ve 22 Tauri DB/repository/service/IPC dosyasını içerir. Ölçülen
tüm kaynak toplamı lines `%82.15`, functions `%65.38`, regions `%82.57`'tir;
blocking eşik toplam değil, dosya bazında exact ratchet'tır. PR'daki
`CI / verify` job'ı `bun run test:rust:coverage` ile coverage ve ratchet'ı
çalıştırır; ölçüm `PROPTEST_RNG_SEED=2026091001` kullanır.

- `src-tauri/src/lib.rs`: lines `%97.95`, functions `%71.43`, regions `%96.69`.
- `src-tauri/src/commands/period_cmd.rs`: lines `%57.14`, functions `%50.00`,
  regions `%66.67`; `get_periods`, `save_period` ve
  `save_period_with_settings` production handler üzerinden çağrılıyor.

## TypeScript coverage ratchet

- `bun run test:ci`, Bun test coverage'ını LCOV olarak üretir ve
  `docs/typescript-coverage-baseline.json` içindeki 8 finansal adapter/storage
  dosyasının satır/fonksiyon oranlarında düşüşü PR'da engeller.
- Checker eksik/duplicate LCOV kaydında ve baseline düşüşünde fail-closed
  davranır. `browserPayrollStore.ts` için başlangıç tabanı 145/347 satırdır;
  bu oran düşük olduğu için iyileştirme adayı olarak görünür kalır.
- CI, verify job'ında bu komutu her pull request'te çalıştırır.

## Native command IPC

- `src-tauri/src/lib.rs` içindeki production `invoke_handler` Tauri mock
  runtime'ında 38/38 registered komut için doğrudan sınanır. Period/settings,
  personnel/tax-opening, attendance, annual parameters, app settings ve sick
  leave create/read/update/delete turları SQLite state'iyle doğrulanır.
- Aynı test `FINALIZED` durumuna doğrudan geçişin reddedildiğini, retro preview
  ve mutation-policy okumalarını, legacy import ve izole backup replacement
  akışlarını da doğrular. Eksik payroll/retro kimlik veya DRAFT durumlarında
  ilgili komutların fail-closed yanıtları sınanır.
- Aynı IPC testi geçerli bordro için hesaplama → tahakkuk silme → yeniden
  hesaplama → kesinleştirme zincirini SQLite state'i üzerinden tamamlar.
- Mock runtime Tauri dispatch'i, argüman/yanıt serileştirmesini ve uygulama
  state'ini kapsar; işletim sistemi WebView'ini açan masaüstü uçtan uca testi
  değildir.
- `.github/workflows/ci.yml`, Linux'ta `tauri-driver` ve WebKitWebDriver ile
  uygulamayı açar; gerçek WebView render'ını, storage yüklemesini ve
  `get_periods` IPC yanıtını doğrular (`bun run test:e2e:native`).

## Korunan kritik invariant'lar

- `netOdeme = gelirToplam - kesintiToplam`
- PEK tavanı, alt sınır tamamlama ve devreden PEK kapasite korunumu
- artan oranlı GV dilim hesabı, monotonicity ve yeni yılda kümülatif sıfırlama
- aylık asgari ücret GV istisnası hakkının aşılmaması
- DV matrahı/istisnası ve negatif vergi reddi
- aynı input/payment-event sırasının deterministik olması
- STALE dependency'nin fail-closed reddi
- retro replay'in deterministik ve konservatif olması
- puantaj dönem geometrisi, tarih ve rapor overlap doğrulaması
- Decimal JSON round-trip ve scientific notation sınırı

## Bilinen boşluklar

- 2.147 adayın Round 2 gerçek 8-shard aggregate sonucu baseline'a
  kaydedilmiştir: `docs/payroll-mutation-baseline.json` — **360 missed, %80,47**.
  Önceki full baseline listesi aynı dosyada `previousBaseline` altında
  korunur. Scheduled/manual aggregate, score düşüşü, missed/timeout artışı ve
  yeni missed mutant'ta blocking'dir; kapsamlı mutant çalışması her PR'da
  çalıştırılmaz.
- Round 1'deki 494 anlamlı missed mutant için historical conservative triage:
  `REAL_TEST_GAP` 0, `EQUIVALENT` 2, `UNREACHABLE` 0,
  `NEEDS_REVIEW` 492. Ayrıntı ve kapsam sınırı
  `docs/payroll-mutation-survivor-review.md` içindedir; blanket exclusion yoktur.
- Round 2 payroll-engine survivor'ları için conservative triage:
  `REAL_TEST_GAP` 0, `EQUIVALENT` 0, `UNREACHABLE` 0,
  `NEEDS_REVIEW` 116. Ayrıntı `Payroll Engine Quality Round 2` bölümündedir.
- Mock IPC testi 38 registered komutun tümünü çağırır ve başarılı bordro
  hesaplama/silme/yeniden hesaplama/kesinleştirme zincirini kapsar. Linux native
  WebView smoke açılış, storage yükleme ve `get_periods` ile sınırlıdır;
  Windows/macOS native E2E kapsamı açık kalır.
- Linux/macOS generated WASM code/data layout farkı P2 riskidir; package
  allowlist, freshness, adapter ve browser E2E kontrolleri korunmuştur.
- TypeScript kapısı yalnız 8 kritik finansal adapter/storage dosyasını kapsar;
  tüm UI bileşenleri için toplam repository coverage tabanı eklenmemiştir.
- Mutation baseline'da kalan survivor'lar için gerekçe veya kalıcı regression
  testi ayrıca yazılmalıdır; bu turda kesinleştirilemeyen vakalar NEEDS_REVIEW
  olarak bırakılmıştır.
- Yeni gerçek production hesap hatalarının kalıcı regression fixture'ına
  dönüştürülmesi PR şablonunda kontrol edilir; gelecekteki her düzeltmede
  kutucuğun tamamlanması author/reviewer incelemesine bağlıdır.

## Branch protection

MAIN BRANCH PROTECTION: false; manual GitHub configuration required
Required check önerisi: `CI / verify`

Bu yerel değişikliklerle koruma etkinleştirilemedi: GitHub CLI oturumu geçersiz
token bildiriyor. `CI / verify` check'inin main branch protection'a eklenmesi
GitHub erişimi yenilendiğinde tamamlanmalıdır.
