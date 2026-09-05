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
  OPEN_ENDED_TAX_BRACKET_LIMIT,
  createBordroDonemi,
} from './payrollPresentation';

const activePeriod = createBordroDonemi(2026, 9, 2026, 10);
const periodId = activePeriod.id;

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

function legal(overrides: Partial<DönemselKurumDegerleri> = {}): boolean {
  return hasCompletePeriodLegalParameters(institution(overrides), activePeriod);
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

  test('rejects non-finite required values', () => {
    expect(
      hasCompletePeriodIncomeParameters(institution({ gunlukYemek: Number.NaN }), periodId)
    ).toBe(false);
    expect(
      hasCompletePeriodIncomeParameters(
        institution({ gunlukVasitaYol: Number.POSITIVE_INFINITY }),
        periodId
      )
    ).toBe(false);
    expect(
      hasCompletePeriodIncomeParameters(
        institution({ gunlukYemek: null as unknown as number }),
        periodId
      )
    ).toBe(false);
  });
});

describe('dönem vergi ve yasal oran readiness', () => {
  test('accepts a complete legal parameter set', () => {
    expect(hasCompletePeriodLegalParameters(institution(), activePeriod)).toBe(true);
  });

  test('requires every authoritative legal field', () => {
    expect(
      hasCompletePeriodLegalParameters(
        institution({ sgkIsciOraniYuzde: undefined }),
        activePeriod
      )
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(
        institution({ damgaVergisiOraniBinde: undefined }),
        activePeriod
      )
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(institution({ gunlukAsgariUcret: 0 }), activePeriod)
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(institution({ pekTavanKatsayisi: 0.99 }), activePeriod)
    ).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(institution({ isPrimiGruplari: undefined }), activePeriod)
    ).toBe(false);
    expect(hasCompletePeriodLegalParameters(institution({ isPrimiGruplari: [] }), activePeriod)).toBe(
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
        activePeriod
      )
    ).toBe(true);
    expect(
      hasCompletePeriodLegalParameters(
        institution({ isPrimiGruplari: [{ id: '1', ad: 'Grup', oran: -1, aktif: true }] }),
        activePeriod
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
        activePeriod
      )
    ).toBe(false);
  });

  test('requires the active period object and matching period id', () => {
    expect(hasCompletePeriodLegalParameters(institution(), undefined)).toBe(false);
    expect(
      hasCompletePeriodLegalParameters(institution(), { ...activePeriod, id: '2026-10' })
    ).toBe(false);
  });

  test('rejects non-finite required values and accepts legal zero values', () => {
    expect(legal({ sgkIsciOraniYuzde: null as unknown as number })).toBe(false);
    expect(legal({ sgkIsciOraniYuzde: Number.NaN })).toBe(false);
    expect(legal({ sgkIsciOraniYuzde: Number.POSITIVE_INFINITY })).toBe(false);
    expect(
      legal({
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
      })
    ).toBe(true);
  });

  test('accepts percentage 100 and rejects values above the production bound', () => {
    const percentageFields = [
      'sgkIsciOraniYuzde',
      'issizlikIsciOraniYuzde',
      'sendikaAidatiYuzde',
      'besOraniYuzde',
      'geceCalismaPrimiYuzde',
      'geceCalismaTatiliPrimiYuzde',
      'sgkIsverenOraniYuzde',
      'issizlikIsverenOraniYuzde',
    ] as const;

    for (const field of percentageFields) {
      expect(legal({ [field]: 100 })).toBe(true);
      expect(legal({ [field]: 100.01 })).toBe(false);
    }
  });

  test('enforces stamp-tax, minimum-wage, and PEK bounds', () => {
    expect(legal({ damgaVergisiOraniBinde: 1000 })).toBe(true);
    expect(legal({ damgaVergisiOraniBinde: 1000.01 })).toBe(false);
    expect(legal({ gunlukAsgariUcret: 0 })).toBe(false);
    expect(legal({ pekTavanKatsayisi: 0.99 })).toBe(false);
  });

  test('rejects negative optional constrained values', () => {
    expect(legal({ sabitSendikaAidati: -1 })).toBe(false);
    expect(legal({ sabitBesTutar: -1 })).toBe(false);
    expect(legal({ ekOdeme: -1 })).toBe(false);
    expect(legal({ digerGelirVarsayilan: -1 })).toBe(false);
    expect(legal({ ekOdeme: Number.NaN })).toBe(false);
    expect(legal({ digerGelirVarsayilan: Number.POSITIVE_INFINITY })).toBe(false);
    expect(legal({ sabitSendikaAidati: null as unknown as number })).toBe(true);
  });
});

