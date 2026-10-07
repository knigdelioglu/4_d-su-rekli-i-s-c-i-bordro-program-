# Lint/CI handoff — 2026-10-07

## Kök neden ve düzeltme

Başlangıçtaki `bun run lint` 66 TypeScript tanısı verdi: 64 eksik `bun:test`
matcher tanımı, `PuantajGrid` klavye handler'ında typesız React event'i ve
güncel `PayrollStorageDto` içine eski `zamAylari` alanını ekleyen V6 test
fixture'ı. Yerel matcher shim'i kullanılan assertion'lar için tipli hale
getirildi; async `resolves`/`rejects`, `.not`, nesne alt-kümeleri ve
collection/string `toContain` desteği eklendi. `PuantajGrid` handler'ı
`React.KeyboardEvent<HTMLDivElement>` olarak açıkça tiplendi. `zamAylari`,
V6 fixture'ından çıkarılıp yalnız eski V1–V5 test girdileri üretilirken eklendi;
test beklentileri değişmedi.

Puantaj toplu uygulama onay penceresi için browser regression testi eklendi:
Tab ve Shift+Tab son/ilk düğmeler arasında odağı döndürüyor, Escape pencereyi
kapatıyor. Test izole Playwright browser context'inde çalıştı. Vite preview için
localhost bind'i sandbox ilk denemede `EPERM` ile engelledi; izinli escalation
ile yeniden çalıştırıldığında geçti.

## Kanıt

- `bun run lint` — PASS (`tsc --noEmit` ve `verify:production-graph`).
- `bun test` — PASS, 374 test; 0 başarısız, 1693 `expect()` çağrısı.
- `bun run test:e2e:typecheck` — PASS.
- `bun run test:e2e -- e2e/payroll.browser.playwright.ts --grep "bulk attendance confirmation traps keyboard focus and closes with Escape"` — PASS, 1 test.
- `bun run build` — PASS. Vite mevcut 500 kB chunk boyutu uyarısını verdi; build'i engellemedi.
- `bun run verify:payroll-contract` — PASS.
- `bun run verify:wasm-freshness` — PASS.
- `git diff --check` — PASS.

`package.json`, `bun.lock`, `tsconfig.json` ve `.github/workflows/ci.yml`
değişmedi; dependency/lock etkisi yok. Rust/WASM kaynakları değiştirilmedi.
