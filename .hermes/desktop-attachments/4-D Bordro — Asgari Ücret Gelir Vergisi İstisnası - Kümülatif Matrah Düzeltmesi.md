# 4/D Bordro — Asgari Ücret Gelir Vergisi İstisnası / Kümülatif Matrah Düzeltmesi

İlk denetimdeki **5. maddeyi** düzelt: asgari ücret gelir vergisi istisnasını gerçek kümülatif matrah mantığıyla ve GİB uygulamasına uygun şekilde hesapla.

Mevcut SQLite + Rust + Tauri mimarisini koru. Native uygulamada Rust authoritative bordro motoru olmaya devam etsin.

## VS Code context'e ekle

- @src-tauri/src/domain/models.rs
- @src-tauri/src/domain/calculations.rs
- @src-tauri/src/services/payroll_service.rs
- @src-tauri/src/repositories/payroll_repo.rs
- @src-tauri/src/repositories/settings_repo.rs
- @src-tauri/src/db/migrations.rs
- @src-tauri/tests/domain_tests.rs
- @src/types/payroll.ts
- @src/utils/payrollUtils.ts
- @src/components/BordroHesaplama.tsx
- @src/components/PaySlipModal.tsx
- @src/components/PeriodManagerModal.tsx

Kümülatif GV açılışı/devir mantığıyla ilgili mevcut repository/service dosyalarını da kendin bul ve incele.

---

# 1. Önce mevcut hatayı doğrula

Kod değişikliğinden önce gerçek hesap zincirini izle.

Özellikle tespit et:

```text
önceki kümülatif GV matrahı
cari dönem GV matrahı
cari dönem brüt gelir vergisi
asgari ücret GV istisnası
kesilecek gelir vergisi
```

Mevcut motor asgari ücret istisnasını çalışanın:

```text
oncekiKumulatifGvMatrahi
```

değerine göre oranlama/tahmin yoluyla hesaplıyorsa bunu raporla.

Asgari ücret istisnası için çalışanın gerçek kümülatif GV matrahını referans kümülatif matrah olarak kullanma.

---

# 2. İki farklı kümülatif matrahı kesin olarak ayır

### A — Çalışanın gerçek kümülatif GV matrahı

Bu mevcut sistemde zaten bulunuyor:

```text
onceki_kumulatif_gv_matrahi
+
cari_gv_matrahi
```

Bu matrah çalışanın gerçek gelir vergisi dilimini ve cari dönem brüt gelir vergisini belirler.

### B — Asgari ücret istisnasının referans kümülatif matrahı

Bu ayrı bir hesaplamadır.

2026 güncel değerleri:

```text
Brüt asgari ücret = 33.030,00 TL
İşçi SGK = %14
İşçi işsizlik = %1
```

Dolayısıyla aylık referans gelir vergisi matrahı:

```text
33.030
- 4.624,20
-   330,30
= 28.075,50 TL
```

Bu:

```text
asgari_ucret_aylik_gv_matrahi
```

gibi ayrı bir domain kavramı olsun.

**Çalışanın gerçek GV matrahıyla karıştırma.**

---

# 3. Asgari ücret kümülatifi kişi bordrolarından türetilmemeli

Asgari ücret istisnası referansı:

```text
çalışanın işe giriş tarihi
önceki gerçek bordroları
manuel GV açılış matrahı
gerçek önceki kümülatif GV matrahı
```

üzerinden ilerlememeli.

Bu referans **takvim ayına göre asgari ücretin kendi kümülatif matrahıdır.**

Örneğin Ağustos 2026 için:

```text
28.075,50 × 8
```

asgari ücret referans kümülatifi kullanılır.

Çalışan Ağustos ayında işe başlamış olsa bile istisna için Ocak'tan başlayan gerçek personel bordroları üretmeye çalışma.

Bu iki state'i ayır:

```text
personel_kumulatif_gv_matrahi
asgari_ucret_referans_kumulatif_matrahi
```

