import { describe, expect, test } from 'bun:test';
import { formatPayrollError } from './useBordroCalculationController';

describe('payroll error messages', () => {
  test('turns missing annual parameters into an actionable message', () => {
    expect(
      formatPayrollError(
        new Error('2026 vergi yılı yıllık bordro parametreleri bulunamadı; bordro hesaplanamaz.')
      )
    ).toBe('2026 yılı gelir vergisi tarifesi henüz tanımlı değil. Yıllık Parametreler bölümünü tamamlayın.');
  });

  test('turns missing statutory parameters into an actionable message', () => {
    expect(
      formatPayrollError(new Error('2026 dönemi için zorunlu yasal parametre eksik: gunlukAsgariUcret.'))
    ).toBe('Dönem yasal parametreleri henüz tamamlanmamış. Dönem Parametreleri bölümünü tamamlayın.');
  });
});
