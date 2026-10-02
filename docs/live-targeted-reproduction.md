# Hedefli canlı yeniden üretim raporu

Tarih: 2026-10-02  
Kapsam: Yalnızca tespit, canlı yeniden üretim ve kök neden analizi. Kaynak kodu, migration veya test kodu değiştirilmedi; düzeltme uygulanmadı.

## 0. Build/source eşleşmesi

- Mevcut HEAD: 8d943932d20bfec7457750a379a77d4b81d4ac0c
- HEAD commit zamanı: 2026-10-01 20:18:33 +03:00
- Test edilen HEAD build: target/release/bundle/macos/4D Bordro Programı.app
- Build tamamlanma zamanı: 2026-10-02 16:54:43 +03:00
- Eski uygulama: app/4D Bordro Programı.app; dosya zamanı 2026-10-01 20:13:00 +03:00
- Her iki uygulamanın executable SHA-256 değeri aynıdır:
  f5a1f766b74f14b00f6c0990ef757a2b8c8f7b540569fec68681569c47946609
- Her iki bundle sürümü 0.1.0 ve bundle kimliği com.bordro.app. Executable içinde commit işaretçisi bulunmadı.

Sonuç: Eski ve HEAD uygulamaları arasında byte düzeyinde binary drift bulunmadı. Eski uygulama ayrı process olarak çalıştırıldı ve duplicate T.C. senaryosu bağımsız olarak tekrarlandı; aynı UNIQUE reddi gözlendi. İlgisiz personel silme ve temiz DB senaryoları eski binary üzerinde ayrıca çalıştırılmadı; aynı executable hash'i nedeniyle bu iki senaryo için eski/HEAD farkını gösteren bir kanıt yoktur.

Canlı ana DB, yalnızca salt-okunur sorgularla incelendi:

- /Users/kadir/.4d_bordro/4d_bordro_data/bordro.sqlite

Temiz başlangıç testi production DB'ye dokunmadan ayrı profil üzerinde yapıldı:

- /private/tmp/4d-clean-profile.0qPnHM/.4d_bordro/4d_bordro_data/bordro.sqlite

Build sırasında frontend ve Tauri binary'si mevcut HEAD'den yeniden üretildi. Çalışma sonunda git'te bu rapor dışında kaynak, migration veya test değişikliği yoktur. Önceden mevcut olan docs/live-payroll-findings.md değişikliği korunmuştur.

## FINDING-1 — Aynı T.C. No mevcut personelin üzerine yazmıyor

Önceki iddia:

Yeni personel, mevcut personelin T.C. No'su ile kaydedildiğinde mevcut kişi sessizce ortadan kalkıyor ve yeni kişi onun yerine görünüyordu.

Mevcut HEAD:

8d943932d20bfec7457750a379a77d4b81d4ac0c

Test edilen build:

HEAD'den 2026-10-02 16:54:43 +03:00 tarihinde üretilen 4D Bordro Programı.app. Aynı binary hash'e sahip eski app üzerinde de duplicate T.C. denemesi yapıldı.

Sonuç:

- NOT REPRODUCED — mevcut personelin üzerine yazıldığı iddiası yeniden üretilemedi.
- Gerçek gözlenen davranış EXPECTED BEHAVIOR — T.C. UNIQUE ihlaliyle kayıt reddediliyor.

Minimal yeniden üretim:

1. Personel A, TC Referans Test A, T.C. No 10000000078 ile UI üzerinden oluşturuldu.
2. DB salt-okunur kontrolünde A'nın ID'si p-1790949622073, T.C. No'su 10000000078 ve adı TC Referans Test A olarak kaydedildi.
3. Personel B, TC Çakışma Test B, aynı T.C. No ile kaydedilmeye çalışıldı.
4. Modal iptal edildi; uygulama yeniden başlatıldı; aynı T.C. No tekrar kontrol edildi.

Canlı gözlem:

- A oluşturulurken personel sayısı 56'dan 57'ye çıktı ve modal kapandı.
- B kaydında UI şu hatayı gösterdi: Kayıt başarısız: UNIQUE constraint failed: personnel.tc_no
- Modal hata sonrasında açık kaldı; B kaydı tamamlanmadı.
- Personel sayısı 57'de kaldı.
- A listede kaldı; B listede görünmedi.
- Yeniden başlatma sonrasında A aynı ad ve T.C. No ile listede kaldı.
- Eski app üzerindeki bağımsız denemede de Drift Referans Test kaydı korunurken aynı UNIQUE constraint failed: personnel.tc_no hatası alındı.

