# Oturum 3 sonuçları

İlk rapor kota nedeniyle ara sonuçtu. Aşağıdaki kota sonrası devam doğrulaması önceki açık maddeleri günceller ve nihai durumu belirler; ilk bölümlerdeki ara “eksik/açık” ifadelerin yerini alır. Production kodunu orchestrator yazmadı; gerçek düzeltmeler ayrı GPT-6 Luna High thread’lerinde yapıldı. Banka, SGK Kontrol, Excel/PDF/yazdırma, web/native karşılaştırması ve genel adversarial UX test edilmedi.

## Başlangıç ve referans

Başlangıç UI JSON yedeği: `initial.json`. 7 personel, 17 dönem (2025-12–2027-04), 84 puantaj, 117 bordro: 110 CALCULATED, 6 STALE, 1 FINALIZED. 3 hastalık raporu, 2 yıllık parametre, 17 dönem kurum ayarı, 0 vergi açılışı, 10 revizyon, 17 retro batch, 58 allocation vardı. Mevcut retro kayıtları önceki oturumlarda çalıştırılmamıştı.

Personeller: p-1 Ahmet Yılmaz; p-2 Ayşe Kaya; p-4 Fatma Şahin; p-1790755971479 Kesinti Deneme; p-3 Mehmet Demir; p-5 Mustafa Çelik; p-1790754828761 Oturum Kontrol. Dönemler 15–14 geometrisindedir.

FINALIZED: p-1790754828761_2027-03, NORMAL, ödeme 2027-04-14, brüt 96065.98, kesinti 20958.30, net 75107.68, GV önce 0 / cari ve yeni 75791.08, PEK 89165.98, carry boş. Oturumdaki ilk retro bunu değiştirmedi.

Retro sonrası tam referans: `reference.json`: 118 bordro (100 CALCULATED,17 STALE,1 FINALIZED),12 revizyon/override,18 batch,62 allocation. Bu dosyalar personel kimlikleri, dönemler, puantaj, tüm parasal snapshotlar, kesintiler, raporlar, kurum/yıllık parametreler ve retro grafiğini içerir; exact karşılaştırma kaynağıdır. Vergi açılışı olmadığı için dolu vergi açılışı restore senaryosu yapılmadı.

## Retro Preview

**PASS — kontrollü p-1 senaryosu.** Ücret 2643.28; yürürlük 2026-09-15–2026-11-14; imza 2026-09-30; ödeme 2026-12-14. Eylül eski93071.80/hedef99467.80/önceki retro3198/yeni fark3198; Ekim eski95515.08/hedef102111.08/önceki3298/yeni3298; Kasım eski/hedef117422.23/yeni fark0. Toplam6496.00. Aynı dönem iki kez sayılmadı, farklı personel dahil olmadı. Kaynak PEK farkı6100.

Preview draft revizyon/override kaydeder; authoritative bordro ve batch dizileri exact aynı kaldı. Başka sekmeye gidip dönmek preview’ı uygulamadı; geçici form kayboldu. Aynı seçili revizyonun tekrar preview sonucu aynıydı. İmzasız birden çok yeni revizyonun id/tarih önceliği determinism testinden ayrı tutuldu.

**FAIL→kod düzeltildi, canlı tekrar eksik:** yeni personelin FINALIZED dönemine preview, ilgisiz eski dönem puantajını istiyordu (Bulgu1).

## Retro Apply

**PASS — kontrollü p-1 artı fark.** Ayrı event retro-823c0460-a659-4d59-8bb3-6c1ab399252f, sequence4, CALCULATED oluşturuldu. Brüt6496.00 − kesinti2708.53 = net3787.47. İşçi SGK909.44, işsizlik64.96, GV1490.83, DV49.30, BES194.00. GV489682.24 +5521.60 =495203.84. Kaynak ücret PEK6100, ödeme PEK396; boş carry korundu.

Geçmiş parasal alanlar üzerine yazılmadı. 11 sonraki bordronun yalnız durum/güncelleme zamanı değişti; FINALIZED exact korundu. Eksi fark / receivable / mahsup akışı canlı çalıştırılmadı.

## Retro determinism / FINALIZED ilişkisi

**PASS — uygulanan revizyonu tekrar preview:** fark0, uygulanabilir düğme kapalı; duplicate ödeme oluşmadı. Kapat/aç sonrası geçmiş korundu. FINALIZED kayıt ilk artı fark sırasında değişmedi.

**EKSİK:** doğrudan FINALIZED kaynağa ayrı retro apply; düzeltme sonrası aynı FINALIZED preview’ın canlı tekrarı; eksi farkın tekrar uygulanma güvenliği. Bu nedenle FINALIZED retro kabul kriterlerinin tamamına PASS verilmiyor.

## Persistence

**PASS:** ilk retro sonrası uygulama tamamen kapatıldı, process yokluğu doğrulandı, aynı build açıldı. UI JSON snapshot tüm domainlerde exact aynıydı; yalnız exportedAt farklıydı. Parasal değer veya durum değişmedi.

