# Sentetik Bordro UX ve Motor Stres Testi

## Senaryo

- Hedef: 18 ardışık 15–14 bordro dönemi, Eylül 2026–Şubat 2028; uygulamadaki 16 sentetik işçi. Tüm işlemler yalnız arayüzden yürütüldü.
- Sentetik zam planı: Mart 2027 +%15, Eylül 2027 +%10, Mart 2028 +%12. Mart/Eylül zam ayları ücret takvimine kaydedildi; hedef dönem değerlerine zamlar uygulanmadı.
- Yol artışı planı: 1 Aralık 2026 +%10, 1 Haziran 2027 +%12, 1 Aralık 2027 +%8.
- Tediye planı: 2026’da 1 ödeme; 2027’de 2 ödeme. TİS planı: 2026’da 2 ödeme; 2027’de 2 ödeme. Tutarlar henüz tahakkuk ettirilmedi.
- Geriye dönük ödeme planı: Aralık 2027 bordrosunda önceki 2 aylık fark.

## UX bulguları

1. Puantaj hızlı girişinden sonra Eylül 2026’da 16 çalışanın her birinde 22 Ç ve 8 T günü görüldü.
2. Tediye/TİS takvimi referans amaçlı; ödeme tutarlarının kişi ve dönem bazında bordro ekranında ayrıca girilmesi gerekiyor.
3. TİS takviminde üçüncü satır eklenip ad/ay değiştirildi. “Tediye & TİS Referanslarını Kaydet” işleminden sonra yalnızca iki TİS satırı görünür kaldı; üçüncü satır kalıcı olmadı.
4. Üç gelecek tarihli yol artışı Eylül 2026 ücret ekranına girilebildi, ancak kaydetme sırasında “Value must be less than or equal to 2026-10-14” uyarısı çıktı. Tarih kontrolü kaydetme aşamasında çalışıyor. Başarısız satırlar silindi.

## Eylül 2026 motor denemesi

- İlk 5.000 TL brüt TİS denemesinde motor 2026 asgari ücret GV referans zincirinin eksik olduğunu ve 01 vergi ayı için dönem/devir başlangıcı gerektiğini bildirdi; tahakkuk oluşmadı.
- Zinciri ilerletmek için Aralık 2025–Ağustos 2026 dönemlerinde 16 personel için önceki normal ve asgari GV tutarları sıfır kaydedildi. Şubat–Haziran 2026 ara dönemleri arayüzden oluşturuldu; gelir parametreleri sıfır kaldı ve bu dönemlerde maaş hesaplanmadı.
- Aralık 2025–Eylül 2026 dönemlerinin yasal oranları, Eylül ücret parametreleri ve 2026 yıllık GV tarifesi arayüzden yeniden kaydedildi; başarı bildirimleri alındı.
- Eylül kontrol ekranı ücret/yardım parametrelerini, yasal oranları, yıllık GV tarifesini ve puantajı hazır/tamam gösterdi. Buna rağmen normal maaşların toplu hesaplaması 0/16 oldu; 16 personelin her birinde “Dönem yasal parametreleri henüz tamamlanmamış. Dönem Parametreleri bölümünü tamamlayın.” hatası görüldü.
- Ücret, yasal oran ve yıllık GV kayıtları tekrar kaydedilip yeniden hesaplandı; hata değişmedi. İlk çalışanda tekil “Hesapla” da aynı hatayı verdi.
- Sentetik01 için 5.000 TL brüt TİS tahakkuku yeniden denendi. Aynı hata oluştu; kayıt oluşmadı (0 tahakkuk).

## Dönem sonuçları

| Bordro dönemi | Sonuç | Kontroller / bulgular |
|---|---|---|
| Eylül 2026 (15.09–14.10) | Normal maaş 0/16; TİS 0 kayıt | 16 normal bordro motor hatası; dönem kontrolleri hazır gösterirken motor yasal parametreleri eksik bildiriyor. |

## Durum ve kapsam sınırı

- İlk hedef dönem Eylül 2026’da maaş ve TİS motorları hesaplama yapmadı. Dönem parametreleri ve vergi referansları arayüzden kaydedildikten sonra hata devam etti.
- Kalan 17 hedef dönem hesaplanmadı. Zam değerleri, yol artışları, tediye/TİS tahakkukları ve Aralık 2027 geriye dönük ödeme uygulanmadı.
- Ocak ve Mayıs 2027 karşılaştırma thread’leri açılmadı; bu aylara ait paylaşılabilir bordro çıktısı oluşmadı. Thread raporu bulunmuyor.
- 18 aylık senaryo tamamlanmadı; ilk dönem motor/ayar tutarsızlığı sonraki bordrolara geçişi engelledi.
