# 4/D Bordro lisans servisi

Bu klasör ayrı Netlify projesidir. Fonksiyonlar kalıcı site kapsamlı Netlify Blobs kullanır; bordro demosunun `netlify.toml` ayarları değiştirilmez. Preview, test, development ve production kayıtları farklı store adlarına yazılır.

## Şu anki davranış

Masaüstü derlemesinde Rust yapılandırması `BORDRO_LICENSE_MODE` değerini okumadığında mod `optional` olur. Bu mevcut varsayılan ve production derlemesi için geçerlidir: başlangıçta lisans isteği yapılmaz, aktivasyon ekranı açılışı durdurmaz, hesaplama ve kesinleştirme aynı şekilde çalışır. Lisans ayarları yalnızca kullanıcı bu bölümü açarsa yerel durum okur; ağ isteği ancak anahtar gönderme veya “Durumu yenile” eylemiyle yapılır.

`BORDRO_LICENSE_MODE=required` yalnızca native Rust derlemesine açıkça verildiğinde zorunlu modu üretir. Rust her hesaplama ve kesinleştirme komutunun içinde imza, cihaz ve zaman sınırını doğrular. Bu karar React state’ine veya web sayfasındaki bir boolean’a bağlı değildir. Zorunlu mod, API adresi/açık anahtar eksik ya da izin geçersiz olduğunda fail-closed davranır. Görüntüleme, mevcut kayıtlar ve JSON yedeği bu kontrolden etkilenmez.

İlk aktivasyon çevrimiçi başlar ve anahtarı sunucuya gönderir. Sunucu anahtarı SHA-256 özetiyle bulur ve cihaz talebini beklemeye alır. Yönetici panelinde onay sonrası uygulamada “Durumu yenile” seçilir. Lisans/cihaz sınırı güçlü okuma ve ETag koşullu yazımla güncellenir; eşzamanlı son cihaz hakkı yarışında yalnızca bir onay kazanır. Cihaz değiştirmek için panelde eski cihaz hakkını boşa çıkarın, sonra yeni talebi onaylayın.

İmzalı izin, Ed25519 ile `v1.<base64url-payload>.<base64url-signature>` biçiminde üretilir. İmzaya `kid`, lisans kimliği, cihaz özeti, lisans sonu, son başarılı çevrimiçi doğrulama ve çevrimdışı bitiş tarihi dahildir. Rust tarafında yalnızca `src-tauri/license-public-keys.json` içindeki açık anahtarlar bulunur; özel anahtar Netlify ortamında kalır.

Başarılı çevrimiçi doğrulamadan 7 gün sonra uygulama açık kaldığı sürece arka plan kontrolü dener; uygulama kapalıyken servis veya daemon gerekmez. Ağ hatası mevcut imzalı izni silmez ve yeniden deneme aralığına girer. Sunucunun imzalı izni açıkça reddetmesi/iptal etmesi yerel durumu reddedilmiş yapar. Bir izin son başarılı doğrulamadan en fazla 30 gün kullanılabilir; lisansın kendi bitişi daha erkense o tarih geçerlidir. Süre dolunca bordro hesaplama ve kesinleştirme durur; görüntüleme ve yedek alma devam eder.

Cihaz kimliği kullanıcı verisi değildir: uygulama yerel uygulama dizininde rastgele bir UUID saklar, sunucuya yalnızca SHA-256 özeti gönderilir. Dosyayı silme, sistemi kopyalama veya uygulamayı tersine mühendislikle inceleme cihaz eşleşmesini etkileyebilir. Bu mekanizma izinsiz yeniden satışı zorlaştırır; kopyalanmayı veya saat müdahalesini mutlak olarak engellemez. Son görülen sistem saatinin geriye alınması yerelde yakalanır, fakat korumalı donanım saati olmayan çevrimdışı bir bilgisayarda yerel kayıtların eski yedekten döndürülmesi kesin olarak önlenemez. Bağlantı yokken sunucu iptalinin öğrenilmesi de en çok 30 günlük çevrimdışı izin sınırıyla gecikir.

## Netlify projesini kurma

1. Netlify’da **yeni ve ayrı bir site** oluşturun; base directory `license-service`, config file `license-service/netlify.toml` olsun. Üretim bordro demosunun mevcut site ayarlarını bu servis için kullanmayın.
2. `cd license-service && bun install` ve yerelde `bun run dev` çalıştırın. Netlify Dev’in Blobs emülatörü `development` store kullanır. Deploy preview/branch deploy `preview`, production `production`, test kodu `test` store’una ayrılır.
3. Önceki sayfada listelenen ortam değişkenlerini yalnızca bu Netlify projesinin sunucu ortamına ekleyin. `BORDRO_ADMIN_PASSWORD_HASH` için parolayı terminalde gizli alın ve özeti üretin:

   ```sh
   read -s -r admin_password
   printf '%s' "$admin_password" | node scripts/hash-password.mjs
   unset admin_password
   ```

   `BORDRO_ADMIN_SESSION_SECRET` için en az 32 rastgele bayt üretip Netlify ortam değişkenine kaydedin. Bu değer, Blobs token'ı veya parola özeti istemciye/repoya konmaz.