DB/state kanıtı:

- Duplicate denemesinden sonra personnel içinde 10000000078 için satır sayısı 1'dir.
- Satır: p-1790949622073 | 10000000078 | TC Referans | Test A
- A'nın ID'si duruyor; adı değişmedi; yeni B ID'si oluşmadı.
- Eski app denemesinde de 10000000634 için tek satır vardır: p-1790957013148 | 10000000634 | Drift Referans | Test.

Kod yolu:

PersonelFormModal.tsx:123-155
-> usePayrollMutationController.ts:280-284
-> tauriBridge.ts:73-75
-> native save_personnel command
-> PersonnelRepository::save, personnel_repo.rs:114-145
-> PersonnelRepository::save_in_transaction, personnel_repo.rs:220-247
-> SQLite personnel tablosu.

Form yeni kayıt için id değerini p-<Date.now()> biçiminde ayrı üretir. Repository önce yalnızca aynı id'yi arar ve SQL'de yalnızca ON CONFLICT(id) DO UPDATE kullanır. personnel.tc_no üzerindeki UNIQUE kısıtı aynı T.C. No'yu farklı id ile ekleme girişimini reddeder; mevcut kişinin ID'si hiçbir aşamada yeniden kullanılmadı.

Kök neden:

Önceki canlı rapordaki sessiz overwrite davranışı mevcut HEAD runtime'ında kanıtlanmadı. Mevcut çağrı zincirinde gözlenen ve kaynakla doğrulanan kök neden, farklı id ile gelen duplicate T.C. kaydının SQLite UNIQUE kısıtında reddedilmesidir. Eski/stale binary ihtimali bu senaryoda binary hash eşitliği ve eski app bağımsız denemesiyle desteklenmedi.

Düzeltme gerekli mi?:

- Hayır — overwrite bulgusu için.

Önerilen öncelik:

- Yok.

## FINDING-2 — İlgisiz personel silme, Retro Hedef'in retro zincirini bozmadı

Önceki iddia:

Retro sahibi Ada Korkmaz silinmediği halde başka örnek çalışanlar silindikten sonra Ada'nın retro kaydı STALE oldu ve sonraki bordrosu hesaplanamaz hale geldi.

Mevcut HEAD:

8d943932d20bfec7457750a379a77d4b81d4ac0c

Test edilen build:

HEAD'den yeniden üretilen build; canlı ana DB üzerinde yalnız UI mutation'ları ve salt-okunur DB snapshot'ları kullanıldı.

Sonuç:

- NOT REPRODUCED — başka personel silme -> Retro Hedef retro zinciri nedenselliği yeniden üretilemedi.
- Gerçek tekil mutation aramasında STALE üreten işlem bulundu: Retro Hedef'in kaynak dönemindeki puantaj değişikliği.

Minimal yeniden üretim:

1. Retro Hedef Test R, Silinecek A Test S ve Silinecek B Test S oluşturuldu.
2. Retro Hedef için 2026-12 ve 2027-01 normal bordroları hesaplandı; ikisi de CALCULATED duruma getirildi.
3. İki kaynak dönemli, pozitif fark üreten retro revision ve payment event oluşturuldu.
4. Başlangıç snapshot'ı alındı.
5. Yalnız Silinecek A UI üzerinden silindi ve hemen snapshot alındı.
6. Yalnız Silinecek B UI üzerinden silindi ve hemen snapshot alındı.
7. Son olarak Retro Hedef silinmeye çalışıldı; işlem tamamlanmadı, modal iptal edildi.
8. Önceki olayın muhtemel gerçek mutation'ını bulmak için Retro Hedef'in 2027-01 puantajında 15 Ocak günü Ç'den T'ye çevrildi ve her mutation sonrasında snapshot alındı.

Canlı gözlem:

