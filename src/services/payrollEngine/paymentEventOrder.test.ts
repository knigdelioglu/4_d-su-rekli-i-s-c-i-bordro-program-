import { expect, test } from 'bun:test';
import { comparePaymentEvents, nextPaymentSequence } from './paymentEventOrder';
import type { PayrollDatasetSnapshot } from './types';
import type { BordroDonemi } from '../../types/payroll';

const period = { id: 'august', taxYear: 2026, taxMonth: 8, bitisTarihi: '2026-08-14' } as BordroDonemi;
const event = (id: string, date: string, sequence: number, accrualType: string, donemId = period.id) =>
  ({ id, accrualId: id, personelId: 'p', donemId, paymentDate: date, sequence, accrualType });

test('all types allocate zero first and share same-day sequence across work periods', () => {
  const dataset = { periods: [period, { ...period, id: 'other' }], payrolls: [] } as unknown as PayrollDatasetSnapshot;
  expect(nextPaymentSequence(dataset, 'p', period, '2026-08-10')).toBe(0);
  dataset.payrolls = [event('tediye', '2026-08-10', 0, 'TEDIYE'),
    event('normal', '2026-08-10', 1, 'NORMAL', 'other')] as unknown as PayrollDatasetSnapshot['payrolls'];
  expect(nextPaymentSequence(dataset, 'p', period, '2026-08-10')).toBe(2);
  expect(nextPaymentSequence(dataset, 'p', period, '2026-08-14')).toBe(0);
  expect(nextPaymentSequence(dataset, 'another', period, '2026-08-10')).toBe(0);
});

test('canonical date and sequence precede type and id', () => {
  const events = [event('normal', '2026-08-14', 0, 'NORMAL'), event('tediye', '2026-08-10', 0, 'TEDIYE'),
    event('tis', '2026-08-25', 0, 'TIS_IKRAMIYE')];
  expect(events.sort((a, b) => comparePaymentEvents(a, b, period)).map((e) => e.id)).toEqual(['tediye', 'normal', 'tis']);
  events.forEach((e, index) => { e.paymentDate = '2026-08-10'; e.sequence = index; });
  expect(events.reverse().sort((a, b) => comparePaymentEvents(a, b, period)).map((e) => e.id)).toEqual(['tediye', 'normal', 'tis']);
});
