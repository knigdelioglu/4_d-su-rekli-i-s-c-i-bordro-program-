# INVARIANTS.md — 4/D Sürekli İşçi Bordro Programı

## İKİ KÜMÜLATİF GV MATRAHI (2026-08-07 göreviyle netleşti)
- **A — Gerçek kümülatif GV matrahı**: `onceki_kumulatif_gv_matrahi + cari_gv_matrahi` → gerçek dilim ve brüt GV'yi belirler. `get_previous_cumulative_gv` + `personnel_tax_opening` + devir zinciri (dokunulmaz, ZED-sonrası sabit).
- **B — Asgari ücret takvim referans kümülatifi**: Kişi bordrolarından **türetilmez**. Aynı yılın önceki her takvim ayının kendi dönem ayarlarıyla toplanır (`get_previous_cumulative_asgari_gv` — `has_payroll` kapısı KALDIRILDI). Aylık referans matrah = brüt asgari − işçi SGK − işsizlik (2026: 33.030 − 4.624,20 − 330,30 = **28.075,50**).
- İstisna = `tax(ref_onceki + aylık) − tax(ref_onceki)` (marginal, matrah indirimi DEĞİL); `uygulanan = min(brüt GV, istisna)`; `kesilen = max(0, brüt − uygulanan)`; ayda BİR kez.

## YUVARLAMA POLİTİKALARI (üç ayrı)
- `round2` = `round_dp(2)` MidpointNearestEven — genel varsayılan (gece primi vb.).
- `round_sgk_amount` = MidpointAwayFromZero — YALNIZ SGK prim kalemleri.
- `round_gv_amount` = MidpointAwayFromZero — YALNIZ GV kalemleri (GİB: 4.211,325 → 4.211,33). GV'de SGK policy'si kullanılmaz; round2 GV'de kullanılmaz.

## DOKUNULMAZ (kapsam dışı — 2026-08-07)
- GÇ/GÇT, rapor, PEK alt sınırı, SGK prim yuvarlaması, %21,75 / %2 işveren primleri, 300 TL SGK yemek istisnası, iş primi grup/oran mantığı ve yuvarlaması, devreden PEK, Tediye/TİS otomatik hesaplama, damga vergisi davranışı.
- `get_previous_cumulative_gv` (gerçek) ve tax opening mantığı.
- FINALIZED bordro yeniden hesaplanamaz; snapshot (`gv_snapshot_json` dahil) geri yüklenirken değerler değişmez.

## SNAPSHOT
- `payroll_records.gv_snapshot_json` kolonu (is_primi_snapshot_json deseni): `GvHesapDetayi` — cariGvMatrahi, yeniKumulatifGvMatrahi, brutGelirVergisi, asgariUcretGvMatrahi, asgariUcretReferansKumulatifMatrahi (**cari ay dahil ×m**), asgariUcretGvIstisnasi, uygulananGvIstisnasi, kesilenGelirVergisi.
