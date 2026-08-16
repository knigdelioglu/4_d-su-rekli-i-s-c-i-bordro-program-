# Bordro Hesaplama Motoru Sağlamlaştırma Planı

**Durum:** Planlandı  
**Hedef dal:** `main`  
**Kapsam:** 4/D bordro hesaplama motoru, bordro ledger bütünlüğü ve kritik backend invariantları  
**Temel ürün kararı:** Uygulama geçmiş dönem bordrolarını yeniden üretmeyecek ve eski mevzuatı tarihsel bir kural arşivi olarak modellemeyecek.

### Uygulama durumu — 16 Ağustos 2026

- ✅ Faz 1 — Devreden PEK işçi prim matrahı tamamlandı.
- ✅ Faz 2 — Puantaj tarih/dönem invariantı tamamlandı.
- ✅ Faz 3 — Kümülatif GV / STALE / FINALIZED zinciri tamamlandı.
- ✅ Faz 4 — Örtüşen rapor kayıtları write path'te fail-closed reddediliyor; adjacency ayrı episode olarak kalıyor.
- ✅ Faz 5 — Period-local effective-date segmentleri, bağımsız SGK/GV yemek istisnaları ve resolved statutory snapshot tamamlandı.
- ⏳ Faz 6–11 bu planın kalan işleri olarak devam ediyor.

---

## 1. Amaç

Bu planın amacı mevcut bordro motorunun temel matematiğini yeniden yazmak değil; hesaplama zincirinde tespit edilen para hatalarını, veri bütünlüğü açıklarını ve dönemler arası bağımlılık sorunlarını fail-closed hale getirmektir.

Öncelik sırası:

1. Doğrudan yanlış para üreten hesap hatalarını kapatmak.
2. Bordro motoruna giren puantaj/veri setini güvenilir hale getirmek.
3. Kümülatif vergi ve devreden PEK zincirini deterministik yapmak.
4. Rapor, vergi ayı, parametre ve persistence invariantlarını backend seviyesinde zorunlu kılmak.
5. Gelecekte aktif bir 15–14 döneminin ortasında yasal parametre değişirse yalnız ilgili açık/gelecek dönemi doğru parçalayabilmek.

---

## 2. Kesin kapsam kararı

### 2.1 Kapsam dışı

Aşağıdakiler **yapılmayacak**:

- Geçmiş bordro dönemleri için yeniden hesaplama motoru.
- Eski yılların mevzuatını tarihsel olarak saklayan genel bir rules engine.
- 2025, erken 2026 veya başka kapanmış dönemlere ait eski oranları sırf bordro yeniden üretmek için backfill etme.
- Kapanmış bordroları güncel kurallarla yeniden üretme.
- Geriye dönük fark bordrosu / ek bordro / mahsup bordrosu altyapısı.
- Geçmiş FINALIZED bordroları yeni parametrelerle yeniden hesaplama.
- Tediye ve TİS ikramiyesini otomatik formülle üretme kapsamını genişletme.

### 2.2 Kapsam içinde

Aşağıdakiler **desteklenecek**:

- Mevcut ve gelecek bordro dönemleri.
- Açık veya gelecekteki bir 15–14 dönemi içinde yeni bir yasal parametre yürürlüğe girerse, yalnız o dönem için gerekli eski/yeni parçalama.
- FINALIZED bordroda kullanılan tüm hesap parametrelerinin snapshot olarak korunması.
- Cari yıldaki kümülatif GV ve devreden PEK zincirinin güvenli devamı.
- Backend seviyesinde tarih, matrah, oran ve ledger invariantları.

### 2.3 Effective-date yaklaşımının sınırı

Bu proje genel amaçlı tarihsel mevzuat motoru kurmayacak.

Gerekli davranış şudur:

> Açık veya gelecekteki bir 15–14 bordro dönemi, yürürlük tarihi o dönemin içine düşen yeni bir yasal parametre değişikliğini doğru hesaplayabilmelidir.

