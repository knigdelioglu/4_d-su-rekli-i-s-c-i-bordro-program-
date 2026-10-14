import { describe, expect, test } from 'bun:test';
import { validatePuantajOzeti } from './payrollPayloadSchema';

const ordinarySummary = {
  'Ç': 15,
  T: 4,
  G: 1,
  'İ': 2,
  'GÇ': 3,
  'GÇT': 1,
  R: 2,
};

describe('PuantajOzeti serde parity', () => {
  test('accepts omitted zero-valued Üİ and D fields in current snapshots', () => {
    expect(() => validatePuantajOzeti(ordinarySummary, '$.puantajOzeti')).not.toThrow();
    expect(() => validatePuantajOzeti({ ...ordinarySummary, 'Üİ': 0, D: 0 }, '$.puantajOzeti')).not.toThrow();
  });

  test('validates nonzero Üİ and D counts if serialized', () => {
    expect(() => validatePuantajOzeti({ ...ordinarySummary, 'Üİ': 3, D: 2 }, '$.puantajOzeti')).not.toThrow();
    expect(() => validatePuantajOzeti({ ...ordinarySummary, 'Üİ': '3' }, '$.puantajOzeti'))
      .toThrow('$.puantajOzeti["Üİ"] tam sayı olmalıdır.');
    expect(() => validatePuantajOzeti({ ...ordinarySummary, D: null }, '$.puantajOzeti'))
      .toThrow('$.puantajOzeti.D tam sayı olmalıdır.');
    expect(() => validatePuantajOzeti({ ...ordinarySummary, D: 1.25 }, '$.puantajOzeti'))
      .toThrow('$.puantajOzeti.D tam sayı olmalıdır.');
    expect(() => validatePuantajOzeti({ ...ordinarySummary, 'Üİ': undefined }, '$.puantajOzeti'))
      .toThrow('$.puantajOzeti["Üİ"] tam sayı olmalıdır.');
  });

  test('retains strict required counters', () => {
    const { R: _missing, ...withoutReport } = ordinarySummary;
    expect(() => validatePuantajOzeti(withoutReport, '$.puantajOzeti'))
      .toThrow('$.puantajOzeti.R zorunlu alan eksik.');
  });
});
