# PROJECT.md — 4/D Sürekli İşçi Bordro Programı

## Amaç
4/D (sürekli işçi) bordro programı. SQLite + Rust (Tauri) + React/TS (Vite) mimarisi.
**Rust authoritative bordro motorudur** — ücret hesapları Rust tarafında yapılır, TS yalnızca UI/sunum.

## Stack & komutlar
- Frontend: React 19 + Vite 6 + Tailwind 4 + TypeScript (bun)
- Backend: Rust + Tauri v2 + SQLite (rusqlite)
- Build/test/lint:
  - `bun run lint` — tsc --noEmit
  - `bun run build` — vite build
  - Rust: `cargo test` (src-tauri altında; tests/domain_tests.rs, tests/native_smoke_test.rs)
  - TS unit: src/utils/*.test.ts (bun test)

## Kritik domain alanları (mevcut)
- Asgari ücret GV istisnası, kümülatif GV matrahı (personel gerçek + asgari ücret referansı ayrımı — görev 2026-08-07)
- SGK/PEK, damga vergisi, iş primi, GÇ/GÇT — **bu görev kapsamı dışında, dokunulmayacak**

## Git notları
- main dalı, origin/main'den 1 commit önde (266a909 "iş primi")
- Kullanıcı değişiklikleri: .gitignore'a `.aider*` eklendi (korunacak)
- Commit/push/reset/restore yasak (orchestrator kuralı)
