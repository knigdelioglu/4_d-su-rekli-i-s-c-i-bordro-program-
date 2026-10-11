import { describe, expect, test } from 'bun:test';
import type { Personel, PersonelPuantaj, SickLeaveRecord } from '../types/payroll';
import { createPuantajBulkPlan } from './puantajBulkPlan';

const period = {
  periodId: '2027-01',
  periodStartInclusive: '2027-01-15',
  periodEndInclusive: '2027-02-14',
};

function person(id: string): Personel {
  return { id, ad: id, soyad: 'İşçi' } as Personel;
}

function attendance(personnelId: string, gunler: Record<string, string>): PersonelPuantaj {
  return {
    id: `${personnelId}_${period.periodId}`,
    personelId: personnelId,
    donemId: period.periodId,
    gunler,
  } as PersonelPuantaj;
}

describe('createPuantajBulkPlan', () => {
  test('previews old/new codes and keeps multiple reported R days for multiple employees', () => {
    const reportRecords: SickLeaveRecord[] = [
      { id: 'report-a', personnelId: 'a', startDate: '2027-01-16', endDate: '2027-01-17' },
      { id: 'report-b', personnelId: 'b', startDate: '2027-01-16', endDate: '2027-01-16' },
    ];
    const plan = createPuantajBulkPlan({
      ...period,
      personnel: [person('a'), person('b')],
      attendances: [
        attendance('a', { '2027-01-15': 'T', '2027-01-16': 'R', '2027-01-17': 'R' }),
        attendance('b', { '2027-01-15': 'İ', '2027-01-16': 'R', '2027-01-17': 'GÇ' }),
      ],
      sickLeaveRecords: reportRecords,
      dates: ['2027-01-15', '2027-01-16', '2027-01-17'],
      code: 'Ç',
    });

    expect(plan.rows).toEqual([
      { personnelId: 'a', personnelName: 'a İşçi', date: '2027-01-15', previousCode: 'T', nextCode: 'Ç', status: 'APPLY' },
      { personnelId: 'a', personnelName: 'a İşçi', date: '2027-01-16', previousCode: 'R', nextCode: 'Ç', status: 'PRESERVED_R' },
      { personnelId: 'a', personnelName: 'a İşçi', date: '2027-01-17', previousCode: 'R', nextCode: 'Ç', status: 'PRESERVED_R' },
      { personnelId: 'b', personnelName: 'b İşçi', date: '2027-01-15', previousCode: 'İ', nextCode: 'Ç', status: 'APPLY' },
      { personnelId: 'b', personnelName: 'b İşçi', date: '2027-01-16', previousCode: 'R', nextCode: 'Ç', status: 'PRESERVED_R' },
      { personnelId: 'b', personnelName: 'b İşçi', date: '2027-01-17', previousCode: 'GÇ', nextCode: 'Ç', status: 'APPLY' },
    ]);
    expect(plan.preservedRows).toHaveLength(3);
    expect(plan.reportMismatches).toEqual([]);
    expect(plan.updates).toHaveLength(2);
    expect(plan.updates[0].gunler['2027-01-16']).toBe('R');
    expect(plan.updates[0].gunler['2027-01-17']).toBe('R');
    expect(plan.updates[1].gunler['2027-01-16']).toBe('R');
  });

  test('blocks the batch when a sick-leave record does not match the stored day code', () => {
    const plan = createPuantajBulkPlan({
      ...period,
      personnel: [person('a')],
      attendances: [attendance('a', { '2027-01-15': 'Ç', '2027-01-16': 'T' })],
      sickLeaveRecords: [{
        id: 'report-a', personnelId: 'a', startDate: '2027-01-16', endDate: '2027-01-16',
      }],
      dates: ['2027-01-15', '2027-01-16'],
      code: 'T',
    });

    expect(plan.reportMismatches).toEqual([{
      personnelId: 'a',
      personnelName: 'a İşçi',
      date: '2027-01-16',
      previousCode: 'T',
      nextCode: 'T',
      status: 'REPORT_MISMATCH',
    }]);
    expect(plan.updates[0].gunler['2027-01-16']).toBe('T');
  });
});
