import React, { useEffect, useState } from 'react';
import type {
  CompensationBoundaryScope,
  CompensationPolicyDraftValidation,
  CompensationPolicyLoadData,
  CompensationPolicyPageProps,
  CompensationPolicyStateDraft,
  RoadRoundingMode,
} from '../../services/compensationPolicy/contract';
import {
  applyCompensationPolicyPercentageIncrease,
  formatCompensationPolicyDecimalForUser,
  findScopedSameDateActiveLeaf,
  getCompensationPolicyDraftFieldErrors,
} from './compensationPolicyDecimal';

type Draft = CompensationPolicyPageProps['draft'];

export interface CompensationPolicyFormProps {
  draft: Draft;
  loadData: CompensationPolicyLoadData;
  validation: CompensationPolicyDraftValidation;
  disabled: boolean;
  showValidation: boolean;
  scopeSelection: CompensationBoundaryScope | null;
  roundingSelection: RoadRoundingMode | null;
  onDraftChange(draft: Draft): void;
  onSelectBoundaryScope(scope: CompensationBoundaryScope): void;
  onSelectRoadRoundingMode(mode: RoadRoundingMode): void;
  onCopyActiveState(): void;
  canApplyPercentageIncrease: boolean;
  groupsOnly?: boolean;
}

const inputClassName = 'min-h-10 w-full rounded-md border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 font-mono focus:border-indigo-600 focus:outline-none focus:ring-2 focus:ring-indigo-200 disabled:bg-slate-100';
const compactGroupInputClassName = inputClassName.replace('min-h-10', 'min-h-8 sm:min-h-10').replace('px-3 py-2', 'px-2 py-1 sm:px-3 sm:py-2');
const errorClassName = 'mt-1 text-xs font-medium text-rose-700';

interface TextDecimalInputProps {
  id: string;
  label: string;
  value: string;
  error?: string;
  help?: string;
  disabled: boolean;
  onChange(value: string): void;
}

const TextDecimalInput: React.FC<TextDecimalInputProps> = ({ id, label, value, error, help, disabled, onChange }) => (
  <div className="min-w-0">
    <label htmlFor={id} className="mb-1 block text-sm font-semibold text-slate-800">{label}</label>
    <input
      id={id}
      type="text"
      inputMode="decimal"
      autoComplete="off"
      spellCheck={false}
      value={value}
      aria-invalid={Boolean(error)}
      aria-describedby={[help ? `${id}-help` : null, error ? `${id}-error` : null].filter(Boolean).join(' ') || undefined}
      className={`${inputClassName} ${error ? 'border-rose-500 focus:border-rose-600 focus:ring-rose-100' : ''}`}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value)}
    />
    {help && <p id={`${id}-help`} className="mt-1 text-xs text-slate-500">{help}</p>}
    {error && <p id={`${id}-error`} className={errorClassName}>{error}</p>}
  </div>
);

interface OptionalDecimalInputProps {
  id: string;
  label: string;
  toggleLabel: string;
  value: CompensationPolicyStateDraft['isPrimiYuzde'];
  error?: string;
  help?: string;
  disabled: boolean;
  onChange(value: CompensationPolicyStateDraft['isPrimiYuzde']): void;
}

