import { describe, expect, test } from 'bun:test';
import type { DönemselKurumDegerleri } from '../../types/payroll';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from '../../utils/payrollPresentation';
import { formatPeriodSettingsSaveError, hasValidInitialWorkBonusGroups, isPositiveDailyBaseWage, savePeriodSettings } from './periodSettingsSave';

const persisted: DönemselKurumDegerleri = {
  ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
  donemId: '2027-03',
  gunlukTabanUcret: 2443.28,
};
const edited: DönemselKurumDegerleri = { ...persisted, gunlukTabanUcret: 2443.29 };

describe('period settings save feedback', () => {
  test('requires a positive daily base wage for first-period creation', () => {
    expect(isPositiveDailyBaseWage('')).toBe(false);
    expect(isPositiveDailyBaseWage('0')).toBe(false);
    expect(isPositiveDailyBaseWage('-0.01')).toBe(false);
    expect(isPositiveDailyBaseWage('2443.28')).toBe(true);
  });

  test('requires institution work-bonus group names and rates without supplying invented defaults', () => {
    expect(hasValidInitialWorkBonusGroups([])).toBe(false);
    expect(hasValidInitialWorkBonusGroups([{ id: 'g1', ad: '', oran: 5 }])).toBe(false);
    expect(hasValidInitialWorkBonusGroups([{ id: 'g1', ad: 'Temizlik', oran: Number.NaN }])).toBe(false);
    expect(hasValidInitialWorkBonusGroups([{ id: 'g1', ad: 'Temizlik', oran: 5 }])).toBe(true);
  });

  test('never renders object identity for supported error shapes', () => {
    const messages = [
      formatPeriodSettingsSaveError(new Error('Hata oluştu.')),
      formatPeriodSettingsSaveError('Düz metin hata.'),
      formatPeriodSettingsSaveError({ type: 'PayrollFinalized', message: 'Bordro kesinleşmiş.' }),
      formatPeriodSettingsSaveError({ message: 'Mesaj alanı.' }),
      formatPeriodSettingsSaveError({ message: { code: 'E_TEST' } }),
      formatPeriodSettingsSaveError({ unexpected: true }),
      formatPeriodSettingsSaveError(null),
      formatPeriodSettingsSaveError(undefined),
    ];

    expect(messages).toEqual([
      'Ücret ayarları kaydedilemedi: Hata oluştu.',
      'Ücret ayarları kaydedilemedi: Düz metin hata.',
      'Kesinleştirilmiş bordro nedeniyle ücret ayarları değiştirilemedi. Bordro kesinleşmiş.',
      'Ücret ayarları kaydedilemedi: Mesaj alanı.',
      'Ücret ayarları kaydedilemedi: {"code":"E_TEST"}',
      'Ücret ayarları kaydedilemedi. Lütfen tekrar deneyin.',
      'Ücret ayarları kaydedilemedi. Lütfen tekrar deneyin.',
      'Ücret ayarları kaydedilemedi. Lütfen tekrar deneyin.',
    ]);
    expect(messages.every((message) => !message.includes('[object Object]'))).toBe(true);
  });

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

  test('does not re-save unchanged sections (no ALL/PERIOD invalidation on a no-op save)', async () => {
    const saved: string[] = [];
    const outcome = await savePeriodSettings(
      { ...persisted },
      [7, 1],
      persisted,
      [1, 7],
      async () => { saved.push('params'); },
      async () => { saved.push('months'); }
    );

    expect(saved).toEqual([]);
    expect(outcome).toEqual({ kind: 'success' });
  });

  test('saves only the zam schedule when only the schedule changed', async () => {
    const saved: string[] = [];
    await savePeriodSettings(
      persisted,
      [1, 4, 7],
      persisted,
      [1, 7],
      async () => { saved.push('params'); },
      async () => { saved.push('months'); }
    );

    expect(saved).toEqual(['months']);
  });

  test('saves only period params when the zam schedule is unchanged', async () => {
    const saved: string[] = [];
    await savePeriodSettings(
      edited,
      [1, 7],
      persisted,
      [1, 7],
      async () => { saved.push('params'); },
      async () => { saved.push('months'); }
    );

    expect(saved).toEqual(['params']);
  });
});