---

# 4. 15–14 bordro dönemini istisna hesabında günlük bölme

Program puantajda 15–14 dönem kullanıyor olabilir.

Asgari ücret GV istisnası:

```text
15 Nisan–14 Mayıs günleri
```

gibi günlük parçalanarak iki aya bölünmemeli.

İstisna, **ücretin ödendiği/bordronun ait olduğu ödeme ayındaki** asgari ücret istisnasıdır.

Mevcut `PayrollPeriod` içinde ödeme ayını temsil eden authoritative yıl/ay alanını belirle ve onu kullan.

Yeni belirsiz tarih varsayımı üretme.

Gerekirse domain seviyesinde açık:

```text
tax_year
tax_month
```

kavramlarını kullan.

---

# 5. Çalışanın cari gelir vergisi

Önce çalışanın gerçek cari dönem vergisini istisna öncesi hesapla.

Temel yöntem:

```text
önceki_vergi
=
tax(onceki_kumulatif_gv_matrahi)

yeni_kumulatif_vergi
=
tax(
    onceki_kumulatif_gv_matrahi
    + cari_gv_matrahi
)

cari_brut_gelir_vergisi
=
yeni_kumulatif_vergi
- önceki_vergi
```

Buradaki `tax(...)` mevcut ücret gelir vergisi tarifesi fonksiyonunu kullansın.

Çalışanın matrahından asgari ücret matrahını çıkarıp kalan tutara doğrudan oran uygulama.

---

# 6. 2026 ücret tarifesi

Mevcut parametre sistemi varsa onu kullan.

2026 ücret gelirleri için güncel tarife:

```text
190.000 TL'ye kadar                         %15

400.000 TL'nin 190.000 TL'si için
28.500 TL, fazlası                          %20

Ücret gelirlerinde 1.500.000 TL'nin
400.000 TL'si için 70.500 TL, fazlası       %27

Ücret gelirlerinde 5.300.000 TL'nin
1.500.000 TL'si için 367.500 TL, fazlası    %35

5.300.000 TL üzeri                          %40
```

Tarifeyi calculation içinde dağınık magic number olarak çoğaltma.

Mevcut tax-bracket/domain modelini kullan.

---

# 7. Asgari ücretin ilgili aya ait vergisini ayrı hesapla

Ödeme ayı `m` ise:

```text
asgari_onceki_kumulatif
=
aylik_asgari_gv_matrahi × (m - 1)

asgari_cari_kumulatif
=
aylik_asgari_gv_matrahi × m
```

Aynı ücret tarifesini bunlara uygula:

```text
asgari_ucret_aylik_vergi_istisnasi
=
tax(asgari_cari_kumulatif)
-
tax(asgari_onceki_kumulatif)
```

Böylece vergi dilimi değiştikçe istisna da otomatik değişir.

Aylık istisnayı `%15` sabit oranla bütün yıl hesaplama.

---

# 8. 2026 regression referansı

Mevcut güncel parametrelerle hesap motorunun aşağıdaki sonuçları üretmesini doğrula:

```text
Ocak       4.211,33 TL
Şubat      4.211,33 TL
Mart       4.211,33 TL
Nisan      4.211,33 TL
Mayıs      4.211,33 TL
Haziran    4.211,33 TL
Temmuz     4.537,75 TL
Ağustos    5.615,10 TL
Eylül      5.615,10 TL
Ekim       5.615,10 TL
Kasım      5.615,10 TL
Aralık     5.615,10 TL
```

Ancak bu tabloyu authoritative hardcoded lookup olarak kullanma.

Motor bu değerleri:

```text
asgari ücret
+ SGK/işsizlik işçi oranları
+ 2026 gelir vergisi tarifesi
+ ödeme ayı
```

üzerinden türetsin.

---

# 9. Uygulanacak istisna vergiyle sınırlı

GİB kuralına göre istisna nedeniyle sağlanan menfaat:

