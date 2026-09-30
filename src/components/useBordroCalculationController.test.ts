import { describe, expect, test } from 'bun:test';
import {
  formatPayrollError,
  formatStalePayrollMessage,
  isPayrollTaxOpeningConfigurationError,
  tryAcquireSupplementaryPaymentScope,
} from './useBordroCalculationController';
import { paymentEventSequenceScopeKey } from '../services/payrollEngine/paymentEventOrder';

describe('payroll error messages', () => {
  test('explains that stale payroll source data changed without blaming a prior period', () => {
    const message = formatStalePayrollMessage('Ayşe Kaya');

    expect(message).toContain('Ayşe Kaya bordrosu kaynak verilerindeki değişiklik nedeniyle güncelliğini yitirdi');
    expect(message).toContain('yeniden hesaplayın');
    expect(message).not.toContain('önceki dönem değişikliği');
  });

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

  test('formats structured Tauri domain errors instead of showing object identity', () => {
    expect(
      formatPayrollError({
        type: 'ValidationError',
        message: 'p-1_2026-09 tahakkuku STALE durumda; önceki zincir çözümlenemiyor.',
      })
    ).toBe('p-1_2026-09 tahakkuku STALE durumda; önceki zincir çözümlenemiyor.');
  });

  test('formats attendance save errors safely across native, Error, and string variants', () => {
    expect(
      formatPayrollError({
        type: 'PayrollFinalized',
        message: 'Kesinleştirilmiş bordro tarihçesini etkileyen veri değiştirilemez.',
      })
    ).toBe('Kesinleştirilmiş bordro tarihçesini etkileyen veri değiştirilemez.');
    expect(formatPayrollError(new Error('Kayıt reddedildi.'))).toBe('Kayıt reddedildi.');
    expect(formatPayrollError('Kayıt reddedildi.')).toBe('Kayıt reddedildi.');
  });

  test('does not expose object identity for an unprintable error object', () => {
    const error: Record<string, unknown> = {};
    error.self = error;

    expect(formatPayrollError(error)).toBe('Beklenmeyen bir hata oluştu.');
    expect(formatPayrollError(error).includes('[object Object]')).toBe(false);
  });

  test('formats a structured retro preview failure as its readable message', () => {
    const error = {
      type: 'ValidationError',
      message: '2027-01 dönemindeki tahakkuk authoritative değil; retro hesap durduruldu.',
    };

    expect(formatPayrollError(error)).toBe(error.message);
    expect(formatPayrollError(error)).not.toBe(String(error));
  });

  test('turns an unresolved legacy tax opening into an actionable message', () => {
    const message = formatPayrollError(
      new Error(
        'Legacy GV opening için legacy başlangıç ayı period ID ile çözülemiyor; explicit effectiveFromPeriodId girin.'
      )
    );

    expect(message).toBe(
      'Kümülatif gelir vergisi açılışının başlangıç dönemi eksik veya geçersiz. Bordro ekranındaki "Önceki Kümülatif Matrah Girişi" bölümünü açıp tutarı aktif dönemle kaydedin; ardından bordroyu yeniden hesaplayın.'
    );
    expect(isPayrollTaxOpeningConfigurationError(message)).toBe(false);
    expect(
      isPayrollTaxOpeningConfigurationError(
        'Hesaplama hatası: Legacy GV opening için explicit effectiveFromPeriodId girin.'
      )
    ).toBe(true);
    expect(
      isPayrollTaxOpeningConfigurationError(
        'Mevcut normal GV opening effective dönemi eksik; kayıt güvenli biçimde güncellenemiyor.'
      )
    ).toBe(true);
  });
});

test('does not allocate one payment sequence scope to overlapping submissions', () => {
  const pending = new Set<string>();
  const scope = paymentEventSequenceScopeKey('person-1', 2026, 8, '2026-08-10');
  const release = tryAcquireSupplementaryPaymentScope(pending, scope);

  expect(release !== null).toBe(true);
  expect(tryAcquireSupplementaryPaymentScope(pending, scope)).toBe(null);
  release!();
  release!();
  const releaseAfterRetry = tryAcquireSupplementaryPaymentScope(pending, scope);
  expect(releaseAfterRetry !== null).toBe(true);
  releaseAfterRetry!();
});