- Başlangıçta Retro Hedef'in normal kaynak bordroları CALCULATED, retro batch CALCULATED, linked RETRO_ADJUSTMENT payment CALCULATED durumundaydı.
- Pozitif retro batch ID'si retro-0cbb12f3-4999-43d6-8215-9b710093c5d4'tür.
- Silinecek A silindikten sonra Retro Hedef batch'i, linked payment'i ve iki kaynak bordrosu aynı CALCULATED durumunda kaldı.
- Silinecek B silindikten sonra da aynı değerlerde kaldı.
- A ve B'nin silinmesi personel listesini 60'tan 59'a, sonra 58'e düşürdü; Retro Hedef listede ve DB'de kaldı.
- Retro Hedef silme onayına basma girişimi sonrasında modal kapanmadı ve görünür bir hata metni çıkmadı. Vazgeç ile kapatıldı. DB'de hedef personel, retro batch ve ödeme kaydı kaldı; silme tamamlanmış sayılmadı.
- Retro Hedef'in 2027-01 puantajı değiştirildikten hemen sonra hedef normal bordrosu STALE, retro batch STALE ve linked payment STALE oldu. 2026-12 normal bordrosu CALCULATED kaldı.
- Hedef normal bordrosu UI'daki Yeniden Hesapla ile başarıyla CALCULATED duruma getirildi; retro batch ve linked payment STALE olarak kaldı.

DB/state kanıtı:

Başlangıç:

- batch retro-0cbb12f3-4999-43d6-8215-9b710093c5d4: CALCULATED / UNSETTLED, total_gross_delta 16864125 kuruş, payable 16864125 kuruş.
- linked payment aynı ID'li RETRO_ADJUSTMENT kaydı: CALCULATED.
- 2026-12 NORMAL: CALCULATED.
- 2027-01 NORMAL: CALCULATED.

Silinecek A ve B silme adımları sonrasında:

- Hedef batch, linked payment ve hedef normal bordrolarında delta yoktur.
- Silinecek A ve B personnel tablosundan kalkmıştır; Retro Hedef satırı kalkmamıştır.

Puantaj mutation sonrasında:

- 2027-01 NORMAL: STALE.
- retro-0cbb12f3-4999-43d6-8215-9b710093c5d4 batch: STALE / UNSETTLED.
- Aynı ID'li RETRO_ADJUSTMENT payment: STALE.
- 2026-12 NORMAL: CALCULATED.

Normal bordro yeniden hesaplandıktan sonra:

- 2027-01 NORMAL: CALCULATED.
- Retro batch ve linked RETRO_ADJUSTMENT payment: STALE.

Kod yolu:

İlgisiz personel silme:

Personel ekranı delete handler
-> usePayrollMutationController.ts:350-353
-> tauriBridge.ts:87-89
-> native delete_personnel command
-> PersonnelRepository::delete, personnel_repo.rs:252-285
-> PayrollInvalidationRepository::assert_mutation_allowed
-> PayrollMutation::Person.

Politika, policies.rs:587-592 içinde Person mutation için retro batch'i yalnız batch.personnelId == mutation personnelId olduğunda etkilenmiş sayar. apply_impact, payroll_invalidation_repo.rs:119-128 içinde yalnız impact.affectedRetroBatches üyelerini STALE yapar. Bu nedenle Silinecek A veya B'nin ID'si Retro Hedef'in ID'si değildir ve hedef batch'e etki üretmez.

Gerçek STALE üreten mutation:

Puantaj UI
-> usePayrollMutationController.ts:477-481
-> tauriBridge.ts:121-123
-> attendance_cmd.rs::save_attendance
-> AttendanceRepository::save_in_transaction, attendance_repo.rs:184-224
-> PayrollMutation::PersonPeriod
-> payroll-core policies.rs:589-592
-> PayrollInvalidationRepository::apply_impact.

Bu mutation Retro Hedef ID'si ve 2027-01 source period ID'siyle gönderildi. Hem normal bordro hem de allocations içinde bu kaynak döneme bağlı retro batch etkilendi; canlı snapshot tam olarak bu sonucu gösterdi.

Kök neden:

Önceki “başka personel silme Ada'yı STALE yaptı” nedenselliği mevcut HEAD'de doğrulanmadı ve ID-bazlı politika ile uyumlu değildir. Kontrollü tek mutation aramasında kanıtlanan gerçek neden, Retro Hedef'in kaynak dönem puantajının değiştirilmesidir. Normal bordronun yeniden hesaplanabilmesi, blokajın sonraki normal bordroda kalıcı olmadığını; stale retro payment'in ise retro zincirinin içinde kaldığını gösterdi.