UI reset sonrası kapat/aç ve başarısız restore sonrası kapat/aç da sağlam state’i korudu. Native View menüsünde yalnız Toggle Full Screen bulundu; WebView sağ-tık menüsünde Reload yoktu. Cmd+R, Geriye Dönük Farklar önizlemesi görünürken denendi; form ve geçici önizleme kaldı, dolayısıyla document reload gerçekleştiği doğrulanmadı. Tauri config/build ayarlarında devtools veya özel reload komutu/menüsü/shortcut’ı bulunmuyor. Başarılı full referans restore sonrası persistence testi yapılmadı.

## Backup

**Kısmi PASS / açık P2.** UI ile gerçek, boş olmayan JSON yedekler alındı; güncel sözleşme **V5**. Dönem, personel, kurum ayarları, puantaj, bordro, taxOpenings, rapor, yıllık parametre, zam ayları, revizyon/override/batch/allocation dahil. README V2 açıklaması güncel sürümle geride kalmış durumda.

Native Blob indirme yolu bazı build/çalıştırmalarda kilitlendi. Yeni native save-dialog düzeltmesi kaynakta var; son test edilen binary içinde yok ve canlı doğrulanmadı (Bulgu5).

## Restore

**PASS — 5 NORMAL içeren izolasyon yedeği finansal restore.** valid.json iki kez canlı başarıyla yüklendi. Beş kişinin brüt/kesinti/net toplamı472639.03/116225.71/356413.32. Rust migration entegrasyon testinde full 118 bordrolu referans da restore oldu; finansal alanlar, durumlar ve retro grafiği karşılaştırıldı.

**FAIL — exact audit metadata:** canlı restore sonGuncellemeTarihi değerlerini değiştirdi. Önceki full fixture testi audit zamanlarını dışlıyordu; son düzeltme bunu kaldırmayı hedefliyor. Exact full referansın UI restore + export karşılaştırması yapılmadı. `valid-restored.json` en son eski download dosyasına denk geldiğinden yeni restore kanıtı olarak kullanılmamalı; gerçek metadata kanıtı `timestamp-repro.json` ve `valid-restored.sql`dır.

Orijinal FINALIZED veri seti toplu reseti tasarım gereği engelledi. Kapalı SQLite dosyası `/tmp/session3-preserved-native.sqlite` olarak korunarak izolasyon kuruldu. Mevcut uygulama hâlâ izolasyondaki 5 personel/9 dönem/45 puantaj/5 CALCULATED NORMAL setindedir; orijinal 7 personelli set uygulamaya geri alınmadı. Tam referans JSON da korunmaktadır.

## Bozuk Backup / Atomiklik

| Vaka | Canlı sonuç | State |
|---|---|---|
| Geçersiz JSON syntax | Açık parse hatası, ret | Korundu |
| Zorunlu personeller alanı eksik | Alan yolu ile ret | Korundu |
| Yanlış hizmetYili tipi | Integer kontrolü, ret | Korundu |
| Decimal netOdeme=abc | Exact Decimal kontrolü, ret | Korundu |
| Ghost personel foreign key | Referans kontrolü, ret | Korundu |
| Duplicate TC | Duplicate kontrolü, ret | Korundu |
| Duplicate kişi+dönem puantaj | Duplicate kontrolü, ret | Korundu |
| Bilinmeyen sürüm999 | Desteklenmeyen sürüm, ret | Korundu |
| Finansal toplamı tutarsız net | Toplam kontrolü, ret | Korundu |
| Yapısal olarak geçerli, gelir/brüt/net +1 sahteciliği | Rust canonical replay p-5 gelir1/replay0 farkıyla ret, iki kez | Tüm SQL exact; kapat/aç sonrası da exact |
| Bilinmeyen root/personel ekstra alanları | Kabul | Native SQLite alanları saklamıyor |
| Restore onayını iptal | İşlem uygulanmadı | Exact korundu |

İlk syntax vakanın SQL kontrolü hemen sonraki ikinci retle birlikte yapıldı. Diğer ilk negatif vakalarda tam SQL state kontrol edildi. Son transaction-içi replay retinde tüm tabloların exact SQL dump karşılaştırması iki kez yapıldı; refresh yerine tam process kapat/aç sonrası da değişmedi. Dosya sonundaki newline finansal/state farkı sayılmadı. P0/P1 kısmi restore corruption gözlenmedi.

README’nin unknown-field koruma vaadi tarayıcı IndexedDB içindir; native sadece kabul ediyor, saklamıyor. Tarayıcı uyumluluk karşılaştırması bu oturumun kapsamı dışında kaldı.

## Bulgular

