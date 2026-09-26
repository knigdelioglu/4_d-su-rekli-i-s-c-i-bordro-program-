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
