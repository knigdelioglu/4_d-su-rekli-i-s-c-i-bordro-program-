import { describe, expect, test } from 'bun:test';
import type { BordroKaydi, RetroAdjustmentBatch } from '../types/payroll';
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

describe('retro payment event recovery', () => {
  test('offers replay only when a payable calculated ledger has a stale matching event', () => {
    expect(retroPaymentEventNeedsReplay(batch, payroll)).toBe(true);
    expect(retroPaymentEventNeedsReplay(batch, undefined)).toBe(false);
    expect(retroPaymentEventNeedsReplay({ ...batch, status: 'STALE' } as RetroAdjustmentBatch, payroll)).toBe(false);
    expect(retroPaymentEventNeedsReplay(batch, { ...payroll, status: 'CALCULATED' } as BordroKaydi)).toBe(false);
    expect(retroPaymentEventNeedsReplay(batch, { ...payroll, accrualId: 'other' } as BordroKaydi)).toBe(false);
    expect(retroPaymentEventNeedsReplay(batch, { ...payroll, personelId: 'other' } as BordroKaydi)).toBe(false);
    expect(retroPaymentEventNeedsReplay(batch, { ...payroll, paymentDate: '2027-03-15' } as BordroKaydi)).toBe(false);
    expect(retroPaymentEventNeedsReplay({ ...batch, payableSettlementAmount: 0 } as RetroAdjustmentBatch, payroll)).toBe(false);
  });
});
