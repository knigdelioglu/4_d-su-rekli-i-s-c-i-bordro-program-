import { describe, expect, test } from 'bun:test';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { mergeBrowserTaxOpening, usePayrollMutationController } from './usePayrollMutationController';
import type { PayrollMutationControllerOptions } from './usePayrollMutationController';
import type { PayrollStorageDto } from '../services/payrollEngine/decimalBoundary';
import type { PayrollBoundaryPayroll, PayrollEngine, PayrollDatasetSnapshot } from '../services/payrollEngine/types';
import { queueAuthoritativePayloadCommit } from '../services/storage/authoritativePayloadCommit';
import { tauriBridge } from '../services/tauriBridge';

test('native personnel deletion waits for every database deletion and list refresh', async () => {
  const originalDelete = tauriBridge.deletePersonnel;
  const events: string[] = [];
  let controller!: ReturnType<typeof usePayrollMutationController>;
  let refreshResult = true;
  tauriBridge.deletePersonnel = async (id) => { events.push(`delete:${id}`); };
  function Harness() {
    controller = usePayrollMutationController({
      isNative: true, payrollDataset: {} as PayrollDatasetSnapshot,
      loadData: async () => { events.push('refresh'); return refreshResult; },
    } as unknown as PayrollMutationControllerOptions);
    return null;
  }
  try {
    renderToString(createElement(Harness));
    for (let i = 0; i < 6; i++) await controller.handleDeletePersonel(`person-${i}`);
    expect(events).toEqual(Array.from({ length: 6 }, (_, i) => [`delete:person-${i}`, 'refresh']).flat());
    refreshResult = false;
    await expect(controller.handleDeletePersonel('person-6')).rejects.toThrow('Personel silindi, ancak liste yenilenemedi');
  } finally {
    tauriBridge.deletePersonnel = originalDelete;
  }
});

test('consecutive personnel deletions are saved in order and use the latest dataset', async () => {
  const ids = Array.from({ length: 6 }, (_, index) => `person-${index}`);
  let payload = {
    exportedAt: '', personeller: ids.map((id) => ({ id })), donemler: [],
    kurumDegerleriMap: {}, puantajlar: [], bordrolar: [], taxOpenings: [],
    sickLeaveRecords: [], annualPayrollParameters: [], compensationRevisions: [],
    compensationRevisionOverrides: [], retroBatches: [], retroAllocations: [],
  } as unknown as PayrollStorageDto;
  let saved = payload;
  let generation = 0;
  const observedCounts: number[] = [];
  const queue = { current: Promise.resolve() };
  let controller!: ReturnType<typeof usePayrollMutationController>;
  function Harness() {
    controller = usePayrollMutationController({
      isNative: false, authoritativePayload: payload,
      payrollDataset: {} as PayrollDatasetSnapshot,
      donemler: [], puantajlar: [], bordrolar: [], sickLeaveRecords: [],
      compensationRevisions: [], compensationRevisionOverrides: [], retroBatches: [],
      payrollEngine: { evaluateMutationPolicy: async (_mutation, dataset) => {
        observedCounts.push(dataset.personnel.length);
        return { affectedPayrolls: [], blockedByFinalized: [], affectedRetroBatches: [], blockedByFinalizedRetroBatches: [] };
      } } as PayrollEngine,
      updateAuthoritativePayload: () => { throw new Error('Expected durable commit'); },
      commitAuthoritativePayload: (update) => queueAuthoritativePayloadCommit(queue, {
        getCurrent: () => payload, getGeneration: () => generation,
        markDirty: () => { generation++; },
        persist: async (next) => { saved = next; },
        publish: (next) => { payload = next; }, missingPayloadMessage: 'missing',
      }, update),
      loadData: async () => {},
    } as PayrollMutationControllerOptions);
    return null;
  }
  renderToString(createElement(Harness));
  await Promise.all(ids.map((id) => controller.handleDeletePersonel(id)));
  expect(observedCounts).toEqual([6, 5, 4, 3, 2, 1]);
  expect(payload.personeller).toEqual([]);
  expect(saved.personeller).toEqual([]);
});