```text
asgari ücretin ilgili ayda hesaplanan vergisini
```

aşamaz.

Aynı zamanda kişinin o ay hesaplanan gelir vergisinden daha fazla istisna uygulanamaz.

Bu nedenle:

```text
uygulanacak_gv_istisnasi
=
min(
    cari_brut_gelir_vergisi,
    asgari_ucret_aylik_vergi_istisnasi
)
```

ve:

```text
kesilecek_gelir_vergisi
=
max(
    0,
    cari_brut_gelir_vergisi
    - uygulanacak_gv_istisnasi
)
```

olmalı.

Negatif GV üretme.

---

# 10. Asgari ücret istisnası MATRAH indirimi değildir

Şu yanlış modele izin verme:

```text
vergiye_tabi_matrah
=
cari_gv_matrahi
- 28.075,50
```

ve ardından kalan tutara oran uygulama.

İstisna:

```text
matrahtan düşülen sabit tutar
```

değil;

```text
hesaplanan gelir vergisinden düşülen,
asgari ücrete isabet eden vergi tutarıdır.
```

Vergi dilimi tayininde istisna kapsamındaki tutar da dikkate alınır.

---

# 11. Gerçek kümülatif GV açılışını koru

Daha önce çözülen:

```text
personnel_tax_opening
```

ve gerçek bordrolardan otomatik kümülatif GV taşıma mantığına dokunma.

Örneğin:

```text
2026 açılış/devir = 120.000
Mayıs cari GV matrahı = 65.000
```

ise kişinin gerçek vergi hesabında:

```text
önceki kümülatif = 120.000
cari sonrası = 185.000
```

kullanılmalı.

Ancak Mayıs asgari ücret istisnasını:

```text
120.000
```

üzerinden oranlamaya çalışma.

Mayıs'ın asgari ücret referansı kendi takvim ayı hesabından gelmeli.

Bu, görevin ana regression noktasıdır.

---

# 12. Aynı ayda birden fazla ücret kalemi

Maaşla birlikte aynı bordro döneminde:

```text
mesai
iş primi
gece primi
sosyal yardım
manuel ücret niteliğindeki gelir
ikramiye/tediye tutarı
```

varsa bunlar mevcut GV matrah kurallarına göre toplam ücret matrahına dahil edilir.

Asgari ücret GV istisnası aynı ödeme ayındaki ücret toplamına **bir kez** uygulanır.

Her gelir kalemine ayrı ayrı istisna verme.

---

# 13. Bordro sonucu / snapshot

Bordroda denetlenebilir şekilde en az şu alanları ayır:

```text
oncekiKumulatifGvMatrahi
cariGvMatrahi
yeniKumulatifGvMatrahi

brutGelirVergisi
asgariUcretGvMatrahi
asgariUcretReferansKumulatifMatrahi
asgariUcretGvIstisnasi
uygulananGvIstisnasi

kesilenGelirVergisi
```

Naming'i mevcut conventions'a göre uyarlayabilirsin.

`FINALIZED` bordro yeniden açıldığında kullanılan:

```text
kümülatif matrah
istisna
brüt vergi
kesilen vergi
```

değerleri değişmemeli.

Mevcut snapshot JSON yaklaşımını kullan.

---

# 14. UI

PaySlip/Bordro detayında gereksiz kalabalık oluşturmadan denetlenebilir şekilde göster:

```text
GV Matrahı
Önceki Kümülatif GV Matrahı
Yeni Kümülatif GV Matrahı
Hesaplanan GV (İstisna Öncesi)
Asgari Ücret GV İstisnası
Kesilen Gelir Vergisi
```

Asgari ücret istisnasının kişinin gerçek kümülatif matrahıyla aynı şeymiş gibi gösterilmemesine dikkat et.

---

# 15. Regression testleri

En az aşağıdaki testleri ekle.

### Test A — Ocak 2026

```text
Asgari ücret = 33.030
SGK %14
İşsizlik %1
```

Beklenen:

