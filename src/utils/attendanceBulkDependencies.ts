import type { PayrollDatasetSnapshot, PayrollEngine } from '../services/payrollEngine';
import { describeError } from './errorMessage';
export interface AttendanceBulkDependency {
  personnelId: string;
  blocked: boolean;
  message: string;
}
/** Read-only use of the same shared policy that authorizes the eventual atomic write. */
export async function previewAttendanceBulkDependencies(engine: PayrollEngine, dataset: PayrollDatasetSnapshot,
  periodId: string, personnelIds: string[]): Promise<AttendanceBulkDependency[]> {
  const results: AttendanceBulkDependency[] = [];
  for (const personnelId of personnelIds) {
    try {
      const impact = await engine.evaluateMutationPolicy({ kind: 'PERSON_PERIOD', personnelId, periodId }, dataset);
      const periods = [...new Set(impact.blockedByFinalized.map((key) => dataset.periods.find((period) => period.id === key.periodId)?.donemAdi ?? key.periodId))];
      const retro = impact.blockedByFinalizedRetroBatches.map((id) => dataset.retroBatches?.find((batch) => batch.id === id)?.id ?? id);
      const blocked = Boolean(periods.length || retro.length);
      results.push({ personnelId, blocked, message: blocked
        ? [periods.length ? `Kesinleşmiş bordro: ${periods.join(', ')}` : '', retro.length ? `Kesinleşmiş retro: ${retro.join(', ')}` : ''].filter(Boolean).join(' · ')
        : `Uygun · ${impact.affectedPayrolls.length} bordro ve ${impact.affectedRetroBatches.length} retro kaydı yeniden hesaplanacak.` });
    } catch (error) { results.push({ personnelId, blocked: true, message: `Bağımlılık kontrol edilemedi: ${describeError(error)}` }); }
  }
  return results;
}
