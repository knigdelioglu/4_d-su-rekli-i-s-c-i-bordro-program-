# Bugfix — Puantaj Girildi rozeti vs. bordro hesaplama (IPC arg uyumsuzluğu)

**Tarih:** 2026-08-08 · **Tip:** bugfix · **Backend:** opencode (STRICT, deepseek-v4-flash)

## Contract özeti
- Kapsam: puantaj save → calculate zincirinde source-of-truth / ID eşleşme hatası.
- Kapsam dışı: PEK, GV, SGK, iş primi, yemek, GÇ/GÇT, rapor, Tediye/TİS, formüller, DB redesign, git commit.
- Edit scope: src/**, src-tauri/** (path-level STRICT); bash/task/webfetch deny.
- Test A-D: save→calculate, reconnect, yanlış period_id, rozet semantiği.

## Routing gerekçesi
FAST PATH: kullanıcı task type + backend (opencode) + model (deepseek v4 flash) + policy (STRICT) explicit;
capabilities.json güncel (opencode 1.18.15, strict_direct=true, last_detected 2026-08-08). Detection/routing araştırması atlandı.

## Kök neden
Frontend `tauriBridge.ts` invoke arg anahtarları (camelCase: personnelId/periodId) Rust komut
parametreleriyle (snake_case: personnel_id/period_id) eşleşmiyordu → Tauri v2 deserialization hatası,
hesaplama DB'ye hiç ulaşmadan düşüyordu. Save tarafı tek kelimelik `attendance` anahtarıyla çalıştığı
için rozet (SQLite readback'ten) görünüyor, hesaplama (aynı anahtarla aramayıp invoke'da patlayarak) 0 kayıt dönüyordu.

## Düzeltme
- `src/services/tauriBridge.ts`: calculate_payroll → {personnel_id, period_id}; set_payroll_status,
  save_tax_opening, migrate_legacy_payload anahtarları Rust parametre adlarıyla birebir eşleştirildi. Formüller/rust servisleri değişmedi.

## Testler
- domain_tests.rs: Test A/B/C/D (in-memory + geçici disk DB; kullanıcı verisine dokunmaz) — 61/61 geçti.
- tauriBridge.test.ts (yeni): IPC anahtar eşleşmesi — 6/6 geçti.
- `bun test src/`: 34 pass / 0 fail · `bun run build`: OK · `bun run lint`: 1 önceden var olan hata (src/utils/isPrimeGroup.test.ts:38, bu görevle ilgisiz).

## Konvansiyon notu
Tauri v2 invoke arg anahtarları Rust parametre adlarıyla birebir eşleşmeli (snake_case).
Yeni komut eklerken tauriBridge.ts'te anahtar adlarını Rust imzasıyla aynı yaz.