### 1. P2 — FINALIZED personel retro preview ilgisiz tarihçe istiyor
- Yeniden üretme: Oturum Kontrol; yürürlük2027-03-15–2027-04-14, imza2027-04-30, ücret2543.28, ödeme2027-05-14; preview iki kez.
- Beklenen: personelin mevcut kaynak dönemlerini kullanmak. Gerçekleşen: 2026-09 puantajı bulunamadı; akış bloklandı, veri değişmedi.
- Kök neden: önceki ALL_PERSONNEL revizyonlarının en erken tarihinden başlayan dönem seçimi, seçili personelin tarihçesiyle sınırlanmıyordu.
- Düzeltme: kaynak non-retro bordro veya puantaj bulunan dönemleri seç; gerekli gerçek kaynak eksikse fail-closed. Fazla dar revision-overlap önerisi reddedildi, downstream replay korundu.
- Thread: 01a0f1d0-b3b1-7f91-8570-7a1d2bf49b61.
- Dosyalar: crates/payroll-core/src/retro.rs; crates/payroll-core/tests/retro_regression.rs; WASM çıktıları.
- Test: retro_regression56/56; ilgili Bun preview/recovery2/2 PASS. Son test binary’sine dahil, orijinal FINALIZED canlı tekrar **yapılmadı**.

### 2. P2 — Native restore onayı sessiz iptal ve anlaşılmaz hata
- Yeniden üretme: Yedek Yükle ile dosya seç; native window.confirm false dönerek import hiç başlamıyordu. FINALIZED reset hatası [object Object] idi.
- Beklenen: açık onay, gerçek import, anlaşılır ret. Veri korunuyordu ancak restore kullanılamıyordu.
- Düzeltme: App modalı, Vazgeç/Escape/işlem sırasında disable; başarılı commit+reload sonrası bildirim; DomainError formatter. Başarıdan sonra reload hatası ayrıca açıklanıyor.
- Thread: 01a0f1d5-f529-7c93-8306-e9e3862e17a7.
- Dosyalar: src/App.tsx, components/TopBar.tsx, components/DataBackupPage.tsx, hooks/useBackupController.ts ve testi, services/storage/confirmedBackupImport.ts ve testi.
- Test: modal/helper/controller9/9 PASS (son export ilavesiyle ilgili birleşik frontend suite78/78). Canlı modal, iptal, syntax/validation hataları ve valid import PASS. FINALIZED reset yeni mesajı orijinal setle canlı tekrar edilmedi.

### 3. P1 — Uygulamanın ürettiği retro yedeği lifecycle kontrolünde reddediliyor
- Yeniden üretme: artı retro sonrası reference.json parse; CALCULATED batch ile STALE ödeme bağı reddediliyor. Aynı kayıt kaynakta gerçek downstream invalidation sonucu oluşuyor.
- Beklenen: geçerli kaynak grafiği geri yüklenmeli. Etki: retro içeren kendi yedeği restore edilemiyor; corruption yok.
- Kök neden: frontend/native validator lifecycle’ı eşit durumlarla sınırlıyordu.
- Düzeltme: yalnız CALCULATED batch + STALE ödeme istisnası; kişi, kimlik, tarih, tür ve gross eşleşmesi katı kaldı. Global STALE düşürme önerisi reddedildi.
- Thread: 01a0f1db-8fa6-75f0-9ac8-3b77e9e0e429.
- Dosyalar: payrollPayloadSchema.ts, browserPayrollStore.test.ts, migration_service.rs, migration_legacy_serde_parity_test.rs.
- Test: browser59/59, migration17/17 PASS; forged lifecycle bağları ret. Full referansın UI restore canlı doğrulaması **eksik**.

### 4. P1 — Decimal scale ve eski optional PEK snapshot yüzünden kendi yedeği reddediliyor
- Yeniden üretme: valid.json native restore iki kez; iş primi219.9/219.90 lexical farkı nedeniyle canonical replay ret. Full eski referansta yeni optional aylık PEK alanları eksik.
- Beklenen: Decimal sayısal eşitlik, eski optional alan yokluğu korunmalı; gerçek sahte finansal değer reddedilmeli.
- Kök neden: serde_json string eşitliği; sonradan eklenen optional alanların zorunlu replay eşliği.
- Düzeltme: typed exact Decimal karşılaştırma; supplied optional değerleri hâlâ katı doğrulama. Number/float dönüşümü yok.
- Thread: 01a0f1ea-5162-79c3-9fce-507a2815fcfb.
- Dosyalar: migration_service.rs, migration_legacy_serde_parity_test.rs.
- Test: Decimal unit1/1; migration17/17 PASS. Son binary canlı valid restore başarı iki kez; tutarlı görünür forged gelir Rust’ta iki kez ret ve SQL exact.