Örnek olarak gelecekte bir asgari ücret, PEK sınırı veya SGK yemek istisnası değişikliği 1 Ocak tarihinde yürürlüğe girerse `15.12–14.01` dönemi gerektiğinde iki segmente ayrılır. Kapanmış geçmiş dönemleri desteklemek için eski mevzuat kataloğu tutulmaz.

---

## 3. Yeniden önceliklendirilmiş bulgular

| Öncelik | Bulgu | Karar |
|---|---|---|
| P0 | Devreden PEK işçi SGK/işsizlik matrahına girmiyor | Doğrudan hesap hatası; ilk düzeltme |
| P0 | Puantaj tarih/dönem invariantı eksik | Motor girdisi güvenilir değil; ikinci düzeltme |
| P0/P1 | Kümülatif GV stale dependency | Bordro ledger zinciri deterministik değil |
| P1-high | Örtüşen rapor kayıtları ayrı episode sayılıyor | Overlap reject; otomatik merge yok |
| P1-high | Aktif/gelecek dönem içi yasal parametre değişimi | Tarihsel arşiv yok; yalnız period-local segment |
| P1 | Aynı `taxYear/taxMonth` birden fazla normal dönem alabiliyor | Unique invariant |
| P1 | GV indirim kalemleri eksik modellenmiş | Uygunluk/limit kontrollü model |
| P1 | PEK tavan katsayısı ve parasal parametre validasyonları zayıf | Fail-closed validation |
| P1 | Personel OKS özel oranı doğrulanmıyor | Üyelik/oran invariantı |
| P1 | Decimal → kuruş dönüşümünde overflow sessizce `0` olabiliyor | Sessiz fallback kaldırılacak |
| P1/P2 | Negatif net ödeme kaydedilebiliyor | Normal bordro persistence engellenecek |
| P2 | Çoklu aktif Tediye/TİS `.find()` ile sessiz seçiliyor | Otomatik hesap kaldırılıp manuel kapsam korunacak |

---

# 4. Uygulama planı

## Faz 0 — Domain sözleşmesini sabitle

### Amaç

Kod değişikliğine başlamadan bordro motorunun değişmez kurallarını test edilebilir invariantlara dönüştürmek.

### Eklenecek invariantlar

1. `finalPek` hiçbir koşulda `pekUstSinir` değerini aşamaz.
2. PEK'e cari ayda dahil edilen devreden tutar, işçi ve işveren prim matrahlarında aynı PEK tabanının parçasıdır.
3. Bir puantaj tarihi yalnız kendi bordro döneminin `[baslangicTarihi, bitisTarihi]` aralığında olabilir.
4. Aynı takvim günü tek puantaj kodu taşıyabilir.
5. Puantaj kayıt sayısı dönemin gerçek takvim günü sayısını aşamaz.
6. Kurumca ücretli rapor günü puantajda `R` olmak zorundadır.
7. Aynı personele ait rapor kayıtları tarih olarak örtüşemez.
8. Normal bordro dönemlerinde `(taxYear, taxMonth)` tekildir.
9. `STALE` bordro kümülatif zincirin authoritative girdisi olamaz.
10. FINALIZED bordro değiştirilemez ve kendinden önceki mutable vergi zincirine dayanamaz.
11. Parasal persistence dönüşümünde overflow/parse sorunu `0 TL` üretemez.
12. Negatif net normal bir bordro olarak sessizce kaydedilemez.

### Çıktı

- Domain testlerinde ortak assertion/helper fonksiyonları.
- Kod yorumları yerine testle korunan invariant seti.

---

## Faz 1 — P0: Devreden PEK işçi prim matrahını düzelt

### Mevcut sorun

`calculate_prime_esas_kazanc()` gelen devreden PEK'i `pek_matrah_adayi/finalPek` içine ekliyor. Buna karşılık işçi SGK ve işsizlik matrahı `hesaplananPek.min(pekUstSinir)` üzerinden kuruluyor. `hesaplananPek` yalnız cari ayın `ham_pek` değerini taşıdığı için cari ay PEK'ine gerçekten dahil edilen devreden tutar işçi prim matrahından düşüyor.