describe('browser tax opening mutation', () => {
  test('persists value and effective period as one canonical pair', () => {
    const next = mergeBrowserTaxOpening([], {
      id: 'person-1_2026',
      personnelId: 'person-1',
      year: 2026,
      gvCumulativeOpening: '185000',
      effectiveFromPeriodId: 'donem-2026-08',
    });

    expect(next).toEqual([
      {
        id: 'person-1_2026',
        personnelId: 'person-1',
        year: 2026,
        gvCumulativeOpening: '185000',
        effectiveFromPeriodId: 'donem-2026-08',
      },
    ]);
  });

  test('clears both components together and preserves explicit zero', () => {
    const existing = {
      id: 'person-1_2026',
      personnelId: 'person-1',
      year: 2026,
      gvCumulativeOpening: '185000',
      effectiveFromPeriodId: 'donem-2026-08',
      asgariGvCumulativeOpening: '0',
      asgariGvEffectiveFromPeriodId: 'donem-2026-08',
    } as const;
    const zero = mergeBrowserTaxOpening([existing], {
      ...existing,
      gvCumulativeOpening: '0',
      effectiveFromPeriodId: 'donem-2026-08',
    });
    expect(zero[0].gvCumulativeOpening).toBe('0');
    expect(zero[0].effectiveFromPeriodId).toBe('donem-2026-08');

    const cleared = mergeBrowserTaxOpening(zero, {
      ...existing,
      gvCumulativeOpening: null,
      effectiveFromPeriodId: null,
      asgariGvCumulativeOpening: null,
      asgariGvEffectiveFromPeriodId: null,
    });
    expect(cleared[0].gvCumulativeOpening).toBe(null);
    expect(cleared[0].effectiveFromPeriodId).toBe(null);
    expect(cleared[0].asgariGvCumulativeOpening).toBe(null);
    expect(cleared[0].asgariGvEffectiveFromPeriodId).toBe(null);
  });

  test('preserves an existing normal pair while saving an independent asgari pair', () => {
    const existing = {
      id: 'person-1_2026',
      personnelId: 'person-1',
      year: 2026,
      gvCumulativeOpening: '185000',
      effectiveFromPeriodId: 'donem-2026-08',
    } as const;
    const next = mergeBrowserTaxOpening([existing], {
      id: existing.id,
      personnelId: existing.personnelId,
      year: existing.year,
      asgariGvCumulativeOpening: '90000',
      asgariGvEffectiveFromPeriodId: 'donem-2026-08',
    });

    expect(next[0]).toEqual({
      id: existing.id,
      personnelId: existing.personnelId,
      year: existing.year,
      gvCumulativeOpening: '185000',
      effectiveFromPeriodId: 'donem-2026-08',
      asgariGvCumulativeOpening: '90000',
      asgariGvEffectiveFromPeriodId: 'donem-2026-08',
    });
  });
});

describe('browser save invalidates dependent retro batches', () => {
  function harness(accrualType: 'NORMAL' | 'RETRO_ADJUSTMENT' = 'NORMAL') {
    const payroll = {
      id: 'payroll', accrualId: 'payroll', personelId: 'person', donemId: 'period',
      accrualType, status: 'CALCULATED', paymentDate: '2026-01-31', sequence: 0,
      sonGuncellemeTarihi: '2026-01-31T00:00:00Z',
    } as PayrollBoundaryPayroll;
    let payload = {
      bordrolar: [payroll], kurumDegerleriMap: {}, annualPayrollParameters: [],
      retroBatches: [
        { id: 'affected', status: 'CALCULATED', settlementStatus: 'OVERPAYMENT' },
        { id: 'unrelated', status: 'CALCULATED' },
        { id: 'finalized', status: 'FINALIZED' },
        { id: 'payroll', status: 'CALCULATED', settlementStatus: 'UNSETTLED' },
      ],
    } as unknown as PayrollStorageDto;
    const dataset = { payrolls: [payroll] } as PayrollDatasetSnapshot;
    const impact = {
      affectedPayrolls: [], blockedByFinalized: [],
      affectedRetroBatches: ['affected', 'finalized'], blockedByFinalizedRetroBatches: [],
    };
    let controller: ReturnType<typeof usePayrollMutationController>;
    function Harness() {
      controller = usePayrollMutationController({
        isNative: false,
        payrollEngine: { evaluateMutationPolicy: async () => impact } as unknown as PayrollEngine,
        payrollDataset: dataset, authoritativePayload: payload,
        donemler: [], puantajlar: [], bordrolar: [], sickLeaveRecords: [],
        compensationRevisions: [], compensationRevisionOverrides: [], retroBatches: [],
        updateAuthoritativePayload: (update) => { payload = update(payload); },
        loadData: async () => {},
      } as PayrollMutationControllerOptions);
      return null;
    }
    renderToString(createElement(Harness));
    return { controller: controller!, payroll, payload: () => payload };
  }

  function assertDependentStatuses(payload: PayrollStorageDto) {
    expect(payload.retroBatches.find((batch) => batch.id === 'affected')?.status).toBe('STALE');
    expect(payload.retroBatches.find((batch) => batch.id === 'unrelated')?.status).toBe('CALCULATED');
    expect(payload.retroBatches.find((batch) => batch.id === 'finalized')?.status).toBe('FINALIZED');
  }

  test('recalculating an existing payroll invalidates even a batch without a payment row', async () => {
    const h = harness();
    await h.controller.handleSaveBordro(h.payroll);
    assertDependentStatuses(h.payload());
    expect(h.payload().bordrolar[0].status).toBe('CALCULATED');
  });

  test('saving annual parameters invalidates dependent retro batches', async () => {
    const h = harness();
    await h.controller.handleSaveAnnualPayrollParameters({
      year: 2026, gelirVergisiDilimleri: [{ limit: 190000, oran: 0.15 }],
    });
    assertDependentStatuses(h.payload());
  });

  test('finalizing a retro payment preserves its paid state while invalidating followers', async () => {
    const h = harness('RETRO_ADJUSTMENT');
    await h.controller.handleSaveBordro({ ...h.payroll, status: 'FINALIZED' }, undefined, 'ENGINE_FINALIZATION');
    assertDependentStatuses(h.payload());
    expect(h.payload().retroBatches.find((batch) => batch.id === 'payroll')).toMatchObject({
      status: 'FINALIZED', settlementStatus: 'PAID', finalizedAt: h.payroll.sonGuncellemeTarihi,
    });
  });
});
