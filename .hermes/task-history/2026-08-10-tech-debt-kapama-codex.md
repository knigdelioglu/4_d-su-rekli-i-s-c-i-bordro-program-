# REFACTOR — Teknik borç kalan açıklarının kapatılması (Codex)

**Tarih:** 2026-08-10 · **Tip:** refactor (+bugfix: Decimal::MAX, test onarımı) · **Backend:** codex (gpt-5.6-luna, max reasoning) · **Policy:** WORKSPACE · **Task:** re-20260810-135306-79bd9d · **Süre:** ~42 dk · **Exit:** 0

## Contract özeti
Kullanıcının bağımsız kontrol raporuna (15 maddelik iş emrinin follow-up'ı) dayalı kalan açıklar:
1. `Decimal::MAX` son vergi dilimi limiti SQLite'a sığmıyor → temiz native kurulumda yükleme/bordro bozuluyor; native smoke test kırık.
2. Migration idempotency (eski DB'lerde devir kolonları tekrar ekleniyor).
3. 11 başarısız Rust domain testi — fixture'lar yeni kurum ayarı/yıllık parametre sözleşmesine uyarlanmamış.
4. Backend doğrulama: rapor/dönem tarihleri, son vergi dilimi sınırı, bilinmeyen puantaj kodları (sessiz varsayılan → hata).
5. Dönem + kurum ayarı iki ayrı transaction → tek transaction.
6. N+1/full scan (payroll_service.rs:28) → toplu/hedefli sorgular.
7. V1 import'ta yıllık parametreler boş kalıyor → varsayılan doldurma.
8. Clippy 7 hata + `cargo fmt --check` kırık.
9. Kullanılmayan bağımlılıklar (@google/genai, dotenv, express, motion) + CI eksikliği.
10. bun doğrulaması (tam yol `/Users/kadir/.bun/bin/bun`).

Kısıtlar: git commit/reset/checkout yasak (HEAD 380d835 sabit tutuldu), INVARIANTS.md dokunulmazları (GÇ/GÇT, PEK, iş primi, devreden PEK, tax opening, FINALIZED, yuvarlama), kullanıcı veri dizinleri yasak, test silme/validation zayıflatma yok. Bu görevde ara test çalıştırma serbestti (kırık testlerin onarımı görevin parçası).

## Routing gerekçesi
FAST PATH: kullanıcı task type (REFACTOR) + backend (codex) + model (gpt-5.6-luna max) + policy (WORKSPACE) explicit. capabilities.json güncel (codex 0.146.0, last_detected 2026-08-08). WORKSPACE = kullanıcının explicit seçimi; çatışma yok.

## Yapılan (codex raporu + bağımsız doğrulama)
- SQLite-safe açık uçlu vergi tarifesi (Decimal::MAX → SQLite'a sığan temsil + uygulama katmanında açık uç semantiği); native smoke düzeldi.
- Migration idempotency, V1 yıllık parametre doldurma, devir öncelik kuralı (personel devir alanı vs vergi açılışı tablosu — netleştirildi).
- Tarih/puantaj kodu/vergi parametresi/eksik yasal ayar doğrulamaları (sessiz varsayılan → hata).
- Dönem + kurum ayarı tek transaction.
- Payroll full-scan/N+1 → toplu/hedefli sorgular.
- Bağımlılık temizliği + `.github/workflows/ci.yml` + `src-tauri/tests/remaining_debt_tests.rs` (6 yeni regresyon testi).
- NOT: task_runtime.py'de `capture` komutu yok (doc'da var) — skill'e patch atıldı; session id codex banner'ından alındı.

## Doğrulama (Hermes tarafından yeniden çalıştırıldı — 7/7)
- `cargo test`: **69/69** (62 domain + 1 native smoke + 6 yeni) ✅
- `cargo clippy --all-targets --all-features -- -D warnings`: temiz ✅
- `cargo fmt --check`: temiz ✅
- `~/.bun/bin/bun test`: 34/34 (95 expect) ✅
- `~/.bun/bin/bun run lint` (tsc --noEmit): temiz ✅
- `~/.bun/bin/bun run build`: başarılı (yalnız 682KB chunk uyarısı) ✅
- `git diff --check`: temiz ✅ · `bun install --frozen-lockfile`: başarılı (2 paket) ✅
- HEAD `380d835` sabit; commit yapılmadı (karar kullanıcıda).

## Riskler / Sonraki adım
- Vite 682KB bundle uyarısı devam (kod-splitting ayrı iş).
- Gerçek Tauri UI akışı masaüstünde yeniden build + kullanıcı testi gerektirir.
- 45 dosyalık değişiklik uncommitted; kullanıcı isterse commit ayrı adım.