### Hedef tasarım

PEK sonucunda birbirinden açıkça ayrılmış alanlar bulunmalı:

- `hamPek`: yalnız cari dönem kazançları.
- `devredenPekKullanilan`: bu ay tavana sığdırılıp PEK'e eklenen geçmiş ücret dışı ödeme.
- `primMatrahi`: cari ayda işçi ve işveren prim hesabının authoritative PEK matrahı.
- `finalPek`: gerekiyorsa alt/üst sınır sonucunu gösteren nihai PEK snapshot'ı.

İşçi SGK ve işsizlik primi `primMatrahi` üzerinden hesaplanmalı. Alt sınır tamamlama nedeniyle yalnız işverene ait olan fark tekrar işçi matrahına sokulmamalı.

### Dokunulacak ana dosyalar

- `src-tauri/src/domain/calculations.rs`
- `src-tauri/src/domain/models.rs`
- `src-tauri/tests/domain_tests.rs`
- Devreden PEK regression testleri
- Gerekirse `src/types/payroll.ts` yalnız snapshot alanları UI'ya taşınacaksa

### Zorunlu testler

1. Devreden PEK yok — mevcut sonuç değişmemeli.
2. Gelen devreden PEK tamamen tavana sığıyor.
3. Gelen devreden PEK kısmen tavana sığıyor.
4. Gelen devreden PEK için tavan boşluğu yok.
5. Birden fazla devreden kayıt var.
6. İki aylık ömür korunuyor.
7. Yıl geçişi davranışı korunuyor.
8. Alt sınır tamamlama ile devreden PEK aynı senaryoda.
9. İşçi ve işveren matrahları arasındaki fark yalnız mevzuat gereği alt sınır işveren farkından kaynaklanabiliyor; devreden PEK'ten kaynaklanamıyor.

### Kabul kriteri

Cari ay PEK'ine fiilen eklenen her devreden tutar işçi %14 ve işsizlik %1 hesabında da yer almalı; tavan dışı kalan tutar primlendirilmemeli.

---

## Faz 2 — P0: Puantaj tarih/dönem invariantını backend'e taşı

### Mevcut sorun

`AttendanceRepository` kod değerlerini doğruluyor fakat tarihlerin:

- parse edilebilir olduğunu,
- ilgili dönemin içinde olduğunu,
- dönemin gerçek gün sayısını aşmadığını

backend seviyesinde garanti etmiyor.

### Hedef tasarım

Yeni authoritative doğrulama:

`validate_attendance_for_period(attendance, period)`

Şunları kontrol etmeli:

1. Her key `%Y-%m-%d` formatında gerçek tarih olmalı.
2. Tarih `period.baslangicTarihi <= date <= period.bitisTarihi` olmalı.
3. Kod `Ç/T/G/İ/GÇ/GÇT/R` kümesinden olmalı.
4. HashMap anahtarı nedeniyle aynı tarih iki kod taşıyamaz; import/migration yollarında da bu invariant korunmalı.
5. Toplam kayıt sayısı dönem içindeki takvim günü sayısını aşamaz.
6. `PersonelPuantaj.donemId` yüklenen `BordroDonemi.id` ile eşleşmeli.
7. PayrollService hesaplamadan hemen önce validation'ı tekrar çalıştırmalı.

### Mimari karar

Sadece UI validation yeterli değildir. Doğrulama service/domain seviyesinde uygulanacak; repository/import yolları da aynı doğrulamayı kullanacaktır.

### Zorunlu testler

- Dönemden bir gün önce `Ç` → reject.
- Dönemden bir gün sonra `Ç` → reject.
- Geçersiz tarih `2026-02-30` → reject.
- 31 günlük çalışma döneminde 31 farklı geçerli tarih → kabul; SGK prim günü ayrıca 30 tavanına göre davranır.
- Dönemin dışına eklenen 32. tarih → reject.
- Bilinmeyen kod → reject.
- Normal 15–14 puantaj → mevcut sonuç değişmez.

### Kabul kriteri

