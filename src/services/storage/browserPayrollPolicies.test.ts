import { describe, expect, test } from 'bun:test';
import { BordroDonemi, BordroKaydi } from '../../types/payroll';
import {
  assertBrowserMutationAllowed,
  invalidateBrowserPayrolls,
} from './browserPayrollPolicies';

const periods: BordroDonemi[] = [
  {
    id: '2026-01',
    yil: 2026,
    ay: 1,
    baslangicTarihi: '2026-01-15',
    bitisTarihi: '2026-02-14',
    donemAdi: 'Ocak 2026',
    taxYear: 2026,
    taxMonth: 2,
  },
  {
    id: '2026-02',
    yil: 2026,
    ay: 2,
    baslangicTarihi: '2026-02-15',
    bitisTarihi: '2026-03-14',
    donemAdi: 'Şubat 2026',
    taxYear: 2026,
    taxMonth: 3,
  },
];

const payroll = (periodId: string, status: BordroKaydi['status']): BordroKaydi =>
  ({ personelId: 'person-1', donemId: periodId, status } as BordroKaydi);

describe('browser payroll invalidation policy', () => {
  test('source mutations stale calculated dependents but preserve finalized records', () => {
    const result = invalidateBrowserPayrolls(
      [payroll('2026-01', 'CALCULATED'), payroll('2026-02', 'FINALIZED')],
      periods,
      { kind: 'PERSON_PERIOD', personnelId: 'person-1', periodId: '2026-01' }
    );

    expect(result[0].status).toBe('STALE');
    expect(result[1].status).toBe('FINALIZED');
  });

  test('a mutation affecting finalized history is rejected', () => {
    expect(() =>
      assertBrowserMutationAllowed(
        [payroll('2026-02', 'FINALIZED')],
        periods,
        { kind: 'PERSON_PERIOD', personnelId: 'person-1', periodId: '2026-01' }
      )
    ).toThrow('Kesinleştirilmiş');
  });
});