const OptionalDecimalInput: React.FC<OptionalDecimalInputProps> = ({ id, label, toggleLabel, value, error, help, disabled, onChange }) => {
  const [lastRaw, setLastRaw] = useState(value.kind === 'VALUE' ? value.raw : '');
  useEffect(() => {
    if (value.kind === 'VALUE') setLastRaw(value.raw);
  }, [value]);
  return (
    <div className="min-w-0 space-y-2">
      <label htmlFor={`${id}-presence`} className="flex min-h-8 items-center gap-2 text-sm font-medium text-slate-800">
        <input
          id={`${id}-presence`}
          type="checkbox"
          checked={value.kind === 'VALUE'}
          disabled={disabled}
          className="h-4 w-4 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
          onChange={(event) => onChange(event.target.checked ? { kind: 'VALUE', raw: lastRaw } : { kind: 'ABSENT' })}
        />
        {toggleLabel}
      </label>
      {value.kind === 'VALUE' && (
        <div>
          <label htmlFor={id} className="mb-1 block text-xs font-semibold text-slate-700">{label}</label>
          <input
            id={id}
            type="text"
            inputMode="decimal"
            autoComplete="off"
            spellCheck={false}
            aria-invalid={Boolean(error)}
            aria-describedby={[help ? `${id}-help` : null, error ? `${id}-error` : null].filter(Boolean).join(' ') || undefined}
            className={`${inputClassName} ${error ? 'border-rose-500 focus:border-rose-600 focus:ring-rose-100' : ''}`}
            value={value.raw}
            disabled={disabled}
            onChange={(event) => {
              setLastRaw(event.target.value);
              onChange({ kind: 'VALUE', raw: event.target.value });
            }}
          />
          {error && <p id={`${id}-error`} className={errorClassName}>{error}</p>}
          {help && <p id={`${id}-help`} className="mt-1 text-xs text-slate-500">{help}</p>}
        </div>
      )}
    </div>
  );
};

