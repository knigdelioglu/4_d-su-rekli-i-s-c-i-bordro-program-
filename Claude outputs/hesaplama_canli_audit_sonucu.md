## Hesaplama canlı audit sonucu (devam raporu – E, F, G, C1–C4, B, O, Q, I, M/N)

### Kapsam
- Yalnızca finansal hesap doğruluğu. UI/UX, export, yedek, banka, SGK kontrol, navigasyon kapsam dışı.
- Yöntem: her vaka için beklenen değerler, üretim kodunu içe aktarmayan ayrı bir Decimal oracle'ında (Python, HALF_UP 0,01) uygulama sonucu görülmeden hesaplandı; aynı girdiler canlı native uygulamaya girildi, alan alan karşılaştırıldı. Kod yalnızca doğrulanmış bir uyuşmazlık adayı çıktıktan sonra, kök neden için okundu (retro.rs, yalnızca okuma). Kod değiştirilmedi.
- Her aileye yeni izole sentetik personel kullanıldı (ZB, ZC, ZÜ). ZB geçmişi: Ağu-2026 → Oca-2027 (Normal, kesinleştirilmiş) + Şub-2027.
- Ayrım: "motor verilen parametrelerle doğru hesaplıyor mu" test edildi. "2027 yasal parametreleri doğru mu" test EDİLMEDİ (uygulamadaki 2027 değerleri 2026 yer tutucularıdır).

### Özet tablo

| Alan | Vaka Sayısı | PASS | FAIL | NOT TESTABLE | Max Fark |
|---|---|---|---|---|---|
| Normal Bordro | 5 | 5 | 0 | 0 | 0,00 TL |
| GV Dilimleri | 13 | 13 | 0 | 0 | 0,00 TL |
| Kümülatif GV | 7 | 7 | 0 | 0 | 0,00 TL |
| GV İstisnası | 5 | 5 | 0 | 0 | 0,00 TL |
| DV | 3 | 3 | 0 | 0 | 0,00 TL |
| PEK Alt Sınır | 5 | 5 | 0 | 0 | 0,00 TL |
| PEK Tavan | 1 | 1 | 0 | 0 | 0,00 TL |
| Payment Events | 7 | 7 | 0 | 0 | 0,00 TL |
| Puantaj/Gün | 8 | 6 | 1 (aday) | 1 | 0,00 (PASS vakalar); B4'te 3.825,00 TL PEK tabanı açıklanamıyor |
| Retro | 9 | 8 | 1 (aday) | 0 | 0,00 (PASS vakalar); FAIL adayında 35.901,00 TL brüt |
| Yuvarlama | 6 | 5 | 0 | 1 | 0,00 TL |

Not: tablo bu devam oturumunda kayda geçen vakaları sayar; önceki oturum raporundaki A/B/C vakaları ayrıca geçerlidir.

