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
  findScopedSameDateActiveLeaf,
  getCompensationPolicyDraftFieldErrors,
} from './compensationPolicyDecimal';

type Draft = CompensationPolicyPageProps['draft'];

export interface CompensationPolicyFormProps {
  draft: Draft;
  loadData: CompensationPolicyLoadData;
  validation: CompensationPolicyDraftValidation;
  disabled: boolean;
  scopeSelection: CompensationBoundaryScope | null;
  roundingSelection: RoadRoundingMode | null;
  onDraftChange(draft: Draft): void;
  onSelectBoundaryScope(scope: CompensationBoundaryScope): void;
  onSelectRoadRoundingMode(mode: RoadRoundingMode): void;
  onCopyActiveState(): void;
  canApplyPercentageIncrease: boolean;
}

const inputClassName = 'min-h-10 w-full rounded-md border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 font-mono focus:border-indigo-600 focus:outline-none focus:ring-2 focus:ring-indigo-200 disabled:bg-slate-100';
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
  value: CompensationPolicyStateDraft['isPrimiYuzde'];
  error?: string;
  help?: string;
  disabled: boolean;
  onChange(value: CompensationPolicyStateDraft['isPrimiYuzde']): void;
}

const OptionalDecimalInput: React.FC<OptionalDecimalInputProps> = ({ id, label, value, error, help, disabled, onChange }) => {
  const [lastRaw, setLastRaw] = useState(value.kind === 'VALUE' ? value.raw : '');
  useEffect(() => {
    if (value.kind === 'VALUE') setLastRaw(value.raw);
  }, [value]);
  return (
    <div className="min-w-0">
    <label htmlFor={`${id}-presence`} className="mb-1 block text-sm font-semibold text-slate-800">{label}</label>
    <select
      id={`${id}-presence`}
      aria-label={`${label} alan durumu`}
      className={inputClassName}
      value={value.kind}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value === 'ABSENT' ? { kind: 'ABSENT' } : { kind: 'VALUE', raw: lastRaw })}
    >
      <option value="ABSENT">Yok — null</option>
      <option value="VALUE">Tutar/oran var</option>
    </select>
    {value.kind === 'VALUE' && (
      <input
        id={id}
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        aria-label={`${label} değeri`}
        aria-invalid={Boolean(error)}
        aria-describedby={error ? `${id}-error` : undefined}
        className={`${inputClassName} mt-2 ${error ? 'border-rose-500 focus:border-rose-600 focus:ring-rose-100' : ''}`}
        value={value.raw}
        disabled={disabled}
        onChange={(event) => {
          setLastRaw(event.target.value);
          onChange({ kind: 'VALUE', raw: event.target.value });
        }}
      />
    )}
    {error && <p id={`${id}-error`} className={errorClassName}>{error}</p>}
    <p className="mt-1 text-xs text-slate-500">Açık sıfır `0` olarak ayrı saklanır.</p>
    {help && <p className="mt-1 text-xs text-slate-500">{help}</p>}
    </div>
  );
};

