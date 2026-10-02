import type { BordroKaydi, RetroAdjustmentBatch, RetroAllocation } from '../types/payroll';

export interface RetroRecoveryAssessment {
  eligible: boolean;
  blockedSourcePeriods: string[];
}

export function retroPaymentEventNeedsReplay(
  batch: RetroAdjustmentBatch,
  payroll: BordroKaydi | undefined,
  allocations: RetroAllocation[],
  payrolls: BordroKaydi[]
): RetroRecoveryAssessment {
  const blockedSourcePeriods = [...new Set(allocations
    .filter((allocation) => allocation.batchId === batch.id)
    .map((allocation) => allocation.sourcePeriodId))]
    .filter((periodId) => {
      const sourceEvents = payrolls.filter((event) =>
        event.personelId === batch.personnelId &&
        event.donemId === periodId &&
        event.accrualType !== 'RETRO_ADJUSTMENT'
      );
      return !sourceEvents.some((event) =>
        event.accrualType === 'NORMAL' &&
        ['CALCULATED', 'FINALIZED'].includes(event.status)
      ) || sourceEvents.some((event) => !['CALCULATED', 'FINALIZED'].includes(event.status));
    });
  const eligible = (batch.status === 'CALCULATED' || batch.status === 'STALE') &&
    batch.payableSettlementAmount > 0 &&
    batch.settlementStatus !== 'PAID' &&
    batch.settlementStatus !== 'OVERPAYMENT' &&
    batch.settlementStatus !== 'SETTLED_BY_OFFSET' &&
    payroll?.accrualType === 'RETRO_ADJUSTMENT' &&
    payroll.accrualId === batch.id &&
    payroll.personelId === batch.personnelId &&
    payroll.paymentDate === batch.paymentDate &&
    payroll.status === 'STALE' &&
    !blockedSourcePeriods.length;
  return { eligible, blockedSourcePeriods };
}
