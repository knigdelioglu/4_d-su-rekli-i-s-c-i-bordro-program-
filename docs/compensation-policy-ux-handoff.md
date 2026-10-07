# Ücret politikası ekranı: UX teslim notu

## Amaç

Ücret geçmişi olmayan bir bordro döneminde kullanıcının mevcut ücret ayarlarını kayıpsız incelemesini, eksik kapsam kararlarını vermesini, etkileri görmesini ve doğrulanmış başlangıç politikasını kaydetmesini sağlamak. Ekran iç uygulama adlarını ve kayıt kimliklerini göstermemeli; eski ayarları politikaya dönüştürmeden önce kullanıcıya düzenlenebilir taslak olarak sunmalıdır.

## Gerçek kaynak durumu

Salt okunur yerel veritabanı incelemesinde `2026-09` dönemi 15.09.2026–14.10.2026 aralığında. Aynı dönem için ücret politikası sürümü bulunmuyor. `institution_settings` kaydı 05.10.2026 17:24:29 UTC zamanında güncellenmiş; dolayısıyla başlangıç ücreti kaydı eksik, ancak ayar arşivi mevcut.

Arşivden gelen ve ekranda aynen gösterilmesi gereken değerler:

| Alan | Değer |
| --- | ---: |
| Günlük taban ücret | 2.616,26 TL |
| Günlük yemek | 300,75 TL |
| Birleştirilmiş sosyal yardım | 5.089,7 TL |
| Günlük vasıta/yol | 128,93 TL |
| Giyim yardımı | 269,7 TL |
| Hizmet zammı birimi | 24,67 TL |
| Genel iş primi yüzdesi | 0% |
| Gece çalışma primi | 0% |
| Gece çalışma tatili primi | 0% |
| Ek ödeme | 0 TL |
| Diğer gelir varsayılanı | 0 TL |

İş primi grupları: 1. Grup %9, 2. Grup %8, 3. Grup %7 ve UX Deneme Grubu %6; dördü de etkin. Grup eşleştirme kodları kaynakta korunur. Kaynaktaki `0`, tutarı sıfır olan bir değerdir; `null`/uygulanmıyor durumundan farklıdır.

## UX düzenleme taslağı

- Başlangıç kaydı yoksa sayfa başlığı “Başlangıç ücretlerini oluştur” olur; dönem ve geçerlilik tarihi ile kaynak tutarlarının formu kaydedilmemiş taslak olduğu açıkça görünür. Yinelenen ücret özeti kaldırılır.
- Günlük ücretler ve yan haklar 2–3 sütunda, ortak format notuyla düzenlenir. Dört prim grubu tek karşılaştırma tablosunda gösterilir; dar ekranda grup adı/oran ve etkinlik/kod/işlem okunabilir satırlara akar. Grup kimliği yalnız açılabilir personel eşleştirme ayrıntısında kalır.
- Kapsam ve yuvarlama seçenekleri “Hesaplama ayrıntıları” disclosure içinde kalır; özet kayıtlı seçimi veya “Seçim gerekli” durumunu söyler. İlk seçim hatasında bölüm açılır ve kullanıcı eksik karara yöneltilir.
- İş akışı tutarlar → hesaplama kararları → salt okunur önizleme → ayrı başlangıç onayı → kaydetme sırasındadır. Onay, tarih veya ücret değişince geçersizleşir; backend onay alanı, CAS, blocker ve yeniden yükleme davranışı değişmez.
- Boş/null ve açık sıfır korunur. Genel iş primi, grup oranının yedeği gibi sunulmaz; uyumluluk ayrıntısında yeni motorun bu alanı grup oranı yerine kullanmadığı açıkça belirtilir.

## Ekran düzeni ve içerik