Düzeltme gerekli mi?:

- Hayır — ilgisiz personel silme davranışı için.
- STALE retro recovery ayrı FINDING-3 kapsamında ürün riski olarak değerlendirilmelidir.

Önerilen öncelik:

- Yok — silme -> ilgisiz retro etkisi için.

## FINDING-3 — STALE retro recovery'nin kırılma noktası

Önceki iddia:

STALE retro kayıtlarının “Ödeme olayını yenile” akışıyla toparlanabildiği, ancak bazı batch/payment state kombinasyonlarında recovery'nin kapalı döngüye girdiği bildirildi.

Mevcut HEAD:

8d943932d20bfec7457750a379a77d4b81d4ac0c

Test edilen build:

HEAD'den yeniden üretilen build; FINDING-2'deki kaynak mutation ve Geriye Dönük Farklar UI akışı kullanıldı.

Sonuç:

- PARTIALLY CONFIRMED.
- STALE batch + STALE payment canlı olarak üretildi.
- Bu state'te mevcut UI'da recovery düğmesi çıkmadı; yeni preview oluşturulabilse de aynı stale payment zincirine payment event yazılamadı.
- Normal bordro yeniden hesaplandı; stale retro batch/payment recovery olmadı.
- Negatif settlement için batch CALCULATED + payment yok durumu üretildi ve ödeme oluşturma düğmesi doğru olarak kapalı kaldı.

State sonuçları:

| State kombinasyonu | Canlı üretim | UI sonucu | Sınıflandırma |
|---|---|---|---|
| 3A: batch CALCULATED, payment STALE | Test edilen normal UI mutation'ları bu ara durumu üretmedi; puantaj mutation'ı ikisini birlikte STALE yaptı | “Ödeme olayını yenile” düğmesi gözlenmedi | STATE UI ÜZERİNDEN ÜRETİLEMİYOR |
| 3B: batch STALE, payment STALE | Retro Hedef'in 2027-01 puantajı değiştirildi | Batch satırında recovery düğmesi yok; preview mümkün, payment creation stale chain hatasıyla durdu | RECOVERY YOK |
| 3C: batch STALE, payment CALCULATED | Test edilen normal UI mutation'ları bu ara durumu üretmedi | Bu state için recovery yolu gözlenmedi | STATE UI ÜZERİNDEN ÜRETİLEMİYOR |
| 3D: batch CALCULATED, payment yok | Günlük taban ücret override değeri 1 ile negatif retro preview kaydedildi | Payable 0 olduğu için payment event düğmesi yok; settlement batch saklandı | RECOVERY YOK; no-payment settlement için beklenen davranış |

Minimal yeniden üretim — 3B:

1. Retro Hedef'in iki normal kaynak bordrosu ve pozitif retro payment event'i CALCULATED durumda hazırlandı.
2. 2027-01 puantajında tek gün Ç'den T'ye çevrildi.
3. Hemen sonra normal bordro STALE, retro batch STALE, linked payment STALE olarak görüldü.
4. Geriye Dönük Farklar ekranında hedef satırı Geçersizleşti / Ödeme bekliyor durumuna geçti; action column içinde “Ödeme olayını yenile” yoktu.
5. Hedef normal bordrosu “Yeniden Hesapla” ile hesaplandı. Normal bordro CALCULATED oldu; retro batch/payment STALE kaldı.
6. Aynı hedef için yeni bir revision preview oluşturuldu. Preview sonucu pozitif payable gösterdi.
7. Geriye Dönük Fark Tahakkuku Oluştur denemesi yeni batch/payment yaratmadı ve şu hatayı verdi: Payment-event/PEK zinciri çözülemez: retro-0cbb12f3-4999-43d6-8215-9b710093c5d4 tahakkuku STALE durumda; authoritative state belirlenemiyor. Çözüm: Bordro Hesaplama ekranında ilgili kişinin bordrosunu “Hesapla” ile veya “Tüm Hesaplanabilir Bordroları Hesapla” ile yeniden hesaplayın; önceki güncelliğini yitirmiş tahakkuklar sırasıyla otomatik yeniden hesaplanır.

Minimal yeniden üretim — 3D:

