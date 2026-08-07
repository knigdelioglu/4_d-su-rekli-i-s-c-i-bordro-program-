# DISCOVERY GÖREVİ — 4/D Bordro Asgari Ücret GV İstisnası / Kümülatif Matrah (READ-ONLY)

## ROLE
Sen bir keşif (discovery) analistisin. **KESİNLİKLE HİÇBİR DOSYA DEĞİŞTİRME, OLUŞTURMA VEYA SİLME.** Salt okunur analiz yap. Kod değişikliği yapma, test çalıştırma (yavaş derlemeler başlatma). Sadece dosyaları oku ve incele.

## PROJE
4/D sürekli işçi bordro programı: SQLite + Rust (Tauri, src-tauri/) + React/TS (src/).
Rust authoritative bordro motorudur. Çalışma dizini: /Users/kadir/Downloads/4_d-sürekli-işçi-bordro-programı

## GÖREV DOSYASI (aşağıda, tam metin)
Aşağıdaki görev dosyası uygulanacak görevin tamamıdır. Sen bu görevi **UYGULAMA** — sadece uygulanabilmesi için gereken keşfi yap.

--- GÖREV DOSYASI BAŞLANGIÇ ---
# 4/D Bordro — Asgari Ücret Gelir Vergisi İstisnası / Kümülatif Matrah Düzeltmesi

İlk denetimdeki 5. maddeyi düzelt: asgari ücret gelir vergisi istisnasını gerçek kümülatif matrah mantığıyla ve GİB uygulamasına uygun şekilde hesapla.

Mevcut SQLite + Rust + Tauri mimarisini koru. Native uygulamada Rust authoritative bordro motoru olmaya devam etsin.

Kümülatif GV açılışı/devir mantığıyla ilgili mevcut repository/service dosyalarını da kendin bul ve incele.

