# Bordro Test Güvence Durumu

Bu belge release kanıtının kısa manifestidir. Coverage ve mutation sayıları
komut çıktısından/CI artifact'larından alınır. Mutation baseline artık gerçek
8-shard CI artifact'ına dayanır; `pendingEvidence` yalnız bağımsız golden kanıtı
henüz tamamlanmamış fixture'ları belirtir.

## Son doğrulama kimliği

- Tarih: `2026-09-19`
- Verification identity: ölçülen kod commit'i
  `c9d0e9b838fbf2c6ae1a9fc1f1b07eaba11ea906`
- Normal CI / verify: `35439156749` — PASS
- Full mutation workflow: `35439833175` — PASS

## Golden corpus

- Fixture sayısı: **33**
- Kapsam: `G001`–`G033`, 2026 yasal parametre/sürüm sözleşmesi
- Blocking test: `cargo test -p payroll-core --test golden_payroll_corpus`
- Bağımsız evidence: **15/33 verified** — `G001, G003, G006, G007, G009,
  G010, G011, G013, G014, G016, G017, G029, G030, G031, G032`;
  kalan **18** fixture `pendingEvidence` durumundadır.
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
- Altyapı hataları (baseline, shard, JSON veya eksik artifact) blocking'dir;
  `missed`/`timeout` quality findings advisory'dir.
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
cargo llvm-cov --package payroll-core --all-features --json --summary-only
node scripts/check-rust-coverage.mjs <coverage.json> docs/payroll-coverage-baseline.json
```

| Kritik modül | Lines | Functions | Regions |
|---|---:|---:|---:|
| `calculations.rs` | 84.82% | 82.09% | 91.12% |
| `gv_exemption.rs` | 100.00% | 100.00% | 100.00% |
| `payroll_engine.rs` | 78.56% | 65.46% | 84.14% |
| `policies.rs` | 81.64% | 79.45% | 84.18% |
| `retro.rs` | 80.20% | 72.35% | 85.16% |
| `validation.rs` | 71.81% | 57.69% | 79.90% |

Ratchet dosya bazında lines/functions/regions metriklerinin exact
`covered/count` oranını baseline'ın altına indirmeye izin vermez; dosya/metric
eksikliği, duplicate path ve NaN/undefined değerler fail'dir. Coverage job yalnız weekly/manual çalışır ve ölçüm sırasında
`PROPTEST_RNG_SEED=2026091001` kullanır; normal PR property job'ı sabitlenmez.

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
  korunur. Quality findings advisory, infrastructure failure blocking olmaya
  devam eder.
- Round 1'deki 494 anlamlı missed mutant için historical conservative triage:
  `REAL_TEST_GAP` 0, `EQUIVALENT` 2, `UNREACHABLE` 0,
  `NEEDS_REVIEW` 492. Ayrıntı ve kapsam sınırı
  `docs/payroll-mutation-survivor-review.md` içindedir; blanket exclusion yoktur.
- Round 2 payroll-engine survivor'ları için conservative triage:
  `REAL_TEST_GAP` 0, `EQUIVALENT` 0, `UNREACHABLE` 0,
  `NEEDS_REVIEW` 116. Ayrıntı `Payroll Engine Quality Round 2` bölümündedir.
- Golden corpus'un 33 fixture'ından yalnız 15'i bağımsız evidence ile verified;
  kalan 18'i `pendingEvidence` durumundadır. Bu durum 33/33 bağımsız doğrulama
  iddiası olarak sunulmaz.
- Linux/macOS generated WASM code/data layout farkı P2 riskidir; package
  allowlist, freshness, adapter ve browser E2E kontrolleri korunmuştur.
- Frontend için bu fazda yeni toplam coverage yüzdesi kapısı eklenmedi; mevcut
  WASM/browser adapter ve E2E testleri korunmaktadır.
- Mutation baseline'da kalan survivor'lar için gerekçe veya kalıcı regression
  testi ayrıca yazılmalıdır; bu turda kesinleştirilemeyen vakalar NEEDS_REVIEW
  olarak bırakılmıştır.
- Yeni gerçek production hesap hatalarının kalıcı regression fixture'ına
  dönüştürülmesi hâlâ inceleme/bug-fix akışının sorumluluğundadır.

## Branch protection

MAIN BRANCH PROTECTION: false; manual GitHub configuration required
Required check önerisi: `CI / verify`
