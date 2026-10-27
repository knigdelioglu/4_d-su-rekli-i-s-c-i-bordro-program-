import { expect, test } from 'bun:test';
import { findFirstStalePriorEvent, replaySamePeriodPayrollFollowers, replayThroughRetroBatch } from './replayChain';
import type { PayrollDatasetSnapshot } from './types';
import type { BordroDonemi, BordroKaydi } from '../../types/payroll';
const period = { id: '2028-09', taxYear: 2028, taxMonth: 10, donemAdi: 'Eylül 2028' } as BordroDonemi;
const event = (id: string, type: BordroKaydi['accrualType'], sequence: number, status = 'STALE') => ({
  id, accrualId: id, personelId: 'person', donemId: period.id, accrualType: type, paymentDate: '2028-10-14', sequence, status,
}) as BordroKaydi;
function fixture(events: BordroKaydi[]) {
  return { personnel: [], periods: [period], payrolls: events, retroBatches: [{ id: 'retro', personnelId: 'person', paymentDate: '2028-10-14',
    status: 'STALE', payableSettlementAmount: '100', totalGrossDelta: '100', settlementStatus: 'UNSETTLED' }], retroAllocations: [] } as unknown as PayrollDatasetSnapshot;
}

test('replays source NORMAL, retro ledger/payment and following payments in canonical order', async () => {
  const normal = event('normal', 'NORMAL', 0), retro = event('retro', 'RETRO_ADJUSTMENT', 1), later = event('tediye', 'TEDIYE', 2);
  const dataset = fixture([later, retro, normal]), order: string[] = [];
  const result = await replayThroughRetroBatch({ batchId: 'retro', getDataset: () => dataset,
    recalculate: async (record) => { order.push(record.id); record.status = 'CALCULATED'; },
    recoverRetro: async () => { order.push('retro'); expect(normal.status).toBe('CALCULATED'); retro.status = 'CALCULATED'; dataset.retroBatches[0].status = 'CALCULATED'; },
  });
  expect(result).toEqual({ ok: true, replayed: 3 });
  expect(order).toEqual(['normal', 'retro', 'tediye']);
});
test('normal followers automatically recover stale retro before supplementary payments', async () => {
  const retro = event('retro', 'RETRO_ADJUSTMENT', 1), later = event('later', 'SUPPLEMENTAL', 2);
  const dataset = fixture([later, retro]), order: string[] = [];
  const result = await replaySamePeriodPayrollFollowers({ personnelId: 'person', period, getDataset: () => dataset,
    recalculate: async (record) => { order.push(record.id); record.status = 'CALCULATED'; },
    recoverRetro: async () => { order.push('retro'); retro.status = 'CALCULATED'; dataset.retroBatches[0].status = 'CALCULATED'; },
  });
  expect(result).toEqual({ ok: true, replayed: 2 }); expect(order).toEqual(['retro', 'later']);
});
test('does not accept a calculated payment backed by a stale entitlement ledger', async () => {
  const retro = event('retro', 'RETRO_ADJUSTMENT', 1, 'CALCULATED'), dataset = fixture([retro]);
  const prior = findFirstStalePriorEvent([retro], new Map([[period.id, period]]), 'person', event('later', 'NORMAL', 2), period, dataset.retroBatches as never);
  expect(prior?.payroll.id).toBe('retro');
  const result = await replaySamePeriodPayrollFollowers({ personnelId: 'person', period, getDataset: () => dataset,
    recalculate: async () => {}, recoverRetro: async () => {},
  });
  expect(result.ok).toBe(false); expect(result.error).toContain('güncel kayıt doğrulanamadı');
});
test('never continues after missing source authority or falsely reports a stale write as recovered', async () => {
  const retro = event('retro', 'RETRO_ADJUSTMENT', 1), later = event('later', 'TEDIYE', 2), dataset = fixture([retro, later]);
  let calls = 0;
  const result = await replaySamePeriodPayrollFollowers({ personnelId: 'person', period, getDataset: () => dataset,
    recalculate: async () => { calls++; }, recoverRetro: async () => { throw new Error('2027-01 kaynak döneminin NORMAL bordrosu bulunamadı; özgün yedeği kontrol edin.'); },
  });
  expect(calls).toBe(0); expect(result.error).toContain('2027-01'); expect(result.error).toContain('özgün yedeği');
  const stuck = await replaySamePeriodPayrollFollowers({ personnelId: 'person', period, getDataset: () => fixture([later]), recalculate: async () => {} });
  expect(stuck.ok).toBe(false); expect(stuck.replayed).toBe(0);
});
test('zero settlement retires the obsolete event and still allows its next payment', async () => {
  const later = event('later', 'TEDIYE', 2), dataset = fixture([event('retro', 'RETRO_ADJUSTMENT', 1), later]);
  const result = await replayThroughRetroBatch({ batchId: 'retro', getDataset: () => dataset,
    recalculate: async (record) => { record.status = 'CALCULATED'; }, recoverRetro: async () => {
      dataset.retroBatches[0] = { ...dataset.retroBatches[0], status: 'CALCULATED', payableSettlementAmount: '0', totalGrossDelta: '0' };
      dataset.payrolls = dataset.payrolls.filter((record) => record.id !== 'retro');
    },
  });
  expect(result.ok).toBe(true); expect(later.status).toBe('CALCULATED');
});
test('finalized and paid history is never sent to recovery', async () => {
  for (const finalization of ['batch', 'event', 'paid']) {
    const retro = event('retro', 'RETRO_ADJUSTMENT', 1), dataset = fixture([retro]); let calls = 0;
    if (finalization === 'batch') dataset.retroBatches[0].status = 'FINALIZED';
    if (finalization === 'event') retro.status = 'FINALIZED';
    if (finalization === 'paid') dataset.retroBatches[0].settlementStatus = 'PAID';
    const result = await replayThroughRetroBatch({ batchId: 'retro', getDataset: () => dataset, recalculate: async () => { calls++; }, recoverRetro: async () => { calls++; } });
    expect(result.ok).toBe(false); expect(result.error).toContain('düzeltme revizyonu'); expect(calls).toBe(0);
  }
});
test('a successor invalidating the recovered ledger is reported as a dependency cycle', async () => {
  const retro = event('retro', 'RETRO_ADJUSTMENT', 1), later = event('later', 'TEDIYE', 2), dataset = fixture([retro, later]);
  const result = await replayThroughRetroBatch({ batchId: 'retro', getDataset: () => dataset,
    recoverRetro: async () => { retro.status = 'CALCULATED'; dataset.retroBatches[0].status = 'CALCULATED'; },
    recalculate: async (record) => { record.status = 'CALCULATED'; dataset.retroBatches[0].status = 'STALE'; },
  });
  expect(result.ok).toBe(false); expect(result.error).toContain('bağımlılık döngüsü');
});
