import { describe, expect, test } from 'bun:test';
import type {
  AnnualPayrollParameters,
  DönemselKurumDegerleri,
  PuantajOzeti,
} from '../types/payroll';
import {
  DEFAULT_KURUM_DEGERLERI,
  formatCompactPuantaj,
  hasCompleteAnnualPayrollParameters,
  hasCompletePeriodIncomeParameters,
  hasCompletePeriodLegalParameters,
} from './payrollPresentation';

const periodId = '2026-09';

function institution(
  overrides: Partial<DönemselKurumDegerleri> = {}
): DönemselKurumDegerleri {
  return {
    donemId: periodId,
    ...DEFAULT_KURUM_DEGERLERI,
    ...overrides,
  };
}

function annual(
  overrides: Partial<AnnualPayrollParameters> = {}
): AnnualPayrollParameters {
  return {
    year: 2026,
    gelirVergisiDilimleri: [
      { limit: 190000, oran: 0.15 },
      { limit: 400000, oran: 0.2 },
    ],
    ...overrides,
  };
}

describe('dönem ücret ve yardım readiness', () => {
  test('requires the active period values and matching period id', () => {
    expect(hasCompletePeriodIncomeParameters(undefined, periodId)).toBe(false);
    expect(hasCompletePeriodIncomeParameters(institution(), '2026-10')).toBe(false);
    expect(
      hasCompletePeriodIncomeParameters(
        institution({ gunlukYemek: undefined }),
        periodId
      )
    ).toBe(false);
  });

  test('accepts zero for required values where the payroll domain permits it', () => {
    expect(
      hasCompletePeriodIncomeParameters(
        institution({
          gunlukYemek: 0,
          birlestirilmisSosyalYardim: 0,
          gunlukVasitaYol: 0,
          giyimYardimi: 0,
          hizmetZammiBirimi: 0,
        }),
        periodId
      )
    ).toBe(true);
  });

  test('does not treat an invalid daily base wage as ready', () => {
    expect(
      hasCompletePeriodIncomeParameters(institution({ gunlukTabanUcret: 0 }), periodId)
    ).toBe(false);
    expect(
      hasCompletePeriodIncomeParameters(institution({ gunlukTabanUcret: -1 }), periodId)
    ).toBe(false);
  });
});

describe('dönem vergi ve yasal oran readiness', () => {
  test('accepts a complete legal parameter set', () => {
    expect(hasCompletePeriodLegalParameters(institution(), periodId)).toBe(true);
  });

  test('requires every authoritative legal field', () => {
    expect(
      hasCompletePeriodLegalParameters(institution({ sgkIsciOraniYuzde: undefined }), periodId)
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(
        institution({ damgaVergisiOraniBinde: undefined }),
        periodId
      )
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(institution({ gunlukAsgariUcret: 0 }), periodId)
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(institution({ pekTavanKatsayisi: 0.99 }), periodId)
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(institution({ isPrimiGruplari: undefined }), periodId)
    ).toBe(false);
    expect(hasCompletePeriodLegalParameters(institution({ isPrimiGruplari: [] }), periodId)).toBe(
      false
    );
  });

  test('accepts legal zero values and rejects invalid group rates', () => {
    expect(
      hasCompletePeriodLegalParameters(
        institution({
          sgkIsciOraniYuzde: 0,
          issizlikIsciOraniYuzde: 0,
          sendikaAidatiYuzde: 0,
          besOraniYuzde: 0,
          geceCalismaPrimiYuzde: 0,
          geceCalismaTatiliPrimiYuzde: 0,
          sgkIsverenOraniYuzde: 0,
          issizlikIsverenOraniYuzde: 0,
          damgaVergisiOraniBinde: 0,
          gunlukYemekIstisnasiSGK: 0,
          gunlukYemekIstisnasiGV: 0,
          isPrimiGruplari: [{ id: '1', ad: 'Grup', oran: 0, aktif: true }],
        }),
        periodId
      )
    ).toBe(true);
    expect(
      hasCompletePeriodLegalParameters(
        institution({ isPrimiGruplari: [{ id: '1', ad: 'Grup', oran: -1, aktif: true }] }),
        periodId
      )
    ).toBe(false);
  });

  test('rejects repeated active group identities', () => {
    expect(
      hasCompletePeriodLegalParameters(
        institution({
          isPrimiGruplari: [
            { id: '1', ad: 'Bir', oran: 0, aktif: true },
            { id: '1', ad: 'İki', oran: 0, aktif: true },
          ],
        }),
        periodId
      )
    ).toBe(false);
  });
});

describe('yıllık GV tarifesi readiness', () => {
  test('accepts the active year with valid tariff brackets', () => {
    expect(hasCompleteAnnualPayrollParameters(annual(), 2026)).toBe(true);
  });

  test('rejects a missing/empty tariff or a different year', () => {
    expect(hasCompleteAnnualPayrollParameters(undefined, 2026)).toBe(false);
    expect(
      hasCompleteAnnualPayrollParameters(annual({ gelirVergisiDilimleri: [] }), 2026)
    ).toBe(false);
    expect(hasCompleteAnnualPayrollParameters(annual({ year: 2025 }), 2026)).toBe(false);
  });

  test('rejects invalid bracket limits and rates', () => {
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ gelirVergisiDilimleri: [{ limit: 190000, oran: 1.01 }] }),
        2026
      )
    ).toBe(false);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ gelirVergisiDilimleri: [{ limit: 0, oran: 0 }] }),
        2026
      )
    ).toBe(false);
  });
});

describe('kompakt puantaj sunumu', () => {
  test('keeps non-zero attendance codes in a compact readable order', () => {
    const summary: PuantajOzeti = { Ç: 20, T: 8, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 1 };
    expect(formatCompactPuantaj(summary)).toBe('20 Ç · 8 T · 1 R');
  });
});