### 5. P2 — Native JSON indirme kilitleniyor
- Yeniden üretme: Yedek İndir, Blob download; aynı sorun farklı bundle denemelerinde ve son build’de tekrar. CUA timeout/noWindows; OS log main-thread dispatch stuck. Bazı önceki indirmeler başarıyla dosya oluşturmuştu.
- Beklenen: dosya oluşturulması ve yanıt veren uygulama. Gerçekleşen: son indirme tamamlanmadı, process TERM ile kapatıldı; SQLite korunmuştu.
- Kök neden: WebKit Blob/anchor download yolu native main thread’de takılıyor; Rust save dialog ile bypass edildi, OS altındaki kesin çağrı nedeni tam kanıtlanmadı.
- Thread: 01a0f1fe-7beb-7212-829b-29eea0a36263.
- Dosyalar: hooks/useBackupController.ts/test, services/tauriBridge.ts/test, src-tauri/src/commands/backup_cmd.rs, commands/mod.rs, lib.rs, Cargo.toml/Cargo.lock, generated ACL schemas, tests/fixtures/session3-atomic-reference.json.
- Yeni resmi Tauri dialog bağımlılığı indirildi. Frontend başarı/iptal/yazma hatası testleri PASS. Kodlama chat’i hedef native dosya yazma testlerini 2/2 PASS bildirdi; orchestrator tekrar çalıştırmadı. **Yeni build ve canlı indirme testi yapılmadı; açık FAIL.**

### 6. P2 — Restore audit zamanlarını değiştiriyor
- Yeniden üretme: valid.json iki kez başarıyla restore;5 sonGuncellemeTarihi10:29’dan restore anına değişti; timestamp-repro.json.
- Beklenen: yedekteki authoritative audit metadata exact korunmalı. Finansal değer/durum değişmedi, tarihçe izlenebilirliği etkilendi.
- Kök neden: ordinary repository save Utc::now, restore için aynı yol.
- Düzeltme kaynakta: restore-specific payroll/annual parameter API, migration callsite yönlendirmesi; normal mutation timestamp davranışı korunuyor. Full reference test audit dışlaması kaldırıldı; kalıcı fixture taşındı.
- Thread: 01a0f1ff-d92c-76e3-ad91-ea86e80a4b84.
- Dosyalar: payroll_repo.rs, annual_payroll_parameters_repo.rs, migration_service.rs, migration_legacy_serde_parity_test.rs, browserPayrollStore.test.ts, tests/fixtures/session3-reference.json.
- Browser59/59 PASS. Timestamp native regression derlemesi paylaşılan yeni backup_cmd.rs WebviewWindow: CommandArg hatasında durdu; bu son export düzeltmesiyle ilişkiliydi. **Yeni build/canlı exact restore testi yapılmadı; açık FAIL.**

## Test sonuçları

Orchestrator’ın tamamlayıp sonucunu gördüğü hedef testler: retro Rust56/56; migration integration17/17; Decimal unit1/1; Bun import/controller/bridge/storage birleşik78/78; preview/recovery2/2. Tüm Rust workspace çalıştırılmadı.

TypeScript noEmit kontrolü FAIL: önceden mevcut test matcher toContain ve syncPuantajForSickLeaveDelete undefined hataları; kapsam dışı otomatik düzeltilmedi. Release build Vite üzerinden başarılıydı, bu TypeScript kontrolünün geçtiği anlamına gelmez.

Export chat’i son hedefli Rust dosya yazma testlerini2/2 PASS bildirdi (generic Runtime komut hatası düzeltildi). Timestamp exact migration testi son kez yeniden çalıştırılmadı. Son iki kaynak düzeltmesinden sonra final integration/build/canlı tekrar tamamlanmadığından tüm current working tree için geçer raporu verilmiyor.

## Build

- Başlangıç commit c518317417a5bf50121ea1cf0e8dd60a9a8e5f79; son HEAD211cb19dba93ba8da9d61eaf5099b98773d8d751. Oturum sırasında dış işlem commit oluşturdu; orchestrator commit/push yapmadı.
- Çalışma ağacı kirli: oturum düzeltmeleri, test/fixture/kanıt dosyaları, WASM çıktıları ve resmi dialog dependency/generated schemas. Son kaynak ağacı son test binary’sinden ileride.
- İlk core build: bun run wasm:build, ardından cargo tauri build --bundles app.
- Son test edilen build komutu: cargo tauri build --bundles app --config '{"identifier":"com.bordro.session3.validation"}'. Başarılı; bundle ditto ile kurulup ad hoc codesign yapıldı.
- Kullanılan bundle: /Applications/4D Bordro Session 3.app.
- Bundle ID: com.bordro.session3.validation.
- Executable: Contents/MacOS/bordro-programi.
- SHA256: 8543feb507cbbf62010e0043fdb8f45ae2e6e8279080cbceb374a602392f217b.
- Her kurulumdan önce eski process kapatılıp yokluğu kontrol edildi. Son binary import modal/lifecycle/Decimal/core düzeltmelerini içerir; native download ve timestamp düzeltmelerini içermez.

## Kapsam sınırlamaları ve devamda yapılması gerekenler