1. **Dönem başlığı:** “Eylül 2026 · 15.09.2026–14.10.2026”. Dönem adı ve tarih bir kez gösterilir.
2. **Başlangıç durumu:** “Bu dönem için başlangıç ücreti kaydedilmemiş.” Arşiv değeri kullanılabiliyorsa kaynağın taslağa yüklendiği ve yüklemenin kayıt oluşturmadığı açıklanır. Eksik kaynak alanı tahmin edilmez.
3. **Ücret formu:** Günlük tutarlar ve yan haklar masaüstünde üç, dar görünümde iki sütunda sunulur. Ortak format açıklaması alanların üzerinde bir kez gösterilir; yinelenen ücret özeti yoktur.
4. **Prim grupları:** Dört grup tek tabloda karşılaştırılır. Dar görünümde grup adı/oran ile etkinlik, kod ayrıntısı ve kaldırma işlemi kompakt satırlarda kalır; mevcut dört grup 390×844 görünümde kaydırma alanına sığar. Personel eşleştirme kodu kapalı “Kod” ayrıntısında gösterilir ve yalnız açıldığında düzenlenir.
5. **İşlem ve kararlar:** Başlangıç tarihi ile kısa açıklama ücret alanlarının önünde bulunur. Kapsam ve yuvarlama “Hesaplama ayrıntıları” içinde kapalıdır; özet kayıtlı kararı veya “Seçim gerekli” durumunu bildirir. Önizleme istendiğinde eksik karar bölümü açılır, ilk eksik seçenek odaklanır ve kaydırma alanında görünür kalır.
6. **İleri ayrıntılar:** Gece/tatil oranları, ek gelir varsayılanları, eski genel iş primi uyumluluk alanı ve yüzde artış aracı kapalı ayrıntı bölümlerinde tutulur. Boş değer ve açık `0` onay kutusu ile ayrılır; genel iş primi alanı grup oranının yedeği olarak tanıtılmaz.
7. **Önizleme ve alt işlem alanı:** Kullanıcı önce salt okunur etki önizlemesini görür, başlangıç ücretini ayrı düğmeyle onaylar ve son olarak kaydeder. Her durumda yalnız sıradaki işlem sunulur. Form kaydırılır; işlem alanı uygulama görünümünün altına sabitlenir ve güncel olmayan önizleme varken kaydetme düğmesi kaldırılır. Başlangıç/orta/son kaydırmada düğmeler görünür; sayfa dışı taşma olmaz.

## Kayıt ve güvenlik davranışı

- Arşiv değerlerini yüklemek yalnızca düzenlenebilir taslağı doldurur. Yükleme, önizleme ve form düzenleme politika sürümü oluşturmaz.
- Sayısal tutarlar JavaScript sayısına çevrilmez. Kaynak ondalıkları metin olarak korunur; arşiv değeri kaybolursa veya biçimi beklenen türde değilse taslak boş kalır.
- `null`/boş değer ile açık `0` ayrı saklanır. Açık değer alanı kapalıysa `null` korunur; sıfır girdiyse alan açık kalır.
- Dönem başlangıç tarihi geçerlilik tarihi olarak önerilir; kullanıcı değiştirebilir. Neden/açıklama varsayılan olarak boş kalır.
- Başlangıç arşivinde kapsam ve yol yuvarlama kararı bulunmadığı için bu seçimler boş başlar; kullanıcı seçmeden kayıt gönderilemez. Kaydedilmiş etkin politika yeniden açıldığında kayıtlı seçimler forma yüklenir. Aynı tarih düzeltmesinde seçilen hedef sürümün ücret durumu ve kapsam/yuvarlama seçimleri birlikte kopyalanır.
- Önizleme kapsamı, doğrudan ücret dönemlerini, bordro bağımlılıklarını, kesinleşmiş engelleri ve geçmiş farkı kayıtlarına etkileri gösterir. Kullanıcıya iç batch/payroll/version kimlikleri veya `STALE`, `settlement`, `FULL_EARNING` gibi kodlar gösterilmez.
- Başlangıç politikasının açık onayı ayrı düğme ve ayrı onay durumu olarak korunmuştur; kaydetme işleminden önce kullanıcı bu onayı ayrıca verir.
- Kayıt sonrası başarı yalnız güncel ücret bilgileri yeniden yüklendiğinde bildirilir. Kayıt yazıldıktan sonra yeniden yükleme başarısız olursa ekran “kayıt tamamlandı, doğrulama yenilenemedi” durumunu gösterir ve yeniden yüklemeyi sunar.

