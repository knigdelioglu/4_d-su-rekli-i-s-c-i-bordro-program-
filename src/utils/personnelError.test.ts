import { describe, expect, test } from 'bun:test';
import { formatPersonnelSaveError } from './personnelError';

describe('personnel save errors', () => {
  test('explains finalized payroll rejection from a structured Tauri error', () => {
    const nativeError = {
      type: 'PayrollFinalized',
      message: 'Kesinleştirilmiş bordro/retro tarihçesini etkileyen veri değiştirilemez: p-1 / 2027-03 / normal.',
    };

    const message = formatPersonnelSaveError(nativeError);

    expect(message).toBe(
      'Bu değişiklik kesinleştirilmiş bir bordroyu etkilediği için personel bilgileri kaydedilemedi.'
    );
    expect(message).not.toContain('[object Object]');
  });

  test('preserves readable validation messages', () => {
    expect(
      formatPersonnelSaveError({ type: 'ValidationError', message: 'T.C. Kimlik numarası geçersiz.' })
    ).toBe('T.C. Kimlik numarası geçersiz.');
    expect(formatPersonnelSaveError(new Error('Alan zorunludur.'))).toBe('Alan zorunludur.');
  });
});
