# INVARIANTS.md — 4/D Sürekli İşçi Bordro Programı

## İKİ KÜMÜLATİF GV MATRAHI (2026-08-07 göreviyle netleşti)
- **A — Gerçek kümülatif GV matrahı**: `onceki_kumulatif_gv_matrahi + cari_gv_matrahi` → gerçek dilim ve brüt GV'yi belirler. `get_previous_cumulative_gv` + `personnel_tax_opening` + devir zinciri (dokunulmaz, ZED-sonrası sabit).
- Gerçek kümülatif zincir tahakkuk seviyesindedir: önceki vergi ayları + aynı `taxYear/taxMonth` içindeki daha önceki authoritative tahakkukların GV matrahları birlikte hesaba katılır.
- **B — Asgari ücret takvim referans kümülatifi**: Kişi bordrolarından **türetilmez**. Aynı yılın önceki her takvim ayının kendi dönem ayarlarıyla toplanır (`get_previous_cumulative_asgari_gv` — `has_payroll` kapısı KALDIRILDI). Aylık referans matrah = brüt asgari − işçi SGK − işsizlik (2026: 33.030 − 4.624,20 − 330,30 = **28.075,50**).
- İstisna = `tax(ref_onceki + aylık) − tax(ref_onceki)` (marginal, matrah indirimi DEĞİL). Aynı vergi ayında birden fazla tahakkuk varsa aylık hak tekrar açılmaz: `kalan = aylık_hak − aynı_ay_önce_kullanılan`; `uygulanan = min(brüt GV, kalan)`; `kesilen = max(0, brüt − uygulanan)`.

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

## ÇOKLU TAHAKKUK / ACCRUAL (2026-09-04 production hardening)
- `BordroDonemi` çalışma dönemi konteyneridir; bordro hesabının bağımsız ödeme düğümü `payroll_records` içindeki tahakkuktur.
- Tahakkuk tipleri: `NORMAL`, `TEDIYE`, `TIS_IKRAMIYE`, `SUPPLEMENTAL`.
- Aynı personel+dönem için yalnız **bir** `NORMAL` tahakkuk olabilir. Ek tahakkuk sayısı birden fazla olabilir.
- Authoritative sıralama: `taxYear/taxMonth` → `paymentDate` → `sequence` → `accrualId`.
- `paymentDate` açık ve gerçek ödeme/tahakkuk tarihidir; ilgili dönemin `taxYear/taxMonth` değeriyle uyumlu olmak zorundadır.
- Her tahakkuk bağımsız payment event’tir. Tahakkuk tipi order belirlemez. Tüm türlerde `sequence>=0`; sequence yalnız aynı ödeme tarihinin tie-breaker değeridir. Aynı tarihli tahakkuklar sequence ile deterministik sıralanır ve aynı vergi ayı+tarih+sequence kombinasyonu benzersizdir.
- Mevcut tahakkukun `accrualId`, `accrualType`, `paymentDate` ve `sequence` metadata'sı değiştirilemez; yeniden hesaplama yalnız parasal sonucu güncelleyebilir.
- NORMAL zorunlu ilk event değildir; TEDIYE → NORMAL → TİS desteklenir. Cari state’i etkileyen önceki event’ler CALCULATED/FINALIZED olmalıdır.
- Aynı-ay GV/DV istisnası ve PEK kapasitesi canonical payment order üzerinden paylaşılır. GV allocation pure `gv_exemption::GvExemptionState` policy’sidir; mutable bakiye tablosu yoktur.
- Backdated insert, recalculation ve delete mutable downstream kayıtlarını STALE yapar; FINALIZED downstream’u etkileyen mutation reddedilir.
- Ek tahakkuk normal maaş gelirlerini yeniden üretmez. `TEDIYE` yalnız tediye brütünü, `TIS_IKRAMIYE` yalnız TİS ikramiye brütünü, `SUPPLEMENTAL` yalnız kendi brütünü üretir.
- Önceki tahakkuk zincirinde `DRAFT` veya `STALE` kayıt varsa kümülatif/PEK state'i sessiz fallback ile devam ettirilmez; hesap fail-closed olmalıdır.
- `FINALIZED` tahakkuk immutable'dır. Önceki bir mutable düğüm değişince sonraki mutable bağımlılar STALE olabilir; downstream FINALIZED bağımlılığı bozan mutation reddedilir.

