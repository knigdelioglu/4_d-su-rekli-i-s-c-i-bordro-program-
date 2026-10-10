import type { MutationImpact, PayrollMutation } from '../payrollEngine/types';

/**
 * Browser persistence adapter for the core-owned mutation decision.
 *
 * This module intentionally contains no period ordering, tax, or FINALIZED
 * business rule. WASM returns the impact; this adapter only rejects the
 * returned blockers and applies the returned keys to the UI snapshot.
 */
export type BrowserPayrollMutation = PayrollMutation;

export interface MutationBlockLabels {
  person?: (personnelId: string) => string | undefined;
  period?: (periodId: string) => string | undefined;
}

export function assertBrowserMutationImpactAllowed(
  impact: MutationImpact,
  labels: MutationBlockLabels = {}
): void {
  if (impact.blockedByFinalized.length === 0 && impact.blockedByFinalizedRetroBatches.length === 0) return;
  const examples = impact.blockedByFinalized
    .slice(0, 3)
    .map((key) =>
      `${labels.person?.(key.personnelId) ?? key.personnelId} – ${labels.period?.(key.periodId) ?? key.periodId}`
    );
  const remaining = impact.blockedByFinalized.length - examples.length;
  if (remaining > 0) examples.push(`ve ${remaining} kayıt daha`);
  if (impact.blockedByFinalizedRetroBatches.length > 0) {
    examples.push(`${impact.blockedByFinalizedRetroBatches.length} kesinleşmiş geriye dönük fark ödemesi`);
  }
  throw new Error(
    `Kesinleştirilmiş bordro/retro tarihçesini etkileyen veri değiştirilemez (${examples.join(', ')}). ` +
      'Değişikliği kesinleşmiş son dönemden sonraki bir döneme uygulayın; kesinleşmiş dönemlerdeki ücret farkları için Geriye Dönük Farklar ekranını kullanın.'
  );
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

function parseNumeric(v?: number | string | null): number {
  if (v === null || v === undefined) return 0;
  if (typeof v === 'number') return v;
  const parsed = Number(v);
  return Number.isNaN(parsed) ? 0 : parsed;
}

function optNumberEqualsOrZero(a?: number | string | null, b?: number | string | null): boolean {
  const v1 = parseNumeric(a);
  const v2 = parseNumeric(b);
  return Math.abs(v1 - v2) < 0.000001;
}

function optBoolEqualsOrFalse(a?: boolean | null, b?: boolean | null): boolean {
  return (a ?? false) === (b ?? false);
}

function kesintilerHaveFinancialChanges(
  a?: any | null,
  b?: any | null
): boolean {
  const k1 = a ?? {};
  const k2 = b ?? {};
  if (!optBoolEqualsOrFalse(k1.sendikaUyesi, k2.sendikaUyesi)) return true;
  if (!optNumberEqualsOrZero(k1.sabitSendikaAidati, k2.sabitSendikaAidati)) return true;
  if (!optBoolEqualsOrFalse(k1.besUyesi, k2.besUyesi)) return true;
  if (!optNumberEqualsOrZero(k1.oksOraniYuzde, k2.oksOraniYuzde)) return true;
  if (!optNumberEqualsOrZero(k1.sabitBesTutar, k2.sabitBesTutar)) return true;
  if (!optNumberEqualsOrZero(k1.icraTutar, k2.icraTutar)) return true;
  if (!optNumberEqualsOrZero(k1.nafakaTutar, k2.nafakaTutar)) return true;
  if (!optNumberEqualsOrZero(k1.kisiBorcuTutar, k2.kisiBorcuTutar)) return true;
  if (!optNumberEqualsOrZero(k1.dogumAskerlikBorclanmasiTutar, k2.dogumAskerlikBorclanmasiTutar)) return true;
  if (!optNumberEqualsOrZero(k1.hayatSaglikSigortasiTutar, k2.hayatSaglikSigortasiTutar)) return true;
  if (!optNumberEqualsOrZero(k1.digerKesintiTutar, k2.digerKesintiTutar)) return true;

  const g1 = k1.gvIndirimleri ?? {};
  const g2 = k2.gvIndirimleri ?? {};
  if (!optNumberEqualsOrZero(g1.dogumAskerlikGvIndirimTutar, g2.dogumAskerlikGvIndirimTutar)) return true;
  if (!optNumberEqualsOrZero(g1.hayatSigortasiPrimiTutar, g2.hayatSigortasiPrimiTutar)) return true;
  if (!optNumberEqualsOrZero(g1.saglikSigortasiPrimiTutar, g2.saglikSigortasiPrimiTutar)) return true;

  return false;
}

export function personnelHasFinancialChanges(
  current: any,
  updated: any
): boolean {
  return (
    current.grup !== updated.grup ||
    (current.hizmetYili ?? 0) !== (updated.hizmetYili ?? 0) ||
    !optNumberEqualsOrZero(current.devirKumulatifGvMatrahi, updated.devirKumulatifGvMatrahi) ||
    (current.devirKumulatifGvMatrahiYili ?? null) !== (updated.devirKumulatifGvMatrahiYili ?? null) ||
    (current.devirKumulatifGvMatrahiBaslangicAyi ?? null) !== (updated.devirKumulatifGvMatrahiBaslangicAyi ?? null) ||
    !optNumberEqualsOrZero(current.devirKumulatifAsgariGvMatrahi, updated.devirKumulatifAsgariGvMatrahi) ||
    (current.devirKumulatifAsgariGvMatrahiYili ?? null) !== (updated.devirKumulatifAsgariGvMatrahiYili ?? null) ||
    kesintilerHaveFinancialChanges(current.kesintiler, updated.kesintiler)
  );
}

export function taxOpeningHasFinancialChanges(
  current?: any | null,
  updated?: any | null
): boolean {
  if (!current && !updated) return false;
  if (!current && updated) {
    return (
      parseNumeric(updated.gvCumulativeOpening) > 0 ||
      Boolean(updated.effectiveFromPeriodId) ||
      parseNumeric(updated.asgariGvCumulativeOpening) > 0 ||
      Boolean(updated.asgariGvEffectiveFromPeriodId)
    );
  }
  if (current && !updated) {
    return (
      parseNumeric(current.gvCumulativeOpening) > 0 ||
      Boolean(current.effectiveFromPeriodId) ||
      parseNumeric(current.asgariGvCumulativeOpening) > 0 ||
      Boolean(current.asgariGvEffectiveFromPeriodId)
    );
  }
  return (
    !optNumberEqualsOrZero(current.gvCumulativeOpening, updated.gvCumulativeOpening) ||
    (current.effectiveFromPeriodId ?? null) !== (updated.effectiveFromPeriodId ?? null) ||
    !optNumberEqualsOrZero(current.asgariGvCumulativeOpening, updated.asgariGvCumulativeOpening) ||
    (current.asgariGvEffectiveFromPeriodId ?? null) !== (updated.asgariGvEffectiveFromPeriodId ?? null)
  );
}

export function validatePayrollStatusTransition(current: string, next: string): void {
  if (next === 'FINALIZED') {
    throw new Error("FINALIZED geçişi için finalize_payroll API'si kullanılmalıdır.");
  }
  if (current === next) return;
  if (current === 'FINALIZED') {
    throw new Error('Kesinleştirilmiş (FINALIZED) bordronun durumu değiştirilemez.');
  }
  if (current === 'STALE' && next === 'CALCULATED') {
    throw new Error(
      'Yeniden hesaplanmamış STALE kayıt doğrudan CALCULATED yapılamaz; bordro yeniden hesaplanmalıdır.'
    );
  }
  if (current === 'DRAFT' && next === 'CALCULATED') {
    throw new Error(
      'DRAFT kayıt doğrudan CALCULATED yapılamaz; bordro hesaplanmalıdır.'
    );
  }
}


