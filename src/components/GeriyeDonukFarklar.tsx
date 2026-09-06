import React, { useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ClipboardCheck,
  FileClock,
  Plus,
  RefreshCw,
  Save,
  WalletCards,
  X,
} from 'lucide-react';
import {
  BordroDonemi,
  CompensationRevision,
  CompensationRevisionOverride,
  CompensationRevisionReason,
  CompensationRevisionScope,
  CompensationRevisionStatus,
  BordroKaydi,
  Personel,
  RetroAdjustmentBatch,
  RetroAllocation,
  RetroEarningCode,
  RetroParameterKey,
} from '../types/payroll';
import type { RetroCalculationResultModel } from '../services/payrollEngine';
import { formatTL } from '../utils/payrollPresentation';

export interface RetroPreviewInput {
  batchId: string;
  revision: CompensationRevision;
  overrides: CompensationRevisionOverride[];
  personnelId: string;
  paymentDate: string;
  calculatedAt: string;
  description?: string | null;
}

interface GeriyeDonukFarklarProps {
  donemler: BordroDonemi[];
  personeller: Personel[];
  revisions: CompensationRevision[];
  overrides: CompensationRevisionOverride[];
  batches: RetroAdjustmentBatch[];
  allocations: RetroAllocation[];
  bordrolar: BordroKaydi[];
  onSaveRevision: (
    revision: CompensationRevision,
    overrides: CompensationRevisionOverride[]
  ) => Promise<void>;
  onCalculatePreview: (request: RetroPreviewInput) => Promise<RetroCalculationResultModel>;
  onSaveBatch: (result: RetroCalculationResultModel) => Promise<void>;
  onCreatePayment: (result: RetroCalculationResultModel) => Promise<void>;
}

const REASON_LABELS: Record<CompensationRevisionReason, string> = {
  COLLECTIVE_AGREEMENT: 'Toplu iş sözleşmesi',
  ADMINISTRATIVE_DECISION: 'İdari karar',
  COURT_DECISION: 'Mahkeme kararı',
  PAY_CORRECTION: 'Yanlış tahakkuk düzeltmesi',
  MISSING_ACCRUAL: 'Eksik tahakkuk',
  OTHER: 'Diğer',
};

const SCOPE_LABELS: Record<CompensationRevisionScope, string> = {
  ALL_PERSONNEL: 'Tüm personel',
  SELECTED_PERSONNEL: 'Seçili personel',
  PERSONNEL_GROUP: 'Personel grubu',
};

const PARAMETER_LABELS: Record<RetroParameterKey, string> = {
  GUNLUK_TABAN_UCRET: 'Günlük taban ücret',
  GUNLUK_YEMEK: 'Günlük yemek',
  BIRLESTIRILMIS_SOSYAL_YARDIM: 'Birleştirilmiş sosyal yardım',
  GUNLUK_VASITA_YOL: 'Günlük vasıta / yol',
  GIYIM_YARDIMI: 'Giyim yardımı',
  HIZMET_ZAMMI_BIRIMI: 'Hizmet zammı birimi',
  IS_PRIMI_YUZDE: 'İş primi oranı (%)',
  GECE_CALISMA_PRIMI_YUZDE: 'Gece çalışma primi (%)',
  GECE_CALISMA_TATILI_PRIMI_YUZDE: 'Gece çalışması tatili primi (%)',
  EK_ODEME: 'Ek ödeme',
  DIGER_GELIR: 'Diğer gelir',
  TEDIYE: 'Tediye',
  TIS_BONUS: 'TİS ikramiyesi',
};

const EARNING_LABELS: Record<RetroEarningCode, string> = {
  BASE_WAGE: 'Taban ücret',
  NIGHT_WORK: 'Gece çalışması',
  NIGHT_HOLIDAY: 'Gece çalışması tatili',
  WORK_PREMIUM: 'İş primi',
  SOCIAL_AID: 'Sosyal yardım',
  MEAL: 'Yemek',
  TRANSPORT: 'Vasıta / yol',
  CLOTHING: 'Giyim',
  SERVICE_INCREMENT: 'Hizmet zammı',
  TIS_BONUS: 'TİS ikramiyesi',
  TEDIYE: 'Tediye',
  SUPPLEMENTAL: 'Ek ödeme',
  OTHER: 'Diğer',
};

