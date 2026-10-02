import { describe, expect, test } from 'bun:test';
import type { BordroKaydi, RetroAdjustmentBatch, RetroAllocation } from '../types/payroll';
import { retroPaymentEventNeedsReplay } from './retroEventRecovery';

const batch = {
  id: 'retro-1',
  personnelId: 'person-1',
  paymentDate: '2027-03-14',
  status: 'CALCULATED',
  payableSettlementAmount: 100,
} as RetroAdjustmentBatch;

const payroll = {
  accrualId: 'retro-1',
  personelId: 'person-1',
  paymentDate: '2027-03-14',
  accrualType: 'RETRO_ADJUSTMENT',
  status: 'STALE',
} as BordroKaydi;

const allocations = [{ batchId: 'retro-1', sourcePeriodId: 'period-1' }] as RetroAllocation[];
const sourcePayroll = {
  personelId: 'person-1', donemId: 'period-1', accrualType: 'NORMAL', status: 'CALCULATED',
} as BordroKaydi;

describe('retro payment event recovery', () => {
  test('recovers calculated/stale batches only with a stale matching event and authoritative source payroll', () => {
    expect(retroPaymentEventNeedsReplay(batch, payroll, allocations, [sourcePayroll]).eligible).toBe(true);
    expect(retroPaymentEventNeedsReplay({ ...batch, status: 'STALE' } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(true);
    expect(retroPaymentEventNeedsReplay(batch, undefined, allocations, [sourcePayroll]).eligible).toBe(false);
    expect(retroPaymentEventNeedsReplay(batch, { ...payroll, status: 'CALCULATED' } as BordroKaydi, allocations, [sourcePayroll]).eligible).toBe(false);
    expect(retroPaymentEventNeedsReplay({ ...batch, payableSettlementAmount: 0 } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(false);
    expect(retroPaymentEventNeedsReplay({ ...batch, settlementStatus: 'PAID' } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(false);
  });

  test('reports source periods that must be recalculated and hides finalized records', () => {
    const staleSource = { ...sourcePayroll, status: 'STALE' } as BordroKaydi;
    expect(retroPaymentEventNeedsReplay({ ...batch, status: 'STALE' } as RetroAdjustmentBatch, payroll, allocations, [staleSource]))
      .toEqual({ eligible: false, blockedSourcePeriods: ['period-1'] });
    expect(retroPaymentEventNeedsReplay({ ...batch, status: 'FINALIZED' } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(false);
    expect(retroPaymentEventNeedsReplay(batch, { ...payroll, status: 'FINALIZED' } as BordroKaydi, allocations, [sourcePayroll]).eligible).toBe(false);
  });
});
