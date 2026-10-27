import React, { useSyncExternalStore } from 'react';
import { bulkPayrollJobs } from '../services/bulkPayrollJob';
import type { BordroDonemi, Personel } from '../types/payroll';

export function BulkPayrollProgress({ periods, personnel, onOpen, onResume, activePeriodId }: {
  periods: BordroDonemi[]; personnel: Personel[]; activePeriodId?: string;
  onOpen?: (periodId: string) => void; onResume?: () => void;
}) {
  const state = useSyncExternalStore(bulkPayrollJobs.subscribe, bulkPayrollJobs.getSnapshot, bulkPayrollJobs.getSnapshot);
  const job = state.job;
  if (!job) return state.error ? <p role="alert">{state.error}</p> : null;
  const count = (status: string) => job.items.filter((item) => item.status === status).length;
  const success = count('success'), errors = count('error'), skipped = count('skipped');
  const done = success + errors + skipped;
  return <section className="my-3 rounded-lg border border-indigo-200 bg-indigo-50 p-3 text-sm" aria-label="Toplu işlem ilerlemesi" data-testid="bulk-payroll-progress">
    <p role="status" aria-live="polite">
      <strong>{periods.find((period) => period.id === job.periodId)?.donemAdi ?? job.periodId} — Toplu {job.accrualType === 'NORMAL' ? 'bordro' : job.accrualType === 'TEDIYE' ? 'tediye' : job.accrualType === 'TIS_IKRAMIYE' ? 'TİS ikramiyesi' : 'ek ödeme'}</strong>
      {' · '}{state.busy && job.status === 'running' ? 'Sürüyor' : job.status === 'paused' ? 'Sürdürülmeyi bekliyor' : 'Tamamlandı'}
      {' · '}{done}/{job.items.length} kişi · Başarı: {success} · Hata: {errors} · Atlanan: {skipped}
    </p>
    <progress value={done} max={job.items.length || 1} className="w-full" aria-label="Tamamlanan personel" />
    {state.error && <p role="alert">{state.error}</p>}
    <details><summary className="cursor-pointer">Kişi bazında sonuçlar</summary>
      <ul className="max-h-48 overflow-auto">{job.items.map((item) => {
        const person = personnel.find((entry) => entry.id === item.personnelId);
        const labels = { pending: 'Bekliyor', running: 'Hesaplanıyor', success: 'Başarılı', error: 'Hata', skipped: 'Atlandı' };
        return <li key={item.personnelId}>{person ? `${person.ad} ${person.soyad}` : item.personnelId}: {labels[item.status]}{item.message ? ` — ${item.message}` : ''}</li>;
      })}</ul>
    </details>
    {!state.busy && job.status === 'paused' && <div className="mt-2 flex gap-3">
      {onResume && activePeriodId === job.periodId
        ? <button type="button" className="rounded bg-indigo-700 px-3 py-2 text-white" onClick={onResume}>Yarım kalan işlemi sürdür</button>
        : onOpen && <button type="button" className="rounded bg-indigo-700 px-3 py-2 text-white" onClick={() => onOpen(job.periodId)}>İşlemin dönemini aç</button>}
      <button type="button" onClick={() => bulkPayrollJobs.close()}>İşlemi kapat (kayıtlar korunur)</button>
    </div>}
  </section>;
}