1. Son iki düzeltmenin targeted native testleri, diff final inceleme, release build ve Computer Use tekrarları.
2. Full118 bordrolu referansın UI restore, yeniden hesaplatmadan exact domain/audit karşılaştırması ve kapat/aç.
3. FINALIZED personel preview düzeltmesinin canlı tekrarı, ayrı retro apply; eksi fark/mahsup/receivable ve idempotency.
4. Ürün normal UI’sında FINALIZED tarihçe toplu reset/replace’i engelliyor; bu guard gevşetilmedi. UI sadece default örnek set reset sunuyor; tam boş reset görünür yolu bulunmadı. İzolasyon hazırlığı dosya seviyesindeydi, bunun UI temizleme olduğu iddia edilmiyor.
5. Gerçek native refresh kanıtı yok; tam kapat/aç kullanıldı. Orijinal set /tmp/session3-preserved-native.sqlite ve reference.json içinde güvenli, halen test setinden ayrı.
6. Dolu tax opening ve dolu PEK carry ile backup restore özel vakası yapılmadı; mevcut snapshot’taki boş/var olan alanlar karşılaştırıldı.
7. Oturum4 alanlarına geçilmedi.

## Kota sonrası devam doğrulaması — 2026-09-30

Bu bölüm önceki rapordaki “canlı tekrar eksik/açık” durumlarını supersede eder. Önceki oturumda PASS verilen maaş, kesinti, puantaj ve bozuk backup senaryoları gereksiz yere yeniden çalıştırılmadı.

### Zorunlu kabul kriterleri

| Kriter | Sonuç | Kanıt |
|---|---|---|
| Native JSON download | PASS | Son ana bundle’da Save dialog açıldı; seçilen hedefe 804,933 bayt JSON yazıldı, dosya parse edildi ve UI yanıt vermeye devam etti. |
| Save-dialog cancel | PASS | Aynı bundle’da ikinci dialog iptal edildi; hata/corruption oluşmadı, uygulama responsive kaldı. |
| Restore audit timestamp exactness | PASS | Export/restore düzeltmelerini içeren bundle’da 118 bordrolu referansın restore öncesi/sonrası ve kapat/aç sonrası karşılaştırmaları sıfır domain/audit farkı verdi. exportedAt gibi doğal export metadata’sı dışarıda bırakıldı. Son retro-only rebuild’de mevcut FINALIZED durumdan referans restore denemesi guard tarafından reddedildi; bu ret sonrası mevcut export, önceki exportla bütün JSON alanlarında aynı kaldı. |
| Full 118 bordrolu UI restore | PASS | Referans JSON normal Yedekten Geri Yükle UI akışından yüklendi; bordrolar yeniden hesaplatılmadı. full-restore-comparison.json ve full-restore-reopen-comparison.json içinde diffCount 0. |
| Restore sonrası kapat/aç persistence | PASS | Restore edilmiş verinin process kapatılıp yeniden açıldıktan sonraki export’u referansla exact eşleşti. |
| FINALIZED retro preview düzeltmesi | PASS | Oturum Kontrol için 15.03.2027–14.04.2027 kapsamlı preview yalnız gerçek kaynak dönemlerini kullandı; ilgisiz 2026-09 puantajı istemedi. Preview authoritative kaydı değiştirmedi. |
| FINALIZED retro apply | PASS | Ayrı 2027-05-14 retro batch oluşturuldu; brüt fark 3,307.00 TL, net 2,364.21 TL. |
| FINALIZED kaynak snapshot değişmezliği | PASS | p-1790754828761_2027-03 kaydı referans JSON’a tüm alanlarıyla exact eşit kaldı: brüt 96,065.98, kesinti 20,958.30, net 75,107.68, GV cari/yeni kümülatif 75,791.08, PEK 89,165.98, FINALIZED status ve audit timestamp’leri dahil. |
| Negatif retro / receivable | PASS | Ayşe Kaya için brüt fark −3,268.00 TL; payable 0, mahsup 0, receivable/açık receivable 3,268.00 TL. Ayrı OVERPAYMENT batch üretildi; geçmiş payroll kayıtları değiştirilmedi. |
| Negatif retro idempotency | PASS — close/open; gerçek WebView reload alt kontrolü NOT TESTABLE | Kapat/aç sonrası aynı input preview’ı daha önce PASS oldu. Bu turdaki reload öncesi canlı preview aynı ekonomik girdilerle yeni delta/payable 0, açık receivable 3,268.00 TL verdi; ödeme oluşturma kapalı kaldı. Bordro, batch, allocation, puantaj ve dönem dizileri exact aynı; negatif batch tek kaldı. |

### WebView reload alt kontrolü — NOT TESTABLE

