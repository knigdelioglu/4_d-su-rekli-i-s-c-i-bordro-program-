# Bordro Test Güvence Durumu

Bu belge release kanıtının kısa manifestidir. Coverage ve mutation sayıları
komut çıktısından/CI artifact'larından alınır; mutation full-run sonucu oluşana
kadar `pending` alanları bilinçli olarak skor uydurmaz.

## Son doğrulama kimliği

- Tarih: `2026-09-10`
- Last verified baseline commit: `77ff5237b28a88bc8d868b7edb14aa26e009628c`
- Not: Bu SHA, doğrulamanın dayandığı son commit'tir. Aşağıdaki güvence
  değişiklikleri çalışma ağacında ve henüz commit edilmemiştir; bu belge
  çalışma ağacını commit edilmiş release kanıtı olarak göstermemelidir.

## Golden corpus

- Fixture sayısı: **33**
- Kapsam: `G001`–`G033`, 2026 yasal parametre/sürüm sözleşmesi
- Blocking test: `cargo test -p payroll-core --test golden_payroll_corpus`
- Bağımsız evidence: **15/33 verified** — `G001, G003, G006, G007, G009,
  G010, G011, G013, G014, G016, G017, G029, G030, G031, G032`;
  kalan **18** fixture `pendingEvidence` durumundadır.
- Evidence formatı: `crates/payroll-core/tests/golden/evidence/Gxxx.md`;
  loader kritik bağlantının varlığını ve zorunlu hesap bölümlerini kontrol eder.

## 2026 statutory parameter reference

- Bağımsız fixture: `crates/payroll-core/tests/statutory/2026.json`.
- `AnnualPayrollParameters::default_for_2026()` yıllık GV dilimleri ve yıllık
  brüt asgari ücret tavanı ile bu fixture'a exact Decimal karşılaştırılır.
- Günlük/aylık asgari ücret, işçi/işveren SGK ve işsizlik oranları, PEK tavan
  katsayısı, damga vergisi, GV yemek istisnası ve aylık GV referans matrahı da
  period alanları olarak karşılaştırılır.
- SGK yemek istisnası için mevcut 4/D uygulama snapshot'ı `300.00` TL'dir;
  resmi SGK 4/a sayfasındaki `158.00` TL ile kapsam farkı bulunduğundan alan
  `needs_authoritative_verification` olarak işaretlidir. Bu belirsizlik
  çözülmeden production parametresi değiştirilmemiştir.

## Property ve oracle

- Property integration testleri: **19** (`crates/payroll-core/tests/property_tests.rs`)
- Property alanları: normal bordro finansal korunum, GV, PEK/carry,
  payment-event, retro determinism/conservation, validation ve Decimal.
- Test-only oracle modülleri: `tax_oracle`, `pek_oracle`, `exemptions_oracle`,
  `deductions_oracle`, `normal_payroll_oracle`.
- Oracle production hesap helper'larını import etmez; differential testler
  property suite içinde çalışır.

## Mutation evidence

- Araç: `cargo-mutants 27.1.0`
- Kritik kapsam: `calculations.rs`, `gv_exemption.rs`, `payroll_engine.rs`,
  `policies.rs`, `retro.rs`, `validation.rs`.
- Aday mutant sayısı: **2.147** (liste baseline'ı).
- Full run: **pending**; haftalık/manual workflow, 8 shard ve aggregate artifact
  `payroll-core-mutation-summary` olarak tanımlı. Her shard'ın canonical yolu
  `target/cargo-mutants/outcomes.json`'dır; cargo-mutants 27.1.0'un tool-native
  `target/mutants.out` klasörü CI'da tek seferde bu dizine taşınır.
- Altyapı hataları (baseline, shard, JSON veya eksik artifact) blocking'dir;
  `missed`/`timeout` quality findings full baseline oluşana kadar advisory'dir.
- Mutation summary tek makine-okunur JSON artifact üretir; full 2.147-mutant
  sonucu henüz mevcut değildir.
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

Ratchet dosya bazında lines/functions/regions metriklerinin baseline'ın altına
inmesine izin vermez; dosya/metric eksikliği, duplicate path ve NaN/undefined
değerler fail'dir. Coverage job yalnız weekly/manual çalışır ve ölçüm sırasında
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

- 2.147 adayın tam mutation skoru periyodik/manual shard job tamamlandığında
  artifact'lardan birleştirilecektir; yalnız quality sonucu advisory olan
  geçiş adımıdır. Infrastructure başarısızlığı blocking'dir.
- Full mutation baseline gerçek 8-shard artifact'ı olmadan doldurulmamıştır;
  `docs/payroll-mutation-baseline.json` pending sentinel değerleri taşır.
- Frontend için bu fazda yeni toplam coverage yüzdesi kapısı eklenmedi; mevcut
  WASM/browser adapter ve E2E testleri korunmaktadır.
- Mutation baseline'da surviving mutant bulunursa her vaka için eşdeğerlik/
  ulaşılamazlık gerekçesi veya kalıcı regression testi ayrıca yazılmalıdır.
- Yeni gerçek production hesap hatalarının kalıcı regression fixture'ına
  dönüştürülmesi hâlâ inceleme/bug-fix akışının sorumluluğundadır.

## Branch protection

MAIN BRANCH PROTECTION: manual GitHub configuration required
Required check önerisi: `CI / verify`
