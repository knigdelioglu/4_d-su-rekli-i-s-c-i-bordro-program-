import { describe, expect, test } from 'bun:test';
import { mergeBrowserTaxOpening } from './usePayrollMutationController';

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
