import type { PayrollDatasetSnapshot } from './types';

export function assertPayrollCalculationSnapshotCurrent(
  calculationSnapshot: PayrollDatasetSnapshot,
  currentSnapshot: PayrollDatasetSnapshot
): void {
  if (calculationSnapshot !== currentSnapshot) {
    throw new Error(
      'Hesaplama sırasında bordro verileri değişti. Eski hesaplama kaydedilmedi; bordroyu yeniden hesaplayın.'
    );
  }
}

/**
 * Browser/WASM calculates from the supplied in-memory snapshot and saves it in
 * a separate step, so that snapshot must remain current through calculation.
 * Tauri calculates and persists inside one SQLite transaction from its live
 * database; its successful mutation emits a refreshed UI snapshot before the
 * command returns, so object identity is not a valid post-calculation guard.
 */
export function assertPayrollCalculationSnapshotCurrentForEngine(
  engineKind: 'tauri' | 'wasm',
  calculationSnapshot: PayrollDatasetSnapshot,
  currentSnapshot: PayrollDatasetSnapshot
): void {
  if (engineKind === 'wasm') {
    assertPayrollCalculationSnapshotCurrent(calculationSnapshot, currentSnapshot);
  }
}
