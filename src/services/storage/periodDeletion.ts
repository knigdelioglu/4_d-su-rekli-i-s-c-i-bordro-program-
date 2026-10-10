import type { MutationImpact } from '../payrollEngine';
import type { PayrollStorageDto } from '../payrollEngine/decimalBoundary';

/** Safe period deletion: rejects deleting periods with finalized records or affecting downstream finalized history. */
export function deletePeriodFromPayload(
  current: PayrollStorageDto,
  periodId: string,
  impact: MutationImpact
): PayrollStorageDto {
  if (!current.donemler.some((period) => period.id === periodId)) {
    throw new Error('Silinecek dönem bulunamadı.');
  }
  if (current.bordrolar.some((payroll) => payroll.donemId === periodId && payroll.status === 'FINALIZED')) {
    throw new Error('Kesinleştirilmiş (FINALIZED) bordro içeren dönem silinemez.');
  }
  const hasFinalizedRetro = (current.retroBatches ?? []).some(
    (batch) =>
      batch.status === 'FINALIZED' &&
      ((current.retroAllocations ?? []).some(
        (a) => a.batchId === batch.id && a.sourcePeriodId === periodId
      ) ||
        current.bordrolar.some(
          (p) => p.donemId === periodId && p.accrualId === batch.id
        ))
  );
  if (hasFinalizedRetro) {
    throw new Error('Kesinleştirilmiş (FINALIZED) geriye dönük fark içeren dönem silinemez.');
  }
  if (impact.blockedByFinalized.length > 0 || impact.blockedByFinalizedRetroBatches.length > 0) {
    throw new Error(
      'Dönem silme işlemi sonraki kesinleştirilmiş bordro veya retro kayıtlarını etkilediği için izin verilmez.'
    );
  }

  const removedBatches = new Set([
    ...impact.affectedRetroBatches,
    ...current.retroAllocations.filter((allocation) => allocation.sourcePeriodId === periodId).map((allocation) => allocation.batchId),
    ...current.bordrolar.filter((payroll) => payroll.donemId === periodId && payroll.accrualType === 'RETRO_ADJUSTMENT' && payroll.status !== 'FINALIZED').map((payroll) => payroll.accrualId!),
  ]);
  const affected = impact.affectedPayrolls;
  const donemler = current.donemler.filter((period) => period.id !== periodId);
  const nextActivePeriod = [...donemler].sort(
    (left, right) => left.yil - right.yil || left.ay - right.ay || left.id.localeCompare(right.id)
  )[0];
  const kurumDegerleriMap = { ...current.kurumDegerleriMap };
  delete kurumDegerleriMap[periodId];
  const taxOpenings = current.taxOpenings
    .flatMap((opening) => {
      const next = { ...opening };
      let affectedOpening = false;
      if (next.effectiveFromPeriodId === periodId) {
        affectedOpening = true;
        next.effectiveFromPeriodId = null;
        next.gvCumulativeOpening = null;
      }
      if (next.asgariGvEffectiveFromPeriodId === periodId) {
        affectedOpening = true;
        next.asgariGvEffectiveFromPeriodId = null;
        next.asgariGvCumulativeOpening = null;
      }
      if (
        affectedOpening &&
        next.gvCumulativeOpening == null &&
        next.asgariGvCumulativeOpening == null
      ) {
        return [];
      }
      return [next];
    });
  return {
    ...current,
    donemler,
    aktifDonemId: current.aktifDonemId === periodId ? nextActivePeriod?.id ?? '' : current.aktifDonemId,
    kurumDegerleriMap,
    puantajlar: current.puantajlar.filter((record) => record.donemId !== periodId),
    taxOpenings,
    bordrolar: current.bordrolar
      .filter((payroll) => payroll.donemId !== periodId && !removedBatches.has(payroll.accrualId ?? ''))
      .map((payroll) =>
        payroll.status !== 'FINALIZED' &&
        affected.some((key) => key.personnelId === payroll.personelId && key.periodId === payroll.donemId && (!key.accrualId || key.accrualId === (payroll.accrualId ?? payroll.id)))
          ? { ...payroll, status: 'STALE' } : payroll),
    retroBatches: (current.retroBatches ?? []).filter((batch) => !removedBatches.has(batch.id)),
    retroAllocations: (current.retroAllocations ?? []).filter((allocation) => !removedBatches.has(allocation.batchId)),
  };
}
