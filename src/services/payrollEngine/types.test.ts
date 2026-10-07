import { describe, expect, test } from 'bun:test';
import type { PayrollPeriodInstitutionSettings } from '../../types/payroll';

describe('active period settings boundary', () => {
  test('contains legal, deduction, meal-tax, and Tediye/TİS inputs only', () => {
    const active: PayrollPeriodInstitutionSettings = {
      donemId: '2026-09',
      yemekTamGvIstisnaDamgaTabi: true,
      besOraniYuzde: 5,
      tediyeListesi: [{ id: 3, ad: 'Avans', odemeAyi: 'Mart', gunSayisi: 1, sabitTutar: 1000, aktifDonemdeOdensin: true }],
    };

    expect(active.besOraniYuzde).toBe(5);
    expect(active.tediyeListesi?.[0].sabitTutar).toBe(1000);
    expect(Object.keys(active).some((key) => [
      'gunlukTabanUcret', 'gunlukYemek', 'birlestirilmisSosyalYardim', 'gunlukVasitaYol',
      'transportRateChanges', 'giyimYardimi', 'hizmetZammiBirimi', 'isPrimiYuzde',
      'isPrimiGruplari', 'geceCalismaPrimiYuzde', 'geceCalismaTatiliPrimiYuzde',
      'ekOdeme', 'digerGelirVarsayilan',
    ].includes(key))).toBe(false);
  });
});