describe('iş primi grupları production parity', () => {
  test('rejects duplicate active ids and names after trimming', () => {
    expect(
      legal({
        isPrimiGruplari: [
          { id: ' 1 ', ad: 'Bir', oran: 0, aktif: true },
          { id: '1', ad: 'İki', oran: 0, aktif: true },
        ],
      })
    ).toBe(false);
    expect(
      legal({
        isPrimiGruplari: [
          { id: '1', ad: ' Grup ', oran: 0, aktif: true },
          { id: '2', ad: 'Grup', oran: 0, aktif: true },
        ],
      })
    ).toBe(false);
  });

  test('does not include duplicate inactive groups in the active identity sets', () => {
    expect(
      legal({
        isPrimiGruplari: [
          { id: '1', ad: 'Pasif', oran: 0, aktif: false },
          { id: ' 1 ', ad: ' Pasif ', oran: 100, aktif: false },
          { id: '2', ad: 'Aktif', oran: 0, aktif: true },
        ],
      })
    ).toBe(true);
  });

  test('validates every group, including inactive groups', () => {
    expect(legal({ isPrimiGruplari: [{ id: '  ', ad: 'Grup', oran: 0, aktif: true }] })).toBe(
      false
    );
    expect(
      legal({ isPrimiGruplari: [{ id: '1', ad: 'Grup', oran: 100.01, aktif: false }] })
    ).toBe(false);
    expect(legal({ isPrimiGruplari: [{ id: '1', ad: 'Grup', oran: Number.NaN }] })).toBe(false);
  });
});

describe('statutory parameter segments production parity', () => {
  test('accepts missing, empty, and valid segment lists', () => {
    expect(legal({ statutoryParameterSegments: undefined })).toBe(true);
    expect(legal({ statutoryParameterSegments: [] })).toBe(true);
    expect(
      legal({
        statutoryParameterSegments: [
          { effectiveFrom: '2026-09-15', gunlukAsgariUcret: 1102 },
        ],
      })
    ).toBe(true);
    expect(
      legal({
        statutoryParameterSegments: [
          { effectiveFrom: '2026-09-15', gunlukAsgariUcret: 1102 },
          { effectiveFrom: '2026-10-01', pekTavanKatsayisi: 9.5 },
          { effectiveFrom: '2026-10-14', gunlukYemekIstisnasiGV: 0 },
        ],
      })
    ).toBe(true);
  });

  test('rejects duplicate, decreasing, out-of-period, and invalid calendar dates', () => {
    expect(
      legal({
        statutoryParameterSegments: [
          { effectiveFrom: '2026-09-15', gunlukAsgariUcret: 1102 },
          { effectiveFrom: '2026-09-15', pekTavanKatsayisi: 9.5 },
        ],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [
          { effectiveFrom: '2026-10-01', gunlukAsgariUcret: 1102 },
          { effectiveFrom: '2026-09-20', pekTavanKatsayisi: 9.5 },
        ],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-09-14', gunlukAsgariUcret: 1102 }],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-10-15', gunlukAsgariUcret: 1102 }],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-09-31', gunlukAsgariUcret: 1102 }],
      })
    ).toBe(false);
  });

  test('requires an override and validates each override bound', () => {
    expect(legal({ statutoryParameterSegments: [{ effectiveFrom: '2026-09-15' }] })).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-09-15', gunlukAsgariUcret: 0 }],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-09-15', pekTavanKatsayisi: 0.99 }],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-09-15', gunlukYemekIstisnasiSGK: -1 }],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-09-15', gunlukYemekIstisnasiGV: -1 }],
      })
    ).toBe(false);
    expect(
      legal({
        statutoryParameterSegments: [{ effectiveFrom: '2026-09-15', pekTavanKatsayisi: Number.NaN }],
      })
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
    expect(hasCompleteAnnualPayrollParameters(annual({ year: 0 }), 2026)).toBe(false);
  });

  test('rejects invalid bracket limits, rates, and non-increasing limits', () => {
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
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ gelirVergisiDilimleri: [{ limit: 190000, oran: -0.01 }] }),
        2026
      )
    ).toBe(false);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({
          gelirVergisiDilimleri: [
            { limit: 190000, oran: 0.15 },
            { limit: 190000, oran: 0.2 },
          ],
        }),
        2026
      )
    ).toBe(false);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ gelirVergisiDilimleri: [{ limit: Number.NaN, oran: 0 }] }),
        2026
      )
    ).toBe(false);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({
          gelirVergisiDilimleri: [
            { limit: OPEN_ENDED_TAX_BRACKET_LIMIT + 1, oran: 0.4 },
          ],
        }),
        2026
      )
    ).toBe(false);
  });

  test('validates the optional annual insurance GV cap without migration defaults', () => {
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ sigortaGvYillikBrutAsgariUcretTavani: 0 }),
        2026
      )
    ).toBe(false);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ sigortaGvYillikBrutAsgariUcretTavani: -1 }),
        2026
      )
    ).toBe(false);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ sigortaGvYillikBrutAsgariUcretTavani: 396360 }),
        2026
      )
    ).toBe(true);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ sigortaGvYillikBrutAsgariUcretTavani: undefined }),
        2026
      )
    ).toBe(true);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ sigortaGvYillikBrutAsgariUcretTavani: null as unknown as number }),
        2026
      )
    ).toBe(true);
    expect(
      hasCompleteAnnualPayrollParameters(
        annual({ sigortaGvYillikBrutAsgariUcretTavani: Number.POSITIVE_INFINITY }),
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
