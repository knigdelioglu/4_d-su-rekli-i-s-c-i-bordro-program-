# 4/D Bordro — 15–14 Dönemi → Vergi/Ödeme Ayı Eşlemesi Salt-Okuma Denetimi

Son yapılan asgari ücret GV istisnası düzeltmesinde, 15–14 bordro döneminin doğru vergi ayına bağlandığını bağımsız olarak denetle.

Kesin kurallar:
- Kod değiştirme.
- Test/build çalıştırma.
- cargo test, bun test, tsc, vite, tauri build/dev çalıştırma.
- Git işlemi yapma.
- Yalnız gerçek mevcut kod yolunu oku.
- Mevzuat veya yeni bordro kuralı üretme.
- Geliştirici raporuna güvenme; implementation'ı takip et.

## İncelenecek dosyalar

- src-tauri/src/domain/models.rs
- src-tauri/src/domain/calculations.rs
- src-tauri/src/services/payroll_service.rs
- src-tauri/src/services/cumulative_tax_service.rs
- src-tauri/src/repositories/payroll_repo.rs
- src-tauri/src/repositories/settings_repo.rs
- src/types/payroll.ts
- src/utils/payrollUtils.ts
- src/components/PeriodManagerModal.tsx
- src/components/BordroHesaplama.tsx
- src/components/PaySlipModal.tsx

PayrollPeriod / Donem modelinin tanımlandığı başka dosyaları da kendin bul.

# Denetlenecek ana soru

15–14 bordro döneminde asgari ücret GV istisnasının hangi takvim ayına göre seçildiğini belirle.

Örneğin dönem:

15 Haziran 2026 – 14 Temmuz 2026

ise sistem bu bordroyu hangi vergi ayına bağlıyor?

Eğer bu dönem kurumda "Temmuz 2026 bordrosu" ise beklenen:

tax_year = 2026
tax_month = 7

ve asgari ücret GV istisnası:

Temmuz 2026 → 4.537,75 TL

olmalı.

Şunlardan herhangi biriyle yanlışlıkla Haziran seçilmemeli:

- period.start_date.month()
- 15 Haziran tarihinin ayı
- puantajın ilk gününün ayı

# DENETİM A — PayrollPeriod authoritative ay alanı

PayrollPeriod/Donem modelini incele.

Şunları açıkça yaz:

- dönem başlangıç tarihi
- dönem bitiş tarihi
- yıl
- ay
- varsa ödeme ayı / bordro ayı / vergi ayı alanı

Asgari ücret GV istisnasında authoritative olarak hangi alan kullanılıyor?

# DENETİM B — Asgari ücret referans kümülatifi

Asgari ücret referans kümülatifini oluşturan fonksiyonu gerçek çağrı zincirinden takip et.

Şu formül hangi `month` değeriyle çalışıyor?

asgari_referans_onceki = aylık_asgari_gv_matrahi × (month - 1)
asgari_referans_cari = aylık_asgari_gv_matrahi × month

`month` değeri:

- bordro/vergi ayından mı,
- period başlangıç ayından mı,
- period bitiş ayından mı

geliyor?

Dosya + fonksiyon + satır ver.

# DENETİM C — 15 Haziran–14 Temmuz örneği

Gerçek kodu elle takip et:

Dönem: 15.06.2026 – 14.07.2026

Eğer period kaydı Temmuz 2026 olarak tanımlıysa hesap motorunun kullandığı:

tax_year
tax_month
asgariUcretReferansKumulatifMatrahi
asgariUcretGvIstisnasi

değerlerini çıkar.

Beklenen:

tax_year = 2026
tax_month = 7

Aylık asgari GV matrahı: 28.075,50 TL

Temmuz cari referans kümülatifi: 28.075,50 × 7 = 196.528,50 TL

Temmuz GV istisnası: 4.537,75 TL

Haziran istisnası olan 4.211,33 TL uygulanıyorsa FAIL ver.

# DENETİM D — Ay sınırı günlük bölünüyor mu?

15–14 dönemindeki günlerin:

15–30 Haziran
1–14 Temmuz

şeklinde ayrılıp iki farklı GV istisnası hesaplanmadığını doğrula.

Asgari ücret GV istisnası tek bordro/vergi ayına ait tek istisna olmalı.

Günlük prorata veya iki aylık istisna toplamı üretilmemeli.

# DENETİM E — UI dönem oluşturma

PeriodManagerModal ve ilgili dönem oluşturma kodunu incele.

Kullanıcı:

Temmuz 2026

dönemi oluşturduğunda tarih aralığı:

15 Haziran – 14 Temmuz

ise DB/model içinde:

yil = 2026
ay = 7

olarak mı saklanıyor?

Eğer ay otomatik olarak `start_date.month()` üzerinden 6 oluyorsa FAIL.

# DENETİM F — Rust ve TS fallback tutarlılığı

Rust authoritative hesap ile TS fallback aynı vergi ayını mı kullanıyor?

Şu tehlikeyi kontrol et:

Rust → period.ay
TS → new Date(period.baslangic).getMonth()

gibi iki farklı davranış olmasın.

Native'de Rust authoritative olsa da semantic divergence varsa PARTIAL ver.

# DENETİM G — Snapshot

Bordro hesaplandıktan sonra kullanılan:

tax_year / tax_month
asgari ücret referans kümülatifi
asgari ücret GV istisnası

sonradan dönem tarihi/ayar değişikliğinden etkilenmeyecek şekilde snapshot'ta korunuyor mu?

FINALIZED bordro için özellikle kontrol et.

# Sonuç

Yalnız şu formatta raporla:

DENETİM A — PASS / FAIL / PARTIAL
DENETİM B — PASS / FAIL / PARTIAL
...
DENETİM G — PASS / FAIL / PARTIAL

Her FAIL/PARTIAL için:
- dosya
- fonksiyon
- satır
- mevcut davranış
- beklenen davranış

En sonda şu örneği tek tabloda ver:

15.06.2026–14.07.2026 dönemi
Bordro/vergi ayı:
Kullanılan tax_month:
Asgari referans kümülatifi:
Uygulanan GV istisnası:

Son karar:

15–14 → vergi ayı eşlemesi DOĞRU / YANLIŞ / KISMEN DOĞRU
