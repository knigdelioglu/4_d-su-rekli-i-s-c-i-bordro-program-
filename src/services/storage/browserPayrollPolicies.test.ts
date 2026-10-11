import { describe, expect, test } from 'bun:test';
import type { BordroKaydi } from '../../types/payroll';
import {
  applyBrowserRetroBatchImpact,
  applyBrowserPayrollImpact,
  assertBrowserMutationImpactAllowed,
  personnelHasFinancialChanges,
  taxOpeningHasFinancialChanges,
  validatePayrollStatusTransition,
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

  test('names finalized retro blockers when ordinary payroll blockers are absent', () => {
    expect(() =>
      assertBrowserMutationImpactAllowed({
        affectedPayrolls: [],
        blockedByFinalized: [],
        affectedRetroBatches: [],
        blockedByFinalizedRetroBatches: ['retro-finalized'],
      })
    ).toThrow('1 kesinleşmiş geriye dönük fark ödemesi');
  });

  test('assertBrowserMutationImpactAllowed rejects ALL mutation when finalized records exist', () => {
    expect(() =>
      assertBrowserMutationImpactAllowed({
        affectedPayrolls: [{ personnelId: 'person-1', periodId: '2026-01' }],
        blockedByFinalized: [{ personnelId: 'person-1', periodId: '2026-01' }],
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

describe('personnelHasFinancialChanges', () => {
  const baseline = {
    id: 'p1',
    ad: 'Ali',
    soyad: 'Veli',
    tcNo: '12345678901',
    iban: 'TR123',
    grup: '1. Grup',
    hizmetYili: 5,
    devirKumulatifGvMatrahi: '1000',
    devirKumulatifGvMatrahiYili: 2026,
    devirKumulatifGvMatrahiBaslangicAyi: 1,
    devirKumulatifAsgariGvMatrahi: '500',
    devirKumulatifAsgariGvMatrahiYili: 2026,
    kesintiler: {
      sendikaUyesi: true,
      sabitSendikaAidati: 100,
      besUyesi: false,
      oksOraniYuzde: 3,
      sabitBesTutar: 0,
      icraTutar: 0,
      nafakaTutar: 0,
      kisiBorcuTutar: 0,
      dogumAskerlikBorclanmasiTutar: 0,
      hayatSaglikSigortasiTutar: 0,
      digerKesintiTutar: 0,
      gvIndirimleri: {
        dogumAskerlikGvIndirimTutar: 0,
        hayatSigortasiPrimiTutar: 0,
        saglikSigortasiPrimiTutar: 0,
      },
    },
  };

  test('returns false for no changes or purely non-financial metadata changes', () => {
    expect(personnelHasFinancialChanges(baseline, { ...baseline })).toBe(false);
    expect(
      personnelHasFinancialChanges(baseline, {
        ...baseline,
        ad: 'Mehmet',
        soyad: 'Demir',
        tcNo: '99999999999',
        iban: 'TR999',
      })
    ).toBe(false);
  });

  test('detects top-level financial changes', () => {
    expect(personnelHasFinancialChanges(baseline, { ...baseline, grup: '2. Grup' })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, hizmetYili: 6 })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, devirKumulatifGvMatrahi: '2000' })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, devirKumulatifGvMatrahiYili: 2027 })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, devirKumulatifGvMatrahiBaslangicAyi: 2 })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, devirKumulatifAsgariGvMatrahi: '600' })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, devirKumulatifAsgariGvMatrahiYili: 2027 })).toBe(true);
  });

  test('detects deductions and tax discounts changes', () => {
    const k = baseline.kesintiler;
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, sendikaUyesi: false } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, sabitSendikaAidati: 200 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, besUyesi: true } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, oksOraniYuzde: 4 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, sabitBesTutar: 50 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, icraTutar: 500 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, nafakaTutar: 300 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, kisiBorcuTutar: 150 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, dogumAskerlikBorclanmasiTutar: 75 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, hayatSaglikSigortasiTutar: 80 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, digerKesintiTutar: 90 } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, gvIndirimleri: { ...k.gvIndirimleri, dogumAskerlikGvIndirimTutar: 10 } } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, gvIndirimleri: { ...k.gvIndirimleri, hayatSigortasiPrimiTutar: 20 } } })).toBe(true);
    expect(personnelHasFinancialChanges(baseline, { ...baseline, kesintiler: { ...k, gvIndirimleri: { ...k.gvIndirimleri, saglikSigortasiPrimiTutar: 30 } } })).toBe(true);
  });

  test('treats an enforcement change as financial and invalidates mutable payroll and retro dependencies', () => {
    const changed = {
      ...baseline,
      kesintiler: { ...baseline.kesintiler, icraTutar: 125.5 },
    };
    expect(personnelHasFinancialChanges(baseline, changed)).toBe(true);

    const impact = {
      affectedPayrolls: [
        { personnelId: 'person-1', periodId: '2028-01' },
        { personnelId: 'person-1', periodId: '2028-02' },
      ],
      blockedByFinalized: [],
      affectedRetroBatches: ['retro-calculated', 'retro-finalized'],
      blockedByFinalizedRetroBatches: [],
    };
    const payrolls = applyBrowserPayrollImpact([
      payroll('2028-01', 'CALCULATED'),
      payroll('2028-02', 'FINALIZED'),
    ], impact);
    const retroBatches = applyBrowserRetroBatchImpact([
      { id: 'retro-calculated', status: 'CALCULATED' },
      { id: 'retro-finalized', status: 'FINALIZED' },
    ], impact);

    expect(payrolls.map((item) => item.status)).toEqual(['STALE', 'FINALIZED']);
    expect(retroBatches.map((item) => item.status)).toEqual(['STALE', 'FINALIZED']);
  });
});

