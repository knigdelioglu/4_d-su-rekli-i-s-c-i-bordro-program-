import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_PRODUCTION_KURUM_DEGERLERI,
  getDefaultAnnualPayrollParameters,
  ensureAnnualPayrollParameters,
} from './payrollDefaults';
import { DEFAULT_KURUM_DEGERLERI } from '../../utils/payrollPresentation';

describe('production payroll defaults', () => {
  test('provides the complete supported 2026 annual package', () => {
    const defaults = getDefaultAnnualPayrollParameters(2026);
    expect(defaults).toEqual({
      year: 2026,
      gelirVergisiDilimleri: [
        { limit: 190000, oran: 0.15 },
        { limit: 400000, oran: 0.2 },
        { limit: 1500000, oran: 0.27 },
        { limit: 5300000, oran: 0.35 },
        { limit: 1_000_000_000_000_000, oran: 0.4 },
      ],
      sigortaGvYillikBrutAsgariUcretTavani: 396360,
    });
    expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.gunlukAsgariUcret).toBe(1101);
    expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.pekTavanKatsayisi).toBe(9);
    expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.gunlukYemekIstisnasiSGK).toBe(300);
    expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.gunlukYemekIstisnasiGV).toBe(300);
    expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.isPrimiGruplari).toEqual([]);
    expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.tediyeListesi).toEqual([]);
    expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.tisIkramiyeListesi).toEqual([]);
  });

  test('does not silently create a future-year copy', () => {
    expect(getDefaultAnnualPayrollParameters(2027)).toBe(undefined);
    expect(ensureAnnualPayrollParameters([], 2027)).toEqual([]);
  });

  test('uses the same statutory period package for demo and production bootstrap', () => {
    const fields = [
      'sgkIsciOraniYuzde',
      'issizlikIsciOraniYuzde',
      'gelirVergisiOraniYuzde',
      'damgaVergisiOraniBinde',
      'gunlukAsgariUcret',
      'pekTavanKatsayisi',
      'gunlukYemekIstisnasiSGK',
      'gunlukYemekIstisnasiGV',
      'sgkIsverenOraniYuzde',
      'issizlikIsverenOraniYuzde',
    ] as const;
    expect(fields.map((field) => DEFAULT_PRODUCTION_KURUM_DEGERLERI[field])).toEqual(
      fields.map((field) => DEFAULT_KURUM_DEGERLERI[field])
    );
  });

  test('does not change an existing annual package when adding another person', () => {
    const existing = getDefaultAnnualPayrollParameters(2026)!;
    expect(ensureAnnualPayrollParameters([existing], 2026)).toEqual([existing]);
  });
});
