# CODING CONTRACT — Asgari Ücret GV İstisnası / Kümülatif Matrah Düzeltmesi (STRICT)

## GÖREV
`.hermes/desktop-attachments/4-D Bordro — Asgari Ücret Gelir Vergisi İstisnası - Kümülatif Matrah Düzeltmesi.md` dosyasını **eksiksiz** uygula. Bu contract, görev dosyasıyla birlikte tek otorite kaynağıdır; çelişki olursa bu contract önceliklidir.

## PROJE (kısa)
SQLite + Rust (Tauri) + React/TS (Vite). **Rust authoritative bordro motorudur.** Frontend'deki payrollUtils.ts hesap aynasıdır ve Rust ile tutarlı olmalıdır (mevcut mimari deseni).

## DISCOVERY BULGULARI (doğrulanmış, ön kabul)
1. **Ana hata:** `src-tauri/src/services/cumulative_tax_service.rs` içindeki `get_previous_cumulative_asgari_gv` fonksiyonu, asgari ücret referans kümülatifini çalışanın **kendi bordro kayıtlarına** bağlıyor (`has_payroll` kontrolü; bordrosu olmayan ay `continue` ile atlanıyor). Yıl ortasında işe başlayan/gap olan personelde referans kümülatif 0 kalıyor → istisna Ocak seviyesinde (4.211,33) hesaplanıyor; doğrusu ödeme ayının takvim pozisyonuna göre (ör. Ağustos: 28.075,50×8−28.075,50×7).
2. **Mevcut doğru çekirdek:** `calculations.rs` `calculate_gelir_vergisi_2026(matrah, kumulatif_onceki)` = round2( tax(kumulatif_onceki+matrah) − tax(kumulatif_onceki) ) — doğru yöntem (matrah indirimi değil). `gelir_vergisi = max(0, ham_gv − istisna)` — doğru. 2026 dilimleri `get_gelir_vergisi_dilimleri_2026()` — doğru. SGK yuvarlaması (`round_sgk_amount`, MidpointAwayFromZero) GV'de kullanılmıyor — doğru.
3. **Yuvarlama bulgusu (görev 17'nin "nedeni tespit et" maddesi):** GV hesabı `round2` = `round_dp(2)` = **MidpointNearestEven** kullanıyor. Ocak istisnası = tax(28.075,50)−tax(0) = 4.211,325 → tie → NearestEven **4.211,32** üretir; GİB uygulaması/beklenen değer **4.211,33** (yarım kuruş yukarı). Temmuz 4.537,75 ve Ağustos 5.615,10 tam değerlerdir (tie yok), mevcut yuvarlamayla da doğru çıkıyor. **Karar:** GV kalemlerinde (brüt GV, asgari istisna, kesilen GV, asgari matrah ara değerleri) yarım-kuruş-yukarı yuvarlama (MidpointAwayFromZero) uygulanacak — ayrı bir `round_gv_amount` (veya eşdeğeri) ile; `round2`'nin diğer kullanımları (gece primi vb.) DEĞİŞMEYECEK; `round_sgk_amount` ve SGK hesabı HİÇ değişmeyecek. Sonuç: Ocak–Haziran 4.211,33, Temmuz 4.537,75, Ağustos–Aralık 5.615,10 olmalı.
4. **Snapshot:** `BordroKaydi`'nda `oncekiKumulatifGvMatrahi` ve `oncekiKumulatifAsgariGvMatrahi` var; görev 13'ün istediği detay alanları (cariGvMatrahi, yeniKumulatifGvMatrahi, brutGelirVergisi, asgariUcretGvMatrahi, asgariUcretReferansKumulatifMatrahi, asgariUcretGvIstisnasi, uygulananGvIstisnasi) YOK. Kod tabanındaki mevcut desen: `PekDetayi`→`pek_detail_json` kolonu, `IsPrimiHesapDetayi`→`is_primi_snapshot_json` kolonu (migration + repo save/load). Aynı desenle **`gv_snapshot_json`** kolonu eklenecek.

## EDIT_SCOPE (yalnız bu dosyalar değiştirilebilir)
1. `src-tauri/src/domain/models.rs` — GvHesapDetayi (PekDetayi/IsPrimiHesapDetayi deseninde, serde ile) + BordroKaydi'na yeni alanlar/gv_snapshot_json
2. `src-tauri/src/domain/calculations.rs` — round_gv_amount (half-up) + GV hesap detayını döndür; mevcut signature'ları koruyarak genişlet (IsPrimiHesapDetayi dönüş desenine bak)
3. `src-tauri/src/services/cumulative_tax_service.rs` — get_previous_cumulative_asgari_gv: has_payroll kapısını KALDIR; aynı yılın tüm önceki aylarını kendi dönem ayarlarıyla topla (ayar yoksa varsayılanlar); imza sadeleşirse tüm çağrı noktalarını güncelle
4. `src-tauri/src/services/payroll_service.rs` — detayı doldur, BordroKaydi'na yaz
5. `src-tauri/src/repositories/payroll_repo.rs` — gv_snapshot_json kaydet/yükle (is_primi_snapshot_json deseni; eski kayıtlarda NULL → Option)
6. `src-tauri/src/db/migrations.rs` — payroll_records'a `gv_snapshot_json TEXT` kolonu (mevcut migration deseninde)
7. `src-tauri/tests/domain_tests.rs` — Test A–H (aşağıda)
8. `src/types/payroll.ts` — tipler (BordroKaydi + GvHesapDetayi)
9. `src/utils/payrollUtils.ts` — ayna: takvim bazlı asgari kümülatif + half-up GV yuvarlama + detay alanları; diğer hesap davranışını değiştirme
10. `src/utils/cumulativeGv.test.ts` — TS ayna testleri (en az A, B, C, D)
11. `src/components/PaySlipModal.tsx` — görev 14'ün 6 alanı (GV Matrahı, Önceki Kümülatif, Yeni Kümülatif, Hesaplanan GV (İstisna Öncesi), Asgari Ücret GV İstisnası, Kesilen GV); gerçek kümülatifle istisnayı karıştırma
12. `src/components/BordroHesaplama.tsx` — yalnız gerekirse minimal

## READ_SCOPE (okunabilir, DEĞİŞTİRİLEMEZ)
- `.hermes/**` (görev dosyası dahil), `package.json`, `tsconfig.json`, `vite.config.ts`, `src-tauri/Cargo.toml`, `AGENTS.md`/`CLAUDE.md` varsa
- `src-tauri/src/lib.rs`, `main.rs`, `domain/errors.rs`, `services/{migration_service,sick_leave_service}.rs`, `repositories/{settings_repo,period_repo,tax_opening_repo,personnel_repo,attendance_repo,sick_leave_repo}.rs`, `commands/**`
- `src/App.tsx`, `main.tsx`, `components/PeriodManagerModal.tsx`, `utils/{excelExport,sampleData}.ts`
- Başka dosya gerekirse: KENDİN GENİŞLETME. `NEED_CONTEXT` veya `REQUEST_EDIT_SCOPE` ile dur, bekle.

## UYGULAMA GEREKLİLİKLERİ
- Görev dosyasındaki 1–17 maddelerinin TAMAMI (kapsam dışı hariç).
- İstisna yalnız bir kez (toplam ücret matrahına); negatif GV yok; min(ham_gv, aylık_istisna) semantiği korunacak.
- `get_previous_cumulative_gv` (gerçek kümülatif) ve personnel_tax_opening/devir mantığına DOKUNMA (görev 11).
- FINALIZED bordro yeniden hesaplanamaz (mevcut davranış); kayıtlı değerler değişmez (Test H).
- Damga vergisi davranışını bozma (mevcut doğru hesaplama aynen kalır).
- Kapsam dışı: GÇ/GÇT, rapor, PEK alt sınırı, SGK prim yuvarlaması, %21,75/%2 işveren primleri, 300 TL SGK yemek istisnası, iş primi grup/oran ve yuvarlaması, devreden PEK, Tediye/TİS otomatik hesaplama.
- Git komutları: `git commit/push/reset/restore/stash/checkout/clean` KESİNLİKLE YOK (yalnız status/diff/log okuma).
- Hardcode YASAK: motor değerleri türetir (asgari ücret + SGK/işsizlik oranları + 2026 tarife + ödeme ayı). Beklenen değerler yalnız TEST ASSERT'lerinde sabit olabilir.

## TESTLER (domain_tests.rs'de A–H; TS aynasında A–D)
- A: Ocak 2026 — matrah 28.075,50; istisna 4.211,33
- B: Temmuz — istisna 4.537,75 (dilim geçişi, %15 sabit değil)
- C: Ağustos — istisna 5.615,10
- D: Gerçek kümülatif (0 vs 300.000) farklı personelde aynı ay istisnası AYNI; brüt GV farklı olabilir
- E: Açılış/devir regression: önceki 120.000 + cari 65.000 → yeni kümülatif 185.000; Mayıs istisnası kendi takvim referansından (4.211,33), 120.000'den türetilmez
- F: İstisna brüt GV'yi aşamaz: ham 2.000, istisna hakkı 4.211,33 → uygulanan 2.000, kesilen 0
- G: Çoklu gelir kalemi (maaş+mesai+iş primi+manuel) tek bordroda → istisna bir kez
- H: FINALIZED snapshot: asgari ücret/tarife/gerçek açılış değişse de kayıtlı istisna+kesilen GV değişmez (kaydet→yükle→karşılaştır; yeniden hesaplama FINALIZED'de engellenir)

## DOĞRULAMA (ZORUNLU — komutları SEN çalıştır, çıktıyı raporla)
1. `cargo test` (src-tauri)
2. `bun test`
3. `bun run lint`
4. `bun run build`
Hepsi geçmeli. Her birinin çıktısının ilgili kısmını (test sayıları, hata varsa) STATUS'a yaz. 2026 regression değerlerinin (Ocak 4.211,33 / Temmuz 4.537,75 / Ağustos 5.615,10) test çıktısında göründüğünü göster.

## STATUS FORMATI (bitince)
```
## STATUS: DONE|BLOCKED
### Yapılan değişiklikler (dosya dosya, kısa)
### Test sonuçları (4 komut, çıktı özetleri)
### 2026 regression (Ocak/Temmuz/Ağustos değerleri + hangi testte)
### Invariant doğrulaması (kontrol listesi)
### 8 maddelik rapor (görev dosyasının istediği)
### Kapsam ihlali/kapsam dışı temas (varsa)
### NEED_CONTEXT / REQUEST_EDIT_SCOPE (varsa — DONE yerine BLOCKED)
```
Bir engelde: dur, `## STATUS: BLOCKED` + `NEED_CONTEXT: <soru>` yaz, bekle. Devam etme.
