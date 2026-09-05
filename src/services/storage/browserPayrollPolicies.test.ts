import { describe, expect, test } from 'bun:test';
import type { BordroKaydi } from '../../types/payroll';
import {
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
      })
    ).toThrow('Kesinleştirilmiş');
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
  };
  assertBrowserMutationImpactAllowed(impact);
  const after = applyBrowserPayrollImpact(events, impact).filter((e) => e.id !== 'tediye');
  expect(after.map((e) => e.status)).toEqual(['STALE', 'STALE']);
  expect(after[0].gvDetay).toBe(events[1].gvDetay);
  expect(events[1].status).toBe('CALCULATED');
});