Motor hiçbir koşulda dönem dışı tarihten taban ücret, yemek, yol veya iş primi üretememeli.

---

## Faz 3 — P0/P1: Kümülatif GV dependency ve FINALIZED zincirini sağlamlaştır

### Mevcut sorun

Önceki `CALCULATED` bordro değiştiğinde daha sonraki bordroların kümülatif GV snapshot'ları otomatik olarak geçersiz hale gelmiyor.

### Hedef durum modeli

`BordroStatus` genişletilecek:

- `DRAFT`
- `CALCULATED`
- `STALE`
- `FINALIZED`

### Kurallar

1. Önceki bir `CALCULATED` bordro yeniden hesaplanır ve GV/PEK/net sonuçları değişirse aynı personelin daha sonraki ilgili `CALCULATED` bordroları `STALE` yapılır.
2. `STALE` bordro export/finalize edilemez; yeniden hesaplanmalıdır.
3. Bir bordro `FINALIZED` yapılmadan önce aynı personelin mevcut önceki vergi zincirindeki bordroları `FINALIZED` olmalıdır.
4. Daha sonraki bir `FINALIZED` bordro varsa, onun dayandığı önceki bordronun yeniden hesaplanmasına izin verilmez.
5. Kümülatif sorgular `STALE` ve `DRAFT` kayıtları authoritative toplamdan dışlamalıdır.
6. Preview hesap akışı için önceki `CALCULATED` bordrolar kullanılabilir; fakat değişiklik downstream invalidation üretir.
7. FINALIZED snapshot'lar immutable kalır.

### Dokunulacak ana dosyalar

- `src-tauri/src/domain/models.rs`
- `src-tauri/src/services/payroll_service.rs`
- `src-tauri/src/services/cumulative_tax_service.rs`
- `src-tauri/src/repositories/payroll_repo.rs`
- status kullanan frontend bileşenleri

### Zorunlu testler

1. Mayıs CALCULATED → Haziran CALCULATED → Mayıs değişti → Haziran STALE.
2. STALE Haziran finalize edilemez.
3. Haziran yeniden hesaplanınca CALCULATED olur ve yeni kümülatifi kullanır.
4. Mayıs FINALIZED + Haziran CALCULATED → normal.
5. Haziran FINALIZED iken Mayıs yeniden hesaplanamaz.
6. DRAFT/STALE gv_base kümülatif sorguya girmez.
7. Vergi yılı değişiminde önceki yıl zinciri otomatik taşınmaz.

### Kabul kriteri

Bir bordronun önceki vergi matrahı değiştiğinde daha sonraki bordrolar eski snapshot ile sessizce kullanılmaya devam edememeli.

---

## Faz 4 — P1-high: Örtüşen raporları fail-closed reddet

### Mevcut sorun

Tam aynı `start/end` kayıtları service tarafında `dedup()` ediliyor; fakat `01–03` ve `02–04` gibi örtüşen raporlar ayrı episode sayılabiliyor.

### Ürün kararı

**Otomatik merge yapılmayacak.**

- Overlap → `ValidationError`
- Adjacency (`01–03`, `04–06`) → otomatik olarak aynı olay kabul edilmeyecek.
- Gelecekte gerçek devam raporu ihtiyacı doğarsa `episodeId` / `continuationOf` gibi explicit model tasarlanır.

### Hedef doğrulama

SickLeave save/update sırasında aynı personelin diğer kayıtlarıyla tarih aralığı çakışması kontrol edilir.

Update sırasında kaydın kendi `id` değeri karşılaştırmadan çıkarılır.

Service katmanındaki exact `dedup()` savunması legacy/bozuk DB verisine karşı kalabilir, fakat normal write path overlap üretememelidir.

### Zorunlu testler

- `01–03` + `02–04` → reject.
- `01–03` + `03–05` → reject.
- `01–03` + `04–06` → kabul.
- Exact duplicate farklı id → reject.
- Aynı record update edilip tarih daraltılıyor → kabul.
- 5 episode + 6. episode davranışı korunuyor.
- Dönem/yıl sınırını aşan tek rapor episode davranışı korunuyor.

