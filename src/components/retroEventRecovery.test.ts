import { describe, expect, test } from 'bun:test';
import type { BordroKaydi, RetroAdjustmentBatch, RetroAllocation } from '../types/payroll';
import { isSupersededZeroPayableRetroEvent, retroPaymentEventNeedsReplay } from './retroEventRecovery';

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
    const blocked = retroPaymentEventNeedsReplay({ ...batch, status: 'STALE' } as RetroAdjustmentBatch, payroll, allocations, [staleSource]);
    expect(blocked.eligible).toBe(false);
    expect(blocked.blockedSourcePeriods).toEqual(['period-1']);
    expect(blocked.reason?.includes('period-1')).toBe(true);
    expect(retroPaymentEventNeedsReplay({ ...batch, status: 'FINALIZED' } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(false);
    expect(retroPaymentEventNeedsReplay(batch, { ...payroll, status: 'FINALIZED' } as BordroKaydi, allocations, [sourcePayroll]).eligible).toBe(false);
  });

  // BUG-RETRO-002 — STALE / UNSETTLED recovery must never be a silent dead end.
  const staleBatch = { ...batch, status: 'STALE', settlementStatus: 'UNSETTLED' } as RetroAdjustmentBatch;

  test('E: a STALE/UNSETTLED batch with a STALE event offers recovery', () => {
    expect(retroPaymentEventNeedsReplay(staleBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(true);
  });

  test('E: a STALE batch whose event was deleted or is still CALCULATED is recoverable', () => {
    expect(retroPaymentEventNeedsReplay(staleBatch, undefined, allocations, [sourcePayroll]).eligible).toBe(true);
    expect(retroPaymentEventNeedsReplay(staleBatch, { ...payroll, status: 'CALCULATED' } as BordroKaydi, allocations, [sourcePayroll]).eligible).toBe(true);
  });

  test('E: a STALE batch with zero payable (old preview) is still recoverable', () => {
    expect(retroPaymentEventNeedsReplay({ ...staleBatch, payableSettlementAmount: 0 } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(true);
  });

  test('E: a source period with no payroll at all does not hide recovery (core decides, BUG-RETRO-001 legacy batch)', () => {
    const legacyAllocations = [
      ...allocations,
      { batchId: 'retro-1', sourcePeriodId: 'period-after-effective-to' },
    ] as RetroAllocation[];
    const assessment = retroPaymentEventNeedsReplay(staleBatch, payroll, legacyAllocations, [sourcePayroll]);
    expect(assessment.eligible).toBe(true);
    expect(assessment.blockedSourcePeriods).toEqual([]);
  });

  test('blocked rows always carry an actionable reason', () => {
    const finalizedEvent = retroPaymentEventNeedsReplay(staleBatch, { ...payroll, status: 'FINALIZED' } as BordroKaydi, allocations, [sourcePayroll]);
    expect(finalizedEvent.eligible).toBe(false);
    expect(finalizedEvent.reason).toBeTruthy();
    const mismatched = retroPaymentEventNeedsReplay(staleBatch, { ...payroll, paymentDate: '2027-04-14' } as BordroKaydi, allocations, [sourcePayroll]);
    expect(mismatched.eligible).toBe(false);
    expect(mismatched.reason).toBeTruthy();
  });

  test('paid / offset-settled batches never offer recovery', () => {
    expect(retroPaymentEventNeedsReplay({ ...staleBatch, settlementStatus: 'PAID' } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(false);
    expect(retroPaymentEventNeedsReplay({ ...staleBatch, settlementStatus: 'SETTLED_BY_OFFSET' } as RetroAdjustmentBatch, payroll, allocations, [sourcePayroll]).eligible).toBe(false);
  });

  test('G: a stale event retired by a zero-payable CALCULATED ledger does not block the NORMAL chain', () => {
    const zeroLedger = { ...batch, status: 'CALCULATED', payableSettlementAmount: 0, totalGrossDelta: 0, settlementStatus: 'UNSETTLED' } as RetroAdjustmentBatch;
    expect(isSupersededZeroPayableRetroEvent(payroll, [zeroLedger])).toBe(true);
    expect(isSupersededZeroPayableRetroEvent(payroll, [{ ...zeroLedger, status: 'STALE' } as RetroAdjustmentBatch])).toBe(false);
    expect(isSupersededZeroPayableRetroEvent(payroll, [{ ...zeroLedger, payableSettlementAmount: 10, totalGrossDelta: 10 } as RetroAdjustmentBatch])).toBe(false);
    expect(isSupersededZeroPayableRetroEvent({ ...payroll, status: 'CALCULATED' } as BordroKaydi, [zeroLedger])).toBe(false);
  });
});
