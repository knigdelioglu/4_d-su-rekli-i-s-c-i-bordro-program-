# Geriye dönük farkın güncelliğini geri kazanması

## Kök nedenler ve düzeltme

- Normal bordro zinciri, STALE retro ödeme olayında duruyor; retro ekranı ise kaynak bordrolar STALE olduğunda yeniden hesaplamayı kapatıyordu. İki ekran artık aynı zincir yürütücüsünü kullanır: mevcut kaynak tahakkuklar, retro hakediş/ödeme ve ardından bağlı ödemeler kanonik ödeme sırasıyla hesaplanır. Tamamlandı denmeden kalıcı durum yeniden kontrol edilir.
- Retro kaynak PEK devri, aynı ödeme günündeki önceki normal bordroyu geçersiz kılabilir. Bu bağımlılık korunur. Yerel SQLite işleminde ve tarayıcının sürüm kontrollü tek kaydında, etkilenen önceki tahakkuklar yeni retro dağılımıyla yeniden hesaplanır; hata oluşursa bu güncellemenin tamamı geri alınır. Eski PEK düzeltmesinin sıfıra dönmesi de eski ve yeni dağılımın etkileriyle değerlendirilir.
- Çekirdek, PEK düzeltmesinin uygulandığına yalnız önceki olay doğrudan retro olduğunda inanıyordu. Yeniden hesaplanmış normal bordro ve onu izleyen ödemeler için artık kayıtlı PEK kaynak zinciri doğrulanır. Normal bordronun hesap snapshot'ı, kaynak kimliği, dönem, ay farkı ve kesin ondalıklı devir tutarları eşleşmelidir. Eski veya kaynağı değiştirilmiş snapshot reddedilir.
- Eksik tarihsel normal bordro hatası, genel bir “kesinleşmiş geçmiş değiştirilemez” mesajına çevriliyordu. Asıl kaynak dönem ve uygulanabilir çözüm korunur. Eksik bordro sıfır kabul edilmez veya kendiliğinden oluşturulmaz.

Ek ödeme formunda varsayılan HTML gönderimi, ortak işlem kilidini beklemeden önce eşzamanlı durdurulur. Böylece kaydetme sırasında sayfa yenilenerek işlemin kaybolması engellenir.

Finansal formüller, oranlar, yuvarlama, kesinleşmiş kayıt engelleri ve ödeme/mahsup kapanış kuralları değişmedi. Döngüsel bağımlılık, eksik kaynak, kesinleşmiş kayıt veya başarısız kalıcı güncelleme başarı olarak raporlanmaz.

## Eylül 2028 verisiyle doğrulama

Asıl SQLite veritabanı salt okunur açılıp geçici kopyası üzerinde çalışıldı. Eylül 2028 normal bordrosu hesaplandı; bağlı retro yeniden hesabı şu gerçek nedenle durdu:

- Şubat 2027'yi kapsayan ücret revizyonu, 15 Ocak–14 Şubat 2027 hizmet dönemiyle de kesişiyor.
- İlgili personelin `2027-01` normal bordrosu mevcut değil. Şubat kaydının bulunması Ocak kaynağının yerine geçmez.
- Ocak'a yeni tarihsel normal bordro ekleme politikasının etkisi dört kesinleşmiş kayda ulaşıyor. Bu nedenle yeni bir bordro uydurarak kurtarma yapılamaz.
- Özgün Ocak bordrosu doğrulanmış yedek/kayıtlardan bulunmalı; revizyonun yürürlük tarihleri hatalıysa ancak gerçek karar/sözleşme dayanağıyla düzeltilmelidir. Otomatik kapsam daraltma veya kesinleşmiş geçmişi yeniden yazma uygulanmadı.

Başarısız retro kurtarmasından önce/sonra bütün hesaplama verisi ve normal hesaplamadan önce/sonra kesinleşmiş kayıtlar eşit bulundu. Mevcut sentetik veriler değiştirilmedi.

## Değişikliğin sahipleri

- `src/services/payrollEngine/replayChain.ts`: ekranlar arasında ortak ödeme sırası ve kalıcı durum kontrolü.
- `src/services/payrollEngine/retroRecovery.ts`: tarayıcıda kesin ondalıklı, tek kayıt halinde retro kurtarması.
- `src/hooks/usePayrollMutationController.ts`: yetkili veri kaydı ve ekranlara bağlantı.
- `src-tauri/src/services/payroll_service.rs`: yerel atomik kurtarma ve önceki PEK tüketicilerinin güncellenmesi.
- `crates/payroll-core/src/payroll_engine.rs`: yeniden hesaplanmış PEK kaynak zincirinin doğrulanması.

## Regresyon kontrolleri

Çekirdek hesaplama testleri, yerel retro testleri, zincir sırası testleri ve tarayıcıdaki gerçek ekran akışı kullanılır. Eksik kaynak, kesinleşmiş kayıt, sahte/eski PEK kaynağı, sıfır farka dönüş, mükerrer kayıt ve yazma hatasında geri alma senaryoları kapsanır. Gerçek veri kopyası testi normal çalıştırmada kapalıdır; yalnız açıkça verilen `/private/tmp/` kopyasıyla çalışır.
