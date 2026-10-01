import { describe, expect, test } from 'bun:test';
import { matchesPersonSearch } from './personSearch';

const alp = { ad: 'Alp', soyad: 'Senaryo47', tcNo: '12345678901', grup: 'A', unvan: 'İşçi', iban: 'TR00 0001' };

describe('personnel search', () => {
  test('full name, reversed order, partial and Turkish casing all match', () => {
    expect(matchesPersonSearch(alp, 'Alp Senaryo47')).toBe(true);
    expect(matchesPersonSearch(alp, '  senaryo47   alp ')).toBe(true);
    expect(matchesPersonSearch(alp, 'Alp')).toBe(true);
    expect(matchesPersonSearch(alp, 'İŞÇİ')).toBe(true);
    expect(matchesPersonSearch(alp, 'TR000001')).toBe(true);
    expect(matchesPersonSearch(alp, '')).toBe(true);
  });

  test('a word that matches nothing excludes the person', () => {
    expect(matchesPersonSearch(alp, 'Alp Senaryo48')).toBe(false);
  });
});
