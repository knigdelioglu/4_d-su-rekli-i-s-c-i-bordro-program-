# Bağımsız kod denetimi — 9 Eylül 2026

## Sonuç ve kanıt sınırı

İki hesaplama hatası yeni testlerle önce yeniden üretildi, sonra ortak Rust motorunda düzeltildi. Yeni 11 çekirdek testi, 1 SQLite entegrasyon testi ve 2 Rust/WASM karşılaştırması geçti. Bu sonuç bütün bordro kombinasyonlarında hata kalmadığına dair bir sertifika değildir. Aşağıdaki açık inceleme alanları nedeniyle kapsamın tamamı için temiz denetim görüşü verilmiyor.

Önceki raporlar doğruluk kanıtı olarak kullanılmadı. Önceden mevcut test paketleri çalıştırılmadı: **NOT RUN — USER INSTRUCTION**. Yalnız bu çalışma sırasında yazılan testler çalıştırıldı. Başlangıçta değişmiş `notices.rs` ve `payroll_notice_service.rs` ile çalışma sırasında başka işlemce değiştirilen `payroll_notice_regression_test.rs` ve oluşturulan `deep_financial_audit_20260909.rs` bu denetimin değişiklikleri değildir.

## Düzeltilen bulgular

### 1. Retro yemek farkında ödeme ayının yemek istisnası yeniden indiriliyordu

**Önem: yüksek.** `payroll_engine.rs` içindeki GV matrahı formülü, normal/retro ayrımı yapmadan `income.yemek` üzerinden ödeme döneminin `gvYemekIstisnasiToplam` değerini indiriyordu. Retro allocation politikası TAXABLE olmasına rağmen bu ikinci indirim uygulanıyordu. DV formülünde ise normal bordro ayrımı zaten vardı.

Yeni test kaynakta 31 × 300 TL yemek ödenmiş, aynı günlük istisna tamamen kullanılmış bir dönem oluşturur. Geriye dönük günlük yemek tutarı 400 TL olunca 3.100 TL fark doğar. Kaynak dönem prim farkları 434 TL SGK ve 31 TL işsizliktir. Ödeme ayının normal bordrosu da önceden hesaplanmıştır.

- Beklenen cari GV matrahı: **3.100 − 434 − 31 = 2.635 TL**.
- Düzeltme öncesi: **0 TL**; beklenen **395,25 TL** GV kesilmiyordu.
- Düzeltme sonrası: **2.635 TL matrah, 395,25 TL GV**.

GV formülü normal bordro için hesaplanan yemek istisnasını kullanıyor. Retro işlem ödeme ayındaki normal yemek hakkını yeniden kullanmıyor; retro kalemlerin vergi muamelesi allocation akışında kalıyor. Bu düzeltme kaynak döneminde kullanılmamış bir yemek istisnasının retroya nasıl taşınacağını ayrıca modellemez; o konu aşağıda açıktır.

Kanıt: `audit_retro_meal_must_not_reuse_payment_month_meal_exemption`; gerçek WASM export'u ile aynı request/result karşılaştırması.

### 2. Asgari GV referansında primler toplanarak yuvarlanıyordu

**Önem: kuruş hassasiyeti.** İşçi SGK ve işsizlik bordroda ayrı yuvarlanırken, asgari GV referans matrahında oranlar önce toplanıyordu. Fark tarihsel asgari kümülatiflere de taşınabiliyordu.

Sentetik sınır örneği: günlük asgari 1.101,01 TL, aylık brüt 33.030,30 TL.

- Ayrı primler: **4.624,24 + 330,30 = 4.954,54 TL**.
- Doğru referans matrahı: **28.075,76 TL**.
- Eski birleşik yuvarlama: **28.075,75 TL**.

`calculate_aylik_asgari_ucret_gv_matrahi` iki primi ayrı yuvarlıyor. Kesinti yardımcı akışındaki aynı formül de bu ortak fonksiyonu çağırıyor. Bu örnek güncel 2026 resmi asgari tutarının yanlış olduğunu iddia etmez; motorun parametre sınırındaki aritmetik tutarsızlığını kanıtlar.

Kanıt: `audit_minimum_reference_uses_separately_rounded_worker_premiums` önce FAIL, sonra PASS.

## İncelenen akış ve yeni kanıtlar

