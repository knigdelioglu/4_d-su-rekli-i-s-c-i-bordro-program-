import type { PayrollNotice } from '../types/payrollNotice';
import type { BordroDonemi } from '../types/payroll';
import { comparePaymentEvents } from '../services/payrollEngine/paymentEventOrder';
import type { PayrollBoundaryPayroll } from '../services/payrollEngine';

export function filterFinalizeNotices(
  notices: PayrollNotice[],
  personnelId: string
): PayrollNotice[] {
  return notices.filter(
    (notice) =>
      notice.scope === 'PERIOD' ||
      (notice.scope === 'PERSONNEL' && notice.personnelId === personnelId)
  );
}

export function hasBlockingFinalizeNotice(notices: PayrollNotice[]): boolean {
  return notices.some((notice) => notice.severity === 'CRITICAL');
}

export function findEarlierUnfinalizedPaymentEvents(
  payrolls: readonly PayrollBoundaryPayroll[],
  periods: readonly BordroDonemi[],
  target: PayrollBoundaryPayroll
): PayrollBoundaryPayroll[] {
  const periodById = new Map(periods.map((period) => [period.id, period]));
  const targetPeriod = periodById.get(target.donemId);
  if (!targetPeriod) return [];

  return payrolls
    .filter((payroll) => {
      if (
        payroll.personelId !== target.personelId ||
        payroll.status === 'FINALIZED' ||
        (payroll.donemId === target.donemId &&
          (payroll.accrualId === target.accrualId || payroll.id === target.id))
      ) {
        return false;
      }
      const payrollPeriod = periodById.get(payroll.donemId);
      return Boolean(
        payrollPeriod && comparePaymentEvents(payroll, target, payrollPeriod, targetPeriod) < 0
      );
    })
    .sort((left, right) => {
      const leftPeriod = periodById.get(left.donemId)!;
      const rightPeriod = periodById.get(right.donemId)!;
      return comparePaymentEvents(left, right, leftPeriod, rightPeriod);
    });
}
