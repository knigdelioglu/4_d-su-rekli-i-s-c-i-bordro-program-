# Bordro Test Güvence Durumu

Bu belge release kanıtının kısa manifestidir. Coverage ve mutation sayıları
komut çıktısından/CI artifact'larından alınır; mutation full-run sonucu oluşana
kadar `pending` alanları bilinçli olarak skor uydurmaz.

## Son doğrulama kimliği

- Tarih: `2026-09-10`
- Base HEAD: `ae1449c8060de2c0de2c7cd043f0b0abff68b3cc`
- Not: Doğrulama çalışma ağacındaki Faz 1–3 değişiklikleriyle yapıldı; yukarıdaki
  SHA bu değişikliklerin başladığı commit'tir.

## Golden corpus

- Fixture sayısı: **33**
- Kapsam: `G001`–`G033`, 2026 yasal parametre/sürüm sözleşmesi
- Blocking test: `cargo test -p payroll-core --test golden_payroll_corpus`

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
  `payroll-core-mutation-summary` olarak tanımlı.
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
inmesine izin vermez. İlk koşuda toplam repository yüzdesi için keyfi minimum
dayatılmaz.

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
  artifact'lardan birleştirilecektir; mevcut non-blocking tasarım bilinçli bir
  geçiş adımıdır.
- Frontend için bu fazda yeni toplam coverage yüzdesi kapısı eklenmedi; mevcut
  WASM/browser adapter ve E2E testleri korunmaktadır.
- Mutation baseline'da surviving mutant bulunursa her vaka için eşdeğerlik/
  ulaşılamazlık gerekçesi veya kalıcı regression testi ayrıca yazılmalıdır.
- Yeni gerçek production hesap hatalarının kalıcı regression fixture'ına
  dönüştürülmesi hâlâ inceleme/bug-fix akışının sorumluluğundadır.
