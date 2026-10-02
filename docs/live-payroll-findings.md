# Canlı bordro bulguları — 1 Ekim 2026

## Gereksinim ve kanıt

| Gereksinim | Sorumlu / akış | Kanıt |
| --- | --- | --- |
| Bağımsız uzak yıl kesinleşmesi geçmiş hesabı engellememeli | Rust `policies`: tahakkuk değişikliği → aynı yıl vergi zinciri / yıl sınırında olası PEK devri → kayıt engeli veya invalidation | Core politika ve native SQLite regresyonları; canlı verinin kopyasında hesaplama |
| Kesinleşmiş gerçek bağımlılıklar korunmalı | Aynı yıl veya yıl sınırını köprüleyen tahakkuk zinciri → FINALIZED engeli | Aynı yıl, yıl sonu PEK ve geçişli zincir testleri |
| Eksik görünen ek tahakkukun dönemi bulunabilmeli | `accrualListData` → boş personel satırı → diğer dönem kayıtları, brüt, ödeme tarihi, gerçek durum | Üç tahakkuk türü için birim testi ve Playwright testi |

## Kök nedenler

- Tahakkuk invalidation politikası sonraki bütün yılları sınırsız etkiliyor; ekran da Rust kararından bağımsız olarak herhangi bir sonraki FINALIZED kaydı engelliyordu. Yeni politika aynı ödeme yılındaki vergi zincirini korur. Yıl değiştiğinde aradaki ödeme ayı boşluğu iki aydan büyükse olası PEK devri de sona erdiğinden bağımlılık kesilir. Ara tahakkuklar yıl sonuna kadar bağımlılığı taşıyorsa sonraki yıl korunmaya devam eder. Retro kaynak dönem engelleri korunur.
- Alp Senaryo47'nin üç ek tahakkuku **2027-02 çalışma döneminde**, **2027-03-14 ödeme tarihli** kaydedilmiş. Brüt tutarlar Tediye **24.500 TL**, TİS **18.000 TL**, Ek Ödeme **12.000 TL**. Canlı inceleme sırasında üçü de **STALE** idi. Temmuz–Eylül 2026 filtrelerinde bulunmamaları doğrudur. Kayıt yok mesajının yanında başka dönemlerdeki kayıtların keşfi eklendi; bu kayıtlar aktif dönem toplamına katılmaz.

## Canlı veri kopyası doğrulaması

Kaynak SQLite salt okunur açılıp `/private/tmp/bordro-live-probe.sqlite` kopyasına alındı. Üretim `calculate_payroll_for_accrual_checked` ve repository kayıt yolu kullanıldı; asıl veritabanı değiştirilmedi.

- Mayıs, Haziran, Temmuz 2026: **59 hesaplanan / 60 personel**.
- Ağustos 2026: **59 hesaplanan + 1 mevcut FINALIZED = 60/60**.
- Alp'in Şubat 2027 üç ek tahakkuku kayıtlı tutarlarıyla sırayla yeniden hesaplandı: üç işlem başarılı.
- Mayıs–Temmuzdaki tek kalan personel `Zincir Bir`: asgari GV açılışı 9. vergi ayından başlıyor; bu aylarda açılış başlangıcı aktif ayın ilerisinde. Aynı yıl kesinleşmiş geçmişi de mevcut. 60/60 elde etmek için geçmiş açılışı veya kesinleşmiş bordroları sessizce değiştirmek doğru değildir.

Yerel kaynak değişikliği, asıl veritabanındaki STALE kayıtları kendiliğinden yeniden hesaplamaz. `Hesaplandı` ödeme/banka aktarımı anlamına gelmez.

## Açık uygulamada takip doğrulaması

Kullanıcının 57/60 ve Alp için STALE bildiriminden sonra açık yerel uygulamada gerçek yeniden hesaplama akışı çalıştırıldı. Öncesinde SQLite yedeği `/private/tmp/bordro-before-live-repair.sqlite` olarak alındı.

