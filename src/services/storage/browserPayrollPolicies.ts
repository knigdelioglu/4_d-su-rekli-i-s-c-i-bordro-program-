import type { MutationImpact, PayrollMutation } from '../payrollEngine/types';

/**
 * Browser persistence adapter for the core-owned mutation decision.
 *
 * This module intentionally contains no period ordering, tax, or FINALIZED
 * business rule. WASM returns the impact; this adapter only rejects the
 * returned blockers and applies the returned keys to the UI snapshot.
 */
export type BrowserPayrollMutation = PayrollMutation;

export function assertBrowserMutationImpactAllowed(impact: MutationImpact): void {
  if (impact.blockedByFinalized.length === 0) return;
  const keys = impact.blockedByFinalized
    .map((key) => `${key.personnelId} / ${key.periodId}`)
    .join(', ');
  throw new Error(`Kesinleştirilmiş bordro tarihçesini etkileyen veri değiştirilemez: ${keys}.`);
}

export function applyBrowserPayrollImpact<T extends {
  id: string;
  personelId: string;
  donemId: string;
  status: string;
}>(
  payrolls: T[],
  impact: MutationImpact
): T[] {
  const affected = new Set(
    impact.affectedPayrolls.map((key) => `${key.personnelId}\u0000${key.periodId}`)
  );
  return payrolls.map((payroll) =>
    payroll.status === 'CALCULATED' &&
    affected.has(`${payroll.personelId}\u0000${payroll.donemId}`)
      ? { ...payroll, status: 'STALE' }
      : payroll
  );
}