| Alan | Kaynaktan izlenen sahip/akış | Yeni doğrulama ve sınırı |
|---|---|---|
| Maaş, gelirler | `auto_fill_gelirler_from_puantaj`, günlük gelirler, zam bölme, ana motor | Karma senaryoda taban, gece/gece tatili, iş primi, yemek/yol, sosyal/giyim/hizmet, ek ve diğer gelir toplamı 87.270 TL |
| Kesintiler | PEK → işçi primleri → sendika → GV indirimleri → net | Aynı senaryoda PEK 78.870, işçi SGK 11.041,80, işsizlik 788,70, OKS 2.366, GV matrahı 64.939,50, GV 5.529,60 TL; net eşitliği |
| GV tarifesi | Kümülatif toplam vergi farkı | 351 önceki matrah × 5 cari matrah = **1.755** kombinasyon; bağımsız birim-aralık oracle; son dilim üstü dahil |
| Aylık GV/DV istisnaları | Prior event state → kullanılan hak → cari kesinti | NORMAL ardından TEDİYE/TİS/SUPPLEMENTAL; tek aylık istisna, önceki kümülatif ve ek ödeme matrahı |
| PEK, tavan, carry | Canonical ücret/ücret dışı ayrımı → prim matrahı → outgoing carry | 4 gün × 4 ücret × 3 ikramiye = **48** kombinasyon; ücret tavan aşımı devretmez, ücret dışı aşım devreder; işçi matrahına alt sınır eklenmez |
| Puantaj | Gün kodu → hak günleri → statutory snapshot | 31 takvim günü ücret ve 30 SGK günü ayrımı; bir ödenmeyen R günü |
| Rapor | Başlangıç yılına göre episode kotası → ilk iki gün → tarih kümesi | Yıl aşan rapor, tam mükerrer kayıt, ilk beş episode kotasının tükenmesi; bunun kuruma ait TİS ile uygunluğu ayrıca doğrulanmadı |
| Opening | Modern/legacy çözümleyiciler → önceki GV/asgari | Explicit sıfır ve 189.999 opening; önceki STALE kayıt reddi. Modern/legacy bütün karışımları için yeni test yok |
| Yıl geçişi | taxYear/taxMonth → GV referansı ve PEK carry | Yeni yılda GV/asgari kümülatif sıfır; aynı sınırda PEK devri korunur. Önceki yılın tarifesi sentetik fixture, resmi tarihsel tarife doğrulaması değil |
| Retro | Entitlement replay → source SGK → allocation → ödeme GV/DV | Kaynakta istisnası tükenmiş yemek artışı gerçek replay ile; signed settlement/offset kombinasyonlarının tamamı yeni testle doğrulanmadı |
| Native persistence | Checked service → transaction → immutable snapshot → repository | Gerçek SQLite migration/fresh DB, write/read GV ve PEK snapshot eşitliği, tekrar hesaplamada tek kayıt, finalized sonrası ayar/hesap değişikliği reddi ve kayıt korunması |
| Browser persistence | Exact Decimal DTO → IndexedDB CAS → revision/adoption | Koddan incelendi; canlı IndexedDB çok sekme/çökme/restore testi bu çalışmada yok |
| Runtimelar | Rust core → WASM export → Vite | Normal ve retro düzeltme request'lerinde tüm JSON sonuçları birebir; WASM yeniden üretildi, kaynak hash'i ve web derlemesi geçti |

Native komutun checked hesaplama yolunu kullandığı; kesinleştirmenin kaynaklardan tekrar hesapladığı; repository'nin kimlik, finansal eşitlik ve finalized korumaları incelendi. Browser adapter'da Decimal string sınırı ve CAS kontrolü izlendi. Bunlar canlı UI ve bütün restore geçmişleri için test kanıtı yerine geçmez.

## Açık inceleme alanları / temiz görüşü engelleyen riskler

1. **Dönem içinde değişen günlük yemek tutarı ile günlük istisna sınırı:** normal akış toplam yemek ile toplam yasal hakkın minimumunu alıyor. Zam öncesi düşük ödeme/zam sonrası yüksek ödeme veya değişen yasal segmentlerde günlük sınırların ayrı uygulanması ile aynı sonuç verdiği kanıtlanmadı. `split_puantaj_by_zam_tarihi` ve statutory segment akışı için ayrı günlük oracle gerekir.
2. **NORMAL alt sınır tamamlamasından sonra ek ödeme:** NORMAL işveren alt sınır farkı üretirken ek event aynı ay sadece prim matrahını artırıyor. Aylık işveren toplamının ve daha önce tamamlanmış alt sınırın ek event sonrası uzlaştırılması yeni testlerle kanıtlanmadı.
3. **Aylık sigorta indirimi ve birden çok ödeme:** aday primler yalnız NORMAL'de okunuyor. Sonraki ikramiye ile aylık %15 limitinin genişlemesi, daha önce emilemeyen aday prim ve işe yıl ortası girişte önceki işverende kullanılan yıllık limit için eksiksiz model kanıtı yok.
4. **Retro kapsamı:** negatif fark, nakdi ödeme ile mahsup karışımı, birden çok revision, kaynak ay SGK tavan değişimi, downstream carry replay, kaynakta kullanılmamış GV/DV yemek istisnası ve birden çok kaynak yılın tüm çapraz kombinasyonları yeniden doğrulanmadı.
5. **Kayıt/migration kapsamı:** gerçek eski müşteri veritabanı veya yedek üzerinde migration/restore yapılmadı. Browser çökme ve çok sekme çatışması yeni canlı testle kanıtlanmadı. Native SQLite testi Tauri süreç/UI yaşam döngüsünü kapsamaz.
6. **Hukuki ve sözleşmesel kapsam:** kuruma ait yürürlükteki TİS, personel özel muafiyetleri ve tüm yılların resmi parametre belgeleriyle birebir mutabakat yapılmadı. Kodun ilk beş rapor, iş primi, yardım ve hizmet zammı kuralları kurumsal sözleşme doğrulaması yerine geçmez.
7. **Mevcut hesaplanmış kayıtlar:** düzeltme kayıtlı bordroları otomatik yeniden hesaplamaz veya kesinleşmiş tarihçeyi değiştirmez. Etkilenen retro yemek kayıtları ve bunların sonraki GV zinciri kaynaklardan yeniden değerlendirilmelidir; finalized kayıtlar ayrı düzeltme süreci gerektirir.

