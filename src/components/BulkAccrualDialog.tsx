import React, { useEffect, useRef, useState } from 'react';
import type { BordroDonemi, Personel } from '../types/payroll';
import type { PayrollDatasetSnapshot, PayrollEngine } from '../services/payrollEngine';
import { nextPaymentSequence } from '../services/payrollEngine/paymentEventOrder';
import { isExactDecimalString } from '../services/payrollEngine/decimalBoundary';
import { getDefaultAccrualPaymentDate } from '../utils/payrollPresentation';
import { formatPayrollError, type SupplementaryAccrualDraft } from './useBordroCalculationController';

type Row = { id: string; error?: string; affected: number; retro: number };
export function BulkAccrualDialog({ period, personnel, type, engine, getDataset, onSave, onClose }: {
  period: BordroDonemi; personnel: Personel[]; type: 'TEDIYE' | 'TIS_IKRAMIYE' | 'SUPPLEMENTAL';
  engine: PayrollEngine; getDataset: () => PayrollDatasetSnapshot;
  onSave: (ids: string[], draft: SupplementaryAccrualDraft) => Promise<void>; onClose: () => void;
}) {
  const dialogRef = useRef<HTMLElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialogRef.current?.querySelector<HTMLInputElement>('input')?.focus();
    return () => previous?.focus();
  }, []);
  const [draft, setDraft] = useState<SupplementaryAccrualDraft>({ accrualType: type, paymentDate: getDefaultAccrualPaymentDate(period), grossAmount: '', description: '' });
  const [selected, setSelected] = useState(() => new Set(personnel.map((person) => person.id)));
  const [rows, setRows] = useState<Row[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const change = (patch: Partial<SupplementaryAccrualDraft>) => { setDraft({ ...draft, ...patch }); setRows(null); };
  const preview = async () => {
    if (busy) return;
    if (!isExactDecimalString(draft.grossAmount.trim()) || draft.grossAmount.trim().startsWith('-')) { setError('Brüt tutar geçerli ve negatif olmayan bir tutar olmalıdır.'); return; }
    const date = new Date(`${draft.paymentDate}T00:00:00Z`);
    if (!/^\d{4}-\d{2}-\d{2}$/.test(draft.paymentDate) || !Number.isFinite(date.getTime()) || date.toISOString().slice(0, 10) !== draft.paymentDate) { setError('Geçerli bir ödeme tarihi girin.'); return; }
    setBusy(true); setError(null);
    const results: Row[] = [];
    try {
      const dataset = getDataset();
      for (const person of personnel) {
        try {
          const accrual = { accrualId: `preview_${person.id}_${type}`, accrualType: type, paymentDate: draft.paymentDate,
            sequence: nextPaymentSequence(dataset, person.id, period, draft.paymentDate), grossAmount: draft.grossAmount.trim(), description: draft.description.trim() || null };
          const impact = await engine.evaluateMutationPolicy({ kind: 'ACCRUAL_INSERT', personnelId: person.id, periodId: period.id,
            accrualId: accrual.accrualId, paymentDate: accrual.paymentDate, sequence: accrual.sequence }, dataset);
          if (impact.blockedByFinalized.length || impact.blockedByFinalizedRetroBatches.length) throw new Error(`Kesinleşmiş ${impact.blockedByFinalized.length} bordro ve ${impact.blockedByFinalizedRetroBatches.length} geçmiş farkı kaydı bu işlemi engelliyor.`);
          await engine.validatePayroll({ personnelId: person.id, periodId: period.id, calculatedAt: new Date().toISOString(), manualIncome: null, accrual, dataset });
          results.push({ id: person.id, affected: impact.affectedPayrolls.length, retro: impact.affectedRetroBatches.length });
        } catch (failure) { results.push({ id: person.id, error: formatPayrollError(failure), affected: 0, retro: 0 }); }
      }
      setRows(results);
      setSelected((current) => new Set([...current].filter((id) => results.some((row) => row.id === id && !row.error))));
    } finally { setBusy(false); }
  };
  return <section ref={dialogRef} onKeyDown={(event) => {
    if (event.key === 'Escape' && !busy) { event.stopPropagation(); onClose(); }
    if (event.key !== 'Tab') return;
    const targets = [...(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled)') ?? [])];
    const first = targets[0], last = targets.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }} role="dialog" aria-modal="true" aria-labelledby="bulk-accrual-title" className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/50 p-4">
    <div className="max-h-[90vh] w-full max-w-3xl overflow-auto rounded-xl bg-white p-5 text-sm">
      <h2 id="bulk-accrual-title" className="text-lg font-bold">Toplu {type === 'TEDIYE' ? 'Tediye' : type === 'TIS_IKRAMIYE' ? 'TİS İkramiyesi' : 'Ek Ödeme'} — {period.donemAdi}</h2>
      <p>Her seçili kişi için aynı brüt tutarla ayrı tahakkuk oluşturulur. Önizleme kayıt oluşturmaz; bağımlılıklar kayıt sırasında yeniden denetlenir.</p>
      <fieldset disabled={busy} className="my-4 grid gap-3 sm:grid-cols-2">
        <label className="text-xs font-semibold text-slate-700">Ödeme tarihi <input className="mt-1 block w-full rounded-md border border-slate-300 px-3 py-2 text-sm font-normal focus:border-indigo-600 focus:outline-none focus:ring-1 focus:ring-indigo-600" type="date" value={draft.paymentDate} onChange={(event) => change({ paymentDate: event.target.value })} /></label>
        <label className="text-xs font-semibold text-slate-700">Kişi başı brüt tutar (TL) <input className="mt-1 block w-full rounded-md border border-slate-300 px-3 py-2 text-sm font-normal focus:border-indigo-600 focus:outline-none focus:ring-1 focus:ring-indigo-600" inputMode="decimal" value={draft.grossAmount} onChange={(event) => change({ grossAmount: event.target.value.replace(',', '.') })} /></label>
        <label className="text-xs font-semibold text-slate-700 sm:col-span-2">Açıklama <input className="mt-1 block w-full rounded-md border border-slate-300 px-3 py-2 text-sm font-normal focus:border-indigo-600 focus:outline-none focus:ring-1 focus:ring-indigo-600" value={draft.description} onChange={(event) => change({ description: event.target.value })} /></label>
      </fieldset>
      {error && <p role="alert" className="text-rose-800">{error}</p>}
      <button type="button" className="rounded border border-slate-300 px-3 py-2 text-xs font-semibold disabled:opacity-50" disabled={busy} onClick={() => setSelected(new Set(personnel.filter((person) => !rows?.find((row) => row.id === person.id)?.error).map((person) => person.id)))}>Uygun personelin tümünü seç</button>
      <button type="button" disabled={busy} className="ml-2 rounded border border-slate-300 px-3 py-2 text-xs font-semibold disabled:opacity-50" onClick={() => setSelected(new Set())}>Seçimi temizle</button>
      <div className="my-3 max-h-72 overflow-auto"><table className="w-full text-left text-xs"><thead><tr><th className="p-2">Seçim</th><th className="p-2">Personel</th><th className="p-2">Önizleme / bağımlılıklar</th></tr></thead><tbody>
        {personnel.map((person) => { const row = rows?.find((entry) => entry.id === person.id); return <tr key={person.id} className="border-t border-slate-200">
          <td className="p-2"><input type="checkbox" aria-label={`${person.ad} ${person.soyad} seç`} disabled={busy || Boolean(row?.error)} checked={selected.has(person.id)} onChange={() => setSelected((current) => { const next = new Set(current); next.has(person.id) ? next.delete(person.id) : next.add(person.id); return next; })} /></td>
          <td className="p-2 whitespace-nowrap">{person.ad} {person.soyad}</td><td className="p-2">{row ? row.error ?? `Uygun · Brüt ${draft.grossAmount} TL · ${draft.paymentDate} · ${row.affected} bordro, ${row.retro} retro yeniden hesaplanacak` : 'Önizleme bekliyor'}</td>
        </tr>; })}
      </tbody></table></div>
      <p role="status">Seçilen: {selected.size} · Uygun: {rows?.filter((row) => !row.error).length ?? 0} · Engelli: {rows?.filter((row) => row.error).length ?? 0}</p>
      <div className="mt-3 flex gap-3">
        <button type="button" className="rounded border border-slate-300 px-3 py-2 font-semibold disabled:opacity-50" disabled={busy} onClick={onClose}>Kapat</button>
        <button type="button" className="rounded border border-indigo-500 px-3 py-2 font-semibold text-indigo-800 disabled:opacity-50" disabled={busy} onClick={() => void preview()}>{busy ? 'İşleniyor…' : 'Önizle'}</button>
        <button type="button" className="rounded bg-indigo-700 px-3 py-2 text-white disabled:opacity-50" disabled={busy || !rows || !selected.size}
          onClick={async () => { setBusy(true); try { await onSave([...selected], draft); onClose(); } catch (failure) { setError(formatPayrollError(failure)); } finally { setBusy(false); } }}>Seçili {selected.size} kişiyi kaydet</button>
      </div>
    </div>
  </section>;
}
