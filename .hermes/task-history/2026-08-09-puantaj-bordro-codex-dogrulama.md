# Bugfix — Puantaj→Bordro "puantaj yok" hatası (Codex doğrulama turu)

**Tarih:** 2026-08-09 · **Tip:** bugfix/verify · **Backend:** codex (gpt-5.6-luna, high reasoning) · **Policy:** WORKSPACE

## Contract özeti
- Kapsam: Puantaj Cetveli kaydı sonrası "tüm personel için otomatik bordro" akışı "Hiçbir personelin kayıtlı puantajı bulunamadı..." uyarısı veriyor; kök nedeni bul ve çöz.
- Kapsam dışı: git commit, schema migration, kullanıcı verileri, ilgisiz refactor.
- Kullanıcının mevcut değişiklikleri (3 modified + 3 untracked) korunacak.

## Routing gerekçesi
FAST PATH: kullanıcı task type (bugfix) + backend (codex) + model (gpt-5.6-luna high) explicit; policy "workspace" olarak alındı. capabilities.json güncel (codex 0.146.0, last_detected 2026-08-08). Codex strict_direct=false → STRICT sağlayamaz; belgeli çatışma kararıyla WORKSPACE (workspace-write sandbox) — kullanıcının "workspace çalışsın" ifadesiyle uyumlu.

## Bulgu
Kök neden **2026-08-08 15:07'de çalışma ağacında zaten düzeltilmişti** (önceki opencode oturumu, bkz. bugfix_puantaj_bordro_ipc.md): tauriBridge.ts invoke arg anahtarları camelCase (personnelId/periodId) iken Rust snake_case (personnel_id/period_id) bekliyordu → Tauri v2 deserialization hatası → hesaplama "puantaj yok" mesajına düşüyordu. Düzeltme HEAD'de YOK (a181b59'da bug mevcut), çalışma ağacında uncommitted duruyor.

## Bu turda yapılan
- Codex aynı kök nedeni bağımsız buldu, mevcut düzeltmeyi runtime testleriyle doğruladı (save→readback→calculate zinciri).
- Codex'in tek kod izi: `src/bun-test.d.ts` (+12 satır, .not tip tanımları — lint için).
- tauriBridge.ts / domain_tests.rs / tauriBridge.test.ts / task-history dosyaları önceki oturumdan, dokunulmadı (mtime ile doğrulandı).

## Doğrulama (ajan raporu)
bun run lint / bun run build / bun test (34) / cargo test (61+1) ✅ · puantaj save→calculate senaryosu ✅ · puantajsız dönem hâlâ hata veriyor ✅ · dev server sandbox'ta EPERM (listen) → gerçek UI akışı çalıştırılamadı.

## Riskler / Sonraki adım
- Gerçek Tauri UI doğrulanmadı; kullanıcı yeniden build/restart edip test etmeli (stale build ihtimali yüksek — düzeltme 8 Ağu 15:07'den beri kaynakta).
- Hata yeniden ürerse "tüm personel" toplu akışının farklı bir invoke/okuma yolu olabilir → yeni tur.
- Düzeltme uncommitted; kullanıcı commit etmek isterse ayrı adım.
