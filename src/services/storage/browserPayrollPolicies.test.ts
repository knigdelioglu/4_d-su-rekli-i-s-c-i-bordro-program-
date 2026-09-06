import { describe, expect, test } from 'bun:test';
import type { BordroKaydi } from '../../types/payroll';
import {
  applyBrowserRetroBatchImpact,
  applyBrowserPayrollImpact,
  assertBrowserMutationImpactAllowed,
} from './browserPayrollPolicies';

const payroll = (periodId: string, status: BordroKaydi['status']): BordroKaydi =>
  ({ personelId: 'person-1', donemId: periodId, status } as BordroKaydi);

describe('browser payroll invalidation policy', () => {
  test('applies the core impact to mutable records and preserves finalized records', () => {
    const result = applyBrowserPayrollImpact(
      [payroll('2026-01', 'CALCULATED'), payroll('2026-02', 'FINALIZED')],
      {
        affectedPayrolls: [
          { personnelId: 'person-1', periodId: '2026-01' },
          { personnelId: 'person-1', periodId: '2026-02' },
        ],
        blockedByFinalized: [{ personnelId: 'person-1', periodId: '2026-02' }],
        affectedRetroBatches: [],
        blockedByFinalizedRetroBatches: [],
      }
    );

    expect(result[0].status).toBe('STALE');
    expect(result[1].status).toBe('FINALIZED');
  });

  test('rejects blockers returned by core without recomputing policy', () => {
    expect(() =>
      assertBrowserMutationImpactAllowed({
        affectedPayrolls: [{ personnelId: 'person-1', periodId: '2026-02' }],
        blockedByFinalized: [{ personnelId: 'person-1', periodId: '2026-02' }],
        affectedRetroBatches: [],
        blockedByFinalizedRetroBatches: [],
      })
    ).toThrow('Kesinleştirilmiş');
  });

  test('applies retro batch impact and preserves finalized ledgers', () => {
    const result = applyBrowserRetroBatchImpact(
      [
        { id: 'retro-calculated', status: 'CALCULATED' },
        { id: 'retro-finalized', status: 'FINALIZED' },
      ],
      {
        affectedPayrolls: [],
        blockedByFinalized: [],
        affectedRetroBatches: ['retro-calculated', 'retro-finalized'],
        blockedByFinalizedRetroBatches: ['retro-finalized'],
      }
    );

    expect(result.map((batch) => batch.status)).toEqual(['STALE', 'FINALIZED']);
  });
});

test('delete impact marks all mutable downstream events stale without changing snapshots', () => {
  const events = ['tediye', 'normal', 'tis'].map((id, index) => ({
    id, accrualId: id, personelId: 'p', donemId: 'august',
    status: index === 2 ? 'DRAFT' : 'CALCULATED', gvDetay: { applied: '2000.00' },
  }));
  const impact = {
    affectedPayrolls: events.map((e) => ({ personnelId: e.personelId, periodId: e.donemId, accrualId: e.accrualId })),
    blockedByFinalized: [],
    affectedRetroBatches: [],
    blockedByFinalizedRetroBatches: [],
  };
  assertBrowserMutationImpactAllowed(impact);
  const after = applyBrowserPayrollImpact(events, impact).filter((e) => e.id !== 'tediye');
  expect(after.map((e) => e.status)).toEqual(['STALE', 'STALE']);
  expect(after[0].gvDetay).toBe(events[1].gvDetay);
  expect(events[1].status).toBe('CALCULATED');
});