export const CompensationPolicyForm: React.FC<CompensationPolicyFormProps> = ({
  draft,
  loadData,
  validation,
  disabled,
  scopeSelection,
  roundingSelection,
  onDraftChange,
  onSelectBoundaryScope,
  onSelectRoadRoundingMode,
  onCopyActiveState,
  canApplyPercentageIncrease,
}) => {
  const [percentageRaw, setPercentageRaw] = useState('');
  const [percentageFeedback, setPercentageFeedback] = useState<{ kind: 'success' | 'error'; message: string } | null>(null);
  const fieldErrors = getCompensationPolicyDraftFieldErrors(draft);
  const required = (field: keyof CompensationPolicyStateDraft): string | undefined => fieldErrors[field];
  const effectiveDateError = validation.operationErrors.find((error) => error.includes('tarihi'));
  const reasonError = validation.operationErrors.find((error) => error.includes('gerekçesi'));
  const groupListError = required('isPrimiGruplari');
  const selectedVersionId = draft.operationKind === 'SAME_DATE_CORRECTION'
    ? findScopedSameDateActiveLeaf(loadData.timeline.versions, draft.effectiveFrom)
    : null;
  const correctionDates = Array.from(new Set<string>(loadData.timeline.versions.map((version) => version.effectiveFrom)))
    .filter((date) => findScopedSameDateActiveLeaf(loadData.timeline.versions, date) !== null)
    .sort();

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
    <form className="space-y-5" onSubmit={(event) => event.preventDefault()} aria-label="Ücret politikası formu">
      <fieldset disabled={disabled} className="space-y-5">
        <legend className="sr-only">Ücret politikası ayrıntıları</legend>
        <section aria-labelledby="policy-operation-heading" className="space-y-3">
          <h3 id="policy-operation-heading" className="text-base font-bold text-slate-900">Değişiklik bilgisi</h3>
          <div className="grid gap-3 md:grid-cols-3">
            <div>
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
            </div>
            <div>
              {draft.operationKind === 'SAME_DATE_CORRECTION' ? (
                <>
                  <label htmlFor="policy-effective-date" className="mb-1 block text-sm font-semibold text-slate-800">Düzeltilecek geçerlilik tarihi</label>
                  <select
                    id="policy-effective-date"
                    className={inputClassName.replace('font-mono', '')}
                    value={draft.effectiveFrom}
                    aria-invalid={selectedVersionId === null}
                    aria-describedby={selectedVersionId === null ? 'policy-correction-target-error' : 'policy-correction-target-help'}
                    onChange={(event) => {
                      const effectiveFrom = event.target.value;
                      const leafId = findScopedSameDateActiveLeaf(loadData.timeline.versions, effectiveFrom);
                      change({ effectiveFrom, supersedesVersionId: leafId });
                    }}
                  >
                    <option value="">Scoped sürüm tarihi seçin</option>
                    {correctionDates.map((date) => <option key={date} value={date}>{date}</option>)}
                    {draft.effectiveFrom && !correctionDates.includes(draft.effectiveFrom) && (
                      <option value={draft.effectiveFrom}>{draft.effectiveFrom} — etkin sürüm bulunamadı</option>
                    )}
                  </select>
                  {selectedVersionId ? (
                    <p id="policy-correction-target-help" className="mt-1 text-xs text-slate-600">
                      Bu tarihteki etkin sürüm düzeltilecek: {loadData.timeline.versions.find((version) => version.id === selectedVersionId)?.reason || 'Kayıtlı ücret politikası'}.
                    </p>
                  ) : (
                    <p id="policy-correction-target-error" role="alert" className={errorClassName}>Bu tarihte tek bir etkin sürüm bulunamadı. Düzeltme hedefi authoritative yüklemede doğrulanmalıdır.</p>
                  )}
                </>
              ) : (
                <div className="min-w-0">
                  <label htmlFor="policy-effective-date" className="mb-1 block text-sm font-semibold text-slate-800">
                    {draft.operationKind === 'INITIAL_BASELINE' ? 'Başlangıç politikasının geçerlilik tarihi' : 'Değişikliğin geçerlilik tarihi'}
                  </label>
                  <input
                    id="policy-effective-date"
                    type="date"
                    value={draft.effectiveFrom}
                    disabled={disabled}
                    aria-invalid={Boolean(validation.operationErrors.find((error) => error.includes('tarihi')))}
                    aria-describedby={`policy-effective-date-help${effectiveDateError ? ' policy-effective-date-error' : ''}`}
                    className={`${inputClassName.replace('font-mono', '')} ${validation.operationErrors.find((error) => error.includes('tarihi')) ? 'border-rose-500 focus:border-rose-600 focus:ring-rose-100' : ''}`}
                    onChange={(event) => change({ effectiveFrom: event.target.value })}
                  />
                  <p id="policy-effective-date-help" className="mt-1 text-xs text-slate-500">Başlangıç tarihi politika günüdür; bordro dönemi ve as-of tarihi ayrı bağlamlardır.</p>
                  {effectiveDateError && <p id="policy-effective-date-error" className={errorClassName}>{effectiveDateError}</p>}
                </div>
              )}
            </div>
            <div>
              <label htmlFor="policy-reason" className="mb-1 block text-sm font-semibold text-slate-800">Değişiklik gerekçesi</label>
              <textarea
                id="policy-reason"
                rows={2}
                value={draft.reason}
                aria-invalid={Boolean(reasonError)}
                aria-describedby={`policy-reason-help${reasonError ? ' policy-reason-error' : ''}`}
                className={`${inputClassName.replace('font-mono', '')} resize-y`}
                onChange={(event) => change({ reason: event.target.value })}
              />
              <p id="policy-reason-help" className="mt-1 text-xs text-slate-500">Ücret geçmişinde bu değişikliğin neden yapıldığını gösterecek kısa bir açıklama yazın.</p>
              {reasonError && <p id="policy-reason-error" className={errorClassName}>{reasonError}</p>}
            </div>
          </div>
          {draft.operationKind === 'SAME_DATE_CORRECTION' && draft.supersedesVersionId !== selectedVersionId && (
            <p role="alert" className="rounded-md border border-rose-300 bg-rose-50 p-2 text-sm text-rose-800">
              Düzeltme hedefi yenilenen scoped timeline ile uyuşmuyor. Önizleme öncesinde authoritative yüklemeyi yenileyin.
            </p>
          )}
        </section>

        <section aria-labelledby="policy-flags-heading" className="space-y-3 border-t border-slate-200 pt-4">
          <h3 id="policy-flags-heading" className="text-base font-bold text-slate-900">Ücret ve yuvarlama kapsamı</h3>
          <fieldset className="space-y-2">
            <legend className="text-sm font-semibold text-slate-800">Bu sürüm hangi değişikliği yapıyor?</legend>
            <label className="flex cursor-pointer items-start gap-2 rounded-md border border-slate-200 p-3 hover:bg-slate-50">
              <input
                type="radio"
                name="policy-boundary-scope"
                value="FULL_EARNING"
                checked={scopeSelection === 'FULL_EARNING'}
                onChange={() => onSelectBoundaryScope('FULL_EARNING')}
                className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
              />
              <span>
                <span className="block text-sm font-semibold text-slate-900">Tam kazanç değişikliği (FULL_EARNING)</span>
                <span className="block text-xs text-slate-600">Kazanç hesabında yeni yuvarlama dilimi açar. Tutarlar değişmese bile bu tarih yeni dilimin başlangıcı olur.</span>
              </span>
            </label>
            <label className="flex cursor-pointer items-start gap-2 rounded-md border border-slate-200 p-3 hover:bg-slate-50">
              <input
                type="radio"
                name="policy-boundary-scope"
                value="ROAD_ONLY"
                checked={scopeSelection === 'ROAD_ONLY'}
                onChange={() => onSelectBoundaryScope('ROAD_ONLY')}
                className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
              />
              <span>
                <span className="block text-sm font-semibold text-slate-900">Yalnız yol tutarı (ROAD_ONLY)</span>
                <span className="block text-xs text-slate-600">Mevcut kazanç yuvarlama dilimini bölmez. Yol dışındaki tüm ücret alanları önceki tam durumla aynı kalmalıdır. İlk başlangıç sürümü de bu kapsamı seçebilir.</span>
              </span>
            </label>
            {scopeSelection === null && <p className="text-xs font-medium text-amber-800">Önizleme için kapsam seçeneklerinden birini açıkça seçin.</p>}
          </fieldset>

          <fieldset className="space-y-2">
            <legend className="text-sm font-semibold text-slate-800">Yol tutarı nasıl yuvarlansın?</legend>
            <label className="flex cursor-pointer items-start gap-2 rounded-md border border-slate-200 p-3 hover:bg-slate-50">
              <input
                type="radio"
                name="policy-road-rounding"
                value="PER_COMPENSATION_SLICE"
                checked={roundingSelection === 'PER_COMPENSATION_SLICE'}
                onChange={() => onSelectRoadRoundingMode('PER_COMPENSATION_SLICE')}
                className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
              />
              <span>
                <span className="block text-sm font-semibold text-slate-900">Her tam kazanç diliminde ayrı yuvarla</span>
                <span className="block text-xs text-slate-600">C+GC günlerinin yol tutarı her kazanç dilimi için toplanır ve ayrı yuvarlanır.</span>
              </span>
            </label>
            <label className="flex cursor-pointer items-start gap-2 rounded-md border border-slate-200 p-3 hover:bg-slate-50">
              <input
                type="radio"
                name="policy-road-rounding"
                value="PERIOD_TOTAL"
                checked={roundingSelection === 'PERIOD_TOTAL'}
                onChange={() => onSelectRoadRoundingMode('PERIOD_TOTAL')}
                className="mt-0.5 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
              />
              <span>
                <span className="block text-sm font-semibold text-slate-900">Bütün dönemi tek kez yuvarla</span>
                <span className="block text-xs text-slate-600">Dönemin C+GC yol tutarları birleştirilir ve dönem sonunda bir kez yuvarlanır.</span>
              </span>
            </label>
            {roundingSelection === null && <p className="text-xs font-medium text-amber-800">Yol yuvarlama biçimini açıkça seçin.</p>}
          </fieldset>
        </section>

        {loadData.activePolicyProjection.kind === 'ACTIVE' && draft.operationKind !== 'INITIAL_BASELINE' && (
          <section className="flex flex-wrap items-center justify-between gap-3 border-t border-slate-200 pt-4">
            <p className="max-w-2xl text-xs text-slate-600">
              As-of tarihinde etkin ücret politikasının tam durumu forma kopyalanabilir. Tarih hedefindeki kaynak sürüm, önizlemede yetkili timeline üzerinden doğrulanır.
            </p>
            <button type="button" onClick={onCopyActiveState} className="min-h-10 rounded-md border border-indigo-300 px-3 py-2 text-sm font-semibold text-indigo-800 hover:bg-indigo-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">
              Etkin tam durumu forma kopyala
            </button>
          </section>
        )}

        <section aria-labelledby="policy-money-heading" className="space-y-3 border-t border-slate-200 pt-4">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div>
              <h3 id="policy-money-heading" className="text-base font-bold text-slate-900">Tam ücret durumu</h3>
              <p className="mt-1 text-xs text-slate-600">Her sürüm bütün alanları taşır. Eksik tutar otomatik olarak sıfıra çevrilmez.</p>
            </div>
            <p className="text-xs text-slate-500">Ondalık ayıracı: virgül veya nokta. Binlik ayırıcı kullanmayın.</p>
          </div>
          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
            <TextDecimalInput id="policy-base-wage" label="Günlük taban ücret (TL) · zorunlu" value={draft.gunlukTabanUcret} error={required('gunlukTabanUcret')} disabled={disabled} help="Sıfırdan büyük olmalıdır." onChange={(gunlukTabanUcret) => change({ gunlukTabanUcret })} />
            <TextDecimalInput id="policy-meal" label="Günlük yemek (TL) · zorunlu" value={draft.gunlukYemek} error={required('gunlukYemek')} disabled={disabled} onChange={(gunlukYemek) => change({ gunlukYemek })} />
            <TextDecimalInput id="policy-social" label="Birleştirilmiş sosyal yardım (TL) · zorunlu" value={draft.birlestirilmisSosyalYardim} error={required('birlestirilmisSosyalYardim')} disabled={disabled} onChange={(birlestirilmisSosyalYardim) => change({ birlestirilmisSosyalYardim })} />
            <TextDecimalInput id="policy-road" label="Günlük vasıta/yol (TL) · zorunlu" value={draft.gunlukVasitaYol} error={required('gunlukVasitaYol')} disabled={disabled} onChange={(gunlukVasitaYol) => change({ gunlukVasitaYol })} />
            <TextDecimalInput id="policy-clothing" label="Giyim yardımı (TL) · zorunlu" value={draft.giyimYardimi} error={required('giyimYardimi')} disabled={disabled} onChange={(giyimYardimi) => change({ giyimYardimi })} />
            <TextDecimalInput id="policy-service-unit" label="Hizmet zammı birimi (TL) · zorunlu" value={draft.hizmetZammiBirimi} error={required('hizmetZammiBirimi')} disabled={disabled} onChange={(hizmetZammiBirimi) => change({ hizmetZammiBirimi })} />
          </div>
        </section>

        <section aria-labelledby="policy-premium-heading" className="space-y-3 border-t border-slate-200 pt-4">
          <div>
            <h3 id="policy-premium-heading" className="text-base font-bold text-slate-900">İş primi grupları ve oranlar</h3>
            <p className="mt-1 text-xs text-slate-600">Yüzdeler 0–100 aralığındadır. Aktif gruplar tekil kod/ad ile korunur.</p>
          </div>
          <div className="grid gap-3 md:grid-cols-3">
            <OptionalDecimalInput id="policy-global-premium" label="Genel iş primi yüzdesi · isteğe bağlı alan" value={draft.isPrimiYuzde} error={required('isPrimiYuzde')} help="Yeni ücret hesabında personelin etkin grup oranı kullanılır; bu alan yeni motor için yedek oran değildir." disabled={disabled} onChange={(isPrimiYuzde) => change({ isPrimiYuzde })} />
            <TextDecimalInput id="policy-night-premium" label="Gece çalışma primi (%) · zorunlu" value={draft.geceCalismaPrimiYuzde} error={required('geceCalismaPrimiYuzde')} disabled={disabled} help="0–100 aralığında." onChange={(geceCalismaPrimiYuzde) => change({ geceCalismaPrimiYuzde })} />
            <TextDecimalInput id="policy-holiday-night-premium" label="Gece çalışma tatili primi (%) · zorunlu" value={draft.geceCalismaTatiliPrimiYuzde} error={required('geceCalismaTatiliPrimiYuzde')} disabled={disabled} help="0–100 aralığında." onChange={(geceCalismaTatiliPrimiYuzde) => change({ geceCalismaTatiliPrimiYuzde })} />
          </div>
          <div className="space-y-3">
            {draft.isPrimiGruplari.map((group, index) => (
              <fieldset key={`${group.id}:${index}`} className="grid gap-3 rounded-md border border-slate-200 p-3 sm:grid-cols-2 xl:grid-cols-[1fr_1fr_1fr_auto_auto]">
                <legend className="px-1 text-sm font-semibold text-slate-800">İş primi grubu {index + 1}</legend>
                <div>
                  <label htmlFor={`policy-group-id-${index}`} className="mb-1 block text-xs font-semibold text-slate-700">Grup kodu</label>
                  <input id={`policy-group-id-${index}`} value={group.id} aria-invalid={Boolean(!group.id.trim() || group.id.trim() !== group.id || groupListError?.includes('yinelenen'))} aria-describedby={groupListError ? `policy-group-error-${index}` : undefined} className={inputClassName.replace('font-mono', '')} disabled={disabled} onChange={(event) => change({ isPrimiGruplari: draft.isPrimiGruplari.map((item, itemIndex) => itemIndex === index ? { ...item, id: event.target.value } : item) })} />
                </div>
                <div>
                  <label htmlFor={`policy-group-name-${index}`} className="mb-1 block text-xs font-semibold text-slate-700">Grup adı</label>
                  <input id={`policy-group-name-${index}`} value={group.ad} aria-invalid={Boolean(!group.ad.trim() || group.ad.trim() !== group.ad || groupListError?.includes('yinelenen'))} aria-describedby={groupListError ? `policy-group-error-${index}` : undefined} className={inputClassName.replace('font-mono', '')} disabled={disabled} onChange={(event) => change({ isPrimiGruplari: draft.isPrimiGruplari.map((item, itemIndex) => itemIndex === index ? { ...item, ad: event.target.value } : item) })} />
                </div>
                <TextDecimalInput id={`policy-group-rate-${index}`} label="Oran (%)" value={group.oran} error={required('isPrimiGruplari')} disabled={disabled} onChange={(oran) => change({ isPrimiGruplari: draft.isPrimiGruplari.map((item, itemIndex) => itemIndex === index ? { ...item, oran } : item) })} />
                <label className="flex items-center gap-2 text-sm font-medium text-slate-800">
                  <input type="checkbox" checked={group.aktif} disabled={disabled} className="h-4 w-4 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600" onChange={(event) => change({ isPrimiGruplari: draft.isPrimiGruplari.map((item, itemIndex) => itemIndex === index ? { ...item, aktif: event.target.checked } : item) })} />
                  Etkin
                </label>
                <button type="button" className="min-h-10 rounded-md border border-rose-300 px-3 py-2 text-sm font-semibold text-rose-800 hover:bg-rose-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rose-600" aria-label={`İş primi grubu ${index + 1} satırını kaldır`} onClick={() => change({ isPrimiGruplari: draft.isPrimiGruplari.filter((_, itemIndex) => itemIndex !== index) })}>Grubu kaldır</button>
                {groupListError && <p id={`policy-group-error-${index}`} className="sm:col-span-2 xl:col-span-5 text-xs font-medium text-rose-700">{groupListError}</p>}
              </fieldset>
            ))}
            {draft.isPrimiGruplari.length === 0 && <p id="policy-groups-error" role="alert" className={errorClassName}>{groupListError ?? 'Tam ücret durumu en az bir iş primi grubu içermelidir.'}</p>}
            <button type="button" className="min-h-10 rounded-md border border-slate-300 px-3 py-2 text-sm font-semibold text-slate-800 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600" disabled={disabled} onClick={() => change({ isPrimiGruplari: [...draft.isPrimiGruplari, { id: '', ad: '', oran: '', aktif: false }] })}>İş primi grubu ekle</button>
          </div>
        </section>

        <section aria-labelledby="policy-oneoff-heading" className="space-y-3 border-t border-slate-200 pt-4">
          <div>
            <h3 id="policy-oneoff-heading" className="text-base font-bold text-slate-900">Dönemlik ek gelir varsayılanları</h3>
            <p className="mt-1 text-xs text-slate-600">`Yok` null değeridir; tutar olarak `0` seçilirse açık sıfır korunur.</p>
          </div>
          <div className="grid gap-3 md:grid-cols-2">
            <OptionalDecimalInput id="policy-extra-payment" label="Ek ödeme · isteğe bağlı alan" value={draft.ekOdeme} error={required('ekOdeme')} disabled={disabled} onChange={(ekOdeme) => change({ ekOdeme })} />
            <OptionalDecimalInput id="policy-other-income" label="Diğer gelir varsayılanı · isteğe bağlı alan" value={draft.digerGelirVarsayilan} error={required('digerGelirVarsayilan')} disabled={disabled} onChange={(digerGelirVarsayilan) => change({ digerGelirVarsayilan })} />
          </div>
        </section>

        <details className="border-t border-slate-200 pt-4">
          <summary className="cursor-pointer py-1 text-sm font-semibold text-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">Günlük taban ücret ve yemeği yüzdeyle artır</summary>
          <div className="mt-3 flex flex-wrap items-end gap-3">
            <div className="w-56">
              <TextDecimalInput id="policy-percentage-helper" label="Artış oranı (%)" value={percentageRaw} disabled={disabled} help="0–100. Hesap tam hassasiyetle yapılır; yuvarlama uygulanmaz." onChange={(value) => { setPercentageRaw(value); setPercentageFeedback(null); }} />
            </div>
            <button type="button" className="min-h-10 rounded-md bg-indigo-700 px-3 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600 disabled:cursor-not-allowed disabled:bg-slate-300" disabled={disabled || !canApplyPercentageIncrease || !percentageRaw.trim()} onClick={applyIncrease}>Artışı uygula</button>
            <p className="max-w-md text-xs text-slate-600">Yalnız taban ücret ve günlük yemek değişir. Giyim, yol, sosyal yardım, gruplar ve diğer alanlar mevcut değerlerini korur.</p>
          </div>
          {percentageFeedback && (
            <p role={percentageFeedback.kind === 'error' ? 'alert' : 'status'} className={`mt-2 text-sm ${percentageFeedback.kind === 'error' ? 'text-rose-700' : 'text-emerald-800'}`}>
              {percentageFeedback.message}
            </p>
          )}
          {!canApplyPercentageIncrease && <p className="mt-2 text-xs text-slate-500">Yüzde yardımcısı için tam ücret durumu önce geçerli olmalıdır.</p>}
        </details>
      </fieldset>
    </form>
  );
};
