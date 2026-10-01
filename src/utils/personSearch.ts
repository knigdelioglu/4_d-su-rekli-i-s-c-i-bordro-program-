import type { Personel } from '../types/payroll';

function normalizeSearchText(value: string | undefined | null): string {
  return (value ?? '').toLocaleLowerCase('tr-TR').replace(/\s+/g, ' ').trim();
}

/**
 * Personnel search shared by every list screen. The query is split into
 * words and every word must match some field, so "Alp Senaryo47",
 * "senaryo47 alp" and "Alp" all find the same person (Turkish casing aware).
 */
export function matchesPersonSearch(
  person: Pick<Personel, 'ad' | 'soyad' | 'tcNo'> & Partial<Pick<Personel, 'grup' | 'unvan' | 'iban'>>,
  query: string
): boolean {
  const tokens = normalizeSearchText(query).split(' ').filter(Boolean);
  if (tokens.length === 0) return true;
  const fields = [
    normalizeSearchText(`${person.ad} ${person.soyad}`),
    normalizeSearchText(person.tcNo),
    normalizeSearchText(person.grup),
    normalizeSearchText(person.unvan),
    normalizeSearchText(person.iban),
    normalizeSearchText(person.iban).replace(/ /g, ''),
  ];
  return tokens.every((token) => fields.some((field) => field.includes(token)));
}
