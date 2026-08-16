# 2026-08-17 — Bordro motoru stres hardening

## Kapsam

Güncel `main` bordro motorunda stres/edge-case incelemesinde bulunan aşağıdaki alanlar fail-closed ve mevzuat uyumlu hale getirildi:

1. Ücret ödenmeyen `R` rapor gününün SGK prim gününe yanlış dahil edilmesi.
2. Prim günü olmayan ayda devreden ücret dışı PEK'in yanlış tüketilme riski ve iki aylık taşıma penceresi.
3. 15–14 döneminde asgari ücret değiştiğinde damga vergisi istisnasının eski dönem baseline'ına bağlı kalması.
4. Oransal OKS katkısının cari ayda gerçekten kullanılan devreden PEK'i görmemesi.
5. Serbest/çok uzun/geometrisi bozuk bordro dönemlerinin backend tarafından kabul edilebilmesi.

## Uygulanan kurallar

### Rapor / SGK günü

- `R` artık tek başına primli değildir.
- Production bordro yolu `SickLeaveService::calculate_paid_sick_dates_for_period` ile çözülen kurumca ücretli rapor tarihlerini statutory snapshot'a geçirir.
- Yalnız bu tarihlerdeki `R` primli sayılır.
- Ücretsiz `R` tarihleri prim gününden düşer.
- Eksik gün bulunan dönemlerde tarih konumuna bağlı 0/virtual-day yan etkisi engellendi; SGK eksik-gün hesabı ay/dönem günleri üzerinden yürür, toplam prim günü 30'u aşmaz.

### Devreden PEK

- Cari dönemde prim günü yoksa statutory PEK üst sınırı 0 olduğundan gelen devreden PEK bu ay kullanılmaz.
- Taşıma süresi SGK kuralındaki "ödeme tarihini takip eden iki ay" penceresi olduğundan primsiz ara ay `kalanAySayisi` değerini dondurmaz; kayıt normal şekilde yaşlanır.
- Cari ayda gerçekten PEK'e alınan tutar `primMatrahi` içinde kalır.

### OKS

- Sabit BES/OKS tutarı tanımlıysa mevcut öncelik korunur.
- Oransal katkıda production authoritative matrah `pekDetay.primMatrahi` oldu.
- Böylece cari ayda gerçekten kullanılan devreden PEK katkı matrahına girer; henüz taşınan bakiye ve yalnız işverene ait alt-sınır tamamlama farkı girmez.

### Damga vergisi

- Production bordroda asgari ücret damga istisnası `ResolvedStatutorySnapshot.gvReferansGunlukAsgariUcret` üzerinden çözülür.
- Dönem içinde yürürlüğe giren asgari ücret değişikliğinde dönem başlangıcındaki eski baseline ile fazla kesinti üretilmez.

### Dönem invariantı

- Backend çalışma dönemi yalnız `15 -> takip eden ayın 14'ü` geometrisini kabul eder.
- `yil/ay` başlangıç tarihinin yıl/ayıyla eşleşir.
- `taxYear/taxMonth` ürün sözleşmesi gereği ayrı ve kullanıcı tarafından seçilebilir kalır; backend bitiş ayına zorlamaz.

## Regresyonlar eklendi

Yeni dosya:

`src-tauri/tests/payroll_stress_hardening_regression_test.rs`

Kapsanan senaryolar:

- ücretli ve ücretsiz `R` ayrımı,
- ücretsiz `R` gününün takvim konumuna bağlı sonuç üretmemesi,
- prim günü 0 iken devreden PEK'in kullanılmaması fakat taşıma ömrünün takvimsel yaşlanması,
- kullanılan devreden PEK'in OKS `primMatrahi` içinde görülmesi,
- dönem içi asgari ücret değişikliğinde damga istisnası,
- 15–14 dönem geometrisi ve configurable `taxMonth` sözleşmesi.

## Dokümantasyon

`.hermes/INVARIANTS.md` yeni rapor/SGK, devreden PEK, OKS, damga vergisi ve dönem invariantlarıyla güncellendi.

## Test durumu

**TESTLER ÇALIŞTIRILMADI. BUILD/LINT ÇALIŞTIRILMADI.**

Kullanıcı talebi gereği tüm doğrulamalar işlerin tamamlanmasından sonra tek toplu test aşamasında çalıştırılacak.

İlk iki doğrudan `main` yazımı GitHub Actions workflow'unu tetiklemeye çalıştı; ancak GitHub job'u billing/spending-limit nedeniyle runner'a hiç başlamadı. Sonraki commitlerde `[skip ci]` kullanıldı.
