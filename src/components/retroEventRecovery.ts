import type { BordroKaydi, RetroAdjustmentBatch } from '../types/payroll';

export function retroPaymentEventNeedsReplay(
  batch: RetroAdjustmentBatch,
  payroll: BordroKaydi | undefined
): boolean {
  return batch.status === 'CALCULATED' &&
    batch.payableSettlementAmount > 0 &&
    payroll?.accrualType === 'RETRO_ADJUSTMENT' &&
    payroll.accrualId === batch.id &&
    payroll.personelId === batch.personnelId &&
    payroll.paymentDate === batch.paymentDate &&
    payroll.status === 'STALE';
}