### PASS (özet)
- **E – Kümülatif GV (ZB, 6 ay + Şub-2027):** her ay önceki küm. + matrah = yeni küm.; 400k dilim geçişi (Kasım) doğru; Aralık-2026 → vergi yılı sıfırlanması (önceki 0, istisna 4.211,33, GV 7.157,33) doğru. Ay bazlı net: Eyl 70.543,95; Eki 72.186,83; Kas 66.702,93; Ara 75.107,68; Oca-27 74.007,85; Şub-27 66.734,90. Tüm bordrolar yeniden hesaplamada birebir aynı çıktı (determinizm).
- **F – GV istisnası:** GV < istisna → ödenecek 0, negatif yok (kalan 437,20 / 424,58 / 0,01); F2 BSY 10.000: GV 837,80, DV 49,87, net 39.073,18; F3 asgari zincirinin 190k eşiğini geçmesi: GV brüt 6.232,17, istisna 4.865,10, GV 1.367,07.
- **G – DV:** istisna altı (224,67), tam (250,70), üstü (300,57 → 49,87) doğru.
- **C1–C4 – PEK alt sınır ±0,01 (31 günlük dönem):** ham 29.601,00 / 29.700,00 / 33.029,99 / 33.030,00 / 33.030,01. Alt sınır tamamlaması tam 33.030,00'da kapanıyor; 33.029,99'da GV 4.211,32 ve kalan istisna 0,01; 33.030,00'da GV brüt 4.211,33 = istisna.
- **B – Puantaj:** tam dönem, ücretli izin (İ), ücretsiz rapor (R), G/GÇ/GÇT, 28/29/30/31 günlük dönemler; alt sınır bazı tam dönemde 30 gün (33.030).
- **H/I – Payment-event sırası:**
  - Zincir A (ZC): Normal → Tediye → TİS → Ek. Zincir B (ZB): Normal → TİS → Tediye → Ek. Aynı tutarlar (10.000 / 20.000 / 5.000), aynı normal bordro (35.901,00; net 31.460,85).
  - Olay bazlı fark (yasal olarak farklı olması gereken alanlar: istisnanın ilk olaya düşmesi): TİS ilk iken GV 2.112,80 / DV 125,77 / net 14.761,43; Tediye ilk iken GV 837,80 / DV 49,87 / net 7.612,33. Tüm olay alanları (PEK, taşıma, küm. GV, kullanılan GV/DV istisnası, net) oracle ile eşleşti.
  - Eşit olması gereken toplamlar: brüt 35.000, SGK 4.900, işsizlik 350, GV 4.025,30, DV 239,62, toplam net 25.485,08 — iki zincirde birebir eşit.
- **M – Çok yüksek gelir:** 6.000.000 TL ek ödeme, ay içinde kullanılan PEK 64.601 → bildirilen PEK 232.669 (tavan), taşıma 5.767.331, SGK 32.573,66, işsizlik 2.326,69, GV matrahı 5.965.099,65, küm. 6.020.010,50 (%35 ve %40 dilim), GV 1.977.267,57, DV 45.540,00, net 3.942.292,08 — oracle ile birebir. (İlk oracle taslağımda alt sınır tamamlamasını tavan kullanımına saymıştım; bu benim oracle hatamdı, alt sınır tamamlaması toplam PEK zaten alt sınırı aşınca devreye girmez; düzeltilince uygulama ile birebir.)
- **N – Çok düşük gelir (1 Ç + 28 R):** brüt 3.530,50, ham PEK 3.230,50, SGK 452,27, işsizlik 32,31, GV brüt 411,89 (istisna ile 0), DV 24,52 (istisna ile 0), net 3.045,92 — birebir.
- **Q – Retro (ZB, taban 2.443,28 → 2.643,28 / 2.543,28, yürürlük 15.11.2026):** bkz. Retro bölümü.
- **O – Yuvarlama:** bkz. Yuvarlama bölümü.

### Uyuşmazlıklar / bulgular

**B4 – 0 çalışılan gün (29 R): sözleşme-bağımlı, FAIL adayı.** Girdi: tüm dönem R, aylık kalemler (BSY 3.000, giyim 300, hizmet zammı 25, ek ödeme 200, diğer gelir 500). Oracle'a göre aylık kalemler tam ödenir, GV/DV istisna ile 0'a iner. Uygulama: brüt 4.025,00; SGK ve işsizlik 0; GV matrahı 4.025,00; "Sonraki Döneme Devreden PEK" 200,00; net 4.025,00. Ham PEK 4.025,00 iken SGK tabanı 0 + devreden 200 = 200; 3.825,00 TL PEK'in karşılığı yok ve 0 gün ile 1 gün arasında SGK süreksizliği var (1 gün çalışmada SGK 452,27, bkz. N). Tekrarlanabilir. Muhtemel kök neden: sıfır prim günü durumunda SGK tabanının 0'a düşürülmesi. Finansal etki: işçi SGK+işsizlik eksik kesinti 15 % × 3.825 = 573,75 TL ve işveren primi eksikliği. Ürün kararı gerekir (0 gün için prim hiç alınmaz mı?).