- Mayıs, Haziran ve Temmuz 2026 toplu hesapları ayrı ayrı çalıştırıldı: her ay **59/60 Hesaplandı**. Oturum Kontrol ve Zincir İki artık hesaplanabiliyor; önceki 57/60 bu ayların eski kayıt durumuydu.
- Üç ayın tek engeli Zincir Bir: hem normal GV hem asgari GV devri **2026-08 çalışma döneminden** başlıyor. Daha eski ayların doğru başlangıç matrahı tanımlı değil. Ayrıca Ağustos'tan itibaren aynı yıl kesinleşmiş bordroları bulunduğundan bağımlı geçmişi hesaplamak bu kesinleşmiş kayıtların korunmasıyla da değerlendirilmelidir. 60/60 için doğrulanmış geçmiş matrah bilgisi ve kesinleşmiş zincirin kontrollü düzeltilmesi gerekir; test sayısını tamamlamak için açılışlar uydurulmadı.
- Mayıs–Temmuz hesabı sonraki Ağustos'taki iki bağımlı kaydı STALE yaptı. Ağustos toplu hesabı tekrar çalıştırılarak zincir yenilendi: arayüz **60/60**, SQLite **59 CALCULATED + 1 FINALIZED**.
- Alp'in **Şubat 2027** tediyesi, TİS ikramiyesi ve ek ödemesi bu sırayla arayüzden yeniden hesaplandı. Üçünün de SQLite'a kaydedilen durumu **CALCULATED / Hesaplandı**; brütleri **24.500 / 18.000 / 12.000 TL**, ödeme tarihi **14.03.2027** olarak korundu.
- Bu üç tahakkukun satır durumu için beklenen sonuç `Hesaplandı`dır. Tediye/TİS ekranındaki “Referans takvimde ödeme bekliyor” kurum takviminin `aktifDonemdeOdensin` işaretidir; tahakkukun ödeme durumu değildir. Geriye dönük farklar ekranındaki “Ödeme bekliyor” ise ayrı bir mahsuplaşma durumudur.
- Mevcut **8 FINALIZED** kaydın tüm alanları yedekle karşılaştırıldığında değişmedi. Banka aktarımı veya kesinleştirme yapılmadı. Kalıcı dönem sayıları, Alp'in üç CALCULATED kaydı ve kesinleşmiş kayıtların değişmediği SQLite assertion kontrolleriyle doğrulandı; `git diff --check` geçti. Bu takipte uygulama kodu değiştirilmedi.

## Doğrulama komutları

- `cargo test -p payroll-core` — PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml --test cumulative_gv_stale_chain_regression_test --test multi_accrual_regression_test --test payroll_dependency_state_regression_test` — PASS, 24 test.
- `bun test src/components/useBordroCalculationController.test.ts src/components/Listeler/accrualListData.test.ts` — PASS, 19 test. STALE mesajı için eski test beklentisi mevcut çözüm yönlendirmesine güncellendi.
- `bun run test:e2e -- --grep 'missing supplementary rows' --workers=1 --output /private/tmp/bordro-playwright-results` — PASS, 1 gerçek tarayıcı testi. Üç tür tahakkuk arayüzden oluşturulur; geçerli STALE zinciriyle yeniden açılır; diğer çalışma döneminde dönem/tutar/durum bilgisi görünür.
- `bun run test:e2e:typecheck`, `bun run verify:production-graph`, `git diff --check` — PASS.
- `bun run wasm:build`, `bun scripts/verify-wasm-freshness.mjs`, `bun run build`, `cargo tauri build --bundles app` — PASS. macOS paketi `target/release/bundle/macos/4D Bordro Programı.app` altında; `/Applications` kurulumu değiştirilmedi.
- `bun run lint` — FAIL: mevcut `src/bun-test.d.ts` matcher bildirimleri çeşitli testlerde kullanılan `toContain`, `toBeDefined`, `toMatchObject`, `rejects` vb. üyeleri içermiyor. Bu görev dışındaki test tipleri değiştirilmedi.

## Değiştirilen dosyalar

`crates/payroll-core/src/policies.rs`, `src/components/useBordroCalculationController.ts`, `src/components/BordroHesaplama.tsx`, `src/components/Listeler/accrualListData.ts`; bunların ilgili TypeScript testleri, `src-tauri/tests/cumulative_gv_stale_chain_regression_test.rs`, `e2e/payroll.browser.playwright.ts`; yeniden üretilen `src/wasm/pkg/payroll_wasm_bg.wasm` ve `source-hash.txt`; bu rapor.
