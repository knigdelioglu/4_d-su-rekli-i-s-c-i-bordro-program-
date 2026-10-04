# Yol yardımı yürürlük tarihleri

Kapsam: dönem başlangıcı günlük tutar + aynı dönem içindeki artan tarih/tutar satırları.
Tarih dahil uygulanır; yalnız Ç ve GÇ günleri ödenir. Zam takviminden bağımsızdır.

Geriye uyumluluk: alanı bulunmayan/null olan eski JSON kayıtları eski zam takvimi
hesabını korur. Kullanıcı yol tutarını değiştirince veya 'Yol hesabını zamdan ayır'
düğmesine basınca [] veya tarihli liste kaydedilir. [] tüm dönem için bağımsız
başlangıç tutarıdır. Yeni dönemlerin varsayılanı bağımsız hesaptır.

| Gereksinim | Sahip / akış | Kanıt |
|---|---|---|
| Çoklu tarih, zamdan bağımsız | transport_rates -> normal payroll; retro replay günlük oran | core regression + retro regression |
| Geçersiz tarih/sıra/tutar reddi | core + SQLite repository; TS save/import sınırı | unit + SQLite + Bun |
| Eski kayıt/yedek korunması | optional serde alan; exact decimal contract | serde + Bun backup + native roundtrip |
| Ücret ekranı, kaydet/yükle | IncomeParametersSection -> mutation controller | Playwright gerçek WASM hesap ve reload |
| Sonraki yeni döneme son tutar | PeriodSettingsPage, eski tarihleri temizle | helper unit |
| STALE / FINALIZED korunması | mevcut PERIOD mutation transaction | native regression |

Mevcut sonraki dönem ayarları kendiliğinden ezilmez; yeni dönem oluşturulurken
kaynak formun son tutarı taşınır. Geriye dönük ücret farkı override'ı tarihli yol
oranından sonra uygulanır. DB kolon değişimi yok; mevcut settings_json genişletilir.

## Bu işte değişen dosyalar

- crates/payroll-core/src/{transport_rates.rs,models.rs,lib.rs,payroll_engine.rs,retro.rs}
- crates/payroll-core/tests/{payroll_engine_regression.rs,retro_regression.rs}
- src-tauri/src/repositories/settings_repo.rs
- src-tauri/tests/remaining_debt_tests.rs
- src/components/Settings/{IncomeParametersSection.tsx,PeriodSettingsPage.tsx}
- src/hooks/usePayrollMutationController.ts
- src/types/payroll.ts
- src/services/storage/{payrollDefaults.ts,payrollPayloadSchema.ts,transportRatesValidation.ts,transportRates.test.ts}
- src/services/payrollEngine/generated/payrollContract.ts
- src/wasm/pkg/{payroll_wasm_bg.wasm,source-hash.txt}
- e2e/payroll.browser.playwright.ts
- docs/transport-effective-dates.md

Önceki görevdeki uyarı/bildirim değişiklikleri korunmuştur. Çalışma sırasında başka
bir işlem tarafından eklenen dönem silme değişikliklerine müdahale edilmemiştir.

## Doğrulama

- cargo test -p payroll-core: PASS, 294 test.
- cargo test -p bordro-programi --test remaining_debt_tests: PASS, 11 test.
- bun test src/services/storage: PASS, 84 test.
- bun test src/services/storage/transportRates.test.ts src/services/storage/mealTaxPolicy.test.ts src/components/Settings/periodSettingsSave.test.ts: PASS, 16 test (5 tanesi yukarıdaki 84 içinde).
- bun run test:e2e:typecheck: PASS.
- bun run test:e2e -- --grep 'dated transport|wage changes remind|meal payroll tax policy': PASS, 3 test; gerçek WASM, reload, STALE, invalid tarih ve yeni döneme aktarım.
- bun run verify:payroll-contract, bun run verify:production-graph, git diff --check: PASS.
- bun run wasm:build, bun run build: PASS.
- bun run lint: FAIL, mevcut bun:test matcher tipi eksikleri; yeni yol bileşenlerinde tip hatası yok.
- Native uygulama canlı UI testi yapılmadı; mobil/masaüstü arayüz tarayıcıda doğrulandı.
- Eski JSON alanını eklemeden yeniden serileştirme testi: PASS. None alanı serialize edilmez; [] açık bağımsız tercih olarak korunur.
- Son serde uyumluluk düzeltmesinden sonra native 11 test tekrar PASS.
- cargo tauri build --config src-tauri/tauri.conf.json --bundles app --ci --no-sign: PASS; target/release/bundle/macos/4D Bordro Programı.app güncellendi.
