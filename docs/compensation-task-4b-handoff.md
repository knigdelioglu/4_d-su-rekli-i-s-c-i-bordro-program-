# Görev 4B — Kalıcı compensation boundary modeli ve parity handoff

## Requirement → owner → flow → success/failure → proof

| Gereksinim | Owner | Veri akışı | Başarı / hata davranışı | Kanıt |
|---|---|---|---|---|
| İki zorunlu boundary enum'u ve scoped resolver invariant'ı | `payroll-core` domain modeli/resolver | Immutable policy version → aktif supersession leaf'leri → döneme kırpılmış resolved segmentler | Canonical enum'lar kayıpsız taşınır; dönem içi çelişkili `ROAD_ONLY`, eksik/invalid metadata veya karma road mode fail-closed; dönem başlangıç baseline'ı `ROAD_ONLY` olabilir | Core policy tests + final persisted-model shadow parity |
| Snapshot schema 2 ve historical provenance | `payroll_snapshot.rs` | Bordro snapshot segmentleri → parent service period validator'ı | Contiguous dates, state ve mode invariant'ları doğrulanır; schema 1/flag'siz experimental current-engine snapshot unsupported kalır; `None` metadata legacy olarak kalır | Core snapshot tests + shared fixture |
| Immutable SQLite policy/snapshot persistence | migrations + policy/payroll repositories | SQL columns/JSON → typed domain values; restore transaction → append-only policy rows | Migration 23 eski satırları `NULL` bırakır; seçilen eksik satır kimliğiyle quarantine hatası verir, dönem kapsamı dışındaki eksik satır projection'ı durdurmaz. Tam geçmiş audit/export ve full-timeline correction eksik satır gördüğünde fail-closed olur. Yeni insert exact flags ister; ID conflict/immutability rollback olur | Fresh/upgrade, scoped read, incomplete-history audit/correction/export, immutability, CALCULATED/FINALIZED restore tests |
| V6, WASM ve browser boundary parity | Rust/TS generated contract, V6 codecs, Decimal adapters | JSON/SQLite/IndexedDB → exact Decimal → strict structural validation | Missing/unknown flags, schema 1, state/date/ID mismatch reddedilir; V5 legacy thresholds ve empty/null adapters korunur | Bun storage/backup/Decimal tests + WASM shared fixture validation |
| Rounding davranışı ve fixture parity | Test-only shadow seam | Final `ResolvedCompensationSegment` → earning slices + dated full-state road amounts → legacy oracle | `FULL_EARNING` tüm earnings'i böler; `ROAD_ONLY` yalnız yol tutarını değiştirir; `PER_COMPENSATION_SLICE`/`PERIOD_TOTAL` yol sonucu aynıdır | 20 shadow full-record equality tests + N≥3 goldens + 38 payroll/14 raise regressions |

## Sınırlar

Eski bayraksız policy satırlarına default değer atanmaz, satırlar silinmez ve finansal değer tahmini/backfill yapılmaz. Böyle bir satır gerekli tarihsel zincirdeyse tam audit/export/correction işlemi de karantinaya girer; yalnızca dönem izdüşümünün kapsamı dışındaki satır seçilmemiş kalabilir. V1–V5 ve `None` payroll metadata legacy uyumluluğunu korur; current-engine snapshot schema 1 veya flagsiz kayıt kabul edilmez.

Bu handoff domain, snapshot, SQLite, native/WASM/browser persistence boundary ve final-model parity kapsamındadır. Normal payroll loader/API/invalidation Task 5, Settings UI Task 7, retro Task 6 ve canonical final replay Task 8 sahipliğindedir. Normal üretici bu görevde current-engine metadata üretmez.