**Retro – hesaplanmamış dönem retro'ya giriyor: FAIL adayı.** ZB'nin Oca-2028 puantajı girilmiş ama bordrosu hesaplanmamıştı. Retro önizlemesi bu dönemi "eski tanınmış 0,00" ile listeledi ve Bitiş tarihi 14.02.2027 olmasına rağmen (Bitiş sonrası) 35.901,00 TL brüt fark ürettirdi; batch bu tutar dahil oluşturulabildi (brüt 55.471,00). Aynı dönem sonradan normal bordro olarak hesaplanınca da 35.901,00 brüt üretti: ikinci ödemeyle çift ödeme riski. Muhtemel kök neden: eski tanınmış tutarı bulunmayan açık dönemin hedef hakkı tamamen fark sayılıyor. Finansal etki: 35.901,00 TL brüt (net ≈ 31.460,85). Not: bu davranış tasarım olabilir; ürün sahibi onaylamalı.

**Retro – SGK tahsisi (bilgi, uyuşmazlık değil):** iş primi farkı "NON_WAGE_PAYMENT_MONTH" olarak kaynak dönem PEK/SGK farkından hariç tutulup ödeme ayında (2028-03) PEK'e giriyor. Bu yüzden kaynak ay satırlarında PEK farkı 6.000 (oracle'ın tek bordro hesabı 6.378), işçi SGK 840 (oracle 892,92). Toplam SGK + işsizlik (kaynak + ödeme ayı) oracle ile eşit (batch net kontrolü aşağıda). Kaynak ay SGK bildirimi için PEK dağılımının bu şekilde olması bir raporlama kararıdır.

**Kapsam notları (bulgu değil):**
- Dönem değişikliği/kesinleştirme sonrası sonraki zincirler "STALE/Yeniden Hesaplanmalı" oluyor; hesap yeniden yapılınca değerler birebir aynı çıkıyor. Bu bir durum tutarlılığı konusudur, aritmetik değil. Kesinleştirilmiş bordrosu olan personele personel kartı değişikliği (OKS) kaydedilemedi (beklenen koruma).
- R gününün ücretli sayılması yalnızca SickLeaveRecord ile mümkün; bu özellik bağımsız test EDİLMEDİ (NOT TESTABLE).
- GÇT/G günlerinin yemek-vasıta-iş primi hakkı doğurmaması uygulamadan çıkarılan kuraldır, spesifikasyonla doğrulanmadı.

### Sınır değerleri
- PEK alt sınır: 33.029,99 → tamamlama var, 33.030,00 ve 33.030,01 → yok; tam sınırda GV brüt 4.211,33 = istisna, 33.029,99'da 4.211,32 (kalan istisna 0,01).
- PEK tavan: 6.000.000 TL olayında bildirilen PEK 232.669 (tavan 297.270 − kullanılan 64.601), taşıma 5.767.331.
- GV dilimleri: 400k geçişi (E), 5,3M ve %40 dilimi (M) doğru; F3'te asgari zincirinin 190k eşiği doğru.
- Yıl geçişi (Aralık→Ocak): kümülatif GV ve asgari sıfırlanıyor, istisna yeniden doğuyor.

### Payment-event zinciri
Sözleşme: Normal bordro ay istisnalarını (GV 4.211,33, DV 250,70) önce tüketir; kalan istisna sonraki olaya, o da bitince sonrakine devreder; küm. GV her olayda yeni matrah kadar artar; PEK her olayda ay içi kullanılanı tavandan düşer, aşan kısım taşınır. Zincir sırası olay bazında GV/DV dağılımını değiştirir, toplamlar eşit kalır (yukarıda). Sıra, aynı tarihte oluşturma sırasıyla belirleniyor.

### Retro
Oracle (kaynak girdilerden, üretim sonuçlarına bakmadan):

| Dönem | Eski bordro | Hedef (2.643,28) | Brüt fark | Uygulama |
|---|---|---|---|---|
| 2026-11 | 92.323,55 | 98.701,55 | 6.378,00 | 6.378,00 |
| 2026-12 | 96.065,98 | 102.679,98 | 6.614,00 | 6.614,00 |
| 2027-01 | 94.766,83 | 101.344,83 | 6.578,00 | 6.578,00 |
| 2027-02 | 86.787,41 | 92.747,41 | 5.960,00 | 5.960,00 |