---

## Faz 5 — P1-high: Yalnız aktif/gelecek dönem için effective-date segment desteği

### Kapsam sınırı

Bu faz **geçmiş mevzuat arşivi değildir**.

Amaç yalnız uygulama kullanımdayken açık/gelecek bir 15–14 döneminin içine yeni bir yasal yürürlük tarihi düşerse doğru hesaplama yapmaktır.

### Tasarım

Genel global tarihsel rules engine yerine **period-local statutory segment** kullanılacak.

Önerilen kavram:

`StatutoryParameterSegment`

Alanlar:

- `effectiveFrom`
- ilgili yasal parametre override'ları

İlk sürümde segmentlenmesi gereken parametreler:

- `gunlukAsgariUcret`
- `pekTavanKatsayisi`
- `gunlukYemekIstisnasiSGK`
- gerekirse işçi/işveren SGK ve işsizlik oranları
- `gunlukYemekIstisnasiGV` ayrı alan olarak

### Kritik model düzeltmesi

`gunlukYemekIstisnasiSGK` gelir vergisi yemek istisnası için tekrar kullanılmayacak.

Ayrı alanlar:

- `gunlukYemekIstisnasiSGK`
- `gunlukYemekIstisnasiGV`

### Segment davranışı

1. Segment yalnız ilgili açık/gelecek döneme uygulanır.
2. Segment tarihi dönem dışında ise reddedilir.
3. Segmentler artan tarihte ve çakışmasız olmalıdır.
4. Hesap her takvim gününü doğru segmentin parametresiyle değerlendirir.
5. Bir sonraki dönem için yeni değer normal dönem baseline ayarı olur; geçmiş katalog tutulmaz.
6. FINALIZED bordro resolved parameter snapshot'ını saklar.

### Zorunlu testler

- Dönem içinde tek değişim günü.
- Değişim tarihinin dönemin ilk günü olması.
- Değişim tarihinin dönemin son günü olması.
- Segment yok → mevcut tek parametre davranışı değişmez.
- SGK yemek istisnası değişirken GV yemek istisnası değişmiyor.
- Günlük asgari ücret değişiminde PEK alt/üst sınırı iki parçadan doğru oluşuyor.
- Segment tarihi dönem dışında → reject.

### Kabul kriteri

Yeni bir yasal değişiklik açık/gelecek 15–14 döneminin ortasında yürürlüğe girdiğinde kullanıcı geçmiş mevzuat kataloğu oluşturmadan ilgili bordroyu doğru hesaplayabilmeli.

---

## Faz 6 — P1: `taxYear/taxMonth` tekilliği

### Ürün kararı

Mevcut ürün yalnız normal bordro üretecek; geçmiş fark/ek bordro kapsam dışı.

Bu nedenle normal `payroll_periods` için:

`UNIQUE(tax_year, tax_month)`

invariantı uygulanabilir.

### Uygulama

1. Service validation ile kullanıcıya anlaşılır hata.
2. DB unique index/constraint ile ikinci savunma katmanı.
3. Migration öncesi mevcut duplicate kayıt varsa açık migration error; sessiz seçim yapılmaz.

### Gelecek notu

İleride ek/mahsup/fark bordrosu ürüne girerse bu constraint gevşetilmez; ayrı `payroll_type` + sequence/domain modeli tasarlanır.

---

## Faz 7 — P1: GV indirilebilir kesintileri doğru modelle

### Mevcut sorun

Production GV matrahı şu kalemleri indiriyor:

- işçi SGK
- işçi işsizlik
- yemek GV istisnası
- sendika aidatı

Modelde bulunan bazı başka kesintiler ise yalnız net ödemeden düşüyor.

### Tasarım kararı

Raw kesinti tutarını doğrudan GV matrahından çıkarmak yerine **GV açısından uygun tutar** ayrıca hesaplanmalı/saklanmalı.

#### Doğum/askerlik borçlanması

