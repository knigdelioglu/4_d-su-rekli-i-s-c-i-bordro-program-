# Task 6 handoff — persisted retro replay and policy invalidation

## Minimum Task 5A API revision (design before implementation)

Task 5A's component and API remain additive: their existing operation, timeline,
payroll effects, and normal payroll token fields are unchanged. Task 6 adds only
retro dependency state needed to make `COMPENSATION_POLICY` mutation safe.

- `CompensationPolicyPreviewToken.retroDependencyStates` is a required,
  canonical-order collection for every affected retro batch. Each entry carries
  the exact persisted batch state, every linked allocation, its revision and
  overrides, source NORMAL payroll identity/provenance, and any linked retro
  payment event state. Save re-reads and compares this complete structured
  token, then re-evaluates the shared pure closure inside the existing native
  SQLite transaction or browser IndexedDB compare-and-swap.
- Preview exposes typed `retroEffects` separately from normal payroll closure
  effects. `CompensationPolicySaveReceipt.invalidatedRetroBatches` reports the
  batch IDs actually changed to `STALE`, and
  `invalidatedRetroPaymentEvents` reports linked calculated payment rows that
  actually changed to `STALE`.
- A retro effect is eligible only when the batch is `CALCULATED`, its settlement
  is `UNSETTLED`, every source NORMAL is `CALCULATED`, and any linked payment
  event is still `CALCULATED` and unpaid. Existing `STALE` batches are outside
  the closure. `FINALIZED`/paid sources, finalized batches, settled/overpayment
  batches, or finalized/paid payment events block the operation with named
  retro blockers. No new batch lifecycle status is introduced.
- On success, the policy append, normal payroll stale changes, eligible retro
  batch stale changes, and eligible linked retro payment row stale changes are
  one atomic mutation. Existing `STALE` semantics represent supersession; old
  allocation rows remain auditable and are not rewritten or deleted.

The token consists of persisted identities and typed state, not a synthetic
revision counter. Missing batch, allocation, revision, override, source record,
or linked-payment provenance cannot be represented as a complete token and
fails closed. No V6/snapshot schema, domain full-state, road flags, or financial
input boundary changes are proposed.

## Requirement → owner → flow → success/failure → proof

| Requirement | Owner / flow | Success and failure behavior | Proof |
|---|---|---|---|
| Historical NORMAL right | `retro.rs`: authoritative source record → validated persisted schema-2 snapshot → replay original entitlement → apply ordered revision state → allocations/SGK/tax/carry/settlement | Saved snapshot state and used inputs remain authoritative after live data changes/deletion; invalid provenance or unsupported legacy replay names the limit | `retro_regression` (67) and `payroll_snapshot` (20) |
| Policy mutation closure | `policies.rs` shared by native and WASM | Actual payment-order closure includes affected mutable retro ledger; immutable retro dependencies are typed blockers | `policies_mutation_regression` (9) and native API parity (9) |
| Atomic native mutation | compensation policy service + retro/payroll repositories | Re-evaluate token and closure in SQLite transaction; rollback policy append and all stale effects on failure | `compensation_policy_api_test` (9) and `compensation_policy_entry_test` (2): real schema-2 calculation/retro payment lifecycle, atomic invalidation, typed blockers, and rollback |
| Atomic browser mutation | `CompensationPolicyApi` and browser payroll store | Re-evaluate against IndexedDB transaction snapshot; stale concurrent revision conflicts, no stale snapshot overwrite | `api.browser.test.ts` (4): WASM-calculated schema-2 NORMAL and retro payment, IndexedDB invalidation, stale cross-tab conflict, and actual transaction abort rollback |
| UI visibility | Existing Task 7 accepted component | Display typed retro effect, blocker, and save receipt; no policy form or Decimal flow refactor | `CompensationPolicyPage.test.tsx` (9) and `compensationPolicyDecimal.test.ts` (11) |

## Implemented behavior

- `MutationImpact.retroEffects` is shared by native Rust and WASM. Each entry
  identifies the batch, source periods, linked payment events, action, and
  blockers. Finalized batches, settled batches, finalized source NORMAL rows,
  and finalized payment events return typed blockers. Missing or inconsistent
  replay provenance fails closed as `UNSUPPORTED_HISTORICAL_REPLAY`.