```text
Asgari ücret GV matrahı = 28.075,50
Ocak GV istisnası = 4.211,33
```

### Test B — Temmuz dilim geçişi

Beklenen:

```text
Temmuz GV istisnası = 4.537,75
```

Bu test istisnanın bütün yıl `%15` sabit olmadığını kanıtlasın.

### Test C — Ağustos

Beklenen:

```text
Ağustos GV istisnası = 5.615,10
```

### Test D — Personelin gerçek kümülatifi istisnayı değiştirmiyor

İki personel:

```text
Personel A önceki gerçek kümülatif = 0
Personel B önceki gerçek kümülatif = 300.000
```

aynı ödeme ayında bordrolansın.

Çalışanların:

```text
brüt gelir vergileri
```

farklı olabilir.

Ancak o aya ait:

```text
asgari_ucret_aylik_vergi_istisnasi
```

aynı olmalı.

### Test E — Açılış/devir regression

```text
Önceki gerçek kümülatif GV = 120.000
Cari GV matrahı = 65.000
```

Beklenen gerçek yeni kümülatif:

```text
185.000
```

olsun.

Fakat asgari ücret istisnası ödeme ayının kendi referansından gelsin; `120.000` üzerinden türetilmesin.

### Test F — İstisna brüt vergiyi aşamaz

Cari hesaplanan GV:

```text
2.000 TL
```

ve aylık asgari ücret istisna hakkı:

```text
4.211,33 TL
```

ise:

```text
uygulanan istisna = 2.000
kesilecek GV = 0
```

olmalı.

### Test G — Aynı ay çoklu gelir kalemi

Maaş + iş primi + gece primi + manuel ücret geliri aynı bordroda olsun.

GV matrahları mevcut kurallara göre birleşsin.

Asgari ücret istisnası yalnız **bir kez** uygulansın.

### Test H — FINALIZED snapshot

Bordro FINALIZED yapıldıktan sonra:

```text
asgari ücret
vergi tarifesi
gerçek GV açılışı
```

değiştirilse bile eski bordronun istisna ve kesilen GV değerleri değişmemeli.

---

# 16. Damga vergisini bu görevle karıştırma

Asgari ücret damga vergisi istisnası ayrı konudur.

Mevcut doğru damga vergisi davranışını bozma.

Bu görev esas olarak:

```text
Gelir Vergisi
GV kümülatifi
Asgari Ücret GV İstisnası
```

üzerindedir.

Damga vergisinde açık bir hata bulursan bu görevde genişletme; yalnız raporla.

---

# 17. Yuvarlama

Daha önce SGK için oluşturulan:

```text
round_sgk_amount
```

GV hesabında kullanılmamalı.

Mevcut gelir vergisi rounding policy'sini incele ve koru; sırf SGK'nın midpoint policy'sini GV'ye taşıma.

2026 regression değerleriyle uyuşmuyorsa önce nedeni tespit et.

---

# Kapsam dışı

Dokunma:

- GÇ/GÇT
- rapor
- PEK alt sınırı
- SGK prim yuvarlaması
- %21,75 / %2 işveren primleri
- 300 TL SGK yemek istisnası
- iş primi grup/oran mantığı
- iş primi yuvarlaması
- devreden PEK
- Tediye/TİS otomatik hesaplama

Git commit oluşturma.

Çalışma sonunda kısa rapor ver:

1. Eski asgari ücret GV istisnası nasıl hesaplanıyordu?
2. Çalışanın gerçek kümülatifi ile asgari ücret referans kümülatifi nasıl ayrıldı?
3. 2026 aylık referans GV matrahı kaç?
4. Ocak, Temmuz ve Ağustos istisnaları kaç çıktı?
5. Gerçek GV açılış/devir sistemi değişti mi?
6. İstisna aynı ayda yalnız bir kez mi uygulanıyor?
7. Hangi alanlar snapshot/persistence'a eklendi?
8. Regression testlerinin sonucu ne?