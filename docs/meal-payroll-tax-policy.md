# Yemek yardımının kurum bordrosuyla aynı vergi hesabı

Kullanıcının istediği dönem tercihi: yemek yardımının tamamı gelir vergisinden
istisna, tamamı damga vergisine tabi. Bu, kurum bordrosunu eşlemek için seçilen
hesap yöntemidir. Günlük yasal sınırlar bu tercih nedeniyle değiştirilmez.

| Gereksinim | Sahibi ve kanıt |
|---|---|
| Yemek tutarının yanında varsayılan işaretli seçim | `IncomeParametersSection.tsx`; klavye, kaydetme/yeniden açma ve dar ekran Playwright testi |
| Seçimin kalıcı dönem ayarı olması | Rust/TS `yemekTamGvIstisnaDamgaTabi` boolean; JSON doğrulama, SQLite ve tarayıcı kalıcılık testleri |
| İşaretliyken tam yemek GV istisnası, sıfır yemek damga istisnası | Ortak Rust motoru ve kanonik vergi hesabında ayrı GV/DV girdileri; Rust ve gerçek WASM testleri |
| İşaretsizken önceki günlük sınır yöntemi | Aynı motor; gün bazında segmentli istisna testleri, iki seçenekli bağımsız property oracle |
| SGK ve gelir kalemlerinin korunması | Her iki seçenekte aynı gelir/PEK sonuçlarının karşılaştırılması |
| Retro yemek farkının kaynak dönem tercihini izlemesi | Ortak retro policy çözümleyicisi; üretim ve ödeme ledger doğrulaması |
| Eski yedeklerin finansal geçmişinin korunması | Native/browser içe aktarma uyumluluk eşlemesi; gerçek eski V5 yedeğinin yeniden doğrulanması |

Yeni/alanı eksik normal dönem ayarlarında seçim açıktır. Eski bir yedek
aktarıldığında, bordro veya retro geçmişi bulunan dönemin ayarında bu alan yoksa
önceki hesap yöntemi açıkça `false` olarak taşınır. Böylece kaydedilmiş eski
finansal sonuçlar yeni varsayılan nedeniyle bozulmaz. Açıkça kaydedilmiş `true`
ve `false` seçimleri korunur. Yanlış tipteki değerler reddedilir.

Tercih değişikliği mevcut PERIOD invalidation ve kesinleşmiş tarihçe korumasından
geçer. Yasal snapshot değiştirilmez. Açık bordro yeni sonuç için yeniden
hesaplanmalıdır; gerçek çalışan kayıtlarında toplu değişiklik yapılmadı.

Bu seçenek yalnız yemek vergi yöntemini eşler. Sosyal yardım, giyim ve hizmet
zammındaki diğer bordro farklarını değiştirmez.

## Doğrulama

- `cargo test -p payroll-core`: PASS, 288 test. İki mod bağımsız oracle ile
  rastgele girdilerde de karşılaştırıldı. Eski günlük-sınırlı GV/DV testleri
  açıkça işaretsiz seçeneği kullanır; beklenen finansal değerleri korunmuştur.
- Seçili native testler: PASS, 48 test (`meal_tax_policy_regression_test`,
  `paid_sick_wage_regression_test`, `statutory_segment_regression_test`,
  `payroll_dependency_state_regression_test`, `migration_legacy_serde_parity_test`).
- `bun test`: PASS, 326 test; gerçek WASM ve eski yedek uyumluluğu dahil.
- `bun run test:e2e -- --grep 'meal payroll tax policy'`: PASS, 1 gerçek
  tarayıcı testi. Klavye ile seçim, iki modun kaydı/yeniden açılması, STALE
  invalidation, tekrar hesap ve dar ekran kontrolü geçti. Mobil ekran görüntüsü
  incelendi; seçim yemek kutusunun yanında ve görünürdür.
- `bun run wasm:build`, `bun run build`, `bun run test:e2e:typecheck`,
  `bun run verify:wasm-package`, `bun run verify:wasm-freshness`,
  `bun run verify:production-graph`, `bun run verify:payroll-contract`,
  `cargo clippy -p payroll-core --all-targets -- -D warnings`, `git diff --check`: PASS.
- `bun run lint`: FAIL; önceden bulunan `src/bun-test.d.ts` eksik Bun matcher
  tanımları. Değişen özellik dosyalarında yeni tip hatası bulunmadı.

Verilen bordronun gelir/prim tutarlarıyla kanonik vergi testi: GV matrahı
72.956,13 TL; kesilecek GV 14.083,06 TL; brüt DV 696,71 TL; kesilecek DV
446,01 TL — referansla aynı. Motorun mevcut gelir bölüştürmesiyle yeniden
kurulmuş çalışan senaryosunun neti 62.366,42 TL'dir. Diğer gelir farkları
nedeniyle referans netten 12,98 TL düşüktür; yemek hesabının eşlenmesi bu
kalemlerdeki farkı kapatmaz.

## Değişen dosyalar

- Ortak motor: `crates/payroll-core/src/{models,calculations,payroll_engine,retro}.rs`.
- Ücret ekranı: `src/components/Settings/{IncomeParametersSection,PeriodSettingsPage}.tsx`.
- Tipler/defaultlar: `src/types/payroll.ts`, `src/utils/payrollPresentation.ts`,
  `src/services/storage/payrollDefaults.ts`,
  `src/services/payrollEngine/generated/payrollContract.ts`.
- Kayıt/uyumluluk: `src/services/storage/{payrollPayload,payrollPayloadSchema}.ts`,
  `src-tauri/src/services/migration_service.rs`.
- Rust testleri: `crates/payroll-core/tests/{meal_tax_policy_regression,
  raise_transition_regression,independent_audit_20260908,
  independent_audit_20260909,payroll_engine_regression}.rs`,
  `tests/property/payroll_properties.rs`, `tests/reference/normal_payroll_oracle.rs`;
  günlük-sınırlı G016/G017 golden girdilerine açık `false` tercihi eklendi.
- Native testler: `src-tauri/tests/{meal_tax_policy_regression_test,
  migration_legacy_serde_parity_test}.rs`.
- Tarayıcı/WASM testleri: `src/services/storage/mealTaxPolicy.test.ts`,
  `src/services/payrollEngine/raiseTransition.wasm.test.ts`,
  `e2e/payroll.browser.playwright.ts`.
- Üretilen WASM: `src/wasm/pkg/{payroll_wasm_bg.wasm,source-hash.txt}`.
- Bu rapor ve önceki zam denetim raporundaki güncel tercih notu.

Önceki çalışma ağacı değişiklikleri korunmuştur. Kalıcı veri şeması değiştirilmedi;
seçim mevcut ayar JSON'unda tutulur. Kullanıcının gerçek veritabanı değiştirilmedi.
