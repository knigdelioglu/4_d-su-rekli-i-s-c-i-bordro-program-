# INVARIANTS.md — 4/D Sürekli İşçi Bordro Programı

## İKİ KÜMÜLATİF GV MATRAHI (2026-08-07 göreviyle netleşti)
- **A — Gerçek kümülatif GV matrahı**: `onceki_kumulatif_gv_matrahi + cari_gv_matrahi` → gerçek dilim ve brüt GV'yi belirler. `get_previous_cumulative_gv` + `personnel_tax_opening` + devir zinciri (dokunulmaz, ZED-sonrası sabit).
- **B — Asgari ücret takvim referans kümülatifi**: Kişi bordrolarından **türetilmez**. Aynı yılın önceki her takvim ayının kendi dönem ayarlarıyla toplanır (`get_previous_cumulative_asgari_gv` — `has_payroll` kapısı KALDIRILDI). Aylık referans matrah = brüt asgari − işçi SGK − işsizlik (2026: 33.030 − 4.624,20 − 330,30 = **28.075,50**).
- İstisna = `tax(ref_onceki + aylık) − tax(ref_onceki)` (marginal, matrah indirimi DEĞİL); `uygulanan = min(brüt GV, istisna)`; `kesilen = max(0, brüt − uygulanan)`; ayda BİR kez.

## YUVARLAMA POLİTİKALARI (üç ayrı)
- `round2` = `round_dp(2)` MidpointNearestEven — genel varsayılan (gece primi vb.).
- `round_sgk_amount` = MidpointAwayFromZero — YALNIZ SGK prim kalemleri.
- `round_gv_amount` = MidpointAwayFromZero — YALNIZ GV kalemleri (GİB: 4.211,325 → 4.211,33). GV'de SGK policy'si kullanılmaz; round2 GV'de kullanılmaz.

## RAPOR / SGK PRİM GÜNÜ (2026-08-17 stres hardening)
- Puantajdaki `R` kodu **tek başına SGK prim günü doğurmaz**.
- `R` tarihi yalnız `SickLeaveService::calculate_paid_sick_dates_for_period` sonucunda kurumca ücret ödenen rapor tarihleri arasındaysa primli sayılır.
- Ücret ödenmeyen rapor günü SGK prim gününden düşer; rapor ücreti ödenen gün ücret/prim hakkını korur.
- Tam 15–14 dönemindeki 30 günlük SGK normalizasyonu eksik gün yokken korunur. Ücret hakkı olmayan gün bulunduğunda eksik-gün hesabı gerçek ay/dönem günlerinden yürür ve sonuç hiçbir zaman 30'u aşamaz.
- Rapor günü fiili çalışma değildir; yemek, yol, iş primi ve gece çalışma hak gününe eklenmez.

## DEVREDEN PEK (2026-08-17 stres hardening)
- Cari ayda PEK'e fiilen alınan devreden tutar `devredenPekKullanilan` ve `primMatrahi` içinde yer alır; işçi SGK/işsizlik bu authoritative matrahı izler.
- Cari dönemde SGK prim günü yoksa PEK üst sınırı 0 olduğundan gelen devreden PEK **o ay tüketilemez**.
- Devreden ücret dışı PEK'in taşıma penceresi takvimsel olarak ödemenin yapıldığı ayı takip eden en fazla iki aydır; prim günü olmayan ara ay taşıma ömrünü dondurmaz. `kalanAySayisi` normal biçimde yaşlanır.
- Henüz cari ay PEK'ine alınmamış devreden bakiye işçi primi veya OKS matrahına sokulmaz.

## OKS / BES KATKI PAYI (2026-08-17 stres hardening)
- Oransal OKS hesabının authoritative tabanı işçinin cari ay `pekDetay.primMatrahi` değeridir.
- Bu matrah, cari ayda tavana sığarak gerçekten PEK'e alınan devreden tutarı içerir; henüz taşınan bakiye ile yalnız işverene ait PEK alt-sınır tamamlama farkını içermez.
- Personel veya kurum için pozitif `sabitBesTutar` tanımlıysa mevcut sabit-tutar önceliği korunur.

## DAMGA VERGİSİ ASGARİ ÜCRET İSTİSNASI (2026-08-17 stres hardening)
- Ücret damga vergisi istisnası aylık brüt asgari ücrete isabet eden kısım için uygulanır.
- 15–14 döneminin içinde asgari ücret değişirse production bordro hesabı dönem başlangıcındaki eski baseline'a dönmez; `ResolvedStatutorySnapshot` içindeki ödeme/vergi ayı referansında çözülen günlük asgari ücret kullanılır.
- Damga vergisi negatif olamaz ve istisna ayda bir kez uygulanır.

## BORDRO DÖNEMİ (2026-08-17 stres hardening)
- Çalışma dönemi authoritative olarak **ayın 15'i → takip eden ayın 14'ü** geometrisindedir.
- `BordroDonemi.yil/ay`, başlangıç tarihinin yılını ve ayını ifade eder; başlangıç ayı anlamı değiştirilemez.
- `taxYear/taxMonth` çalışma döneminden ayrı ödeme/tahakkuk metadata'sıdır. Varsayılan öneri bitiş ayıdır fakat kullanıcı tarafından değiştirilebilir; backend bunu zorla bitiş ayına eşitlemez.
- Serbest uzunlukta, 60/100 günlük veya ardışık ay yapısını bozan dönemler veritabanına kaydedilemez.

## DOKUNULMAZ / AYRI SÖZLEŞMELER
- GÇ/GÇT günlerinin fiili çalışma modeli, PEK alt sınırının yalnız işverene ait tamamlama farkı, SGK prim yuvarlaması, işveren prim oranlarının parametrelerden çözülmesi, yemek SGK/GV istisnalarının ayrı parametre olması ve iş primi grup/oran mantığı mevcut sözleşmelerini korur.
- Tediye/TİS otomatik üretilmez; kişi+dönem bazında manuel gelir girdisidir.
- `get_previous_cumulative_gv` (gerçek) ve tax opening mantığı korunur.
- FINALIZED bordro yeniden hesaplanamaz; snapshot (`gv_snapshot_json` dahil) geri yüklenirken değerler değişmez.

## SNAPSHOT
- `payroll_records.gv_snapshot_json` kolonu (is_primi_snapshot_json deseni): `GvHesapDetayi` — cariGvMatrahi, yeniKumulatifGvMatrahi, brutGelirVergisi, asgariUcretGvMatrahi, asgariUcretReferansKumulatifMatrahi (**cari ay dahil ×m**), asgariUcretGvIstisnasi, uygulananGvIstisnasi, kesilenGelirVergisi.
- `statutorySnapshot`, hesapta kullanılan segment çözümünü ve SGK/PEK sınırlarını bordroyla birlikte dondurur.
