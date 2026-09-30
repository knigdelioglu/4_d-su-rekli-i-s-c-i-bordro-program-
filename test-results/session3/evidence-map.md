# Oturum 3 aktif kanıt haritası

- Başlangıç: commit c518317; kirli ağaç Oturum1/2 değişiklikleri. Oturum sırasında dış işlem commit 211cb19 (canlı testler) oluşturdu; orchestrator commit/push yapmadı.
- İlk binary: /tmp/4D Bordro Session 1.app, com.bordro.session1.validation; ilk executable SHA256 1bdf6399cedbcc0b75fff5d55d1a6fcf090b8402f3bc5c37e71b2e5e68276e5b.
- Referans initial.json: UI JSON export. 7 personel,17 dönem,84 attendance,117 payroll (110 calculated,6 stale,1 finalized),3 sick leave,2 annual parameters,10 revisions,17 retro batches,58 allocations;0 tax opening.
- Preview: p1, daily wage2643.28, effective 2026-09-15..2026-11-14, payment2026-12-14. 2026-09 delta3198 +2026-10 delta3298;2026-11 zero. Finalized exact; source payroll entire arrays unchanged. Draft revision/override persistence only.
- Apply: new event retro-823c0460-a659-4d59-8bb3-6c1ab399252f sequence4;gross6496,deductions2708.53,net3787.47;GV before489682.24,current5521.6,after495203.84;sourcePEKdelta6100,paymentPEK396;carry empty.11 later payroll status+updated timestamp changes, source monetary values unchanged;finalized exact.
- Same revision repeat preview gross0 and apply disabled. Native quit process absence checked; reopen domain snapshot exact except exportedAt.
- reference.json UI export post-retro includes118 payroll,18 batch. parse fails CALCULATED batch+STALE matching payment lifecycle. UI import original attempts did not reach import: native window.confirm silent false. These attempts are NOT restore proof.
- UI sample reset on original FINALIZED state twice rejects;[object Object] reason. Domain exact preserved. Product ALL finalized guard intentional.
- Isolated DB setup: original DB preserved at /tmp/session3-preserved-native.sqlite (closed process). New empty DB, UI sample reset→5 persons,9 periods,45 attendance,0 payroll;quit/reopen defaults persisted. Original finalized cannot be cleared by supported native UI.
- Atomic baseline: UI calculate5 NORMAL, then JSON export atomic-reference.json;SQLite exact dump atomic-baseline.sql (fb6a365a67002ff130fcdd8b485246feb0dede834b97c594ad3660ab412a4782).
- Native Blob/anchor JSON download can deadlock WebKit main thread even unique /Applications bundle; repeated on build8543feb5. Earlier relocation only temporarily worked. Export correction delegated separately; file export native save dialog replaces Blob route.

## Coding chat ownership
- 01a0f1d0-b3b1-7f91-8570-7a1d2bf49b61 core retro period/source selection. Reviewed; overbroad revision-overlap filter rejected, downstream3-period replay retained.56 retro_regression PASS,2 Bun preview/recovery PASS.
- 01a0f1d5-f529-7c93-8306-e9e3862e17a7 native restore confirmation/error feedback; App/TopBar/DataBackupPage/controller/helper. In progress.
- 01a0f1db-8fa6-75f0-9ac8-3b77e9e0e429 storage lifecycle validation. Global STALE dropping rejected; keep strict source/context validation.
- 01a0f1ea-5162-79c3-9fce-507a2815fcfb Decimal lexical mismatch and optional historical snapshot backward compatibility; native replay validation. In progress.

## Pending
- New modal build→all live negative fixtures with exact SQLite preservation proof.
- Valid reference restore exact comparison, reopen, finalized-source retro apply, negative retro signed settlement.
- Unknown backup version and extra field native behavior.
- Final build metadata/report. No bank/SGK/export/print/web-native/general fuzz scope.

## Build3 live results
- 01..08,10 negative fixtures rejected with visible errors on modal build; full SQLite state preserved.
- Build8543feb5: coherent forged income fixture11 rejected by canonical Rust replay (p5 income1 vs0), twice; entire SQL identical before/after and after quit/reopen.
- valid.json succeeds twice;5 payroll sonGuncellemeTarihi changed at restore (timestamp-repro.json). Restore audit timestamp correction delegated.
- extra fields fixture09 accepted; unknown sentinels absent in native SQLite; README preservation claim specifically browser snapshot.
- UI reset after5 calculated payroll succeeded→5 personnel,9 periods,45 attendance,0 payroll/retro/sick;quit/process absence/reopen verified no old payroll returned.
- Migration targeted integration17/17 PASS;Bun confirmed import/controller/bridge/storage78/78 PASS;Decimal scale unit1PASS.
- Native export chat01a0f1fe-7beb-7212-829b-29eea0a36263;timestamp chat01a0f1ff-d92c-76e3-ad91-ea86e80a4b84.
