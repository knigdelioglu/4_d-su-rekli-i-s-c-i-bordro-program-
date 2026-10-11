import { describe, expect, test } from 'bun:test';
import type { Personel } from '../types/payroll';
import {
  assertUniqueTcNo,
  DUPLICATE_TC_NO_CODE,
  DUPLICATE_TC_NO_MESSAGE,
  normalizeTurkishIban,
  parseNonNegativeAmountInput,
  PersonnelSaveError,
  validatePersonnelForSave,
  validatePersonnelSnapshotMutation,
} from './personnelValidation';

const validIban = 'TR330006100519786457841326';

function person(overrides: Partial<Personel> = {}): Personel {
  return {
    id: 'person-1',
    tcNo: '11111111110',
    ad: 'Ayşe',
    soyad: 'Yılmaz',
    grup: '1. Grup',
    sgkSicilNo: '',
    iban: validIban,
    hizmetYili: 1,
    kesintiler: {},
    ...overrides,
  };
}

describe('personnel save validation', () => {
  test('normalizes grouping whitespace and case without changing the account field', () => {
    expect(normalizeTurkishIban('  tr33 0006 1005 1978 6457 8413 26  ')).toBe(validIban);
    expect(normalizeTurkishIban(` ${validIban.slice(0, 4)} ${validIban.slice(4)} `)).toBe(validIban);
  });

  test('rejects incomplete, malformed, and checksum-invalid Turkish IBANs', () => {
    const malformed = [
      'TR',
      validIban.slice(0, -1),
      `${validIban}0`,
      `${validIban.slice(0, 2)}34${validIban.slice(4)}`,
      'TR3A0006100519786457841326',
      'TR3300061100519786457841326',
      'TR33000610051978645784132A',
      '',
    ];
    for (const iban of malformed) expect(normalizeTurkishIban(iban)).toBeNull();
  });

  test('rejects negative and malformed fixed deduction input without changing its sign', () => {
    for (const value of ['-1', '-0.01', 'abc', '1.239']) {
      const parsed = parseNonNegativeAmountInput(value);
      expect(parsed.valid).toBe(false);
    }
    expect(parseNonNegativeAmountInput('-1')).toEqual({
      valid: false,
      message: 'Kesinti tutarı negatif olamaz.',
    });
    expect(parseNonNegativeAmountInput('')).toEqual({ valid: true, value: undefined });
    expect(parseNonNegativeAmountInput('0')).toEqual({ valid: true, value: 0 });
    expect(parseNonNegativeAmountInput('0,01')).toEqual({ valid: true, value: 0.01 });
    expect(parseNonNegativeAmountInput('0.01')).toEqual({ valid: true, value: 0.01 });
    expect(parseNonNegativeAmountInput('1.23')).toEqual({ valid: true, value: 1.23 });
  });

  test('validates every persistent fixed deduction and GV deduction field at the browser boundary', () => {
    const deductions = person({
      kesintiler: {
        sendikaUyesi: true,
        sabitSendikaAidati: 0,
        sabitBesTutar: 0.01,
        icraTutar: 0,
        nafakaTutar: 0.01,
        kisiBorcuTutar: 1,
        dogumAskerlikBorclanmasiTutar: 0,
        hayatSaglikSigortasiTutar: 0.01,
        digerKesintiTutar: 0,
        gvIndirimleri: { saglikSigortasiPrimiTutar: 0.01 },
      },
    });
    expect(validatePersonnelForSave(deductions)).toBeNull();
    for (const field of [
      'sabitSendikaAidati',
      'sabitBesTutar',
      'icraTutar',
      'nafakaTutar',
      'kisiBorcuTutar',
      'dogumAskerlikBorclanmasiTutar',
      'hayatSaglikSigortasiTutar',
      'digerKesintiTutar',
    ] as const) {
      const invalid = person({ kesintiler: { [field]: -0.01 } });
      expect(validatePersonnelForSave(invalid)?.field).toBe(`kesintiler.${field}`);
    }
    expect(
      validatePersonnelForSave(
        person({ kesintiler: { gvIndirimleri: { saglikSigortasiPrimiTutar: -1 } } })
      )?.field
    ).toBe('kesintiler.gvIndirimleri.saglikSigortasiPrimiTutar');
  });

  test('keeps old invalid IBAN snapshots readable but rejects a changed invalid record', () => {
    const oldPerson = person({ iban: 'TR' });
    expect(
      validatePersonnelSnapshotMutation(
        { personeller: [oldPerson] },
        { personeller: [oldPerson], exportedAt: 'new' } as { personeller: Personel[] }
      ).personeller[0].iban
    ).toBe('TR');

    expect(() =>
      validatePersonnelSnapshotMutation(
        { personeller: [oldPerson] },
        { personeller: [{ ...oldPerson, ad: 'Yeni Ad' }], exportedAt: 'new' } as {
          personeller: Personel[];
        }
      )
    ).toThrow('IBAN');
  });

  test('rejects duplicate T.C. numbers, but allows a person to keep their own number', () => {
    const employees = [person(), person({ id: 'person-2', tcNo: '22222222220' })];
    expect(() => assertUniqueTcNo(employees, { id: 'new-person', tcNo: ' 11111111110 ' }))
      .toThrow(DUPLICATE_TC_NO_MESSAGE);
    expect(() => assertUniqueTcNo(employees, { id: 'person-1', tcNo: '11111111110' })).not.toThrow();
    try {
      assertUniqueTcNo(employees, { id: 'new-person', tcNo: '11111111110' });
    } catch (error) {
      expect(error instanceof PersonnelSaveError).toBe(true);
      expect((error as PersonnelSaveError).type).toBe(DUPLICATE_TC_NO_CODE);
    }
  });
});
