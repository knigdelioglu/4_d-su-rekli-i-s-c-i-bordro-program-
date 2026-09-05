import { describe, expect, test } from 'bun:test';
import type { DönemselKurumDegerleri, PuantajOzeti } from '../types/payroll';
import {
  DEFAULT_KURUM_DEGERLERI,
  formatCompactPuantaj,
  hasCompletePeriodInstitutionParameters,
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

describe('dönem parametre readiness', () => {
  test('requires the active period institution values and matching period id', () => {
    expect(hasCompletePeriodInstitutionParameters(undefined, periodId)).toBe(false);
    expect(hasCompletePeriodInstitutionParameters(institution(), '2026-10')).toBe(false);
    expect(
      hasCompletePeriodInstitutionParameters(
        institution({ gunlukYemek: undefined }),
        periodId
      )
    ).toBe(false);
  });

  test('accepts zero for required values where the payroll domain permits it', () => {
    expect(
      hasCompletePeriodInstitutionParameters(
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
      hasCompletePeriodInstitutionParameters(institution({ gunlukTabanUcret: 0 }), periodId)
    ).toBe(false);
    expect(
      hasCompletePeriodInstitutionParameters(institution({ gunlukTabanUcret: -1 }), periodId)
    ).toBe(false);
  });
});

describe('kompakt puantaj sunumu', () => {
  test('keeps non-zero attendance codes in a compact readable order', () => {
    const summary: PuantajOzeti = { Ç: 20, T: 8, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 1 };
    expect(formatCompactPuantaj(summary)).toBe('20 Ç · 8 T · 1 R');
  });
});
