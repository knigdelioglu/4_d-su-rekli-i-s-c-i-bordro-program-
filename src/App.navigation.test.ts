import { describe, expect, test } from 'bun:test';
import { getInitialActiveTab } from './App';

function storage(value: string | null): Pick<Storage, 'getItem'> {
  return { getItem: () => value };
}

describe('ilk açılış sekmesi', () => {
  test('storage kaydı yoksa Dönem Özeti açılır', () => {
    expect(getInitialActiveTab(storage(null))).toBe('ozet');
  });

  test('geçerli kayıtlı sekme korunur', () => {
    expect(getInitialActiveTab(storage('puantaj'))).toBe('puantaj');
  });

  test('geçersiz kayıt Dönem Özeti fallbackine döner', () => {
    expect(getInitialActiveTab(storage('gecersiz'))).toBe('ozet');
  });
});