1. Aynı hedef için yeni revision oluşturuldu.
2. GUNLUK_TABAN_UCRET override değeri 1 yapıldı.
3. Preview sonucu -160.872,98 TL toplam brüt fark, 0,00 TL payable settlement ve 160.872,98 TL açık receivable gösterdi.
4. Settlement kaydı saklandı.
5. UI payment event oluşturmadı; geçmişteki satır Hesaplandı / Fazla tahakkuk olarak göründü.

Canlı gözlem:

- 3B durumunda sonraki normal bordro hesabı stale normal kaydı düzeltti; fakat bunun öncelik zincirindeki RETRO_ADJUSTMENT event'ine etkisi olmadı.
- Yeni preview/revision yazılabildi.
- Stale batch ile payment creation aynı batch kimliğini tekrar kullanarak ilerleyemedi.
- 3D'de payment event olmaması UI'da net olarak görüldü; negative settlement batch'i saklandı ancak payroll_records içinde linked RETRO_ADJUSTMENT satırı oluşmadı.

DB/state kanıtı:

- 3B pozitif batch retro-0cbb12f3-4999-43d6-8215-9b710093c5d4: STALE / UNSETTLED.
- 3B linked payment: accrual_id aynı batch ID, RETRO_ADJUSTMENT, STALE.
- 3B kaynak normal bordroları son normal recalc sonrasında CALCULATED.
- 3D batch retro-43d8c24e-6c31-416b-b203-c334568697c1: CALCULATED / OVERPAYMENT, total_gross_delta -16087298 kuruş, payable_settlement_amount 0.
- 3D batch ID için payroll_records içinde linked payment satırı yoktur.

Kod yolu:

Recovery görünürlüğü:

GeriyeDonukFarklar.tsx:610
-> retroEventRecovery.ts:3-14
-> replay koşulu batch.status == CALCULATED, payableSettlementAmount > 0, linked payroll accrual type RETRO_ADJUSTMENT, aynı batch/person/date ve payroll.status == STALE şartlarının tamamını ister.

Replay:

GeriyeDonukFarklar.tsx:363-389
-> preview hesaplama
-> payable pozitif kontrolü
-> usePayrollMutationController içindeki create retro payment handler
-> tauriBridge.createRetroPayment
-> native payroll service create_retro_payment
-> payroll_core::calculate_payroll_checked
-> payroll_engine.rs:1209-1227 ensure_authoritative_payment_event.

Service CALCULATED olmayan batch'i payment event'e çevirmeyi reddeder. Ayrıca payable settlement sıfırsa payment event oluşturmayı reddeder. Stale linked payment, payment-engine preflight içinde authoritative state olarak kullanılamaz ve canlı hata metnini üretir.

Kök neden:

Mevcut recovery düğmesi yalnızca dar bir state için açılıyor: CALCULATED batch + pozitif payable + aynı batch'e bağlı STALE RETRO_ADJUSTMENT payment. Kaynak mutation batch'i de STALE yaptığında bu guard false oluyor. Sonrasında yeni preview mümkün olsa bile create payment service'i stale payment event'i authoritative kabul etmiyor. Bu iki koşul birlikte STALE batch + STALE payment için UI üzerinden kapanmayan bir recovery döngüsü oluşturuyor.

3D negatif settlement için payment event oluşturulmaması kaynak ve canlı davranışla uyumludur; burada recovery kusuru iddia edilmiyor. Asıl düzeltme adayı, pozitif payable içeren STALE retro batch/payment zinciri için audit'i koruyan açık yeniden hesaplama/yeniden bağlama akışıdır.

Düzeltme gerekli mi?:

- Evet — pozitif payable içeren STALE retro batch + STALE payment için recovery yolu açısından.
- Hayır — 3D payable 0 negative settlement'in payment event oluşturmaması için.

Önerilen öncelik:

- P1.

## FINDING-4 — Temiz DB'de ilk dönem varsayılan 0 ile açılamıyor

Önceki iddia:

Temiz DB'de Ocak 2027 dönemi günlük taban ücret 0 olduğu için açılamıyor; önce örnek veri yüklemek gerekiyor.

Mevcut HEAD:

8d943932d20bfec7457750a379a77d4b81d4ac0c

Test edilen build:

