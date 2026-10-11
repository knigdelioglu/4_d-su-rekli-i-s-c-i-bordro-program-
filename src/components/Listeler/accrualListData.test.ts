import { describe, expect, test } from 'bun:test';
import type { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import {
  countAuthoritativeNormalPersonnel,
  describeBankExcludedAccruals,
  filterAccrualRowsByPaymentDate,
  getAuthoritativeAccrualRows,
  getBankTransferableAccrualRows,
  getOtherPeriodAccruals,
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
  iban: 'TR330006100519786457841326',
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
  test('locates supplementary accruals in another work period including stale records', () => {
    const otherPeriod = { ...period, id: '2027-02', donemAdi: 'Şubat 2027', taxYear: 2027, taxMonth: 3 };
    for (const [type, gross] of [['TEDIYE', 24500], ['TIS_IKRAMIYE', 18000], ['SUPPLEMENTAL', 12000]] as const) {
      const saved = { ...payroll(type, type, '2027-03-14', 0, 0, 'STALE'), donemId: otherPeriod.id, gelirToplam: gross };
      const unrelated = { ...saved, id: 'other-person', personelId: 'other' };
      expect(getOtherPeriodAccruals([saved, unrelated], [period, otherPeriod], person.id, type, period.id))
        .toEqual([{ payroll: saved, periodLabel: 'Şubat 2027' }]);
      expect(getAuthoritativeAccrualRows(period, [person], [saved])).toEqual([]);
      expect(getOtherPeriodAccruals([saved], [period, otherPeriod], person.id, type, otherPeriod.id)).toEqual([]);
    }
    expect(getOtherPeriodAccruals([], [period], person.id, 'TEDIYE', period.id)).toEqual([]);
  });
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

  test('bank transfer list includes only finalized payrolls and explains calculated exclusions', () => {
    const payrolls = [
      payroll('NORMAL', 'normal-final', '2026-10-13', 0, 900, 'FINALIZED', 65000),
      payroll('TEDIYE', 'tediye-calculated', '2026-10-20', 1, 120, 'CALCULATED', 11500),
      payroll('SUPPLEMENTAL', 'stale-event', '2026-10-21', 2, 50, 'STALE'),
      payroll('NORMAL', 'draft-event', '2026-10-22', 0, 50, 'DRAFT'),
    ];

    const rows = getBankTransferableAccrualRows(period, [person], payrolls);
    const notice = describeBankExcludedAccruals(period, [person], payrolls);

    expect(rows.map((row) => row.accrualId)).toEqual(['normal-final']);
    expect(notice).toContain('1 hesaplanmış tahakkuk henüz kesinleştirilmedi');
    expect(notice).toContain('yeniden hesaplanmalı');
    expect(notice).toContain('banka listesine alınır');
  });

  test('excludes finalized records with invalid legacy IBANs and keeps year/date totals isolated', () => {
    const june2028 = {
      ...period,
      id: '2028-06',
      yil: 2028,
      ay: 6,
      baslangicTarihi: '2028-06-15',
      bitisTarihi: '2028-07-14',
      donemAdi: 'Haziran 2028',
      taxYear: 2028,
      taxMonth: 7,
    };
    const validSecondPerson = { ...person, id: 'p-2', ad: 'Ayşe' };
    const legacyInvalidPerson = { ...person, id: 'p-3', ad: 'Eski', iban: 'TR' };
    const rows = [
      { ...payroll('NORMAL', 'june-normal', '2028-06-30', 0, 0, 'FINALIZED', 1_744_659.78), donemId: june2028.id },
      { ...payroll('TEDIYE', 'july-payment', '2028-07-10', 1, 0, 'FINALIZED', 120), donemId: june2028.id, personelId: validSecondPerson.id },
      { ...payroll('SUPPLEMENTAL', 'legacy-invalid', '2028-06-30', 2, 0, 'FINALIZED', 900), donemId: june2028.id, personelId: legacyInvalidPerson.id },
    ];

    const people = [person, validSecondPerson, legacyInvalidPerson];
    const bankRows = getBankTransferableAccrualRows(june2028, people, rows);
    const juneRows = filterAccrualRowsByPaymentDate(bankRows, '2028-06-30');
    const juneTotal = juneRows.reduce((sum, row) => sum + row.bordro.netOdeme, 0);
    const notice = describeBankExcludedAccruals(june2028, people, rows);

    expect(bankRows.map((row) => row.accrualId)).toEqual(['june-normal', 'july-payment']);
    expect(juneRows.map((row) => row.accrualId)).toEqual(['june-normal']);
    expect(juneTotal).toBe(1_744_659.78);
    expect(notice).toContain('Türk IBAN doğrulaması başarısız');
    expect(notice).toContain('Eski Yılmaz');
    expect(notice).toContain('banka talimatına aktarılmadı');
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