Bu maddeler yeni testlerle kapanmış hata bulguları olarak sunulmuyor. Kaynak incelemesinin bıraktığı belirli açık alanlardır; tüm kombinasyonlarda doğru hesap iddiasını engellerler.

## Çalıştırılan doğrulamalar

- `PAYROLL_AUDIT_FIXTURES=/private/tmp/payroll-audit-fixtures-20260909 CARGO_TARGET_DIR=/private/tmp/payroll-audit-20260909 cargo test -p payroll-core --test independent_audit_20260909` — **PASS: 11**.
- `cargo test -p bordro-programi --test independent_audit_20260909` — ortak derleme klasörü kilidinde bekledi; durduruldu, test çalışmadı.
- `CARGO_TARGET_DIR=/private/tmp/payroll-audit-20260909 cargo test --offline --manifest-path /private/tmp/payroll-native-audit-20260909/Cargo.toml --test independent_audit_20260909` — **PASS: 1**. Geçici harness doğrudan projenin `db`, `domain`, `repositories`, `services` kaynaklarını path ile derler; Tauri bağımlılığı/komut modülü yoktur. Kaynakta tutulan test normal native pakette de çalıştırılabilir. Harness ayrı dependency çözümü kullandığından tam workspace kilitli native build eşdeğerliği iddia edilmez.
- `PAYROLL_AUDIT_FIXTURES=/private/tmp/payroll-audit-fixtures-20260909 node scripts/independent-audit-20260909.mjs` — **PASS: 2** gerçek WASM karşılaştırması.
- `bun run lint` — **PASS**, TypeScript ve üretim graph kontrolü.
- `CARGO_TARGET_DIR=/private/tmp/payroll-audit-20260909 bun run wasm:build` — **PASS**.
- `bun run build` ve `bun run verify:wasm-freshness` — **PASS**. Vite bundle boyutu uyarısı verdi; hesaplama hatası değil.
- Tüm çalışma ağacı `git diff --check` başka işlemce değiştirilen `payroll_notice_regression_test.rs` sonundaki boş satırı bildirdi; o dosya değiştirilmedi. Bu çalışmanın kendi diff kontrolü ayrıca yapıldı.

## Değişiklikler

- `crates/payroll-core/src/calculations.rs`: ayrı prim yuvarlaması ve ortak referans fonksiyonu.
- `crates/payroll-core/src/payroll_engine.rs`: retroda normal yemek istisnasının tekrar uygulanmasını kaldırma.
- `crates/payroll-core/tests/independent_audit_20260909.rs`, `tests/support/audit_fixture.rs`: yeni bağımsız senaryolar ve sentetik fixture.
- `src-tauri/tests/independent_audit_20260909.rs`: yeni gerçek SQLite testi.
- `scripts/independent-audit-20260909.mjs`: gerçek WASM karşılaştırması.
- `src/wasm/pkg/payroll_wasm_bg.wasm`, `source-hash.txt`: yeniden üretilen browser motoru/provenance.
- Bu rapor. Commit, deploy veya kullanıcı bordrosu üzerinde veri değişikliği yapılmadı.

## Resmi kaynak kontrolü

Ücret dışı ödemelerin tavanı aşan kısmının en fazla takip eden iki ayda PEK'e alınması ve günsüz ay davranışı [SGK prime esas kazanç açıklaması](https://www.sgk.gov.tr/Content/Post/2e0c9e1a-2cfe-4456-af10-49d3de0c58ba/Prime-Esas-Kazanc-Miktarlari-2026-01-14-10-35-39) üzerinden kontrol edildi. Yemek istisnasının GVK 23/8 ve 322 sayılı Tebliğ kapsamı [GİB açıklaması](https://gib.gov.tr/mevzuat/kanun/433/ozelge/21372) ile karşılaştırıldı. Bu kaynak kontrolleri tüm kurumsal TİS ve tarihsel mevzuatın incelendiği anlamına gelmez.