// Tediye/TİS ikramiyesi event-specific payments.  Until their exact payment
// date and partial-period rules are modeled, the core rejects them instead of
// applying one amount to every affected service period.
const PARAMETER_KEYS = (Object.keys(PARAMETER_LABELS) as RetroParameterKey[])
  .filter((key) => key !== 'TEDIYE' && key !== 'TIS_BONUS');

function newId(prefix: string): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Math.random().toString(36).slice(2)}-${Date.now()}`;
}

function initialDate(periods: BordroDonemi[], last: boolean): string {
  const ordered = [...periods].sort((a, b) => a.baslangicTarihi.localeCompare(b.baslangicTarihi));
  return last ? ordered.at(-1)?.bitisTarihi ?? '' : ordered[0]?.baslangicTarihi ?? '';
}

function inputClass(): string {
  return 'w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs text-slate-800 shadow-xs outline-none transition focus:border-indigo-500 focus:ring-2 focus:ring-indigo-100';
}

function amountClass(value: number): string {
  return value < 0 ? 'text-rose-700' : value > 0 ? 'text-emerald-700' : 'text-slate-500';
}

function statusLabel(status?: CompensationRevisionStatus): string {
  switch (status) {
    case 'FINALIZED':
      return 'Kesinleşti';
    case 'CALCULATED':
      return 'Hesaplandı';
    case 'STALE':
      return 'Geçersizleşti';
    default:
      return 'Taslak';
  }
}

function settlementLabel(batch: RetroAdjustmentBatch): string {
  const status = batch.settlementStatus ?? (
    batch.totalGrossDelta < 0 ? 'OVERPAYMENT' : batch.status === 'FINALIZED' ? 'PAID' : 'UNSETTLED'
  );
  switch (status) {
    case 'PAID':
      return 'Ödendi';
    case 'OVERPAYMENT':
      return 'Fazla tahakkuk';
    default:
      return 'Ödeme bekliyor';
  }
}

function allocationForPeriod(
  allocations: RetroAllocation[],
  batchId: string,
  periodId: string
): RetroAllocation[] {
  return allocations.filter(
    (allocation) => allocation.batchId === batchId && allocation.sourcePeriodId === periodId
  );
}

export function GeriyeDonukFarklar({
  donemler,
  personeller,
  revisions,
  overrides: savedOverrides,
  batches,
  allocations,
  bordrolar,
  onSaveRevision,
  onCalculatePreview,
  onSaveBatch,
  onCreatePayment,
}: GeriyeDonukFarklarProps) {
  const [revisionId, setRevisionId] = useState(() => newId('revision'));
  const [batchId, setBatchId] = useState(() => newId('retro'));
  const [reason, setReason] = useState<CompensationRevisionReason>('COLLECTIVE_AGREEMENT');
  const [title, setTitle] = useState('');
  const [effectiveFrom, setEffectiveFrom] = useState(() => initialDate(donemler, false));
  const [effectiveTo, setEffectiveTo] = useState('');
  const [signedAt, setSignedAt] = useState('');
  const [description, setDescription] = useState('');
  const [scope, setScope] = useState<CompensationRevisionScope>('SELECTED_PERSONNEL');
  const [personnelId, setPersonnelId] = useState(personeller[0]?.id ?? '');
  const [personnelGroup, setPersonnelGroup] = useState('');
  const [paymentDate, setPaymentDate] = useState(() => initialDate(donemler, true));
  const [overrideRows, setOverrideRows] = useState<
    Array<{ id: string; parameter: RetroParameterKey; value: string; personnelId: string }>
  >([{ id: newId('override'), parameter: 'GUNLUK_TABAN_UCRET', value: '', personnelId: '' }]);
  const [preview, setPreview] = useState<RetroCalculationResultModel | null>(null);
  const [expandedPeriods, setExpandedPeriods] = useState<Set<string>>(new Set());
  const [isBusy, setIsBusy] = useState(false);
  const [feedback, setFeedback] = useState<{ kind: 'error' | 'success'; text: string } | null>(null);

  const selectedPersonnel = personeller.find((person) => person.id === personnelId);
  const activeRevision = revisions.find((revision) => revision.id === revisionId);
  const revisionOverrides = useMemo(
    () => savedOverrides.filter((override) => override.revisionId === revisionId),
    [revisionId, savedOverrides]
  );
  const previewTotal = preview?.batch.totalGrossDelta ?? 0;
  const hasNegativeAllocation = Boolean(
    preview?.allocations.some((allocation) => allocation.deltaAmount < 0)
  );
  const previewRetroPek = preview?.allocations.reduce(
    (total, allocation) => total + (allocation.retroPekDelta ?? 0),
    0
  ) ?? 0;

  // A preview is a read-only projection. Any authoritative prop change
  // invalidates it before the user can submit the old result.
  useEffect(() => {
    setPreview(null);
  }, [revisions, savedOverrides, batches, allocations, bordrolar]);

  const updateOverride = (id: string, patch: Partial<(typeof overrideRows)[number]>) => {
    setOverrideRows((current) =>
      current.map((row) => (row.id === id ? { ...row, ...patch } : row))
    );
    setPreview(null);
  };

  const makeRevision = (): CompensationRevision => ({
    id: revisionId,
    reason,
    title: title.trim(),
    effectiveFrom,
    effectiveTo: effectiveTo || null,
    decisionDate: signedAt || null,
    signedAt: signedAt || null,
    description: description.trim() || null,
    status: 'DRAFT',
    scope,
    personnelIds: scope === 'SELECTED_PERSONNEL' && personnelId ? [personnelId] : [],
    personnelGroup: scope === 'PERSONNEL_GROUP' ? personnelGroup.trim() || null : null,
  });

  const parseOverrides = (): CompensationRevisionOverride[] => {
    return overrideRows
      .filter((row) => row.value.trim() !== '')
      .map((row) => {
        const value = Number(row.value.replace(',', '.'));
        if (!Number.isFinite(value) || value < 0) {
          throw new Error(`${PARAMETER_LABELS[row.parameter]} için geçerli, negatif olmayan bir değer girin.`);
        }
        return {
          id: row.id,
          revisionId,
          parameter: row.parameter,
          value,
          personnelId: row.personnelId || null,
        };
      });
  };

  const validateForm = (): void => {
    if (!title.trim()) throw new Error('Revision başlığı zorunludur.');
    if (!effectiveFrom || !paymentDate) throw new Error('Yürürlük ve ödeme tarihleri zorunludur.');
    if (!personnelId) throw new Error('Önizleme personeli seçilmelidir.');
    if (scope === 'SELECTED_PERSONNEL' && !personnelId) {
      throw new Error('Seçili personel kapsamı için personel seçilmelidir.');
    }
    if (scope === 'PERSONNEL_GROUP' && !personnelGroup.trim()) {
      throw new Error('Personel grubu kapsamı için grup adı zorunludur.');
    }
  };

  const handlePreview = async () => {
    setFeedback(null);
    setIsBusy(true);
    try {
      validateForm();
      const revision = makeRevision();
      const parsedOverrides = parseOverrides();
      await onSaveRevision(revision, parsedOverrides);
      const result = await onCalculatePreview({
        batchId,
        revision,
        overrides: parsedOverrides,
        personnelId,
        paymentDate,
        calculatedAt: new Date().toISOString(),
        description: description.trim() || null,
      });
      setPreview(result);
      setFeedback({ kind: 'success', text: 'Shadow/replay hesaplaması tamamlandı. Özgün bordro kayıtları değiştirilmedi.' });
    } catch (error) {
      setFeedback({ kind: 'error', text: error instanceof Error ? error.message : String(error) });
    } finally {
      setIsBusy(false);
    }
  };

  const handleCreatePayment = async () => {
    if (!preview) return;
    setFeedback(null);
    setIsBusy(true);
    try {
      if (previewTotal <= 0 || hasNegativeAllocation) {
        throw new Error('Negatif veya sıfır delta otomatik ödeme event’ine dönüştürülemez; fazla tahakkuk olarak incelenmelidir.');
      }
      await onCreatePayment(preview);
      setFeedback({ kind: 'success', text: 'RETRO_ADJUSTMENT payment event’i oluşturuldu ve gerçek ödeme ayının vergi zincirine bağlandı.' });
      setPreview(null);
      setBatchId(newId('retro'));
    } catch (error) {
      setFeedback({ kind: 'error', text: error instanceof Error ? error.message : String(error) });
    } finally {
      setIsBusy(false);
    }
  };

  const handleSaveBatch = async () => {
    if (!preview) return;
    setFeedback(null);
    setIsBusy(true);
    try {
      if (!hasNegativeAllocation) {
        throw new Error('Yalnız negatif farklar fazla tahakkuk kaydı olarak saklanabilir.');
      }
      await onSaveBatch(preview);
      setFeedback({ kind: 'success', text: 'Fazla tahakkuk batch’i kaydedildi; ödeme event’i oluşturulmadı.' });
      setPreview(null);
      setBatchId(newId('retro'));
    } catch (error) {
      setFeedback({ kind: 'error', text: error instanceof Error ? error.message : String(error) });
    } finally {
      setIsBusy(false);
    }
  };

  const addOverride = () => {
    setOverrideRows((current) => [
      ...current,
      { id: newId('override'), parameter: 'GUNLUK_TABAN_UCRET', value: '', personnelId: '' },
    ]);
    setPreview(null);
  };

  const resetRevision = () => {
    setRevisionId(newId('revision'));
    setBatchId(newId('retro'));
    setTitle('');
    setDescription('');
    setEffectiveFrom(initialDate(donemler, false));
    setEffectiveTo('');
    setSignedAt('');
    setOverrideRows([{ id: newId('override'), parameter: 'GUNLUK_TABAN_UCRET', value: '', personnelId: '' }]);
    setPreview(null);
    setFeedback(null);
  };

  return (
    <div className="space-y-5">
      <div className="flex flex-col justify-between gap-3 rounded-2xl border border-indigo-100 bg-gradient-to-r from-indigo-50 via-white to-white p-5 shadow-sm sm:flex-row sm:items-center">
        <div>
          <div className="flex items-center gap-2 text-indigo-700">
            <FileClock aria-hidden="true" className="h-5 w-5" />
            <h1 className="text-lg font-black tracking-tight">Geriye Dönük Farklar</h1>
          </div>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-slate-600">
            Geçmiş FINALIZED bordroları değiştirmeden, tarihsel puantaj ve parametre snapshot’larıyla hedef hakkı yeniden hesaplayın; farkı kaynak dönemlere dağıtıp ödeme tarihinde ayrı bir tahakkuk oluşturun.
          </p>
        </div>
        <button type="button" onClick={resetRevision} className="inline-flex min-h-10 items-center justify-center gap-2 rounded-xl border border-slate-200 bg-white px-3 text-xs font-bold text-slate-700 shadow-xs transition hover:border-indigo-300 hover:text-indigo-700">
          <RefreshCw aria-hidden="true" className="h-4 w-4" /> Yeni revision
        </button>
      </div>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1.2fr)_minmax(340px,0.8fr)]">
        <section className="space-y-5 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
          <div className="flex items-center gap-2 border-b border-slate-100 pb-3">
            <span className="flex h-7 w-7 items-center justify-center rounded-full bg-indigo-600 text-xs font-black text-white">1</span>
            <div>
              <h2 className="text-sm font-black text-slate-900">Revision oluştur</h2>
              <p className="text-[11px] text-slate-500">Neden ve etki kapsamı hesaplamanın audit kimliğini oluşturur.</p>
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <label className="text-xs font-semibold text-slate-700">Neden
              <select className={inputClass()} value={reason} onChange={(event) => { setReason(event.target.value as CompensationRevisionReason); setPreview(null); }}>
                {Object.entries(REASON_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
              </select>
            </label>
            <label className="text-xs font-semibold text-slate-700">Başlık
              <input className={inputClass()} value={title} onChange={(event) => { setTitle(event.target.value); setPreview(null); }} placeholder="2026 TİS ücret farkı" />
            </label>
            <label className="text-xs font-semibold text-slate-700">Yürürlük tarihi
              <input type="date" className={inputClass()} value={effectiveFrom} onChange={(event) => { setEffectiveFrom(event.target.value); setPreview(null); }} />
            </label>
            <label className="text-xs font-semibold text-slate-700">Bitiş tarihi (opsiyonel)
              <input type="date" className={inputClass()} value={effectiveTo} onChange={(event) => { setEffectiveTo(event.target.value); setPreview(null); }} />
            </label>
            <label className="text-xs font-semibold text-slate-700">İmza / karar tarihi
              <input type="date" className={inputClass()} value={signedAt} onChange={(event) => { setSignedAt(event.target.value); setPreview(null); }} />
            </label>
            <label className="text-xs font-semibold text-slate-700">Personel kapsamı
              <select className={inputClass()} value={scope} onChange={(event) => { setScope(event.target.value as CompensationRevisionScope); setPreview(null); }}>
                {Object.entries(SCOPE_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
              </select>
            </label>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <label className="text-xs font-semibold text-slate-700">Önizleme personeli
              <select className={inputClass()} value={personnelId} onChange={(event) => { setPersonnelId(event.target.value); setPreview(null); }}>
                <option value="">Personel seçin</option>
                {personeller.map((person) => <option key={person.id} value={person.id}>{person.ad} {person.soyad} · {person.grup}</option>)}
              </select>
            </label>
            {scope === 'PERSONNEL_GROUP' ? (
              <label className="text-xs font-semibold text-slate-700">Personel grubu
                <input className={inputClass()} value={personnelGroup} onChange={(event) => { setPersonnelGroup(event.target.value); setPreview(null); }} placeholder="1. Grup" />
              </label>
            ) : (
              <div className="rounded-lg border border-slate-100 bg-slate-50 px-3 py-2 text-[11px] text-slate-600">
                <span className="font-bold text-slate-800">{selectedPersonnel ? `${selectedPersonnel.ad} ${selectedPersonnel.soyad}` : 'Personel seçilmedi'}</span>
                <br />Kapsam: {SCOPE_LABELS[scope]}
                <br />Bu önizleme bir personelin batch/payment event’idir.
              </div>
            )}
          </div>

          <label className="block text-xs font-semibold text-slate-700">Açıklama
            <textarea className={`${inputClass()} min-h-20 resize-y`} value={description} onChange={(event) => { setDescription(event.target.value); setPreview(null); }} placeholder="Karar veya sözleşme dayanağı, kapsam ve hesap notu" />
          </label>

          <div className="flex items-center gap-2 border-b border-slate-100 pb-3 pt-2">
            <span className="flex h-7 w-7 items-center justify-center rounded-full bg-indigo-600 text-xs font-black text-white">2</span>
            <div>
              <h2 className="text-sm font-black text-slate-900">Ücret / hak parametreleri</h2>
              <p className="text-[11px] text-slate-500">Yalnız değişen sözleşmesel alanları override edin; yasal parametreler tarihsel snapshot’tan gelir.</p>
            </div>
          </div>

          <div className="space-y-2">
            {overrideRows.map((row) => (
              <div key={row.id} className="grid gap-2 sm:grid-cols-[minmax(0,1.5fr)_minmax(120px,0.8fr)_minmax(0,1fr)_auto]">
                <select aria-label="Revize edilecek parametre" className={inputClass()} value={row.parameter} onChange={(event) => updateOverride(row.id, { parameter: event.target.value as RetroParameterKey })}>
                  {PARAMETER_KEYS.map((key) => <option key={key} value={key}>{PARAMETER_LABELS[key]}</option>)}
                </select>
                <input aria-label="Override değeri" className={inputClass()} value={row.value} onChange={(event) => updateOverride(row.id, { value: event.target.value })} placeholder="Yeni değer" inputMode="decimal" />
                <select aria-label="Override personeli" className={inputClass()} value={row.personnelId} onChange={(event) => updateOverride(row.id, { personnelId: event.target.value })}>
                  <option value="">Tüm kapsam</option>
                  {personeller.map((person) => <option key={person.id} value={person.id}>{person.ad} {person.soyad}</option>)}
                </select>
                <button type="button" aria-label="Parametre satırını kaldır" onClick={() => { setOverrideRows((current) => current.filter((item) => item.id !== row.id)); setPreview(null); }} disabled={overrideRows.length === 1} className="inline-flex min-h-10 items-center justify-center rounded-lg border border-slate-200 px-3 text-slate-400 transition hover:border-rose-200 hover:text-rose-600 disabled:cursor-not-allowed disabled:opacity-40">
                  <X aria-hidden="true" className="h-4 w-4" />
                </button>
              </div>
            ))}
            <button type="button" onClick={addOverride} className="inline-flex min-h-9 items-center gap-1.5 rounded-lg border border-dashed border-indigo-300 px-3 text-[11px] font-bold text-indigo-700 transition hover:bg-indigo-50">
              <Plus aria-hidden="true" className="h-3.5 w-3.5" /> Parametre ekle
            </button>
          </div>

          <div className="flex items-center gap-2 border-b border-slate-100 pb-3 pt-2">
            <span className="flex h-7 w-7 items-center justify-center rounded-full bg-indigo-600 text-xs font-black text-white">3</span>
            <div>
              <h2 className="text-sm font-black text-slate-900">Etkilenen dönemleri önizle ve hesapla</h2>
              <p className="text-[11px] text-slate-500">Dönemler otomatik bulunur; gerçek ödeme tarihi vergi ayını belirler.</p>
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] sm:items-end">
            <label className="text-xs font-semibold text-slate-700">Fark ödeme tarihi
              <input type="date" className={inputClass()} value={paymentDate} onChange={(event) => { setPaymentDate(event.target.value); setPreview(null); }} />
            </label>
            <div className="rounded-lg border border-indigo-100 bg-indigo-50 px-3 py-2 text-[11px] text-indigo-900">
              <span className="font-bold">Payment/tax month:</span> {paymentDate ? paymentDate.slice(0, 7) : '—'}<br />Kaynak ayların GV/DV istisnası yeniden açılmaz.
            </div>
            <button type="button" onClick={() => void handlePreview()} disabled={isBusy} className="inline-flex min-h-10 items-center justify-center gap-2 rounded-xl bg-indigo-600 px-4 text-xs font-black text-white shadow-sm transition hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50">
              <ClipboardCheck aria-hidden="true" className="h-4 w-4" /> {isBusy ? 'Hesaplanıyor…' : 'Farkı hesapla'}
            </button>
          </div>

          {scope !== 'SELECTED_PERSONNEL' && <div className="rounded-xl border border-amber-200 bg-amber-50 p-3 text-[11px] leading-relaxed text-amber-900">Tüm personel / grup kapsamı revision üzerinde saklanır; bu ekran her payment event’ini tek personel batch’i olarak oluşturur. Kapsamdaki her personel için önizleme personelini değiştirip ayrı batch oluşturun.</div>}
          {activeRevision && <div className="text-[11px] text-slate-500">Revision durumu: <span className="font-bold text-slate-700">{statusLabel(activeRevision.status)}</span> · {revisionOverrides.length} kayıtlı override</div>}
        </section>

        <section className="space-y-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
          <div className="flex items-center gap-2 border-b border-slate-100 pb-3">
            <WalletCards aria-hidden="true" className="h-5 w-5 text-indigo-600" />
            <div><h2 className="text-sm font-black text-slate-900">Hesap sonucu</h2><p className="text-[11px] text-slate-500">Target − recognized = yeni delta</p></div>
          </div>

          {feedback && <div role="alert" className={`flex items-start gap-2 rounded-xl border p-3 text-xs font-semibold ${feedback.kind === 'error' ? 'border-rose-200 bg-rose-50 text-rose-800' : 'border-emerald-200 bg-emerald-50 text-emerald-800'}`}>
            {feedback.kind === 'error' ? <AlertTriangle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" /> : <CheckCircle2 aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />}
            <span>{feedback.text}</span>
          </div>}

          {!preview ? (
            <div className="rounded-xl border border-dashed border-slate-200 bg-slate-50 p-6 text-center text-xs leading-relaxed text-slate-500">Revision ve en az bir tarihsel kapsam tanımladıktan sonra hesap önizlemesi burada görünür.</div>
          ) : (
            <>
              <div className="grid gap-2 sm:grid-cols-4">
                <div className="rounded-xl border border-slate-100 bg-slate-50 p-3"><div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">Toplam brüt fark</div><div className={`mt-1 font-mono text-lg font-black ${amountClass(previewTotal)}`}>{formatTL(previewTotal)}</div></div>
                <div className="rounded-xl border border-slate-100 bg-slate-50 p-3"><div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">Kaynak dönem</div><div className="mt-1 text-lg font-black text-slate-900">{preview.periods.length}</div></div>
                <div className="rounded-xl border border-slate-100 bg-slate-50 p-3"><div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">SGK PEK farkı</div><div className="mt-1 font-mono text-lg font-black text-slate-900">{formatTL(previewRetroPek)}</div></div>
                <div className="rounded-xl border border-slate-100 bg-slate-50 p-3"><div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">Ödeme ayı</div><div className="mt-1 text-lg font-black text-slate-900">{preview.batch.paymentDate.slice(0, 7)}</div></div>
              </div>

              {hasNegativeAllocation && <div className="flex items-start gap-2 rounded-xl border border-amber-200 bg-amber-50 p-3 text-xs font-semibold text-amber-900"><AlertTriangle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />Negatif allocation bulundu. Bu sonuç ödeme değildir; fazla tahakkuk kaydı olarak saklanıp ayrıca incelenmelidir.</div>}

              <div className="overflow-x-auto rounded-xl border border-slate-200">
                <table className="min-w-full text-left text-xs">
                  <thead className="bg-slate-50 text-[10px] uppercase tracking-wide text-slate-500"><tr><th className="px-3 py-2">Dönem</th><th className="px-3 py-2 text-right">Eski tanınmış</th><th className="px-3 py-2 text-right">Yeni hak</th><th className="px-3 py-2 text-right">Önceki retro</th><th className="px-3 py-2 text-right">Yeni fark</th></tr></thead>
                  <tbody className="divide-y divide-slate-100">
                    {preview.periods.map((period) => {
                      const expanded = expandedPeriods.has(period.sourcePeriodId);
                      const periodAllocations = allocationForPeriod(preview.allocations, preview.batch.id, period.sourcePeriodId);
                      return <React.Fragment key={period.sourcePeriodId}>
                        <tr className="hover:bg-slate-50">
                          <td className="px-3 py-2 font-bold text-slate-800"><button type="button" className="inline-flex items-center gap-1.5" onClick={() => setExpandedPeriods((current) => { const next = new Set(current); if (next.has(period.sourcePeriodId)) next.delete(period.sourcePeriodId); else next.add(period.sourcePeriodId); return next; })}>{expanded ? <ChevronDown aria-hidden="true" className="h-3.5 w-3.5 text-indigo-600" /> : <ChevronRight aria-hidden="true" className="h-3.5 w-3.5 text-slate-400" />}{period.sourcePeriodId}</button></td>
                          <td className="px-3 py-2 text-right font-mono text-slate-600">{formatTL(period.originalRecognizedAmount)}</td>
                          <td className="px-3 py-2 text-right font-mono font-semibold text-slate-900">{formatTL(period.targetAmount)}</td>
                          <td className="px-3 py-2 text-right font-mono text-slate-600">{formatTL(period.previousAuthoritativeRetroAmount)}</td>
                          <td className={`px-3 py-2 text-right font-mono font-black ${amountClass(period.deltaAmount)}`}>{formatTL(period.deltaAmount)}</td>
                        </tr>
                        {expanded && <tr><td colSpan={5} className="bg-slate-50 px-3 py-3"><div className="space-y-1.5 text-[11px]">{periodAllocations.length === 0 ? <span className="text-slate-500">Bu dönemde yeni allocation yok.</span> : periodAllocations.map((allocation) => <div key={allocation.id} className="rounded-lg bg-white px-3 py-2"><div className="grid grid-cols-[minmax(0,1fr)_auto_auto] gap-3"><span className="font-semibold text-slate-700">{EARNING_LABELS[allocation.earningCode]}</span><span className="font-mono text-slate-500">{allocation.sgkTreatment}</span><span className={`font-mono font-black ${amountClass(allocation.deltaAmount)}`}>{formatTL(allocation.deltaAmount)}</span></div><div className="mt-1 grid gap-x-3 gap-y-0.5 text-[10px] text-slate-500 sm:grid-cols-4"><span>Eski PEK: <b className="font-mono text-slate-700">{formatTL(allocation.originalPek ?? 0)}</b></span><span>PEK farkı: <b className="font-mono text-slate-700">{formatTL(allocation.retroPekDelta ?? 0)}</b></span><span>Yeni PEK: <b className="font-mono text-slate-700">{formatTL(allocation.adjustedPek ?? 0)}</b></span><span>İşçi SGK farkı: <b className="font-mono text-slate-700">{formatTL(allocation.workerSgkDelta ?? 0)}</b></span><span>İşçi işsizlik: <b className="font-mono text-slate-700">{formatTL(allocation.workerUnemploymentDelta ?? 0)}</b></span><span>İşveren SGK: <b className="font-mono text-slate-700">{formatTL(allocation.employerSgkDelta ?? 0)}</b></span><span>İşveren işsizlik: <b className="font-mono text-slate-700">{formatTL(allocation.employerUnemploymentDelta ?? 0)}</b></span><span>GV/DV: <b className="text-slate-700">{preview.batch.paymentDate.slice(0, 7)}</b></span></div></div>)}</div></td></tr>}
                      </React.Fragment>;
                    })}
                  </tbody>
                </table>
              </div>

              <div className="rounded-xl border border-indigo-100 bg-indigo-50 p-3 text-[11px] leading-relaxed text-indigo-950">
                <span className="font-black">Audit:</span> kaynak dönem PEK/SGK farkı allocation üzerinde saklanır; GV ve DV payment month ({preview.batch.paymentDate.slice(0, 7)}) üzerinden canonical payment order’a girer.
              </div>

              <button type="button" onClick={() => void handleCreatePayment()} disabled={isBusy || previewTotal <= 0 || hasNegativeAllocation} className="inline-flex min-h-11 w-full items-center justify-center gap-2 rounded-xl bg-emerald-600 px-4 text-xs font-black text-white shadow-sm transition hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50">
                <Save aria-hidden="true" className="h-4 w-4" /> Geriye Dönük Fark Tahakkuku Oluştur
              </button>
              {hasNegativeAllocation && <button type="button" onClick={() => void handleSaveBatch()} disabled={isBusy} className="inline-flex min-h-11 w-full items-center justify-center gap-2 rounded-xl border border-amber-300 bg-amber-50 px-4 text-xs font-black text-amber-900 transition hover:bg-amber-100 disabled:cursor-not-allowed disabled:opacity-50">
                <Save aria-hidden="true" className="h-4 w-4" /> Fazla Tahakkuk Kaydını Sakla
              </button>}
            </>
          )}
        </section>
      </div>

      <section className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
        <div className="mb-3 flex items-center gap-2"><FileClock aria-hidden="true" className="h-4 w-4 text-slate-500" /><h2 className="text-sm font-black text-slate-900">Geçmiş retro batch’leri</h2></div>
        {batches.length === 0 ? <p className="text-xs text-slate-500">Henüz kaydedilmiş retro batch bulunmuyor.</p> : <div className="overflow-x-auto"><table className="min-w-full text-left text-xs"><thead className="border-b border-slate-100 text-[10px] uppercase tracking-wide text-slate-500"><tr><th className="px-2 py-2">Revision</th><th className="px-2 py-2">Personel</th><th className="px-2 py-2">Ödeme</th><th className="px-2 py-2 text-right">Brüt fark</th><th className="px-2 py-2 text-right">Net ödeme</th><th className="px-2 py-2">Durum</th></tr></thead><tbody className="divide-y divide-slate-100">{[...batches].sort((a, b) => b.paymentDate.localeCompare(a.paymentDate)).map((batch) => { const person = personeller.find((item) => item.id === batch.personnelId); const payroll = bordrolar.find((item) => item.accrualId === batch.id); return <tr key={batch.id}><td className="px-2 py-2 font-semibold text-slate-700">{revisions.find((item) => item.id === batch.revisionId)?.title ?? batch.revisionId}</td><td className="px-2 py-2 text-slate-600">{person ? `${person.ad} ${person.soyad}` : batch.personnelId}</td><td className="px-2 py-2 text-slate-600">{batch.paymentDate}</td><td className={`px-2 py-2 text-right font-mono font-bold ${amountClass(batch.totalGrossDelta)}`}>{formatTL(batch.totalGrossDelta)}</td><td className="px-2 py-2 text-right font-mono text-slate-700">{payroll ? formatTL(payroll.netOdeme) : '—'}</td><td className="px-2 py-2"><div className="space-y-1"><span className={`rounded-full px-2 py-1 text-[10px] font-bold ${batch.status === 'FINALIZED' ? 'bg-emerald-50 text-emerald-700' : batch.status === 'STALE' ? 'bg-amber-50 text-amber-700' : 'bg-indigo-50 text-indigo-700'}`}>{statusLabel(batch.status)}</span><div className="text-[10px] font-semibold text-slate-500">{settlementLabel(batch)}</div></div></td></tr>; })}</tbody></table></div>}
      </section>
    </div>
  );
}
