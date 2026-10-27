# Ücret politikası ve toplu işlem düzeltmeleri — 27 Ekim 2026

## Sonuç ve veri sınırı

Düzeltmeler kaynak koduna uygulandı. Mevcut finansal hesap motoru, kesin ondalık tutarlar, kesinleşmiş bordro ve retro engelleri korunuyor. Asıl kullanıcı veritabanına yazılmadı. Yol yuvarlama onarımı yalnız geçici SQLite kopyasında çalıştırıldı; kopya doğrulama sonrasında silindi. Kurulu `/Applications` paketi değiştirilmedi.

## Kök nedenler ve uygulanan davranış

| İstek | Kök neden / değişiklik | Davranışın sahibi |
| --- | --- | --- |
| Değişmeyen zam taslağının eski sayılması | UI, ham JSON eşitliğine dayanıyordu. Rust serde, boş isteğe bağlı alanı atabiliyor; Decimal ölçekleri değişebiliyor. Kesin ondalık metinle anlamsal işlem anahtarı kullanılıyor. `null` ile açık sıfır, gerçek tutar, tarih, kapsam ve yöntem değişiklikleri ayrı kalıyor. Kayda önizlemedeki doğrulanmış işlem gönderiliyor. | `compensationPolicy/contract.ts`, `Settings/CompensationPolicyPage.tsx` |
| Bağlı yol yuvarlama çakışmaları | Tek sürümü düzeltmek sonraki hizmet döneminde yeni çakışma üretebiliyordu. Bağlı aynı tarih düzeltmeleri birlikte önizleniyor ve tek SQLite / IndexedDB işlemiyle yazılıyor. Nihai zaman çizelgesi mevcut Rust kurallarıyla doğrulanıyor. Eski sürümler, tutarlar ve yürürlük tarihleri değiştirilmiyor. | Native `compensation_policy_service.rs`, tarayıcı `compensationPolicy/api.ts` |
| Toplu hesaplamada ekran yaşam döngüsü | İlerleme ve kilit ekranın yerel state/ref alanlarındaydı. Ortak işlem deposu, kalıcı kişi bazlı kontrol noktaları ve uygulama düzeyinde güncel veri erişimi eklendi. Tekil ve toplu hesaplar ortak kilidi kullanıyor; desteklenen tarayıcılarda pencereler arası Web Lock uygulanıyor. | `bulkPayrollJob.ts`, `useBordroCalculationController.ts`, `App.tsx` |
| Yarım kalan hesaplama | Açılışta süren kişi beklemeye dönüyor. Başarılı kişiler korunuyor; güncelliğini yitiren başarılar sürdürmede tekrar değerlendirilir. Ek tahakkukun işlemden türetilen sabit kimliği ve kayıt öncesi saklanan girdileri, kayıt ile kontrol noktası arasındaki kesintide ikinci kayıt oluşmasını engeller. | Aynı işlem deposu ve hesaplama denetleyicisi |
| Toplu puantaj | Ortak motorun kişi/dönem değişiklik politikasıyla kesinleşmiş bordro ve retro bağımlılıkları salt okunur önizlenir. Engelli kişiler seçilemez; kullanıcı uygun personeli seçer. R koruması ve rapor uyumsuzluğu engeli korunur. Native kayıt, yalnız seçili günleri işlem içindeki güncel veri üzerine uygular; seçilmeyen tarihler kaybolmaz. Seçili kayıtlar atomik yazılır. | `attendanceBulkDependencies.ts`, `PuantajGrid.tsx`, native `attendance_repo.rs` |
| Toplu tediye / TİS / ek ödeme | Personel seçimi, kişi başı brüt tutar, tarih ve açıklama formu; kayıt oluşturmayan uygunluk/bağımlılık önizlemesi; kişi bazlı kayıt ve sürdürme eklendi. Hesaplama başladığında form kapanır ve ortak ilerleme gösterilir. | `BulkAccrualDialog.tsx`, ortak işlem deposu |
| Görünür ilerleme ve mesajlar | Kişi durumu, başarı/hata/atlanan sayıları bütün ekranlarda görünür. Yol yuvarlama, geçmiş hesap ve güncel hesaplama verisi hataları anlaşılır Türkçeye çevrildi. Yeni puantaj için tekrarlanan “Varsayılan Puantajı Oluştur” düğmesi teke indirildi. | `BulkPayrollProgress.tsx`, iki ekran ve hesaplama denetleyicisi |
| T.C. kimlik numarası | Kontrol basamağı doğrulaması eklenmedi. Formdaki 11 hane ve mükerrer kayıt kontrolleri korunur. Kontrol basamağı geçersiz 11 haneli numaranın kaydı ayrıca regresyon testine bağlandı. | Mevcut personel kayıt yolu ve testleri |