- `dogumAskerlikBorclanmasiTutar`: gerçek kesinti/ödeme.
- GV indirimi için uygunluk doğrulaması ve gerekiyorsa ayrı `dogumAskerlikGvIndirimTutar`.
- Uygun GV indirimi `cari_gv_matrah` hesabına dahil edilir.

#### Hayat/sağlık sigortası

Mevcut tek `hayatSaglikSigortasiTutar` alanı doğrudan matrahtan çıkarılmayacak.

Gerekli girdiler ve oran/limitler domain fonksiyonunda değerlendirilip:

`uygulanabilirSigortaGvIndirimi`

üretilir.

### Testler

- İndirim yok baseline.
- Tam uygun borçlanma indirimi.
- Uygun olmayan kesinti netten düşer fakat GV'yi etkilemez.
- Sigorta limitine kadar indirim.
- Limit üstü tutar.
- GV matrahı hiçbir koşulda negatif olmaz.

---

## Faz 8 — P1: Kurum ve personel parametre validasyonlarını sertleştir

### Kurum ayarları

Backend şu invariantları zorunlu kılmalı:

- `gunlukTabanUcret > 0`
- `gunlukYemek >= 0`
- `gunlukVasitaYol >= 0`
- `birlestirilmisSosyalYardim >= 0`
- `giyimYardimi >= 0`
- `hizmetZammiBirimi >= 0`
- `gunlukAsgariUcret > 0`
- `pekTavanKatsayisi >= 1`
- yüzde alanları kendi geçerli aralıklarında
- sabit kesintiler/ödemeler negatif olamaz
- iş primi grup `id` ve `ad` değerleri aktif listede tekil olmalı
- aynı iş primi grubu iki aktif kayıtla tanımlanamaz

### Personel

- `hizmetYili >= 0`
- sabit kesintiler negatif olamaz
- devir matrahları negatif olamaz
- `besUyesi=true` ve özel `oksOraniYuzde` varsa oran en az `%3` olmalı
- oran matematiksel olarak `%100` üstüne çıkamaz
- `besUyesi=false` ise özel oran hesaplamada kullanılmaz

### PEK invariantı

Validation sonrasında şu durum matematiksel olarak mümkün olmamalıdır:

`pekAltSinir > pekUstSinir`

Ek olarak hesap fonksiyonu defensive assertion/error ile bunu tekrar korumalıdır.

---

## Faz 9 — P1: Decimal → kuruş persistence fail-mode'unu düzelt

### Mevcut sorun

`dec_to_kurus()` ve `opt_dec_to_kurus()` dönüşüm başarısızlığında `unwrap_or(0)` ile sıfır üretme riski taşır.

### Hedef

Fonksiyonlar `Result` döndürmeli:

- `dec_to_kurus(...) -> Result<i64>`
- `opt_dec_to_kurus(...) -> Result<Option<i64>>`

Overflow veya temsil edilemeyen değer:

`DomainError::InvalidData` / özel money overflow hatası

üretmeli ve transaction rollback olmalıdır.

### Zorunlu testler

- Normal kuruş dönüşümü.
- Half-cent davranışı.
- i64 sınırına yakın güvenli değer.
- Taşan değer → error.
- Payroll save sırasında tek alan taşarsa hiçbir bordro/kalem kısmi persist edilmemeli.

---

## Faz 10 — P1/P2: Negatif net ödemeyi blokla

### Hedef davranış

Normal bordro için:

`gelirToplam - kesintiToplam < 0`

ise bordro normal `CALCULATED` kayıt olarak persist edilmemeli.

Açık bir domain hatası dönmeli:

`NegativeNetPayment { gelir, kesinti, fark }`

UI kullanıcıya hangi kesintilerin toplam geliri aştığını göstermeli.

Kalan borcun sonraki aya devri gerekiyorsa bu ayrı bir debt/remaining-balance modeliyle yapılır; negatif neti bordro olarak kaydetmek çözüm değildir.

### Testler