describe('taxOpeningHasFinancialChanges', () => {
  const opening = {
    gvCumulativeOpening: 1000,
    effectiveFromPeriodId: '2026-01',
    asgariGvCumulativeOpening: 500,
    asgariGvEffectiveFromPeriodId: '2026-01',
  };

  test('handles null and undefined transitions', () => {
    expect(taxOpeningHasFinancialChanges(null, null)).toBe(false);
    expect(taxOpeningHasFinancialChanges(undefined, undefined)).toBe(false);
    expect(taxOpeningHasFinancialChanges(null, { gvCumulativeOpening: 0 })).toBe(false);
    expect(taxOpeningHasFinancialChanges(null, { gvCumulativeOpening: '100' })).toBe(true);
    expect(taxOpeningHasFinancialChanges(null, { effectiveFromPeriodId: '2026-02' })).toBe(true);
    expect(taxOpeningHasFinancialChanges(null, { asgariGvCumulativeOpening: '50' })).toBe(true);
    expect(taxOpeningHasFinancialChanges(null, { asgariGvEffectiveFromPeriodId: '2026-02' })).toBe(true);
    expect(taxOpeningHasFinancialChanges({ gvCumulativeOpening: 0 }, null)).toBe(false);
    expect(taxOpeningHasFinancialChanges({ gvCumulativeOpening: '100' }, null)).toBe(true);
    expect(taxOpeningHasFinancialChanges({ effectiveFromPeriodId: '2026-02' }, null)).toBe(true);
    expect(taxOpeningHasFinancialChanges({ asgariGvCumulativeOpening: '50' }, null)).toBe(true);
    expect(taxOpeningHasFinancialChanges({ asgariGvEffectiveFromPeriodId: '2026-02' }, null)).toBe(true);
  });

  test('detects differences between existing records', () => {
    expect(taxOpeningHasFinancialChanges(opening, { ...opening })).toBe(false);
    expect(taxOpeningHasFinancialChanges(opening, { ...opening, gvCumulativeOpening: 2000 })).toBe(true);
    expect(taxOpeningHasFinancialChanges(opening, { ...opening, effectiveFromPeriodId: '2026-02' })).toBe(true);
    expect(taxOpeningHasFinancialChanges(opening, { ...opening, asgariGvCumulativeOpening: 600 })).toBe(true);
    expect(taxOpeningHasFinancialChanges(opening, { ...opening, asgariGvEffectiveFromPeriodId: '2026-02' })).toBe(true);
  });
});

describe('validatePayrollStatusTransition', () => {
  test('rejects transition to FINALIZED via status mutation API', () => {
    expect(() => validatePayrollStatusTransition('DRAFT', 'FINALIZED')).toThrow(
      "FINALIZED geçişi için finalize_payroll API'si kullanılmalıdır."
    );
    expect(() => validatePayrollStatusTransition('FINALIZED', 'FINALIZED')).toThrow(
      "FINALIZED geçişi için finalize_payroll API'si kullanılmalıdır."
    );
  });

  test('allows identical non-finalized status (no-op)', () => {
    expect(() => validatePayrollStatusTransition('DRAFT', 'DRAFT')).not.toThrow();
    expect(() => validatePayrollStatusTransition('CALCULATED', 'CALCULATED')).not.toThrow();
    expect(() => validatePayrollStatusTransition('STALE', 'STALE')).not.toThrow();
  });

  test('rejects mutating a FINALIZED record', () => {
    expect(() => validatePayrollStatusTransition('FINALIZED', 'CALCULATED')).toThrow(
      'Kesinleştirilmiş (FINALIZED) bordronun durumu değiştirilemez.'
    );
    expect(() => validatePayrollStatusTransition('FINALIZED', 'DRAFT')).toThrow(
      'Kesinleştirilmiş (FINALIZED) bordronun durumu değiştirilemez.'
    );
  });

  test('rejects uncalculated STALE or DRAFT becoming CALCULATED directly', () => {
    expect(() => validatePayrollStatusTransition('STALE', 'CALCULATED')).toThrow(
      'Yeniden hesaplanmamış STALE kayıt doğrudan CALCULATED yapılamaz; bordro yeniden hesaplanmalıdır.'
    );
    expect(() => validatePayrollStatusTransition('DRAFT', 'CALCULATED')).toThrow(
      'DRAFT kayıt doğrudan CALCULATED yapılamaz; bordro hesaplanmalıdır.'
    );
  });

  test('allows valid lifecycle transitions', () => {
    expect(() => validatePayrollStatusTransition('CALCULATED', 'STALE')).not.toThrow();
    expect(() => validatePayrollStatusTransition('CALCULATED', 'DRAFT')).not.toThrow();
    expect(() => validatePayrollStatusTransition('STALE', 'DRAFT')).not.toThrow();
  });
});