## DEVREDEN PEK (2026-09-04 multi-accrual hardening)
- Cari ayda PEK'e fiilen alınan devreden tutar `devredenPekKullanilan` ve `primMatrahi` içinde yer alır; işçi SGK/işsizlik bu authoritative matrahı izler.
- Cari dönemde SGK prim günü yoksa PEK üst sınırı 0 olduğundan gelen devreden PEK **o ay tüketilemez**.
- Devreden ücret dışı PEK'in taşıma penceresi takvimsel olarak ödemenin yapıldığı ayı takip eden en fazla iki aydır; prim günü olmayan ara ay taşıma ömrünü dondurmaz.
- `kalanAySayisi` **tahakkuk sayısına göre değil vergi ayı geçişine göre** yaşlanır. Aynı `taxYear/taxMonth` içindeki NORMAL → TEDIYE → TİS/SUPPLEMENTAL zincirinde bakiye azalabilir fakat `kalanAySayisi` azalmaz. Bir sonraki vergi ayına geçildiğinde yalnız bir kez yaşlanır.
- Aynı ay içindeki sonraki tahakkuk, önceki tahakkukun `sonrakiDevredenPek` state'inden devam eder; aylık PEK tavanı sıfırdan açılmaz.
- NORMAL daha sonra geldiğinde işveren alt sınır tamamlama farkı, aylık alt sınırdan önceki event’lerin PEK’i düşülerek hesaplanır; işçi matrahına eklenmez.
- Aylık PEK kapasitesi month-to-date paylaşılır: aynı vergi ayındaki tahakkukların authoritative `primMatrahi` toplamı aylık tavanı aşamaz.
- Henüz cari ay PEK'ine alınmamış devreden bakiye işçi primi veya OKS matrahına sokulmaz.

## OKS / BES KATKI PAYI (2026-08-17 stres hardening)
- Oransal OKS hesabının authoritative tabanı işçinin cari tahakkuktaki incremental `pekDetay.primMatrahi` değeridir; aynı ayın önceki tahakkukları ikinci kez primlendirilmez.
- Bu matrah, cari ayda tavana sığarak gerçekten PEK'e alınan devreden tutarı içerir; henüz taşınan bakiye ile yalnız işverene ait PEK alt-sınır tamamlama farkını içermez.
- Personel veya kurum için pozitif `sabitBesTutar` tanımlıysa mevcut sabit-tutar önceliği korunur.

## DAMGA VERGİSİ ASGARİ ÜCRET İSTİSNASI (2026-09-04 multi-accrual hardening)
- Ücret damga vergisi istisnası aylık brüt asgari ücrete isabet eden kısım için uygulanır.
- 15–14 döneminin içinde asgari ücret değişirse production bordro hesabı dönem başlangıcındaki eski baseline'a dönmez; `ResolvedStatutorySnapshot` içindeki ödeme/vergi ayı referansında çözülen günlük asgari ücret kullanılır.
- Damga vergisi negatif olamaz ve aylık hak aynı vergi ayında yalnız bir kez tüketilir. Sonraki tahakkuk `aylık_hak − aynı_ay_önce_kullanılan` kalanından yararlanır; her tahakkukta tam istisna yeniden açılmaz.

## BORDRO DÖNEMİ (2026-08-17 stres hardening)
- Çalışma dönemi authoritative olarak **ayın 15'i → takip eden ayın 14'ü** geometrisindedir.
- `BordroDonemi.yil/ay`, başlangıç tarihinin yılını ve ayını ifade eder; başlangıç ayı anlamı değiştirilemez.
- `taxYear/taxMonth` çalışma döneminden ayrı ödeme/tahakkuk metadata'sıdır. Varsayılan öneri bitiş ayıdır fakat kullanıcı tarafından değiştirilebilir; backend bunu zorla bitiş ayına eşitlemez.
- Serbest uzunlukta, 60/100 günlük veya ardışık ay yapısını bozan dönemler veritabanına kaydedilemez.

