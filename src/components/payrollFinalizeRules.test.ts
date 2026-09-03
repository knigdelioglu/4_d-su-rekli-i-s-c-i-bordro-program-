import { describe, expect, test } from 'bun:test';
import { PayrollNotice } from '../types/payrollNotice';
import {
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
