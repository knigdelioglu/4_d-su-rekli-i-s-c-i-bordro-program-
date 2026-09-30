import { describe, expect, test } from 'bun:test';
import type { BordroDonemi } from '../types/payroll';
import { PayrollNotice } from '../types/payrollNotice';
import type { PayrollBoundaryPayroll } from '../services/payrollEngine';
import {
  findEarlierUnfinalizedPaymentEvents,
  filterFinalizeNotices,
  hasBlockingFinalizeNotice,
} from './payrollFinalizeRules';

const notice = (
  code: string,
  severity: PayrollNotice['severity'],
  scope: PayrollNotice['scope'],
  personnelId?: string
): PayrollNotice => ({
  code,
  severity,
  scope,
  personnelId,
  title: code,
  message: code,
  details: [],
});

describe('payroll finalize review rules', () => {
  test('prior non-finalized payment events block while later stale events do not', () => {
    const periods = [
      { id: '2026-09', taxYear: 2026, taxMonth: 10 },
      { id: '2026-10', taxYear: 2026, taxMonth: 11 },
      { id: '2026-11', taxYear: 2026, taxMonth: 12 },
    ] as BordroDonemi[];
    const payment = (
      id: string,
      periodId: string,
      status: PayrollBoundaryPayroll['status'],
      paymentDate: string
    ) =>
      ({
        id,
        accrualId: id,
        personelId: 'p1',
        donemId: periodId,
        status,
        paymentDate,
        sequence: 0,
      }) as PayrollBoundaryPayroll;
    const target = payment('october-normal', '2026-10', 'CALCULATED', '2026-11-14');
    const prior = payment('september-normal', '2026-09', 'STALE', '2026-10-14');
    const future = payment('november-normal', '2026-11', 'STALE', '2026-12-14');

    expect(findEarlierUnfinalizedPaymentEvents([target, future, prior], periods, target)).toEqual([
      prior,
    ]);
  });

  test('period critical notice blocks every personnel finalization', () => {
    const notices = [notice('MISSING_PERIOD_SETTINGS', 'CRITICAL', 'PERIOD')];
    const relevant = filterFinalizeNotices(notices, 'p1');

    expect(hasBlockingFinalizeNotice(relevant)).toBe(true);
  });

  test('critical notice for another personnel does not block current personnel', () => {
    const notices = [notice('MISSING_ATTENDANCE', 'CRITICAL', 'PERSONNEL', 'p2')];
    const relevant = filterFinalizeNotices(notices, 'p1');

    expect(relevant.length).toBe(0);
  });

  test('warning and info notices remain visible but do not block calculated payroll', () => {
    const notices = [
      notice('INCOME_TAX_BRACKET_TRANSITION', 'WARNING', 'PERSONNEL', 'p1'),
      notice('INCOMING_PEK_CARRY', 'INFO', 'PERSONNEL', 'p1'),
    ];
    const relevant = filterFinalizeNotices(notices, 'p1');

    expect(relevant.length).toBe(2);
    expect(hasBlockingFinalizeNotice(relevant)).toBe(false);
  });
});