## Ekim 2027 / Ocak 2028 kanıtı

Salt okunur kaynakta 1 Kasım 2027 sürümü `PERIOD_TOTAL` yönteminden `PER_COMPENSATION_SLICE` yöntemine, 1 Şubat 2028 sürümü yeniden `PERIOD_TOTAL` yöntemine geçiyor. Bu tarihler 15–14 hizmet dönemlerinin içine düşüyor. Aradaki 1 Ocak 2028 sürümü nedeniyle tekil düzeltme yeni bir Aralık çakışması yaratabiliyor.

Geçici veritabanı kopyasında üretim servisiyle hazırlanan plan, **1 Kasım 2027 ve 1 Ocak 2028** sürümlerine aynı tarihli iki düzeltme ekledi. Ekim 2027 ve Ocak 2028 politika çözümlemesi başarılı oldu. Eski politika sürümleri ve **8 kesinleşmiş bordro**, ham SQLite satırlarının tüm sütunları dahil, değişmedi. Bu kontrol bordroları yeniden hesaplamaz; yol yuvarlama engelinin kaldırıldığını doğrular.

Asıl kayıtlarda onarım için ilgili dönemde Ücret Politikası → **Bağlı düzeltmeleri önizle** → **Önizlenen düzeltmeleri birlikte kaydet** akışı kullanılır. Kayıt sırasında önizleme tokenları, tüm dönemlerin çakışma kuralları ve kesinleşmiş bordro/retro engelleri yeniden kontrol edilir. İkinci sürüm eklenirken hata çıkarsa ilk ekleme de geri alınır.

## Doğrulama

- İlgili TypeScript birim/regresyon testleri: **60 başarılı**; IPC argüman testleri: **13 başarılı**.
- Ücret Politikası render edilmiş tarayıcı testleri: **22 başarılı**; IndexedDB/politika entegrasyonu: **13 başarılı**; PuantajGrid tarayıcı testleri: **2 başarılı**.
- Son üretim çıktısı üzerinde **4 uçtan uca test başarılı**: ekran değiştirerek 5/5 hesaplama ve yeniden açma, üç tahakkuk türü/personel seçimi/mükerrer tıklama, kayıt sonrası kesintiden aynı tahakkukla sürdürme, kesinleşmiş kişiyi dışlayarak yalnız seçilen personelin puantajını değiştirme; mevcut personel kontrolleri de sınandı.
- Native politika API, puantaj dönem bütünlüğü, finansal bütünlük, çoklu tahakkuk ve personel kaydı regresyonları başarılı. Bağlı onarımın ikinci ekleme hatasında tamamen geri alınması ayrıca doğrulandı.
- Gerçek verinin geçici kopyası üzerindeki açıkça işaretlenmiş native onarım testi başarılı; kaynak veritabanı değiştirilmedi.
- `cargo check -p bordro-programi --offline -j 2`, `bun run lint`, `bun run test:e2e:typecheck`, `bun run verify:privacy`, WASM güncellik kontrolü, `bun run build`, `git diff --check`: başarılı.

Tarayıcı testleri, ortamda Playwright'ın indirilmiş Chromium'u bulunmadığından `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` ile mevcut Chrome'u kullanır. Her render/API test dosyası kendi tarayıcı yaşam döngüsüyle ayrı çalıştırılır. Üretim derlemesindeki mevcut büyük Excel dışa aktarım parçası uyarısı bu işlemlerin çalışmasını engellemez.
