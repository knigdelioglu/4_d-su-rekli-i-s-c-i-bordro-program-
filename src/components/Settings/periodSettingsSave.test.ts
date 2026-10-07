import { describe, expect, test } from 'bun:test';
import type { PayrollPeriodInstitutionSettings } from '../../types/payroll';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from '../../utils/payrollPresentation';
import { formatPeriodSettingsSaveError, savePeriodSettings } from './periodSettingsSave';

const persisted: PayrollPeriodInstitutionSettings = {
  ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
  donemId: '2027-03',
  yemekTamGvIstisnaDamgaTabi: true,
};

describe('period settings save feedback', () => {
  test('formats structured errors without object coercion', () => {
    expect(formatPeriodSettingsSaveError({
      type: 'PayrollFinalized',
      message: 'Bordro kesinleşmiş.',
    })).toBe('Kesinleştirilmiş bordro nedeniyle kurum ayarları değiştirilemedi. Bordro kesinleşmiş.');
    expect(formatPeriodSettingsSaveError({ message: { code: 'E_TEST' } }))
      .toBe('Kurum ayarları kaydedilemedi: {"code":"E_TEST"}');
  });

  test('does not persist an unchanged institution setting', async () => {
    const saves: PayrollPeriodInstitutionSettings[] = [];
    const result = await savePeriodSettings(
      { ...persisted },
      persisted,
      (settings) => { saves.push(settings); }
    );

    expect(result).toEqual({ kind: 'success' });
    expect(saves).toEqual([]);
  });

  test('saves only the edited non-compensation institution preference', async () => {
    const saves: PayrollPeriodInstitutionSettings[] = [];
    const edited = { ...persisted, yemekTamGvIstisnaDamgaTabi: false };
    const result = await savePeriodSettings(edited, persisted, (settings) => { saves.push(settings); });

    expect(result).toEqual({ kind: 'success' });
    expect(saves).toEqual([edited]);
  });

  test('returns the persisted form after a failed write', async () => {
    const edited = { ...persisted, yemekTamGvIstisnaDamgaTabi: false };
    const result = await savePeriodSettings(edited, persisted, async () => {
      throw { type: 'PayrollFinalized', message: 'Nihai bordro kilidi.' };
    });

    expect(result).toEqual({
      kind: 'error',
      message: 'Kesinleştirilmiş bordro nedeniyle kurum ayarları değiştirilemedi. Nihai bordro kilidi.',
      paramsForm: persisted,
    });
  });
});