- Mevcut negatif event: Ayşe Kaya (`p-2`), kaynak dönem `2027-01`; revision `revision-788fd525-4342-4625-8417-1ebcde805136`, override `override-cf7a088b-c2fa-46d9-8459-54c00dfaa759` (`GUNLUK_TABAN_UCRET=2443.28`, personel `p-2`).
- Batch `retro-5b80adf8-da00-4820-a59e-85e8b49ffaad`, ödeme tarihi `2027-03-14`: CALCULATED / OVERPAYMENT, brüt `−3268`, payable `0`, offset `0`, recovered `0`, recoverable/outstanding receivable `3268`. Kaynak allocation’lar: 2027-01 BASE_WAGE `−3100` ve WORK_PREMIUM `−168`. Bu OVERPAYMENT için bağlı pozitif payment/accrual veya bordro yok.
- Reload öncesi export `/tmp/session3-fixtures/session3-reload-before.json`: 7 personel, 17 dönem, 84 puantaj, 119 bordro, 20 retro batch, 66 allocation, 15 revision ve 15 override. Negatif batch bir kez mevcuttu.
- Aynı ekonomik girdiler yeniden preview edildi: 15.01.2027–14.02.2027 yürürlük, 02.02.2027 imza, 14.03.2027 ödeme, Ayşe Kaya ve 2,443.28 günlük taban ücret. Sonuç 6 kaynak dönem için toplam yeni brüt fark `0`, payable `0`, mahsup `0`, açık receivable `3268`; payment oluşturma düğmesi kapalıydı.
- Formda kayıtlı eski revision’ı seçme yolu olmadığı için bu preview eski ID’yi yeniden kullanamadı; `revision-bbeee405-3ce4-44c3-8180-dcb3ee7dd10b` ve `override-552dd279-fd69-48c9-85d7-1e69a3f9f339` ID’li bir DRAFT kaydetti. Preview sonrası export `/tmp/session3-fixtures/session3-reload-preview-before.json` ile önceki export’un exact karşılaştırmasında bordro, retro batch, allocation, puantaj, dönem ve personel dizileri aynı kaldı; yalnız beklenen DRAFT revision/override sayısı 15’ten 16’ya çıktı. Negatif batch sayısı 1, ona bağlı payment/accrual sayısı 0 kaldı.
- Reload yolları: uygulama menüsünde About/Services/Hide/Quit; File’da Close Window/Close All; Edit’te Undo/Redo ve standart düzenleme komutları; View’da Toggle Full Screen; Window’da pencere boyutlandırma/Close Window; Help menüsü boş; WebView context menu’de Reload yoktu. Kaynak incelemesinde `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, Tauri builder/IPC ve frontend içinde devtools enablement, reload command, menu item veya accelerator tanımı bulunmadı.
- Cmd+R, görünür form ve geçici preview result marker’ı varken denendi. Sonrasında aynı form, aynı `0` fark/`3268` receivable preview’ı ve aynı uygulama penceresi hâlâ görünüyordu. Bu nedenle Cmd+R document reload kanıtı değildir; marker kaybolmadı. Başka bir production-native reload yolu olmadığı için gerçek reload sonrası preview ve persistence testi çalıştırılmadı.
- Kapat/aç persistence ve negatif retro idempotency PASS sonucu önceki gerçek process kapat/aç testinden korunur. Bu turda uygulama kapatılmadı; kod veya build değişmedi, test suite çalıştırılmadı.

### Devam testlerinde gözlenen finansal ve veri çapraz kontrolleri

- Tam restore referansı test-results/session3/reference.json; SHA-256 90c2095da00fe3f23a483d7e66f70fc51251c4cb04451b67b0f7ad451f0851aa. UI’daki seçilebilir /tmp/session3-fixtures/12-full-reference.json kopyasının hash’i aynı.
- Başarılı full restore/reopen exact raporları test-results/session3/full-restore-comparison.json ve test-results/session3/full-restore-reopen-comparison.json; ikisinde de sıfır fark var. Restore sözleşmesindeki domain ve audit alanları exact karşılaştırıldı; yalnız exportedAt gibi export zamanı alanı dışarıda bırakıldı.
- FINALIZED retro apply sonrası kaynak bordronun tam JSON nesnesi referans nesnesiyle eşit kaldı. Ayrı payment/accrual retro-ba059316-a976-4016-aac6-8894eb807f0b; brüt 3,307.00, payable 3,307.00, net 2,364.21, ödeme 2027-05-14.
- Negatif batch retro-5b80adf8-da00-4820-a59e-85e8b49ffaad: CALCULATED / OVERPAYMENT; gross −3268, payable 0, offset 0, recovered 0, recoverable ve outstanding receivable 3268. Eksi tutar pozitif ödemeye dönüşmedi.
- Önceden bu aynı-negatif-fark akışının tekrar preview’ı outstanding receivable snapshot / chronological settlement replay hatası veriyordu. Hata sırasında duplicate finansal kayıt oluşmamıştı. Düzeltme sonrası tekrar preview 0 delta ile tamamlandı ve apply mümkün olmadı.
- Son negatif preview/export’unda 7 personel, 17 dönem, 84 puantaj, 119 bordro (101 CALCULATED, 17 STALE, 1 FINALIZED), 20 retro batch (12 CALCULATED/UNSETTLED, 7 STALE/UNSETTLED, 1 CALCULATED/OVERPAYMENT), 66 allocation, 3 hastalık raporu, 2 yıllık parametre ve 15 revizyon/override vardı. Bordro türleri: NORMAL 59, TEDIYE 20, TIS_IKRAMIYE 22, RETRO_ADJUSTMENT 12, SUPPLEMENTAL 6. Vergi açılışı ve zam ayları boştu.
- Final bundle’da dolu FINALIZED state’ten referans restore’u tekrar denemek ürünün FINALIZED-history korumasında “mevcut kayıt korundu” mesajıyla reddedildi. Hemen sonraki UI export /tmp/session3-fixtures/post-blocked-restore.json, deneme öncesi /tmp/session3-fixtures/session3-negative-reopen-check.json.json ile bütün JSON alanlarında eşitti; kısmi restore yok.

### Önceki bulguların devam sonucu

| Bulgu | Kota öncesi | Şimdi | Hedef test ve canlı tekrar | Kapanış |
|---|---|---|---|---|
| 1 — FINALIZED personel preview’ında ilgisiz eski puantaj | Düzeltme vardı, canlı tekrar eksikti | Uygun kaynak dönemleriyle preview başarılı; kaynak FINALIZED değişmedi | retro_regression 56/56 ve ilgili Bun preview/recovery testleri; Computer Use ile Oturum Kontrol preview | CLOSED |
| 2 — Native restore onayı/mesaj | Kod düzeltildi; valid import, iptal ve hata akışları canlıydı | Önceki live sonuçlar korundu; son native build üzerinde yeniden uygulanması gerekmedi | Controller/modal/helper testleri dahil frontend targeted testleri PASS | CLOSED |
| 3 — Retro içeren backup lifecycle validator | Full reference UI restore eksikti | 118 bordrolu V5 referansı UI’dan yüklendi ve exact karşılaştırıldı | cargo test -p bordro-programi --test migration_legacy_serde_parity_test 18/18; UI restore ve kapat/aç diff 0 | CLOSED |
| 4 — Decimal scale/optional PEK restore parity | Targeted test vardı, UI full restore eksikti | Full 118 restore alanları exact; forged finansal toplam reddedildi, state korundu | Migration 18/18; Decimal targeted Rust testi 1/1; full restore diff 0 | CLOSED |
| 5 — Native JSON download kilitlenmesi | Kaynak düzeltildi, build/live test eksikti | Son bundle’da gerçek JSON kaydedildi; ikinci dialog iptali ve responsive UI PASS | cargo test -p bordro-programi --lib backup_cmd::tests 2/2; frontend controller/bridge 17/17; Computer Use export/cancel | CLOSED |
| 6 — Restore audit zamanı Utc::now() ile değişiyordu | Kaynak düzeltildi, exact native restore testi eksikti | Audit dahil full referans restore ve yeniden açma exact; son bundle’daki yeniden deneme FINALIZED guard’ında güvenle reddedildi ve state exact korundu | Migration 18/18; iki UI comparison dosyası diff 0; final-bundle fail-closed export exact | CLOSED |
| 7 — Aynı tarihli negatif retro sonrası chronological replay mismatch | Yeni bulgu; negatif farkın ikinci preview’ı hata veriyordu | Preview aynı inputla 0 delta verdi; ek tahakkuk yok | Yeni thread 01a0f222-fa05-75e0-93cb-8c0a858c36f4; 6 retro integration testi PASS; final build’de canlı tekrar PASS | CLOSED |

### Yeni Bulgu 7 — P2, aynı gün negative retro replay sıralaması

- Ekran/işlem: Geriye Dönük Farklar; eksi retro tahakkukundan sonra aynı inputla preview.
- Yeniden üretme: Ayşe Kaya üzerinde 15.01.2027–14.02.2027, 02.02.2027 imza, 14.03.2027 ödeme ve 2,443.28 TL günlük ücret; −3,268.00 TL overpayment/receivable batch’ini oluşturduktan sonra aynı revision’ı tekrar preview et.
- Beklenen: tekrarlanan preview 0 yeni fark göstermeli; receivable aynı kalmalı ve duplicate apply olmamalı.
- Önceki gerçekleşen: outstanding receivable snapshot kronolojik replay ile uyuşmuyor hatası; batch sayısı artmadı, veri bozulmadı.
- Kök neden: aynı paymentDate’te batch’ler rastgele id’ye göre sıralanınca yeni eksi batch eski artı tahakkuktan önce replay edilebiliyordu.
- Düzeltme: replay sırası paymentDate, createdAt, id olarak deterministik hale getirildi; ekonomik retro ve PEK/receivable replay yolları aynı sıralamayı kullanıyor. Invalid createdAt fail-closed.
- Veri/finansal etki: düzeltme öncesi yanlış veya duplicate ödeme oluşmadı; tekrarlı preview bloklanıyordu. Düzeltme sonrası yeni tutar/payable 0, açık borç 3,268 TL olarak kaldı.
- GPT-6 Luna High thread: 01a0f222-fa05-75e0-93cb-8c0a858c36f4.
- Değişen dosyalar: crates/payroll-core/src/retro.rs, crates/payroll-core/tests/retro_regression.rs.
- Targeted testler: same_date_retro_replay_uses_created_at_before_id_and_does_not_repeat_overpayment; negative_retro_delta_is_explicit_and_not_clamped_to_zero; open_overpayment_does_not_create_duplicate_cash_payment; signed_overpayment_ledger_can_be_reconciled_by_a_later_authoritative_revision; partial_overpayment_offset_leaves_only_the_payable_remainder; larger_overpayment_than_new_entitlement_remains_outstanding — 6/6 PASS.
- Canlı doğrulama: son main bundle’de app kapat/aç sonrasında aynı input iki preview’da 0 yeni delta verdi; negative batch tek kaldı. Bu turda da aynı ekonomik girdilerle reload öncesi preview 0 delta verdi ve batch/payment/accrual dizileri exact değişmedi. Gerçek WebView reload’u Cmd+R ile doğrulanamadı; native shell’de başka reload yolu görülmediği için bu alt kontrol NOT TESTABLE kaldı. Tekrarlanabilirlik: düzeltme öncesi iki kez hata, düzeltme sonrası iki kez PASS.

### Son targeted test/build kaydı

- cargo test -p bordro-programi --lib backup_cmd::tests — 2/2 PASS.
- bun test src/hooks/useBackupController.test.ts src/services/tauriBridge.test.ts — 17/17 PASS.
- cargo test -p bordro-programi --test migration_legacy_serde_parity_test — 18/18 PASS.
- Retro integration’da replay regression’ı dahil 6 seçilmiş test — 6/6 PASS; retro workspace’in tamamı yeniden çalıştırılmadı.
- Önceki ilgili retro_regression 56/56, Decimal unit 1/1, Bun import/controller/bridge/storage 78/78 ve preview/recovery 2/2 sonuçları geçerlidir.
- rustfmt --check --edition 2021 crates/payroll-core/src/retro.rs crates/payroll-core/tests/retro_regression.rs ve git diff --check — PASS.
- bun run wasm:build — PASS; freshness hash a6c1bfbbfe126848e10f1b2743245882b4082004e897b7884ac960b7fb44db47.
- Ana son release: cargo tauri build --bundles app --config '{"identifier":"com.bordro.session3.validation"}' — PASS. Vite’in 500 KB üstü chunk uyarısı vardı, build başarısını etkilemedi.
- cargo test --workspace çalıştırılmadı. Eski tsc --noEmit kapsam dışı hataları (toContain matcher ve syncPuantajForSickLeaveDelete undefined) mevcut; bu session fix’leriyle ilgili değiller ve düzeltilmediler.

### Build ve son veri durumu

- HEAD/commit: 211cb19dba93ba8da9d61eaf5099b98773d8d751.
- Çalışma ağacı: dirty; bu oturumun kod düzeltmeleri, targeted test/fixture/rapor dosyaları ve WASM çıktıları korunuyor. Değişiklikler commit edilmedi.
- Son test edilen ana bundle: /Applications/4D Bordro Session 3.app.
- Bundle ID: com.bordro.session3.validation.
- Executable SHA-256: 277ed5903a511ac760d78e5f48d9eeeaa55f04ddff42648d13046120b991a85b.
- Eski com.bordro.app process’i kapalı; test edilen ana validation bundle çalışıyor. Release build kaynak kodu hem native download/timestamp düzeltmelerini hem retro replay düzeltmesini içeriyor.
- Sonradan oluşturulan yardımcı auditvalidation bundle’ı aynı kaynak ağacından derlendi; açıldığında boş state vermediği için temiz restore kanıtı olarak kullanılmadı. Ana restore PASS kanıtı yukarıdaki exact UI karşılaştırmalarıdır.
- Kota sonrası son test verisinin JSON yedeği /tmp/session3-fixtures/post-blocked-restore.json; temizlenmeden önceki karşılaştırma referansı /Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/test-results/session3/reference.json; korunmuş SQLite /tmp/session3-preserved-native.sqlite. Hiçbiri üzerine yazılmadı.
- UI restore’u mevcut FINALIZED test verisini başka bir referansla değiştirmeye karşı tasarım gereği engelledi. Başarısız restore sonrası tüm JSON state exact korundu. Başarılı full referans restore kanıtı ayrı exact comparison dosyalarında duruyor.

### Kapsam sınırlamaları

- **NOT TESTABLE — production native shell exposes no verified WebView reload path.** View/context menülerinde Reload yok; Cmd+R sırasında geçici preview marker’ı kaldı; config, build ve source scan’de devtools veya reload komutu tanımı bulunmadı. Kapat/aç sonrası idempotency PASS sonucu korunuyor.
- Mevcut referansta tax opening yoktu; dolu tax opening ile restore özel vakası yapılamadı. Dolu PEK carry özel vakası da oluşturulmadı; bulunan carry alanları restore karşılaştırmasına dahil edildi.
- Uygulamanın FINALIZED tarihçeyi topluca silebilen boş-state UI akışı yok. FINALIZED koruması gevşetilmedi; restore ancak mevcut history ile çelişmediğinde uygulanıyor.
- Oturum 4 işleri — banka, SGK Kontrol, Excel/PDF/yazdırma, web/native kıyaslaması ve genel adversarial UX — başlatılmadı.

Oturum 3 tamamlandı — WebView reload kriteri NOT TESTABLE; kapat/aç persistence ve negatif retro idempotency PASS.
