import type { MutationImpact } from '../payrollEngine';
import type { PayrollStorageDto } from '../payrollEngine/decimalBoundary';

/** Explicit destructive deletion: finalized history is intentionally disposable. */
export function deletePeriodFromPayload(
  current: PayrollStorageDto,
  periodId: string,
  impact: MutationImpact
): PayrollStorageDto {
  if (!current.donemler.some((period) => period.id === periodId)) {
    throw new Error('Silinecek dönem bulunamadı.');
  }
  const removedBatches = new Set([
    ...impact.affectedRetroBatches,
    ...impact.blockedByFinalizedRetroBatches,
    ...current.retroAllocations.filter((allocation) => allocation.sourcePeriodId === periodId).map((allocation) => allocation.batchId),
    ...current.bordrolar.filter((payroll) => payroll.donemId === periodId && payroll.accrualType === 'RETRO_ADJUSTMENT').map((payroll) => payroll.accrualId!),
  ]);
  const affected = [...impact.affectedPayrolls, ...impact.blockedByFinalized];
  const donemler = current.donemler.filter((period) => period.id !== periodId);
  const kurumDegerleriMap = { ...current.kurumDegerleriMap };
  delete kurumDegerleriMap[periodId];
  return {
    ...current,
    donemler,
    aktifDonemId: current.aktifDonemId === periodId ? donemler[0]?.id ?? '' : current.aktifDonemId,
    kurumDegerleriMap,
    puantajlar: current.puantajlar.filter((record) => record.donemId !== periodId),
    taxOpenings: current.taxOpenings.filter((opening) => opening.effectiveFromPeriodId !== periodId && opening.asgariGvEffectiveFromPeriodId !== periodId),
    bordrolar: current.bordrolar
      .filter((payroll) => payroll.donemId !== periodId && !removedBatches.has(payroll.accrualId ?? ''))
      .map((payroll) => affected.some((key) => key.personnelId === payroll.personelId && key.periodId === payroll.donemId && (!key.accrualId || key.accrualId === (payroll.accrualId ?? payroll.id)))
        ? { ...payroll, status: 'STALE' } : payroll),
    retroBatches: current.retroBatches.filter((batch) => !removedBatches.has(batch.id)),
    retroAllocations: current.retroAllocations.filter((allocation) => !removedBatches.has(allocation.batchId)),
  };
}