- A schema-2 source NORMAL is replayed from its saved compensation segments,
  saved person group, attendance, paid-sick inputs, and other snapshot inputs.
  The replayed original income must match the persisted NORMAL income before a
  target is calculated. Source PEK replay validates each historical payment
  event snapshot and starts with the first event's saved incoming carry and
  same-payment-month PEK, then follows persisted payment date/sequence/accrual
  ordering and each event's statutory snapshot.
- Schema-2 replay tests include a three-segment source (including a ROAD_ONLY
  boundary), manual zero/null income, a person-specific wage override over a
  global override, real supplemental/TEDIYE/TİS payment events, positive and
  negative independently checked earning/PEK/premium deltas, CALCULATED and
  FINALIZED NORMALs, and a fail-closed missing statutory-snapshot case.
- Snapshotless V5 history remains a separate legacy path. It captures the
  saved source settings, personnel, attendance, sick-leave records, raise
  periods, and historical period graph in the dependency token. Missing or
  ambiguous legacy inputs fail closed; this does not upgrade old records or
  introduce another snapshot schema.
- Native save rechecks the token and impact in the SQLite transaction. Browser
  save does the same against the IndexedDB compare-and-swap snapshot. Both
  append the policy and stale eligible batches, linked retro payment events,
  and calculated payroll dependencies atomically. Receipts list the IDs that
  were actually changed. Old allocation rows remain available for audit.
- A linked stale retro payment row is ignored by later PEK/GV ordering only
  when its matching batch is also STALE and still UNSETTLED. This retires the
  unpaid audit event after policy invalidation so subsequent payroll
  recalculation and finalization can proceed; ambiguous or settled history
  still fails closed.
- The settings page displays affected retro batch IDs, source periods, linked
  payment event counts, blocker reasons, and the stale batch/event IDs from the
  save receipt. The payload validator accepts a linked STALE payment event
  only when its retro batch is also STALE and the linking and financial fields
  still match.

## Verification

- `cargo test -p payroll-core --test payroll_engine_regression --test retro_regression` — 105 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --test compensation_policy_api_test --test compensation_policy_entry_test` — 11 passed, including native schema-2 calculation → retro preview/payment → policy invalidation → recalculation/finalization.
- `bun test src/services/compensationPolicy/contract.test.ts` — 8 passed.
- `bun test src/services/compensationPolicy/api.browser.test.ts` — 4 passed, including WASM schema-2 calculation and retro payment persistence plus real IndexedDB abort rollback.
- `bun test src/components/Settings/CompensationPolicyPage.test.tsx src/components/Settings/compensationPolicyDecimal.test.ts` — 20 passed, including preview, blocker, and receipt visibility.
- `bun run wasm:build` — passed and refreshed the package; the browser suite was run against the refreshed WASM.
- `bun run verify:production-graph` — passed.
- `bun run lint` still fails in the repository-wide TypeScript check on unrelated existing diagnostics (including missing Bun matcher typings and existing component typing errors). Filtering its output found no diagnostics in the Task 6 TypeScript files.

## Task 6 file scope

- Core replay and invalidation: `crates/payroll-core/src/payroll_engine/compensation.rs`, `crates/payroll-core/src/payroll_engine.rs`, `crates/payroll-core/src/retro.rs`, `crates/payroll-core/src/policies.rs`, `crates/payroll-core/tests/policies_mutation_regression.rs`, and `crates/payroll-core/tests/retro_regression.rs`.
- Native API and transaction: `src-tauri/src/services/compensation_policy_service.rs`, `src-tauri/src/repositories/payroll_invalidation_repo.rs`, `src-tauri/tests/compensation_policy_api_test.rs`, and `src-tauri/tests/compensation_policy_entry_test.rs`.
- Browser contract and IndexedDB API: `src/services/compensationPolicy/contract.ts`, `src/services/compensationPolicy/api.ts`, `src/services/compensationPolicy/contract.test.ts`, `src/services/compensationPolicy/apiBrowserTestHarness.ts`, and `src/services/compensationPolicy/api.browser.test.ts`.
- UI and payload validation: `src/components/Settings/CompensationPolicyPage.tsx`, `src/components/Settings/CompensationPolicyPageTestHarness.tsx`, `src/components/Settings/CompensationPolicyPage.test.tsx`, `src/hooks/useCompensationPolicyController.ts`, and `src/services/storage/payrollPayloadSchema.ts`.
- This handoff: `docs/compensation-task-6-handoff.md`.

Task 8 legacy cleanup and production activation remain separate gates. No
commit, production activation, or live payroll database mutation was performed.
