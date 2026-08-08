# Task: Rapor Olayı Yıl Sınırı Düzeltmesi (31 Aralık-1 Ocak)

- **Tarih:** 2026-08-08
- **Tip:** bugfix
- **Routing:** agy (Antigravity 1.1.11) + WORKSPACE policy, model `Gemini 3.6 Flash (High)` - kullanicinin acik secimi; FAST PATH (task type/backend/model/policy explicit, capabilities taze).
- **Hata:** `SickLeaveService` icinde `start.year() == period.yil` filtresi - yil asan olayin (orn. 31.12.2026-03.01.2027) ikinci ucretli gunu (01.01.2027) hedef donem hesabinda kayboluyor; ters yonde 01.01.2027 baslangicli olay Aralik donemine dusen ilk 2 gununu kaybediyordu.
- **Cozum:** `calculate_paid_sick_days_from_records` yeniden yazildi - kayitlar `start_date.year()` ile gruplanir, yil ici kronolojik siraya gore ilk-5 belirlenir; olay seviyesinde global ucretli gunler `{start, start+1}` (end ile sinirli) hesaplanir, donem kesisimi (`baslangicTarihi`..=`bitisTarihi`) sayilir.
- **Testler:** Test A-E (29 Aralik / 31 Aralik / 1 Ocak / 6. rapor / 1 gunluk) + yil asan persistence/FINALIZED reload testi eklendi (domain_tests.rs, +6 test). Mevcut 15-14 regression (`test_h_...`) ve rapor persistence testi korundu.
- **Sonuc:** cargo 57+1 smoke passed; bun 28/0; build 3.10s; lint yalniz baseline (`isPrimeGroup.test.ts`).
- **Gozlem:** `sick_leave_records` tablosunda tarih cakismasi (overlap) kistiti yok - repo/service cakisan rapor kaydina izin veriyor; kapsam geregi duzeltilmedi, kullaniciya raporlandi.
- **Degisen dosyalar:** `src-tauri/src/services/sick_leave_service.rs` (mantik), `src-tauri/tests/domain_tests.rs` (+6 test).