ÖZET GEREKSİNİMLER:
1. Mevcut asgari ücret GV istisnası hesabı gerçek hesap zinciri izlenerek doğrulanacak (önceki kümülatif GV matrahı → cari GV matrahı → cari brüt GV → asgari ücret GV istisnası → kesilecek GV). Motor istisnayı çalışanın oncekiKumulatifGvMatrahi değerine göre oranlama/tahminle hesaplıyorsa raporla.
2. İki kümülatif matrah kesin ayrılacak: (A) çalışanın gerçek kümülatif GV matrahı (onceki_kumulatif_gv_matrahi + cari_gv_matrahi), (B) asgari ücret istisnası referans kümülatif matrahı — 2026: brüt asgari ücret 33.030,00 TL, işçi SGK %14, işsizlik %1 → aylık referans GV matrahı 28.075,50 TL = asgari_ucret_aylik_gv_matrahi domain kavramı.
3. Asgari ücret kümülatifi kişi bordrolarından türetilmeyecek; takvim ayına göre asgari ücretin kendi kümülatif matrahı (örn. Ağustos 2026 = 28.075,50 × 8). İki state: personel_kumulatif_gv_matrahi ve asgari_ucret_referans_kumulatif_matrahi.
4. 15–14 bordro dönemi istisna hesabında günlük bölünmeyecek; istisna bordronun ait olduğu ödeme ayındaki asgari ücret istisnasıdır. PayrollPeriod içinde ödeme ayını temsil eden authoritative yıl/ay alanı kullanılacak (gerekirse domain'de tax_year/tax_month).
5. Cari gelir vergisi: onceki_vergi = tax(onceki_kumulatif); yeni_kumulatif_vergi = tax(onceki_kumulatif + cari_matrahi); cari_brut_gv = fark. tax() mevcut ücret GV tarifesi fonksiyonu. Matrahtan asgari ücret matrahını çıkarıp oran uygulama yok.
6. 2026 ücret tarifesi: 190.000'e kadar %15; 400.000'e kadar (190.000'i için 28.500) %20; ücrette 1.500.000'e kadar (400.000'i için 70.500) %27; ücrette 5.300.000'e kadar (1.500.000'i için 367.500) %35; üzeri %40. Mevcut tax-bracket/domain modeli kullanılacak, magic number çoğaltılmayacak.
7. Asgari ücretin ilgili ay vergisi: asgari_onceki = aylik_asgari_gv_matrahi × (m-1); asgari_cari = aylik × m; asgari_ucret_aylik_vergi_istisnasi = tax(asgari_cari) - tax(asgari_onceki). Yıl boyu %15 sabit DEĞİL; dilim değişince istisna otomatik değişir.
8. 2026 regression referansı (motor türetsin, hardcoded lookup olmasın): Ocak 4.211,33 / Şubat 4.211,33 / Mart 4.211,33 / Nisan 4.211,33 / Mayıs 4.211,33 / Haziran 4.211,33 / Temmuz 4.537,75 / Ağustos 5.615,10 / Eylül 5.615,10 / Ekim 5.615,10 / Kasım 5.615,10 / Aralık 5.615,10.
9. uygulanacak_gv_istisnasi = min(cari_brut_gelir_vergisi, asgari_ucret_aylik_vergi_istisnasi); kesilecek_gelir_vergisi = max(0, cari_brut_gelir_vergisi - uygulanacak_gv_istisnasi). Negatif GV yok. İstisna matrah indirimi değildir.
10. Gerçek kümülatif GV açılışı (personnel_tax_opening) ve gerçek bordrolardan otomatik taşıma korunacak; istisna bunlardan türetilmeyecek. Ana regression noktası: açılış 120.000 + Mayıs cari 65.000 → gerçek yeni kümülatif 185.000; Mayıs istisnası 120.000'den oranlanmayacak.
11. Aynı ay çoklu ücret kalemi (mesai, iş primi, gece primi, sosyal yardım, manuel ücret, ikramiye/tediye): toplam matraha dahil; istisna ödeme ayında BİR KEZ uygulanır.
12. Bordro snapshot: oncekiKumulatifGvMatrahi, cariGvMatrahi, yeniKumulatifGvMatrahi, brutGelirVergisi, asgariUcretGvMatrahi, asgariUcretReferansKumulatifMatrahi, asgariUcretGvIstisnasi, uygulananGvIstisnasi, kesilenGelirVergisi (isimlendirme mevcut convention'a uyarlanabilir). FINALIZED yeniden açılınca değerler değişmez; mevcut snapshot JSON yaklaşımı kullanılır.
13. UI: PaySlip/Bordro detayında GV Matrahı, Önceki Kümülatif GV, Yeni Kümülatif GV, Hesaplanan GV (İstisna Öncesi), Asgari Ücret GV İstisnası, Kesilen Gelir Vergisi — kalabalık yapmadan, istisna gerçek kümülatifmiş gibi gösterilmeden.
14. Regression testleri A–H (Ocak 2026 istisna 4.211,33; Temmuz dilim geçişi 4.537,75; Ağustos 5.615,10; personel kümülatifi istisnayı değiştirmiyor; açılış/devir 120.000+65.000=185.000; istisna brüt vergiyi aşamaz (2.000 brüt → 2.000 istisna, 0 kesinti); çoklu kalemde tek istisna; FINALIZED snapshot değişmez).
15. Kapsam dışı: GÇ/GÇT, rapor, PEK alt sınırı, SGK prim yuvarlaması, %21,75/%2 işveren primleri, 300 TL SGK yemek istisnası, iş primi grup/oran mantığı ve yuvarlaması, devreden PEK, Tediye/TİS. Damga vergisi ayrı; bozma. Git commit yok.
--- GÖREV DOSYASI SONU ---

## İNCELEME KAPSAMI (okuma listesi — bunlarla sınırlı değil, ilişkili dosyaları da bul)
Başlangıç dosyaları:
- src-tauri/src/domain/models.rs
- src-tauri/src/domain/calculations.rs
- src-tauri/src/services/payroll_service.rs
- src-tauri/src/repositories/payroll_repo.rs
- src-tauri/src/repositories/settings_repo.rs
- src-tauri/src/db/migrations.rs
- src-tauri/tests/domain_tests.rs
- src/types/payroll.ts
- src/utils/payrollUtils.ts
- src/components/BordroHesaplama.tsx
- src/components/PaySlipModal.tsx
- src/components/PeriodManagerModal.tsx

Ayrıca kümülatif GV açılışı/devir zinciriyle ilgili dosyaları KENDİN BUL ve incele:
- services/cumulative_tax_service.rs, repositories/tax_opening_repo.rs, period_repo.rs, personnel_repo.rs,
- payroll_cmd.rs, period_cmd.rs, settings_cmd.rs, commands/mod.rs, db/migrations.rs içindeki ilgili tablolar,
- src/utils/cumulativeGv.test.ts ve diğer testler,
- src/services/tauriBridge.ts, src/components/PuantajGrid.tsx, PersonelFormModal.tsx (tax opening UI varsa).

## CEVAPLAMAN GEREKEN SORULAR (gerçek kod akışından, tahmin değil)
1. Mevcut asgari ücret GV istisnası şu an NASIL hesaplanıyor? Hangi fonksiyon/lar, hangi dosya:satır? Çalışanın oncekiKumulatifGvMatrahi (veya açılış) değerine göre oranlama/tahmin var mı? Kod parçacıklarıyla göster.
2. Kümülatif GV akışı: bordro hesaplanırken önceki kümülatif nereden geliyor (DB tablosu/kolon, tax opening, önceki bordro snapshot)? cari_gv_matrahi nasıl birikiyor? FINALIZED yapılınca ne yazılıyor? Bordro yeniden açılınca (reopen) ne oluyor?
3. PayrollPeriod modeli: ödeme ayını temsil eden authoritative yıl/ay alanı hangisi? 15–14 dönem (başlangıç/bitiş tarihleri) ayrı mı tutuluyor? Bordro hangi dönem için hesaplanıyor?
4. 2026 ücret GV tarifesi şu an nerede tanımlı (settings parametreleri mi, kod mu)? tax() fonksiyonu hangi dosya:satır? Dilimler nasıl modellenmiş?
5. Brüt GV ve istisna şu an bordro snapshot'ına hangi alanlarla yazılıyor? Snapshot JSON şeması ne? migrations'da hangi tablolar/kolonlar var?
6. Asgari ücret tutarı nereden geliyor (settings_repo/parametre)? SGK/işsizlik oranları nerede?
7. GV yuvarlama policy'si nedir (round_sgk_amount ile karıştırma)? Mevcut gelir vergisi yuvarlaması hangi fonksiyon?
8. Mevcut testler hangileri, hangi dosyalar, hangi değerleri doğruluyor? cumulativeGv.test.ts ne test ediyor?
9. UI'da istisna/kümülatif şu an nasıl gösteriliyor (PaySlipModal, BordroHesaplama)?
10. Kapsam dışı alanlara (SGK, damga, iş primi, PEK, GÇ/GÇT) dokunmadan bu değişikliğin yapılabileceği dosya seti nedir? Bağımlılık zincirinde kırılma riski var mı?

## ÇIKTI FORMATI (kesinlikle bu yapıda, Türkçe)
1. **MEVCUT_DURUM**: madde madde — mevcut istisna hesabı (kod referanslarıyla), kümülatif akış, PayrollPeriod, tarife kaynağı, snapshot şeması, yuvarlama.
2. **HESAP_ZINCIRI**: önceki kümülatif → cari matrah → brüt GV → istisna → kesilecek GV akışının gerçek koddaki adımları (dosya:satır).
3. **EDIT_SCOPE**: değiştirilmesi gerekecek dosyalar (kesin, glob/liste) + her biri için tek cümle gerekçe.
4. **READ_SCOPE**: okunabilir ama değiştirilmemesi gereken dosyalar + gerekçe.
5. **TESTLER**: eklenmesi/değişmesi gereken test dosyaları ve her testin neyi doğrulayacağı (görevdeki Test A–H haritalaması).
6. **INVARIANTS**: korunması gereken davranışsal kurallar (kapsam dışı alanlar dahil).
7. **RISKLER**: kırılma riskleri, belirsizlikler, bilinmeyenler.
8. **KAPSAM_DISI**: bilerek dokunulmayacaklar.
