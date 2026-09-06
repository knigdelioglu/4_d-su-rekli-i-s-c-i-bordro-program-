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
  if (impact.blockedByFinalized.length === 0 && impact.blockedByFinalizedRetroBatches.length === 0) return;
  const keys = impact.blockedByFinalized
    .map((key) => `${key.personnelId} / ${key.periodId} / ${key.accrualId ?? ''}`)
    .join(', ');
  const batches = impact.blockedByFinalizedRetroBatches.join(', ');
  const detail = [keys, batches ? `retro batch: ${batches}` : ''].filter(Boolean).join(', ');
  throw new Error(`Kesinleştirilmiş bordro/retro tarihçesini etkileyen veri değiştirilemez: ${detail}.`);
}

export function applyBrowserRetroBatchImpact<T extends { id: string; status?: string }>(
  batches: T[],
  impact: MutationImpact
): T[] {
  const affected = new Set(impact.affectedRetroBatches);
  return batches.map((batch) =>
    affected.has(batch.id) && batch.status !== 'FINALIZED'
      ? ({ ...batch, status: 'STALE' } as T)
      : batch
  );
}

export function applyBrowserPayrollImpact<T extends {
  id: string;
  personelId: string;
  donemId: string;
  accrualId?: string;
  status: string;
}>(
  payrolls: T[],
  impact: MutationImpact
): T[] {
  const affected = new Set(
    impact.affectedPayrolls.map(
      (key) => `${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`
    )
  );
  const affectedPeriods = new Set(
    impact.affectedPayrolls
      .filter((key) => key.accrualId === undefined || key.accrualId === '')
      .map((key) => `${key.personnelId}\u0000${key.periodId}`)
  );
  return payrolls.map((payroll) =>
    (payroll.status === 'CALCULATED' || payroll.status === 'DRAFT') &&
    (affectedPeriods.has(`${payroll.personelId}\u0000${payroll.donemId}`) ||
      affected.has(
        `${payroll.personelId}\u0000${payroll.donemId}\u0000${payroll.accrualId ?? payroll.id}`
      ))
      ? { ...payroll, status: 'STALE' }
      : payroll
  );
}