- **Q1 (pozitif):** her dönemde taban farkı (gün × 200) + iş primi farkı (%9 × Ç × 200) = oracle ile birebir; dönem toplamı = toplam fark.
- **Q2 (negatif, taban 2.543,28, Kas–Oca):** −3.189,00 / −3.307,00 / −3.289,00, toplam −9.785,00; uygulama: ödenecek 0,00, açık receivable 9.785,00, mahsup 0,00, SGK PEK farkı −9.200,00 (= −3.000 −3.100 −3.100). İşaretler doğru.
- **Q3 (çok dönemli):** batch (Kas–Oca + hesaplanmamış Oca-2028 satırı) brüt 55.471,00; SGK kaynak (taban PEK 46.300) 6.482,00, işsizlik 463,00; ödeme ayı SGK/işsizlik (iş primi farkı 2.871) 401,94 / 28,71; GV matrahı 48.095,35, GV 7.214,30 − istisna 4.211,33 = 3.002,97; DV 421,02 − 250,70 = 170,32; toplam kesinti 10.548,94; net oracle 44.922,06 = uygulama 44.922,06. (Ödeme ayı kuralı uygulamanın belgelenen sözleşmesinden alındı; net uygulama değerinden önce değil sonra karşılaştırıldı, ancak tüm altı bileşen kuruşu kuruşuna uyuştu.)
- **Mükerrer önleme:** ikinci önizlemede aynı dönemler "önceki retro" 6.378 / 6.614 / 6.578 olarak tanındı, yeni fark 0,00; hedef değişince fark = hedef − eski − önceki retro (Q2'deki negatif sonuç bundan).
- Ödeme olayının bordrosu arayüzde görüntülenemedi; ödeme ayı GV/DV alanları batch net'inden çözüldü.

### Yuvarlama
- Politika: her satır HALF_UP ile 0,01'e yuvarlanıyor, sonraki hesaplar yuvarlanmış değerlerle yapılıyor (erken yuvarlama); GV = r2(tax(önceki+matrah) − tax(önceki)).
- SGK tie: ham 33.030,75 → 33.030,75 × %14 = 4.624,3050 → uygulama 4.624,31 (HALF_UP; HALF_EVEN olsaydı 4.624,30). İşsizlik 330,3075 → 330,31. GV matrahı 28.076,13 (SGK ve işsizlik yuvarlanmış değerleriyle; nihai yuvarlama olsaydı 28.076,14); GV brüt 4.211,42, GV 0,09, net 34.376,04 — oracle ile birebir.
- İşsizlik tie: ham 33.030,50 → 330,3050 → 330,31; SGK 4.624,27; GV 0,06; net 34.375,86 — birebir.
- GV tie: 33.030,00'da GV brüt = 4.211,3250 → 4.211,33 (C3).
- **OKS/BES:** PEK 33.030,50 × %3 = 990,915 → uygulama 990,00 TL. Uygulamanın kendi açıklaması "Kuruş kısmı atılır" (TL'ye aşağı kesme); oracle 2 haneli kesme (990,91) veya yuvarlama (990,92) beklerse uyuşmaz, ancak politika arayüzde belgelenmiş. OKS GV matrahından düşülmüyor (matrah 28.075,92), net 33.385,86 oracle ile uyuşuyor. Sonuç: PASS (belgelenmiş politikaya göre); politikanın ürün tarafından onaylanması önerilir.
- NOT TESTABLE: DV yarım-kuruş tie (tarama aralığı 33.030–33.034,99 içinde DV'nin tie değeri yok; diğer aralıklar çalıştırılmadı).

### Güven düzeyi
Test edilen hesaplama alanlarında bağımsız Decimal oracle ile açıklanamayan finansal fark, yukarıda adı geçen iki FAIL adayı (B4 sıfır gün, retro'ya hesaplanmamış dönemin girmesi) dışında bulunmadı. Bu iki aday ürün sözleşmesi kararına bağlıdır. Sonuçlar yalnızca test edilen vakalar ve kullanılan parametrelerle (2027 değerleri 2026 yer tutucusu) sınırlıdır; "tüm hesaplamalar kesinlikle doğru" iddiası yoktur.
