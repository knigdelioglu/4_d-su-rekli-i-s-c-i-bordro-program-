import { describe, expect, test } from 'bun:test';
import type { DönemselKurumDegerleri } from '../../types/payroll';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from '../../utils/payrollPresentation';
import { formatPeriodSettingsSaveError, savePeriodSettings } from './periodSettingsSave';

const persisted: DönemselKurumDegerleri = {
  ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
  donemId: '2027-03',
  gunlukTabanUcret: 2443.28,
};
const edited: DönemselKurumDegerleri = { ...persisted, gunlukTabanUcret: 2443.29 };

describe('period settings save feedback', () => {
  test('formats a structured finalized rejection in Turkish', () => {
    const message = formatPeriodSettingsSaveError({
      type: 'PayrollFinalized',
      message: 'Kesinleştirilmiş bordro mutasyonu reddedildi.',
    });

    expect(message.includes('Kesinleştirilmiş bordro')).toBe(true);
    expect(message.includes('Kesinleştirilmiş bordro mutasyonu reddedildi.')).toBe(true);
    expect(message.includes('[object Object]')).toBe(false);
  });

  test('formats structured messages, Error instances, and strings without object coercion', () => {
    expect(formatPeriodSettingsSaveError({ type: 'ValidationError', message: 'Tutar geçersiz.' }))
      .toBe('Ücret ayarları kaydedilemedi: Tutar geçersiz.');
    expect(formatPeriodSettingsSaveError(new Error('Bağlantı kesildi.')))
      .toBe('Ücret ayarları kaydedilemedi: Bağlantı kesildi.');
    expect(formatPeriodSettingsSaveError('Kayıt reddedildi.'))
      .toBe('Ücret ayarları kaydedilemedi: Kayıt reddedildi.');
    expect(formatPeriodSettingsSaveError({ unexpected: true }).includes('[object Object]')).toBe(false);
  });

  test('returns authoritative form values and an error outcome when persistence rejects', async () => {
    const savedMonths = [1, 7];
    const outcome = await savePeriodSettings(
      edited,
      [1, 4, 7],
      persisted,
      savedMonths,
      async () => { throw { type: 'PayrollFinalized', message: 'Nihai bordro kilidi.' }; },
      async () => undefined
    );

    expect(outcome).toEqual({
      kind: 'error',
      message: 'Kesinleştirilmiş bordro nedeniyle ücret ayarları değiştirilemedi. Nihai bordro kilidi.',
      paramsForm: persisted,
      zamAylariForm: savedMonths,
    });
    expect(outcome.kind).not.toBe('success');
  });

  test('returns success only after both save operations complete', async () => {
    const saved: string[] = [];
    const outcome = await savePeriodSettings(
      edited,
      [1, 7],
      persisted,
      [1],
      async () => { saved.push('params'); },
      async () => { saved.push('months'); }
    );

    expect(saved).toEqual(['params', 'months']);
    expect(outcome).toEqual({ kind: 'success' });
  });

  test('keeps a successfully persisted params form visible when only the later schedule save fails', async () => {
    const outcome = await savePeriodSettings(
      edited,
      [1, 4, 7],
      persisted,
      [1, 7],
      async () => undefined,
      async () => { throw new Error('Ayar kaydı reddedildi.'); }
    );

    expect(outcome).toEqual({
      kind: 'partial-error',
      message:
        'Ücret ayarları kaydedildi. Zam takvimi kaydedilemedi: Ayar kaydı reddedildi.',
      paramsForm: edited,
      zamAylariForm: [1, 7],
    });
  });
});