export const CompensationPolicyForm: React.FC<CompensationPolicyFormProps> = ({
  draft,
  loadData,
  validation,
  disabled,
  showValidation,
  scopeSelection,
  roundingSelection,
  onDraftChange,
  onSelectBoundaryScope,
  onSelectRoadRoundingMode,
  onCopyActiveState,
  canApplyPercentageIncrease,
  groupsOnly = false,
}) => {
  const [percentageRaw, setPercentageRaw] = useState('');
  const [percentageFeedback, setPercentageFeedback] = useState<{ kind: 'success' | 'error'; message: string } | null>(null);
  const fieldErrors = getCompensationPolicyDraftFieldErrors(draft);
  const required = (field: keyof CompensationPolicyStateDraft): string | undefined => showValidation ? fieldErrors[field] : undefined;
  const effectiveDateError = showValidation ? validation.operationErrors.find((error) => error.includes('tarihi')) : undefined;
  const reasonError = showValidation ? validation.operationErrors.find((error) => error.includes('gerekçesi')) : undefined;
  const groupListError = required('isPrimiGruplari');
  const selectedVersionId = draft.operationKind === 'SAME_DATE_CORRECTION'
    ? findScopedSameDateActiveLeaf(loadData.timeline.versions, draft.effectiveFrom)
    : null;
  const correctionDates = Array.from(new Set<string>(loadData.timeline.versions.map((version) => version.effectiveFrom)))
    .filter((date) => findScopedSameDateActiveLeaf(loadData.timeline.versions, date) !== null)
    .sort();
  const canRestorePolicySource = draft.operationKind === 'SAME_DATE_CORRECTION'
    ? selectedVersionId !== null
    : draft.operationKind !== 'INITIAL_BASELINE' && loadData.activePolicyProjection.kind === 'ACTIVE';
  const restoreSourceLabel = draft.operationKind === 'SAME_DATE_CORRECTION'
    ? 'Düzeltilecek kayıt bilgilerini yeniden al'
    : 'Etkin ücretleri ve seçimleri taslağa geri al';
  const calculationSummary = scopeSelection === null || roundingSelection === null
    ? 'Seçim gerekli'
    : `${scopeSelection === 'ROAD_ONLY' ? 'Yalnız yol' : 'Tüm ücretler'} · ${roundingSelection === 'PERIOD_TOTAL' ? 'Dönem toplamı' : 'Kazanç dilimi başına'}`;

  const change = (patch: Partial<Draft>) => onDraftChange({ ...draft, ...patch });
  const applyIncrease = () => {
    const result = applyCompensationPolicyPercentageIncrease(draft, percentageRaw);
    if (result.ok === false) {
      setPercentageFeedback({ kind: 'error', message: result.error });
      return;
    }
    onDraftChange({ ...draft, ...result.draft });
    setPercentageFeedback({ kind: 'success', message: 'Günlük taban ücret ve yemek güncellendi. Diğer ücret alanları değişmedi; yeni değerleri gözden geçirin.' });
  };

  return (
    <form className="space-y-4" onSubmit={(event) => event.preventDefault()} aria-label="Ücret politikası formu">
      <fieldset disabled={disabled} className="space-y-4">
        <legend className="sr-only">Ücret politikası ayrıntıları</legend>
        <section aria-labelledby="policy-operation-heading" className="space-y-3">
          <h3 id="policy-operation-heading" className="text-sm font-bold text-slate-900">{draft.operationKind === 'INITIAL_BASELINE' ? 'Başlangıç tarihi ve açıklama' : 'Geçerlilik tarihi ve açıklama'}</h3>
          <div className="grid gap-3 md:grid-cols-3">
            {!loadData.initialBaselineRequired && <div>
              <label htmlFor="policy-operation" className="mb-1 block text-sm font-semibold text-slate-800">İşlem türü</label>
              <select
                id="policy-operation"
                className={inputClassName.replace('font-mono', '')}
                value={draft.operationKind}
                onChange={(event) => {
                  const operationKind = event.target.value as Draft['operationKind'];
                  const leafId = operationKind === 'SAME_DATE_CORRECTION'
                    ? findScopedSameDateActiveLeaf(loadData.timeline.versions, draft.effectiveFrom)
                    : null;
                  change({ operationKind, supersedesVersionId: leafId });
                }}
              >
                {loadData.initialBaselineRequired && <option value="INITIAL_BASELINE">İlk başlangıç ücret politikası</option>}
                {!loadData.initialBaselineRequired && <option value="NEW_EFFECTIVE_DATE">Yeni geçerlilik tarihi</option>}
                {!loadData.initialBaselineRequired && <option value="SAME_DATE_CORRECTION">Aynı tarihte düzeltme</option>}
              </select>
            </div>}
            <div>
              {draft.operationKind === 'SAME_DATE_CORRECTION' ? (
                <>
                  <label htmlFor="policy-effective-date" className="mb-1 block text-sm font-semibold text-slate-800">Düzeltme tarihi</label>
                  <select
                    id="policy-effective-date"
                    className={inputClassName.replace('font-mono', '')}
                    value={draft.effectiveFrom}
                    aria-invalid={showValidation && selectedVersionId === null}
                    aria-describedby={showValidation && selectedVersionId === null
                      ? 'policy-correction-target-error'
                      : selectedVersionId ? 'policy-correction-target-help' : undefined}
                    onChange={(event) => {
                      const effectiveFrom = event.target.value;
                      const leafId = findScopedSameDateActiveLeaf(loadData.timeline.versions, effectiveFrom);
                      change({ effectiveFrom, supersedesVersionId: leafId });
                    }}
                  >
                    <option value="">Düzeltilecek tarihi seçin</option>
                    {correctionDates.map((date) => <option key={date} value={date}>{date}</option>)}
                    {draft.effectiveFrom && !correctionDates.includes(draft.effectiveFrom) && (
                      <option value={draft.effectiveFrom}>{draft.effectiveFrom} — etkin kayıt bulunamadı</option>
                    )}
                  </select>
                  {selectedVersionId ? (
                    <p id="policy-correction-target-help" className="mt-1 text-xs text-slate-600">
                      Bu tarihteki etkin kayıt düzeltilecek: {loadData.timeline.versions.find((version) => version.id === selectedVersionId)?.reason || 'Kayıtlı ücret bilgisi'}.
                    </p>
                  ) : showValidation && (
                    <p id="policy-correction-target-error" role="alert" className={errorClassName}>Bu tarihte düzeltilebilecek tek bir etkin kayıt bulunamadı. Ücret geçmişini yenileyip tekrar deneyin.</p>
                  )}
                </>
              ) : (
                <div className="min-w-0">
                  <label htmlFor="policy-effective-date" className="mb-1 block text-sm font-semibold text-slate-800">
                    {draft.operationKind === 'INITIAL_BASELINE' ? 'Başlangıç tarihi' : 'Geçerlilik tarihi'}
                  </label>
                  <input
                    id="policy-effective-date"
                    type="date"
                    value={draft.effectiveFrom}
                    disabled={disabled}
                    aria-required="true"
                    aria-invalid={Boolean(effectiveDateError)}
                    aria-describedby={effectiveDateError ? 'policy-effective-date-error' : undefined}
                    className={`${inputClassName.replace('font-mono', '')} ${effectiveDateError ? 'border-rose-500 focus:border-rose-600 focus:ring-rose-100' : ''}`}
                    onChange={(event) => change({ effectiveFrom: event.target.value })}
                  />
                  {effectiveDateError && <p id="policy-effective-date-error" className={errorClassName}>{effectiveDateError}</p>}
                </div>
              )}
            </div>
            <div>
              <label htmlFor="policy-reason" className="mb-1 block text-sm font-semibold text-slate-800">Kısa açıklama</label>
              <textarea
                id="policy-reason"
                rows={2}
                value={draft.reason}
                aria-required="true"
                aria-invalid={Boolean(reasonError)}
                aria-describedby={reasonError ? 'policy-reason-error' : undefined}
                className={`${inputClassName.replace('font-mono', '')} resize-y`}
                onChange={(event) => change({ reason: event.target.value })}
              />
              <p className="mt-1 text-xs text-slate-500">Kayıt için gereklidir.</p>
              {reasonError && <p id="policy-reason-error" className={errorClassName}>{reasonError}</p>}
            </div>
          </div>
          {draft.operationKind === 'SAME_DATE_CORRECTION' && draft.supersedesVersionId !== selectedVersionId && (
            <p role="alert" className="rounded-md border border-rose-300 bg-rose-50 p-2 text-sm text-rose-800">
              Düzeltme hedefi yenilenen ücret geçmişiyle eşleşmiyor. Önizleme öncesinde geçmişi yenileyin.
            </p>
          )}
        </section>

        {canRestorePolicySource && (
          <section className="flex flex-wrap items-center justify-between gap-3 border-t border-slate-200 pt-4">
            <p className="max-w-2xl text-xs text-slate-600">
              Kayıtlı tutar, kapsam ve yol yuvarlama seçimi taslağa birlikte yeniden alınır.
            </p>
            <button type="button" onClick={onCopyActiveState} className="min-h-10 rounded-md border border-indigo-300 px-3 py-2 text-sm font-semibold text-indigo-800 hover:bg-indigo-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">
              {restoreSourceLabel}
            </button>
          </section>
        )}

        {!groupsOnly && <>
        <section aria-labelledby="policy-money-heading" data-testid="compensation-policy-core-fields" className="space-y-3 border-t border-slate-200 pt-3">
          <div>
            <h3 id="policy-money-heading" className="text-sm font-bold text-slate-900">Ücretler ve yan haklar</h3>
            <p className="mt-1 text-xs text-slate-600">Günlük tutarları TL olarak girin. Ondalık için virgül veya nokta kullanın.</p>
          </div>
          <div className="grid gap-x-4 gap-y-3 sm:grid-cols-2 xl:grid-cols-3">
            <TextDecimalInput id="policy-base-wage" label="Günlük taban ücret" value={draft.gunlukTabanUcret} error={required('gunlukTabanUcret')} disabled={disabled} onChange={(gunlukTabanUcret) => change({ gunlukTabanUcret })} />
            <TextDecimalInput id="policy-meal" label="Günlük yemek" value={draft.gunlukYemek} error={required('gunlukYemek')} disabled={disabled} onChange={(gunlukYemek) => change({ gunlukYemek })} />
            <TextDecimalInput id="policy-social" label="Sosyal yardım" value={draft.birlestirilmisSosyalYardim} error={required('birlestirilmisSosyalYardim')} disabled={disabled} onChange={(birlestirilmisSosyalYardim) => change({ birlestirilmisSosyalYardim })} />
            <TextDecimalInput id="policy-road" label="Yol" value={draft.gunlukVasitaYol} error={required('gunlukVasitaYol')} disabled={disabled} onChange={(gunlukVasitaYol) => change({ gunlukVasitaYol })} />
            <TextDecimalInput id="policy-clothing" label="Giyim yardımı" value={draft.giyimYardimi} error={required('giyimYardimi')} disabled={disabled} onChange={(giyimYardimi) => change({ giyimYardimi })} />
            <TextDecimalInput id="policy-service-unit" label="Hizmet zammı birimi" value={draft.hizmetZammiBirimi} error={required('hizmetZammiBirimi')} disabled={disabled} onChange={(hizmetZammiBirimi) => change({ hizmetZammiBirimi })} />
          </div>
        </section>

        </>}

        {groupsOnly && <>
        <section aria-labelledby="policy-premium-heading" className="space-y-2 border-t border-slate-200 pt-3">
          <div>
            <h3 id="policy-premium-heading" className="text-sm font-bold text-slate-900">İş primi grupları</h3>
            <p className="mt-1 text-xs text-slate-600">Personel kartındaki grup ve oran aynı satırda gösterilir.</p>
          </div>
          {groupListError && <p id="policy-group-error" role="alert" className={errorClassName}>{groupListError}</p>}
          {showValidation && draft.isPrimiGruplari.length === 0 && <p id="policy-groups-error" role="alert" className={errorClassName}>En az bir iş primi grubu ekleyin.</p>}
          <table data-testid="policy-premium-group-table" className="block w-full border-collapse text-left text-sm sm:table sm:table-fixed">
            <caption className="sr-only">İş primi grupları, oranları ve etkinlik durumları</caption>
            <colgroup className="hidden sm:table-column-group">
              <col className="sm:w-[55%]" />
              <col className="sm:w-[20%]" />
              <col className="sm:w-[13%]" />
              <col className="sm:w-[12%]" />
            </colgroup>
            <thead className="hidden border-b border-slate-200 text-xs font-semibold text-slate-600 sm:table-header-group">
              <tr>
                <th scope="col" className="px-2 py-2">Grup adı</th>
                <th scope="col" className="px-2 py-2">Oran (%)</th>
                <th scope="col" className="px-2 py-2">Etkin</th>
                <th scope="col" className="px-2 py-2 text-right">İşlemler</th>
              </tr>
            </thead>
            <tbody className="block w-full sm:table-row-group">
              {draft.isPrimiGruplari.map((group, index) => {
                const nameInvalid = showValidation && (!group.ad.trim() || group.ad.trim() !== group.ad || groupListError?.toLocaleLowerCase('tr').includes('yinelenen'));
                const idInvalid = showValidation && (!group.id.trim() || group.id.trim() !== group.id || groupListError?.toLocaleLowerCase('tr').includes('yinelenen'));
                const rateInvalid = Boolean(groupListError && showValidation);
                const updateGroup = (patch: Partial<Draft['isPrimiGruplari'][number]>) => change({
                  isPrimiGruplari: draft.isPrimiGruplari.map((item, itemIndex) => itemIndex === index ? { ...item, ...patch } : item),
                });
                return (
                  <React.Fragment key={`${group.id}:${index}`}>
                    <tr data-testid="policy-premium-group-row" className="grid grid-cols-[minmax(0,1fr)_6rem] gap-x-2 border-b border-slate-200 px-2 py-2 sm:table-row sm:border-0 sm:px-0 sm:py-0">
                      <td className="block min-w-0 py-1 sm:table-cell sm:px-2 sm:py-2">
                        <label htmlFor={`policy-group-name-${index}`} className="mb-1 block text-[11px] font-medium text-slate-600 sm:sr-only">Grup adı</label>
                        <input
                          id={`policy-group-name-${index}`}
                          value={group.ad}
                          aria-invalid={Boolean(nameInvalid)}
                          aria-describedby={groupListError ? 'policy-group-error' : undefined}
                          className={compactGroupInputClassName.replace('font-mono', '')}
                          disabled={disabled}
                          onChange={(event) => updateGroup({ ad: event.target.value })}
                        />
                      </td>
                      <td className="block min-w-0 py-1 sm:table-cell sm:px-2 sm:py-2">
                        <label htmlFor={`policy-group-rate-${index}`} className="mb-1 block text-[11px] font-medium text-slate-600 sm:sr-only">Oran (%)</label>
                        <input
                          id={`policy-group-rate-${index}`}
                          type="text"
                          inputMode="decimal"
                          autoComplete="off"
                          spellCheck={false}
                          value={group.oran}
                          aria-invalid={rateInvalid}
                          aria-describedby={groupListError ? 'policy-group-error' : undefined}
                          className={`${compactGroupInputClassName} ${rateInvalid ? 'border-rose-500 focus:border-rose-600 focus:ring-rose-100' : ''}`}
                          disabled={disabled}
                          onChange={(event) => updateGroup({ oran: event.target.value })}
                        />
                      </td>
                      <td className="block min-w-0 py-1 sm:table-cell sm:px-2 sm:py-2">
                        <div className="flex items-center justify-between gap-2 sm:flex-col sm:items-start sm:justify-start">
                          <label className="inline-flex min-h-7 items-center gap-2 text-sm font-medium text-slate-800">
                            <input
                              type="checkbox"
                              checked={group.aktif}
                              aria-label={`${group.ad || `Grup ${index + 1}`} etkin`}
                              disabled={disabled}
                              className="h-4 w-4 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
                              onChange={(event) => updateGroup({ aktif: event.target.checked })}
                            />
                            Etkin
                          </label>
                          <details open={Boolean(idInvalid)} className="min-w-0 text-[11px] text-slate-600 sm:text-xs">
                            <summary className="cursor-pointer whitespace-nowrap rounded py-1 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">Kod</summary>
                            <label htmlFor={`policy-group-id-${index}`} className="mt-1 block font-semibold text-slate-700">Personel eşleştirme kodu</label>
                            <input
                              id={`policy-group-id-${index}`}
                              value={group.id}
                              aria-invalid={Boolean(idInvalid)}
                              aria-describedby={groupListError ? 'policy-group-error' : undefined}
                              className={`${compactGroupInputClassName.replace('font-mono', '')} mt-1`}
                              disabled={disabled}
                              onChange={(event) => updateGroup({ id: event.target.value })}
                            />
                            <p className="mt-1">Personel kartında seçilen grupla eşleşir.</p>
                          </details>
                        </div>
                      </td>
                      <td className="flex items-center justify-end py-2 sm:table-cell sm:px-2 sm:text-right">
                        <button
                          type="button"
                          className="rounded px-2 py-1 text-xs font-medium text-rose-700 hover:bg-rose-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rose-600"
                          aria-label={`${group.ad || `Grup ${index + 1}`} grubunu kaldır`}
                          disabled={disabled}
                          onClick={() => change({ isPrimiGruplari: draft.isPrimiGruplari.filter((_, itemIndex) => itemIndex !== index) })}
                        >
                          Kaldır
                        </button>
                      </td>
                    </tr>
                  </React.Fragment>
                );
              })}
            </tbody>
          </table>
          <button type="button" className="min-h-8 rounded px-2 py-1 text-xs font-semibold text-indigo-800 underline underline-offset-2 hover:bg-indigo-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600" disabled={disabled} onClick={() => change({ isPrimiGruplari: [...draft.isPrimiGruplari, { id: '', ad: '', oran: '', aktif: false }] })}>Grup ekle</button>
        </section>
        </>}

        {!groupsOnly && <>
        <details className="border-t border-slate-200 pt-3">
          <summary className="cursor-pointer py-1 text-sm font-semibold text-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">
            Gece ve tatil çalışması · gece %{formatCompensationPolicyDecimalForUser(draft.geceCalismaPrimiYuzde)} · tatil %{formatCompensationPolicyDecimalForUser(draft.geceCalismaTatiliPrimiYuzde)}
          </summary>
          <p className="mt-1 text-xs text-slate-600">İlgili çalışma için uygulanacak yüzdeler.</p>
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            <TextDecimalInput id="policy-night-premium" label="Gece çalışma primi (%)" value={draft.geceCalismaPrimiYuzde} error={required('geceCalismaPrimiYuzde')} disabled={disabled} onChange={(geceCalismaPrimiYuzde) => change({ geceCalismaPrimiYuzde })} />
            <TextDecimalInput id="policy-holiday-night-premium" label="Tatil gecesi primi (%)" value={draft.geceCalismaTatiliPrimiYuzde} error={required('geceCalismaTatiliPrimiYuzde')} disabled={disabled} onChange={(geceCalismaTatiliPrimiYuzde) => change({ geceCalismaTatiliPrimiYuzde })} />
          </div>
          <p className="mt-2 text-xs text-slate-500">Yüzdeleri 0–100 arasında girin.</p>
        </details>

        <details id="policy-oneoff-details" className="border-t border-slate-200 pt-4">
          <summary id="policy-oneoff-heading" className="cursor-pointer text-sm font-semibold text-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">Ek gelir varsayılanları</summary>
          <div className="mt-3 grid gap-3 md:grid-cols-2">
            <OptionalDecimalInput id="policy-extra-payment" label="Ek ödeme (TL)" toggleLabel="Ek ödeme tutarı tanımla" value={draft.ekOdeme} error={required('ekOdeme')} disabled={disabled} onChange={(ekOdeme) => change({ ekOdeme })} />
            <OptionalDecimalInput id="policy-other-income" label="Diğer gelir varsayılanı (TL)" toggleLabel="Varsayılan tutar tanımla" value={draft.digerGelirVarsayilan} error={required('digerGelirVarsayilan')} disabled={disabled} onChange={(digerGelirVarsayilan) => change({ digerGelirVarsayilan })} />
          </div>
        </details>

        <details id="policy-legacy-premium-details" className="border-t border-slate-200 pt-3">
          <summary className="cursor-pointer py-1 text-sm font-semibold text-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">Eski genel iş primi alanı · uyumluluk</summary>
          <p className="mt-1 text-xs text-slate-600">Yeni bordro hesaplamasında iş primi oranı personel grubundan alınır; bu alan grup oranının yerine geçmez.</p>
          <div className="mt-3">
            <OptionalDecimalInput id="policy-global-premium" label="Genel iş primi (%)" toggleLabel="Kayıtlı genel oranı koru" value={draft.isPrimiYuzde} error={required('isPrimiYuzde')} disabled={disabled} onChange={(isPrimiYuzde) => change({ isPrimiYuzde })} />
          </div>
        </details>

        <details className="border-t border-slate-200 pt-4">
          <summary className="cursor-pointer py-1 text-sm font-semibold text-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">Günlük taban ücret ve yemeği yüzdeyle artır</summary>
          <div className="mt-3 flex flex-wrap items-end gap-3">
            <div className="w-56">
              <TextDecimalInput id="policy-percentage-helper" label="Artış oranı (%)" value={percentageRaw} disabled={disabled} help="0–100 aralığında." onChange={(value) => { setPercentageRaw(value); setPercentageFeedback(null); }} />
            </div>
            <button type="button" className="min-h-10 rounded-md bg-indigo-700 px-3 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600 disabled:cursor-not-allowed disabled:bg-slate-300" disabled={disabled || !canApplyPercentageIncrease || !percentageRaw.trim()} onClick={applyIncrease}>Artışı uygula</button>
            <p className="max-w-md text-xs text-slate-600">Yalnızca günlük taban ücret ve yemek güncellenir.</p>
          </div>
          {percentageFeedback && (
            <p role={percentageFeedback.kind === 'error' ? 'alert' : 'status'} className={`mt-2 text-sm ${percentageFeedback.kind === 'error' ? 'text-rose-700' : 'text-emerald-800'}`}>
              {percentageFeedback.message}
            </p>
          )}
          {!canApplyPercentageIncrease && <p className="mt-2 text-xs text-slate-500">Tutarlar geçerli olduğunda kullanılabilir.</p>}
        </details>
        </>}

        <details id="policy-calculation-details" className="border-t border-slate-200 pt-3">
          <summary id="policy-calculation-details-summary" className="flex cursor-pointer flex-wrap items-center justify-between gap-2 rounded-md py-2 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">
            <span>2. Hesaplama ayrıntıları</span>
            <span aria-live="polite" className={`rounded-full px-2.5 py-1 text-xs font-semibold ${scopeSelection === null || roundingSelection === null ? 'bg-amber-100 text-amber-950' : 'bg-slate-100 text-slate-700'}`}>
              {calculationSummary}
            </span>
          </summary>
          <div className="grid gap-x-6 gap-y-4 pb-2 pt-2 md:grid-cols-2">
            <fieldset aria-describedby={showValidation && scopeSelection === null ? 'policy-boundary-scope-error' : undefined} className="min-w-0">
              <legend className="mb-1 text-xs font-semibold text-slate-700">Hangi ücretler değişiyor?</legend>
              <label htmlFor="policy-boundary-scope-full" className="flex cursor-pointer items-start gap-2 border-b border-slate-100 py-2">
                <input id="policy-boundary-scope-full" type="radio" name="policy-boundary-scope" value="FULL_EARNING" aria-label="Tüm ücret kalemleri" checked={scopeSelection === 'FULL_EARNING'} onChange={() => onSelectBoundaryScope('FULL_EARNING')} className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600" />
                <span>
                  <span className="block text-sm font-semibold text-slate-900">Tüm ücret kalemleri</span>
                  <span className="block text-xs text-slate-600">Bu tarihten sonra tüm ücret ve yan haklar burada görünen tutarlarla uygulanır.</span>
                </span>
              </label>
              <label htmlFor="policy-boundary-scope-road" className="flex cursor-pointer items-start gap-2 py-2">
                <input id="policy-boundary-scope-road" type="radio" name="policy-boundary-scope" value="ROAD_ONLY" aria-label="Yalnız yol tutarı" checked={scopeSelection === 'ROAD_ONLY'} onChange={() => onSelectBoundaryScope('ROAD_ONLY')} className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600" />
                <span>
                  <span className="block text-sm font-semibold text-slate-900">Yalnız yol tutarı</span>
                  <span className="block text-xs text-slate-600">Diğer ücret ve yan haklar önceki kayıtlı tutarlarda kalır.</span>
                </span>
              </label>
              {showValidation && scopeSelection === null && <p id="policy-boundary-scope-error" role="alert" className={errorClassName}>Değişecek ücret kapsamını seçin.</p>}
            </fieldset>

            <fieldset aria-describedby={showValidation && roundingSelection === null ? 'policy-road-rounding-error' : undefined} className="min-w-0">
              <legend className="mb-1 text-xs font-semibold text-slate-700">Yol tutarı nasıl yuvarlansın?</legend>
              <label htmlFor="policy-road-rounding-slice" className="flex cursor-pointer items-start gap-2 border-b border-slate-100 py-2">
                <input id="policy-road-rounding-slice" type="radio" name="policy-road-rounding" value="PER_COMPENSATION_SLICE" aria-label="Her kazanç diliminde ayrı" checked={roundingSelection === 'PER_COMPENSATION_SLICE'} onChange={() => onSelectRoadRoundingMode('PER_COMPENSATION_SLICE')} className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600" />
                <span>
                  <span className="block text-sm font-semibold text-slate-900">Her kazanç diliminde ayrı</span>
                  <span className="block text-xs text-slate-600">Yol tutarı her tam kazanç dilimi için ayrı yuvarlanır.</span>
                </span>
              </label>
              <label htmlFor="policy-road-rounding-period" className="flex cursor-pointer items-start gap-2 py-2">
                <input id="policy-road-rounding-period" type="radio" name="policy-road-rounding" value="PERIOD_TOTAL" aria-label="Dönem toplamı bir kez" checked={roundingSelection === 'PERIOD_TOTAL'} onChange={() => onSelectRoadRoundingMode('PERIOD_TOTAL')} className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600" />
                <span>
                  <span className="block text-sm font-semibold text-slate-900">Dönem toplamı bir kez</span>
                  <span className="block text-xs text-slate-600">Dönemin yol tutarları birleştirilir ve bir kez yuvarlanır.</span>
                </span>
              </label>
              {showValidation && roundingSelection === null && <p id="policy-road-rounding-error" role="alert" className={errorClassName}>Yol yuvarlama biçimini seçin.</p>}
            </fieldset>
          </div>
        </details>
      </fieldset>
    </form>
  );
};