- Net pozitif.
- Net tam sıfır.
- Net -0,01.
- Büyük icra/kisi borcu kombinasyonu.
- Error durumunda bordro ve line item transaction rollback.

---

## Faz 11 — P2: Tediye ve TİS'i manuel ürün kararına hizala

### Mevcut sorun

Backend aktif listede `.find()` ile ilk aktif Tediye/TİS kaydını otomatik hesaplıyor.

### Ürün kararı

Tediye ve TİS ikramiyesi:

- ekranda görünür kalacak,
- kullanıcı tutarı elle girecek,
- motor otomatik gün × ücret tahmini üretmeyecek.

### Yapılacaklar

1. `aktifDonemdeOdensin` üzerinden otomatik hesap authoritative olmaktan çıkarılacak.
2. Tediye/TİS tutarı manuel payroll input olarak ele alınacak.
3. Manuel değer negatif olamaz.
4. Aynı bordroda birden fazla manuel satır desteklenecekse toplamı açık line-item modeliyle hesaplanacak; `.find()` kullanılmayacak.
5. Mevcut legacy alanlar yalnız migration/display amaçlı korunacaksa hesap motorundan ayrılacak.

---

# 5. Test ve stres stratejisi

## 5.1 Deterministik sınır matrisi

Her kritik formül için tablo-temelli Rust testleri:

- `primGun`: 0, 1, 29, 30, 31
- PEK: alt sınırın hemen altı / eşiti / hemen üstü
- PEK: tavanın hemen altı / eşiti / hemen üstü
- devreden PEK: 0 / kısmi / tam / tavan dışı
- GV kümülatif: vergi dilimi sınırlarının `-0,01 / eşit / +0,01`
- oranlar: minimum / normal / maksimum / geçersiz
- para: 0 / 0,01 / midpoint / büyük tutar / overflow

## 5.2 Ledger senaryoları

En az şu çok dönemli senaryolar kurulmalı:

1. Ocak → Şubat → Mart normal zincir.
2. Önceki CALCULATED bordro değişimi → downstream STALE.
3. FINALIZED zincir mutation blokajı.
4. Devreden PEK iki aylık ömür.
5. Yıl geçişinde devreden PEK.
6. Vergi yılı reseti.
7. Tax opening + gerçek bordro collision.
8. Aynı tax month duplicate period reject.

## 5.3 Puantaj senaryoları

- Tam geçerli 15–14 seti.
- Eksik günler.
- 31 takvim günlük dönem.
- Dönem dışı tarih.
- Geçersiz tarih.
- Rapor + puantaj uyumsuzluğu.
- GÇ/GÇT'nin fiili çalışma/prim günü davranışı.

## 5.4 Rapor senaryoları

- Tek 1 günlük rapor.
- Tek 2+ günlük rapor.
- İlk 5 episode.
- 6. episode.
- Dönem sınırı aşan episode.
- Yıl sınırı aşan episode.
- Exact duplicate.
- Partial overlap.
- Adjacency.

## 5.5 Gelecek effective-date senaryoları

Geçmiş backfill testi yazılmayacak.

Yalnız generic açık/gelecek dönem fixture'ı ile:

- parametre değişimi dönem ortasında,
- değişim ilk gün,
- değişim son gün,
- SGK/GV yemek limitlerinin farklı olması,
- asgari ücret/PEK sınır değişimi

test edilecek.

---

# 6. Uygulama sırası ve bağımlılıklar

Önerilen uygulama sırası:

### Sprint A — Doğrudan para ve giriş güvenliği

1. Faz 0 — invariant sözleşmesi
2. Faz 1 — devreden PEK işçi matrahı
3. Faz 2 — puantaj tarih/dönem validation
4. Faz 8'in kritik kısmı — PEK katsayısı/OKS/negatif parametreler

**Çıkış kriteri:** Motorun doğrudan yanlış para üreten en büyük açıkları kapanmış olmalı.

### Sprint B — Ledger determinismi

5. Faz 3 — STALE/finalization dependency
6. Faz 6 — taxYear/taxMonth uniqueness
7. Faz 9 — persistence overflow fail-closed
8. Faz 10 — negatif net blokajı

