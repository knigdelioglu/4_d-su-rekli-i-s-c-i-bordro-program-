import type { PayrollNotice } from '../types/payrollNotice';

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