## Bu değişiklikte ele alınanlar

- Controller, etkin politika varsa etkin sürümü; başlangıç sürümü eksikse eksiksiz ve türleri doğrulanmış arşiv taslağını form alanlarına yükler.
- Browser ve native arşiv okuyucuları eksik alanları, JSON sayılarını, hatalı grupları ve `aktif` durumu bulunmayan grupları prefill olarak kabul etmez.
- Eski manuel “arşivden taslak hazırla” adımı kaldırılmıştır.
- Hata metinleri kullanıcı diline çevrilmiş; retro etkiler ve durumlar kimlik/kod göstermeden ifade edilmiştir.
- Etkin politikanın kapsam ve yuvarlama tercihleri yeniden açmada korunur; aynı tarih düzeltmesi seçilen hedef sürümün tam state ve tercihlerini kullanır.
- Ücret politikası ekranı gerçek App viewport çerçevesi, TopBar, Sidebar ve CSS ile sınanır. Dar başlıkta dönem seçicisi taşmaz; yan menü uygulama yüksekliğinde kalır; form kaydırılırken işlem alanı yerinde durur.
- Başlangıç kaydı önizleme öncesi onaylanamaz; güncel taslağın önizlemesi ve ayrı onay tamamlanmadan kaydetme sunulmaz. Tarih veya tutar değişince önceki önizleme ve onay geçersizleşir.

## Doğrulama

Controller ve Chromium testleri başlangıç taslağını, `0`/boş değer ayrımını, etkin sürüm önceliğini, kayıtlı `ROAD_ONLY`/`PERIOD_TOTAL` tercihlerinin yeniden açılmasını, aynı tarih düzeltmesi hedef kopyasını, önizleme/onay sırasını, eksik seçim odağını, blocker davranışını ve gerçek uygulama kabuğunda grup tablosu görünürlüğünü kapsar.

## Doğrulama kaydı

- `CAPTURE_COMPENSATION_POLICY_SCREENSHOTS=1 /Users/kadir/.bun/bin/bun test --timeout=30000 src/components/Settings/CompensationPolicyPage.test.tsx src/hooks/useCompensationPolicyController.test.ts` — **PASS**, 20 test, 298 beklenti.
- `PATH="/Users/kadir/.bun/bin:$PATH" bun run lint` — **PASS** (`tsc --noEmit` ve production graph kontrolü).
- `bun run build` — **PASS** (WASM freshness kontrolü ve Vite production build).
- `git diff --check` — **PASS**.

Her iki görünümde de işlem alanı başlangıç/orta/son kaydırmada sabittir ve belge yüksekliği pencere yüksekliğini aşmaz. Masaüstü 1366×768 görünümünde kaydırma alanı `y=88–663`, işlem alanı `y=663–748` konumundadır. Dar 390×844 görünümünde kaydırma alanı `y=124–727`, işlem alanı `y=727–828` konumundadır. Dört grup tablosu masaüstünde `y=207–544`, dar görünümde `y=260–726.8` aralığında kaldı; her iki görünümde yatay taşma ölçülmedi. Eksik hesaplama seçeneği odaklandığında bölüm açılır ve iki hata mesajı işlem alanının üzerinde görünür.

Chromium kanıtları: [masaüstü başlangıç](/private/tmp/compensation-policy-ux-initial-desktop.png), [dar başlangıç](/private/tmp/compensation-policy-ux-initial-narrow.png), [masaüstü grup tablosu](/private/tmp/compensation-policy-ux-groups-desktop.png), [dar grup tablosu](/private/tmp/compensation-policy-ux-groups-narrow.png), [eksik seçim hatası](/private/tmp/compensation-policy-ux-validation.png), [güncel önizleme ve ayrı onay](/private/tmp/compensation-policy-ux-preview.png), [dar işlem alanı](/private/tmp/compensation-policy-ux-narrow-actions.png).

Build başarıyla tamamlandı. Vite, 1,117.52 kB küçültülmüş ana JavaScript parçasının 500 kB uyarı eşiğini aştığına dair mevcut bundle boyutu uyarısını verdi.