## TEDİYE / TİS İKRAMİYESİ GİRİŞ POLİTİKASI
- Yeni Tediye ve TİS ikramiyesi girişleri artık NORMAL bordroya manuel gelir alanı olarak eklenmez; **ayrı tahakkuk** (`TEDIYE` / `TIS_IKRAMIYE`) olarak oluşturulur.
- NORMAL ekranında yeni manual Tediye/TİS girişi yapılmaz. Operasyonel giriş noktası `Ek Tahakkuk` akışıdır.
- Eski V2/V3 backup veya geçmiş NORMAL bordrolarında `gelirler.tediye` / `gelirler.tisIkramiyesi` bulunabilir. Bu değerler backward compatibility amacıyla okunur, read-only legacy olarak gösterilir ve mevcut legacy CALCULATED kayıtların yeniden hesaplanabilirliği korunur.
- Legacy manual Tediye/TİS desteği **yeni veri girişi modeli değildir**; yeni coding çalışmaları eski manuel giriş UI'sını geri getirmemelidir.

## RESMÎ LİSTELER / ÇIKTILAR
- Banka ve kesinti/BES listeleri kişi başına tek bordro varsayamaz; aynı dönemdeki tüm `CALCULATED`/`FINALIZED` tahakkuklar ayrı authoritative payment event olarak değerlendirilir.
- `DRAFT` ve `STALE` tahakkuklar resmî liste/toplamlara girmez.
- Ödeme tarihi filtresi, ekran toplamı, Excel ve print aynı filtrelenmiş accrual dataset'ini kullanmalıdır.
- Personel hesaplanma KPI'sı tahakkuk sayısını değil authoritative `NORMAL` tahakkuku bulunan benzersiz personel sayısını gösterir.

## DOKUNULMAZ / AYRI SÖZLEŞMELER
- GÇ/GÇT günlerinin fiili çalışma modeli, PEK alt sınırının yalnız işverene ait tamamlama farkı, SGK prim yuvarlaması, işveren prim oranlarının parametrelerden çözülmesi, yemek SGK/GV istisnalarının ayrı parametre olması ve iş primi grup/oran mantığı mevcut sözleşmelerini korur.
- `get_previous_cumulative_gv` (gerçek) ve tax opening mantığı korunur; çoklu tahakkuk desteği aynı vergi ayındaki önceki authoritative tahakkukları bu zincire dahil eder.
- FINALIZED bordro/tahakkuk yeniden hesaplanamaz; snapshot (`gv_snapshot_json` dahil) geri yüklenirken değerler değişmez.
- Production hesap motoru `crates/payroll-core`'dur. Tauri ve WASM aynı Rust core'u kullanır; TypeScript tarafında ikinci authoritative bordro formülü oluşturulmaz.

## SNAPSHOT
- `payroll_records.gv_snapshot_json` / `GvHesapDetayi` cari ve yeni kümülatif GV matrahını, brüt GV'yi, aylık asgari ücret GV istisna hakkını, aynı ay önce kullanılan istisnayı, tahakkuk öncesi/sonrası kalanı, uygulanan istisnayı ve kesilen GV'yi denetlenebilir biçimde dondurur.
- Damga vergisi snapshot'ı aylık hak, aynı ay önce kullanılan, cari uygulanan, kalan ve kesilen damga vergisi state'ini taşır.
- `pekDetay`, `devredenPekGelen` ve `sonrakiDevredenPek` tahakkuklar arası month-to-date PEK/devreden PEK zincirinin denetlenebilir state'idir.
- `statutorySnapshot`, hesapta kullanılan segment çözümünü ve SGK/PEK sınırlarını bordroyla birlikte dondurur.
