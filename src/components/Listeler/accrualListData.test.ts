import { describe, expect, test } from 'bun:test';
import type { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import {
  countAuthoritativeNormalPersonnel,
  filterAccrualRowsByPaymentDate,
  getAuthoritativeAccrualRows,
} from './accrualListData';

const period = {
  id: '2026-09',
  yil: 2026,
  ay: 9,
  baslangicTarihi: '2026-09-15',
  bitisTarihi: '2026-10-14',
  donemAdi: 'Eylül 2026',
  taxYear: 2026,
  taxMonth: 10,
} as BordroDonemi;

const person = {
  id: 'p-1',
  ad: 'Ali',
  soyad: 'Yılmaz',
  tcNo: '10000000000',
  iban: 'TR000000000000000000000000',
} as Personel;

function payroll(
  accrualType: BordroKaydi['accrualType'],
  accrualId: string,
  paymentDate: string,
  sequence: number,
  bes: number,
  status: BordroKaydi['status'] = 'CALCULATED',
  netOdeme = 0
): BordroKaydi {
  return {
    id: accrualId,
    personelId: person.id,
    donemId: period.id,
    accrualId,
    accrualType,
    paymentDate,
    sequence,
    status,
    netOdeme,
    kesintiler: { bes },
  } as unknown as BordroKaydi;
}

describe('authoritative accrual list dataset', () => {
  test('keeps every authoritative payment event and excludes DRAFT/STALE', () => {
    const rows = getAuthoritativeAccrualRows(period, [person], [
      payroll('TEDIYE', 'tediye-1', '2026-10-20', 1, 120),
      payroll('NORMAL', 'normal-1', '2026-10-13', 0, 900, 'FINALIZED'),
      payroll('SUPPLEMENTAL', 'stale-1', '2026-10-21', 2, 50, 'STALE'),
    ]);

    expect(rows.map((row) => row.accrualId)).toEqual(['normal-1', 'tediye-1']);
    expect(rows.map((row) => row.accrualTypeLabel)).toEqual(['Normal Maaş', 'Tediye']);
    expect(rows.reduce((total, row) => total + (row.bordro.kesintiler.bes ?? 0), 0)).toBe(1020);
  });

  test('payment date filtering returns the exact accrual subset', () => {
    const rows = getAuthoritativeAccrualRows(period, [person], [
      payroll('NORMAL', 'normal-1', '2026-10-13', 0, 900, 'CALCULATED', 65000),
      payroll('TEDIYE', 'tediye-1', '2026-10-20', 1, 120, 'CALCULATED', 11500),
    ]);

    expect(filterAccrualRowsByPaymentDate(rows, 'all').length).toBe(2);
    expect(rows.reduce((total, row) => total + row.bordro.netOdeme, 0)).toBe(76500);
    expect(filterAccrualRowsByPaymentDate(rows, '2026-10-13').length).toBe(1);
    expect(filterAccrualRowsByPaymentDate(rows, '2026-10-13')[0].bordro.accrualType).toBe('NORMAL');
    expect(filterAccrualRowsByPaymentDate(rows, '2026-10-13')[0].bordro.netOdeme).toBe(65000);
    expect(filterAccrualRowsByPaymentDate(rows, '2026-10-20')[0].bordro.kesintiler.bes).toBe(120);
    expect(filterAccrualRowsByPaymentDate(rows, '2026-10-20')[0].bordro.netOdeme).toBe(11500);
  });

  test('counts people by authoritative NORMAL accrual, not by total accrual count', () => {
    const normalPayrolls = Array.from({ length: 100 }, (_, index) => ({
      ...payroll('NORMAL', `normal-${index}`, '2026-10-13', 0, 0),
      personelId: `person-${index}`,
    }));
    const supplementaryPayrolls = Array.from({ length: 8 }, (_, index) => ({
      ...payroll('TEDIYE', `tediye-${index}`, '2026-10-20', 1, 0),
      personelId: `person-${index}`,
    }));

    expect(
      countAuthoritativeNormalPersonnel(
        [...normalPayrolls, ...supplementaryPayrolls],
        period.id
      )
    ).toBe(100);
  });
});