4. Gerçek Ed25519 özel anahtarını güvenli bir yönetim ortamında üretin; özel anahtarı yalnızca Netlify `BORDRO_LICENSE_SIGNING_PRIVATE_KEY` değişkenine verin. Bu görev gerçek anahtar veya yönetici hesabı oluşturmadı.
5. `BORDRO_LICENSE_KEY_ID` ve aynı `kid` değerinin karşılık gelen base64url Ed25519 açık anahtarını masaüstü uygulamasının `src-tauri/license-public-keys.json` dosyasına ekleyin. Açık anahtar baytlarını PEM'den almak için `node license-service/scripts/public-key-from-pem.mjs < public-key.pem` komutunu kullanın. Dosyada özel anahtar bulunmamalıdır.
6. Servisi preview ortamında gerçek Netlify Dev/preview Blobs ile doğrulayın. Preview store production verisini paylaşmaz. Ardından ayrı siteyi deploy edin. Bu depo çalışmasında Netlify hesabına yayınlama yapılmadı.

Üretim lisans zorunluluğunu açmak için lisans servisinin HTTPS URL'sini mevcut native release build ortamında `BORDRO_LICENSE_API_URL=https://<servis-alanı>/api/license` olarak ve `BORDRO_LICENSE_MODE=required` değerini Rust derlemesinden önce ayarlayın. Mevcut release workflow bu değişkenleri ayarlamaz; bugünkü derleme bu nedenle isteğe bağlı modda kalır. Zorunlu moda geçişten önce test kurumuyla aktivasyon/onay, geçerli imza, sunucu iptali ve ağ kesintisini preview ortamında doğrulayın. Tarayıcı bordro demosu bu iş kapsamında lisans denetimine alınmamıştır.

## Yönetim paneli ve API

Panel: `/admin/`. Oturum sunucu tarafından doğrulanan scrypt parola özeti, HMAC oturumu, `HttpOnly; SameSite=Strict` çerez, aynı-kaynak denetimi ve CSRF token ile korunur. Giriş denemeleri IP adresini saklamadan özet bazında sınırlandırılır. Admin parolası veya Blobs erişim token'ı HTML/JavaScript'e gönderilmez. Lisans anahtarı oluşturulduğunda bir kez döner; servis yalnızca özetini saklar.

API:

- `POST /api/license/activate`: anahtar ve cihaz özetiyle manuel cihaz talebi.
- `POST /api/license/activation-status`: talep sonucu ve varsa imzalı izin.
- `POST /api/license/check`: aktif lisans/cihazı yeniden doğrula veya açık red döndür.
- `/api/license/admin/*`: sunucu oturumu ve CSRF gerektiren yönetim işlemleri.

Servis kurum/lisans adı, anahtar özeti, cihaz özeti, durum ve tarihler saklar. Personel listesi, T.C. kimlik numarası, bordro, maaş, puantaj ve yedek dosyaları kabul edilmez veya iletilmez.

## Yedekleme ve anahtar rotasyonu

Panelin **Yedeği indir** eylemi lisans, anahtar indeksi ve aktivasyon kayıtlarını JSON olarak dışa aktarır; SHA-256 checksum içerir ve imzalama anahtarını içermez. Dosyayı erişimi kısıtlı yerde saklayın. **Yedekten geri yükle** aynı içeriği idempotent şekilde ekler; farklı kayıtların üzerine yazmaz. Yükleme yarıda kalırsa aynı dosya yeniden denenebilir. Yedeği production ve preview arasında taşımak veri kapsamını bilinçli olarak değiştirir.

Asgari protokol rotasyonu: yeni Ed25519 key pair üretin; Netlify sunucusuna yeni `BORDRO_LICENSE_KEY_ID` ve private key'i birlikte yükleyin; masaüstü sürümüne eski ve yeni `kid` public key'lerini ekleyin; eski anahtar imzalı izinleri en fazla 30 gün geçerli tutun; 30 gün geçtikten ve eski izinler yenilendikten sonra sonraki uygulama sürümünden eski public key'i kaldırın. Bir private key sızarsa eski imzalı izinleri çevrimdışı cihazlarda anında geri çekmek mümkün değildir; çevrimiçi iptal ve 30 gün sınırı uygulanır.

## Doğrulama kapsamı

Servis testleri, ETag koşullu yazım semantiğini sunucu dışı atomik Blobs emülatörüyle sınar. Netlify Dev'in gerçek yerel Blobs emulatoru ve gerçek site-wide store bu değişiklik kapsamında ayrıca çalıştırılmadı; deploy sonrası preview ortamında sınanmalıdır. Gerçek production hesabı, yönetici parolası, lisans kaydı ve imzalama anahtarı kurulmadı.
