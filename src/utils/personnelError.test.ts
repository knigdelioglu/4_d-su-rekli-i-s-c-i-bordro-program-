import { describe, expect, test } from 'bun:test';
import { formatPersonnelSaveError, isDuplicateTcNoError } from './personnelError';

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

  test('maps only the structured duplicate T.C. code to its field message', () => {
    const nativeError = { type: 'DUPLICATE_TC_NO', message: 'UNIQUE constraint failed: personnel.tc_no' };
    const browserError = { type: 'PersonnelSaveError', code: 'DUPLICATE_TC_NO' };

    expect(isDuplicateTcNoError(nativeError)).toBe(true);
    expect(isDuplicateTcNoError(browserError)).toBe(true);
    expect(formatPersonnelSaveError(nativeError)).toBe(
      'Bu T.C. kimlik numarasıyla kayıtlı bir personel zaten bulunuyor. Lütfen numarayı kontrol edin.'
    );
    expect(isDuplicateTcNoError({ type: 'DatabaseError', message: 'UNIQUE constraint failed: unrelated.key' })).toBe(false);
    expect(formatPersonnelSaveError({ type: 'DatabaseError', message: 'UNIQUE constraint failed: unrelated.key' }))
      .toBe('UNIQUE constraint failed: unrelated.key');
  });
});
