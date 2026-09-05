import type { BordroDonemi } from '../../types/payroll';
import { getDefaultAccrualPaymentDate } from '../../utils/payrollPresentation';
import type { PayrollDatasetSnapshot } from './types';

/** Presentation/input allocation only; Rust validates uniqueness at calculation. */
export function nextPaymentSequence(
  dataset: PayrollDatasetSnapshot,
  personnelId: string,
  period: BordroDonemi,
  paymentDate: string
): number {
  const sameMonth = new Map(dataset.periods.map((item) => [item.id, item]));
  return dataset.payrolls.reduce((next, event) => {
    const owner = sameMonth.get(event.donemId);
    if (event.personelId !== personnelId || !owner ||
        owner.taxYear !== period.taxYear || owner.taxMonth !== period.taxMonth ||
        (event.paymentDate || getDefaultAccrualPaymentDate(owner)) !== paymentDate) return next;
    return Math.max(next, event.sequence + 1);
  }, 0);
}

/** UI projection of the core canonical order; no type-based ordering. */
export function comparePaymentEvents(
  left: { paymentDate: string; sequence: number; accrualId: string; id: string },
  right: { paymentDate: string; sequence: number; accrualId: string; id: string },
  leftPeriod: BordroDonemi,
  rightPeriod: BordroDonemi = leftPeriod
): number {
  const compareText = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
  return leftPeriod.taxYear - rightPeriod.taxYear ||
    leftPeriod.taxMonth - rightPeriod.taxMonth ||
    compareText(left.paymentDate || getDefaultAccrualPaymentDate(leftPeriod),
      right.paymentDate || getDefaultAccrualPaymentDate(rightPeriod)) ||
    left.sequence - right.sequence ||
    compareText(left.accrualId || left.id, right.accrualId || right.id);
}