HEAD'den yeniden üretilen build, ayrı temiz profil ve resmi UI örnek veri yükleme/reset yolu kullanılarak test edildi.

Sonuç:

- CONFIRMED.

Minimal yeniden üretim:

1. Production DB'ye dokunmamak için ayrı temiz profil açıldı. Başlangıçta personnel 0, periods 0, institution settings 0, payroll 0 idi.
2. Hiç personel oluşturulmadan Dönem Ayarları > Yeni Dönem Aç akışına gidildi.
3. 2027 yılı ve Ocak seçildi; yeni dönem formunda günlük taban ücret alanı yoktu ve görünür pozitif kurum ücreti girişi yapılmadı.
4. Dönemi Oluştur ve Geç tıklandı.
5. Resmi Örnek Verileri Yeniden Yükle akışı kullanıldı ve UI'nin yüklediği örnek aktif dönemin Ücretler ekranında Günlük Taban Ücret 2443,28 TL olarak kaydedildi.
6. Yeni Dönem Aç akışında tekrar Ocak 2027 seçilip oluşturma denendi.

Canlı gözlem:

- Temiz ilk denemede UI şu hatayı gösterdi: İşlem gerçekleştirilemedi: Dönem işlemi kaydedilemedi: 2027-01 dönemi günlük taban ücreti sıfırdan büyük olmalıdır.
- İlk denemeden sonra DB'de 2027-01 period veya settings satırı oluşmadı.
- Örnek veri yükleme UI üzerinden başarılı oldu: Örnek veriler başarıyla yüklendi ve kalıcı olarak kaydedildi.
- Örnek aktif dönemin Ücretler ekranında Günlük Taban Ücret 2443,28 TL görüldü ve kaydedildi.
- Buna rağmen Yeni Dönem Aç > Ocak 2027 denemesi aynı günlük taban ücret sıfırdan büyük olmalıdır hatasıyla sonuçlandı.
- İkinci denemeden sonra da 2027-01 period/settings satırı oluşmadı.
- Yeni Dönem formunun kendisinde günlük taban ücret alanı bulunmadığı için kullanıcı bu akışta değeri doğrudan göremiyor.

Form/payload/backend kanıtı:

- Kaynakta boş kurulum varsayılanı DEFAULT_PRODUCTION_KURUM_DEGERLERI.gunlukTabanUcret = 0'dır.
- PeriodSettingsPage yeni dönem payload'ını paramsForm.gunlukTabanUcret ?? 0 ile kuruyor.
- Tauri bridge save_period_with_settings çağrısına period ve settings nesnelerini gönderiyor.
- IPC payload'ı ayrıca loglanmadı; bu nedenle gönderilen JSON'un ham wire çıktısı raporlanmıyor.
- Bununla birlikte live backend doğrulaması 2027-01 için değerin sıfır eşiğinde kaldığını açıkça gösterdi; settings repository doğrulamasında gunlukTabanUcret <= 0 doğrudan aynı hata metnine dönüştürülüyor.
- DB transaction başarısız olduğu için period ve settings insert'i geri alındı; bu, hatanın period oluşturma sonrasında değil settings validation aşamasında çıktığını gösteriyor.

DB/state kanıtı:

- Temiz ilk deneme: periods 0, institution_settings 0, personnel 0, payroll 0.
- Örnek veri ve ücret kaydı sonrasında: örnek periods 9, institution_settings 9; örnek aktif dönem settings değeri 2443.28.
- İkinci 2027-01 denemesi sonrasında 2027-01 period veya settings satırı yoktur.

Kod yolu:

PeriodSettingsPage.tsx:368-418
-> usePayrollMutationController.ts:375-382
-> tauriBridge.ts:107-114
-> period_cmd.rs:25-33
-> PeriodService::save_period_with_settings, period_service.rs:39-67
-> SettingsRepository::save_institution_settings_in_transaction, settings_repo.rs:135-159
-> payroll-core validate_kurum_degerleri_for_payroll, calculations.rs:650-657.

Kök neden:

Mevcut HEAD'in boş kurulum kurum ücreti varsayılanı bilinçli olarak 0'dır. Yeni dönem UI'si bu değeri yeni döneme taşımadan veya kullanıcıdan yeni dönem akışı içinde pozitif değer almadan backend'e ulaştırıyor; backend sıfır/negatif günlük taban ücrette transaction'ı reddediyor. Örnek veriyle başka bir dönemin ücretini kaydetmek, yeni dönem payload'ına pozitif günlük taban ücret taşımadı. Önceki bulgu bu build'de doğrulanmıştır; eski binary ile HEAD arasında drift yoktur.

