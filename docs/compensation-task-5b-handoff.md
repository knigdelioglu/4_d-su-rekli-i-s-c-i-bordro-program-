# Task 5B handoff — explicit normal compensation path

## Delivery boundary

Task 5B now has a real opt-in NORMAL calculation path through native SQLite and browser/WASM, including persisted schema-2 provenance and provenance-selected finalization. Ordinary calculation remains on the existing legacy path; this handoff does not authorize production activation.

## Requirement and evidence map

| Requirement | Owner and data flow | Success and failure behavior | Evidence |
|---|---|---|---|
| Native policy calculation uses current policy data without the legacy compensation loader | `PayrollService::calculate_payroll_for_accrual_with_compensation_policy_checked` builds `build_compensation_policy_calculation_snapshot_for`, loads the target policy versions, and calls the checked core entry point inside a SQLite transaction. The snapshot contains target attendance, person payroll/payment history, referenced periods, required minimum-GV statutory source months, current tax opening/year parameters, sick-leave history, and the person’s retro batches/allocations. It leaves `zamAylari` empty and loads institution settings only for the active period and required statutory source months. | Missing target statutory data, policy baseline, annual parameters, attendance, or required tax history remains a calculation error. | `compensation_policy_entry_test`: actual SQLite calculation versus core oracle; asserts the old period settings and malformed `zam_aylari` are not loaded. |
| Schema-2 payroll finalizes with its producing engine | Native `finalize_payroll_for_accrual` reads the saved provenance and selects the explicit checked core finalizer for current-engine rows. Browser `finalizePayroll` uses the same persisted provenance to select the new WASM finalizer. The core finalizer requires a valid current NORMAL schema-2 snapshot and retains existing status, prior-event, attendance, and downstream-finalized guards. | Invalid/missing current metadata fails closed. Only metadata-free legacy rows use the legacy finalizer. Persistence and downstream invalidation remain in the caller’s transaction. | Native round-trip recalculates, reads persisted metadata, changes policy, recalculates STALE, then finalizes and compares financial values and outgoing PEK carry to the core oracle. A forced downstream invalidation write failure proves the transaction restores both the source CALCULATED row and downstream CALCULATED row. WASM checks explicit finalization parity, rejects absent provenance, and matches the core’s downstream FINALIZED blocker. |
| Policy edits use compensation-specific dependency scope | Shared `payroll-core` `COMPENSATION_POLICY` mutation roots only NORMAL rows whose service period intersects the inclusive effective policy window. It follows later payment-event order and the existing GV/PEK/carry rules, returns source-period causes, and detects affected retro allocations. Native preview/save and browser preview use this same mutation once per policy window. | FINALIZED payrolls block. Any affected retro batch fails closed with `UNSUPPORTED_HISTORICAL_REPLAY`; frozen Task 5A has no retro-state token. Successful save changes affected CALCULATED rows to STALE in the same native transaction or browser snapshot revision. | Core mutation regression covers exact roots, a later dependent payment, an earlier FINALIZED supplemental event, the carry cutoff, causal periods, and retro allocation scope. Native preview/save confirms only the calculated NORMAL root is invalidated. Chromium IndexedDB test confirms preview closure and CALCULATED→STALE commit. |

## Deliberate limits

- Retro replay and retro-state tokenization remain deferred until the serial Task 6/orchestrator contract review. A policy edit reaching retro history does not append a policy version or mutate a retro ledger.
- The 2027 date cases use a synthetic schema-complete annual parameter fixture to test 15–14 period handling. It is not an official 2027 tariff or a legal calculation input.
- No live application database or payroll record was changed. The entry-path tests use in-memory SQLite and browser IndexedDB test state.

## Verification

| Command | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --test compensation_policy_api_test --test compensation_policy_entry_test -- --test-threads=1` | PASS — 5 tests; includes transactional policy API, native schema-2 calculate/finalize round-trips, injected finalization/invalidation rollback, and 2027 synthetic date cases |
| `cargo test -p payroll-core --test policies_mutation_regression -- --test-threads=1` | PASS — 9 tests |
| `bun run wasm:test` | PASS — 9 WASM tests, including schema-2 explicit finalization parity, provenance rejection, and downstream FINALIZED blocker parity |
| `bun test src/services/compensationPolicy/api.browser.test.ts` | PASS — Chromium/IndexedDB; 1 test, 11 assertions, including a real calculated payroll invalidated to STALE |
| `bun run wasm:build` | PASS — regenerated JS/WASM package and source hash |
| `bun run verify:wasm-freshness && bun run verify:wasm-package` | PASS |
| `bun run build` | PASS — Vite production build; existing large-chunk advisory remains |
| `bunx tsc --noEmit --pretty false` | EXIT 2 — 66 repository diagnostics; no diagnostics in the changed Task 5B TypeScript entry/API/test-harness files; count matches the existing Task 7 baseline |