**Çıkış kriteri:** Kümülatif vergi ve bordro kayıt zinciri sessiz stale/bozuk persistence üretememeli.

### Sprint C — Rapor ve vergi indirimi doğruluğu

9. Faz 4 — sick leave overlap reject
10. Faz 7 — GV indirilebilir kalemler

**Çıkış kriteri:** Rapor episode ve GV matrahı kesinti modeli açık kurallarla hesaplanmalı.

### Sprint D — Geleceğe dönük dönem-içi parametre değişimi

11. Faz 5 — period-local effective-date segments
12. SGK/GV yemek istisnasını ayrı alanlara ayırma

**Çıkış kriteri:** Yeni bir yasal değişiklik aktif/gelecek 15–14 döneminin içine düştüğünde geçmiş mevzuat arşivi olmadan doğru bölünmüş hesap yapılabilmeli.

### Sprint E — Ürün kapsam temizliği

13. Faz 11 — Tediye/TİS manual-only akış
14. Legacy otomatik yolların temizliği
15. Son regression/stress suite

---

# 7. Definition of Done

Plan tamamlanmış sayılabilmesi için:

- P0 maddelerin tamamı kapanmış olmalı.
- P1 data-integrity maddeleri kapanmış olmalı.
- Her düzeltmenin regression testi bulunmalı.
- Yeni invariantlar yalnız UI'da değil backend/domain seviyesinde uygulanmalı.
- FINALIZED bordro immutable kalmalı.
- STALE bordro export/finalize edilememeli.
- Devreden PEK işçi ve işveren prim matrahı aynı cari PEK dahilini temsil etmeli.
- Dönem dışı puantaj bordroya para üretememeli.
- Overlap rapor kaydı oluşturulamamalı.
- Aynı normal vergi ayına iki dönem atanamamalı.
- Decimal overflow `0 TL` üretememeli.
- Negatif net normal bordro olarak persist edilememeli.
- SGK ve GV yemek istisnası farklı parametreler olarak modellenmeli.
- Effective-date desteği yalnız aktif/gelecek dönem ihtiyacını karşılamalı; geçmiş mevzuat rules engine'e dönüşmemeli.
- Tediye/TİS manuel ürün kararıyla uyumlu olmalı.
- Rust domain/integration testleri ve mevcut frontend regression testleri geçmeli.

---

# 8. Özellikle yapılmayacak mimari genişlemeler

Bu hardening çalışması sırasında aşağıdaki yan projeler açılmayacak:

- genel mevzuat versiyonlama platformu,
- geçmiş yıl parameter archive UI,
- retroactive payroll engine,
- otomatik fark bordrosu,
- tüm mevzuatı expression/rules DSL'e taşıma,
- Tediye/TİS otomasyonunu büyütme,
- yalnız teorik gelecekteki ihtiyaçlar için payroll type sistemi.

İhtiyaç ortaya çıkmadan bu soyutlamalar teknik borcu azaltmaz; artırır.

---

# 9. Son hedef mimari

Hesap zinciri şu hale gelmelidir:

```text
Personel + Period
        ↓
Validated Attendance
        ↓
Validated Sick Leave / Paid Sick Dates
        ↓
Resolved Current/Future Period Parameters
        ↓
Income Calculation
        ↓
PEK
  ├─ current earnings
  ├─ devreden PEK actually used
  ├─ employee premium base
  └─ employer premium base
        ↓
GV Base + Explicit Eligible Deductions
        ↓
Cumulative Tax Chain
        ↓
Deductions
        ↓
Net Payment Guard
        ↓
CALCULATED
        ↓
Dependency / STALE checks
        ↓
FINALIZED immutable snapshot
```

Temel ilke:

> Bordro motoru tahmin üretmek yerine geçersiz veya belirsiz girdide açık hata vermeli; geçmiş mevzuatı yeniden inşa etmeye çalışmak yerine mevcut ve gelecek bordroların matematiksel ve ledger bütünlüğünü garanti etmelidir.
