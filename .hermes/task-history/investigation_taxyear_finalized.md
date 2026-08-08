# INVESTIGATION CONTRACT — taxYear/taxMonth Yıl Geçişi + FINALIZED Metadata Koruması (salt-okuma denetim)

- Tarih: 2026-08-08
- Tür: investigation (denetim A–M), KOD DEĞİŞİKLİĞİ YOK
- Backend: opencode 1.18.15 (`~/.opencode/bin`), model `opencode/deepseek-v4-flash-free`
- Policy: STRICT — permission config `/tmp/opencode_strict_audit.json`: read/glob/grep/list yalnız repo kökü (NFD-safe, shell'den alındı), edit/bash/task/external_directory tamamen deny
- Routing gerekçesi: kullanıcı explicit (opencode + deepseek v4 flash free + STRICT) → FAST PATH. capabilities.json güncel (last_detected 2026-08-08, version 1.18.15 eşleşiyor, strict_direct=true). Routing araştırması yapılmadı.
- Kullanıcı iş emri + contract: `/tmp/4d_audit_taskfile.md` (göreli yol zorunluluğu, permission-reddi devam talimatı, STATUS formatı eklendi)
- Run: `opencode run --model opencode/deepseek-v4-flash-free --agent build` (bash kapalı → test/build/git fiziksel olarak imkânsız), log `/tmp/4d_audit_run.log`

## Denetim kapsamı (özet)

- A: taxYear/taxMonth domain anlamı, `ay`'ın vergi hesabında kullanılmaya devam eden kritik yolu var mı
- B: 15.12.2026–14.01.2027 → taxYear=2027/taxMonth=1 varsayılan üretimi
- C: Migration backfill (yıl geçişli eski kayıt: 2026/12 + 2027/1)
- D: UI yeni dönem default'u + manuel override
- E: get_previous_cumulative_gv taxYear/taxMonth filtresi; personnel_tax_opening 2027 kullanımı
- F: Asgari ücret referans kümülatifi yıl sıfırlaması
- G: taxYear/taxMonth sonradan değiştirilebilirliği (tespit)
- H: FINALIZED varken metadata değişikliği → sonraki kümülatif/ref zinciri etkisi
- I: Koruma modeli A veya B var mı (yoksa FAIL)
- J: gv_snapshot_json geri dönüşü (eski bordro görüntüsü)
- K: Rust/TS semantik uyumu (taxYear/taxMonth divergence)
- L: 3 senaryo tablosu
- M: PEK yıl geçişi bulgusu AYRI (bu denetimin sonucuna dahil değil)

## Sonuç## Sonuç (2026-08-08, opencode deepseek-v4-flash-free, salt-okuma)

- DENETİM A — PARTIAL (gerçek kümülatif GV hâlâ yil/ay; asgari ref taxYear/taxMonth)
- DENETİM B — PASS (15.12.2026–14.01.2027 → 2027/1; "2026/1" hiçbir yolda üretilmiyor)
- DENETİM C — PASS (M5 backfill: ay=12 → tax_year=yil+1, tax_month=1)
- DENETİM D — PASS (UI default 2027/Ocak + manuel override mevcut)
- DENETİM E — FAIL (get_previous_cumulative_gv yil/ay filtresi; 2026 kümülatifi 2027 dönemine taşınıyor; opening de period.yil ile)
- DENETİM F — PASS (asgari ref taxYear/taxMonth; yıl başında 0'dan)
- DENETİM G — PASS (tespit: API'de save_period ON CONFLICT DO UPDATE, guard yok)
- DENETİM H — FAIL (kritik: FINALIZED kontrolü yok; metadata değişikliği sonraki asgari ref zincirini bozar)
- DENETİM I — FAIL (koruma A yok, B yok; öneri: save_period'ta FINALIZED varken tax değişikliğini reddet)
- DENETİM J — PASS (snapshot geri dönüyor; oncekiKumulatifAsgariGvMatrahi kolonu yok — minör)
- DENETİM K — PASS (Rust/TS tutarlı; TS legacy fallback yalnız eski kayıtlarda)
- DENETİM L — S1: 2026/7 ✔; S2: 2027/1 ✔ (gerçek kümülatif 2026'ya devam — bkz. E); S3: manuel override ✔
- DENETİM M — N/A (PEK ayrı tutuldu, PASS/FAIL'e dahil edilmedi)

Nihai kararlar:
1. YIL GEÇİŞİ (15.12.2026–14.01.2027 → taxYear=2027/taxMonth=1): **DOĞRU**
2. FINALIZED METADATA KORUMASI: **GÜVENSİZ**
Toplam PASS: 8 (B,C,D,F,G,J,K,L) • PARTIAL: 1 (A) • FAIL: 3 (E,H,I) • N/A: 1 (M)

Doğrulama: git temiz (HEAD 4bb747b, yalnız bu kayıt untracked); edit/bash/task deny → kod değişimi fiziksel olarak imkânsızdı. Tam rapor: ~/Downloads/4d_denetim_raporu.md
