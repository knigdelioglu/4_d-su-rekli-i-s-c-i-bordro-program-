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