Düzeltme gerekli mi?:

- Evet.

Önerilen öncelik:

- P1.

## FINDING-5 — Binary drift kontrolü

Executable SHA-256 değerleri eski app ve HEAD build için aynıdır:

f5a1f766b74f14b00f6c0990ef757a2b8c8f7b540569fec68681569c47946609

| Senaryo | Eski build | Mevcut HEAD build |
|---|---|---|
| Duplicate T.C. | Bağımsız tekrar: mevcut kayıt korunuyor; duplicate kayıt UNIQUE hatasıyla reddediliyor | Aynı davranış: overwrite NOT REPRODUCED; UNIQUE reddi |
| İlgisiz personel silme -> retro | Ayrı canlı tekrar yapılmadı; executable HEAD ile byte-identical | A ve B silme sonrası Retro Hedef CALCULATED kaldı; NOT REPRODUCED |
| Temiz DB ilk dönem | Ayrı canlı tekrar yapılmadı; executable HEAD ile byte-identical | CONFIRMED: temiz ve sample sonrası 2027-01 günlük taban ücret 0 validation hatası |

Karar: Bu çalışma için stale/eski binary'nin önceki bulguları oluşturduğunu gösteren binary drift kanıtı yoktur. Duplicate senaryosu eski app üzerinde ayrıca aynı sonuçla görüldü; 2 ve 4 için eski uygulama üzerinde yeniden işlem yapmaya gerek bırakacak bir binary farkı saptanmadı.

## Son karar tablosu

| Konu | Önceki rapor | Güncel canlı test | Kodla uyum | Düzeltilecek mi? |
|---|---|---|---|---|
| Duplicate T.C. | HIGH | NOT REPRODUCED; UNIQUE rejection | Uyumlu: tc_no UNIQUE, upsert yalnız id üzerinden | Hayır |
| İlgisiz personel silme -> retro | HIGH | NOT REPRODUCED; gerçek STALE mutation puantaj değişikliği | Önceki nedensellik kodla uyumsuz; PersonPeriod mutation sonucu kodla uyumlu | Hayır |
| STALE retro recovery | HIGH | PARTIALLY CONFIRMED; STALE/STALE recovery yok | Kısmen uyumlu: UI guard ve backend authoritative-state reddi gap'i açıklıyor | Evet — P1 |
| Temiz DB ilk dönem | MEDIUM | CONFIRMED; 0 günlük taban ücret hatası | Uyumlu: production default 0, backend validation zorunlu | Evet — P1 |

## DÜZELTME ADAYLARI

1. P1 — Pozitif payable içeren STALE retro batch + STALE payment için audit ledger'ı bozmadan authoritative preview, aynı batch kimliği ve linked payment event'i yeniden oluşturabilen açık recovery lifecycle'ı.
2. P1 — Temiz kurulumda ilk dönem için pozitif kurum ücretlerinin yeni dönem payload'ına taşınması veya ilk dönem açılışında kurum ücretlerini zorunlu ve görünür şekilde alma akışı.

Bu adayların hiçbiri bu çalışma sırasında uygulanmadı.

## Kesin kapsam sonucu

- Duplicate T.C. overwrite: mevcut HEAD'de yeniden üretilemedi; gözlenen gerçek davranış UNIQUE reddidir.
- İlgisiz personel silme -> başka personelin retro'sunu STALE yapma: yeniden üretilemedi.
- Gerçek STALE üreticisi: aynı personelin kaynak dönem puantaj mutation'ı; normal bordro yeniden hesaplanırken retro batch/payment STALE kalıyor.
- STALE retro recovery: batch ve payment birlikte STALE olduğunda mevcut UI üzerinden recovery yok; pozitif payable zinciri için P1 düzeltme adayıdır.
- Temiz DB ilk dönem: mevcut HEAD'de doğrulandı; 0 günlük taban ücret nedeniyle yeni dönem transaction'ı reddediliyor.
- Eski ve HEAD executable'ları arasında binary drift bulunmadı.
