import React, { useEffect, useState } from 'react';
import type {
  CompensationBoundaryScope,
  CompensationPolicyDraftValidation,
  CompensationPolicyLoadData,
  CompensationPolicyPageProps,
  CompensationPolicyStateDraft,
  RoadRoundingMode,
  BenefitDayType,
  BenefitPaymentMethod,
} from '../../services/compensationPolicy/contract';
import {
  applyCompensationPolicyPercentageIncrease,
  formatCompensationPolicyDecimalForUser,
  findScopedSameDateActiveLeaf,
  getCompensationPolicyBenefitRuleErrors,
  getCompensationPolicyDraftFieldErrors,
  parseCompensationPolicyDecimal,
} from './compensationPolicyDecimal';
import { getWasmRuntime } from '../../services/payrollEngine/wasmRuntime';

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
}

const inputClassName = 'min-h-10 w-full rounded-md border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 font-mono focus:border-indigo-600 focus:outline-none focus:ring-2 focus:ring-indigo-200 disabled:bg-slate-100';
const errorClassName = 'mt-1 text-xs font-medium text-rose-700';

const BENEFIT_ITEMS = [
  { key: 'birlestirilmisSosyalYardim', label: 'Birleştirilmiş sosyal yardım' },
  { key: 'giyimYardimi', label: 'Giyim yardımı' },
  { key: 'hizmetZammi', label: 'Hizmet zammı' },
  { key: 'ekOdeme', label: 'Ek ödeme' },
  { key: 'digerGelir', label: 'Diğer gelir' },
] as const satisfies ReadonlyArray<{ key: keyof Draft['socialBenefitAccrualPolicies']; label: string }>;

const BENEFIT_DAY_TYPES: ReadonlyArray<{ value: BenefitDayType; label: string }> = [
  { value: 'WORKED', label: 'Ç / GÇ · çalışılan gün' },
  { value: 'PAID_REST_HOLIDAY_OR_LEAVE', label: 'T / G / İ / GÇT · ücretli tatil ve izin' },
  { value: 'PAID_SICK_LEAVE', label: 'R · ücretli rapor günü' },
  { value: 'UNPAID_SICK_LEAVE', label: 'R · ücretsiz rapor günü' },
  { value: 'UNPAID_LEAVE', label: 'Üİ · ücretsiz izin' },
  { value: 'OUTSIDE_EMPLOYMENT', label: 'D · işe giriş/çıkış nedeniyle hizmet dışı gün' },
];

const PAYMENT_METHODS: ReadonlyArray<{ value: BenefitPaymentMethod; label: string; explanation: string }> = [
  { value: 'FULL', label: 'Tam öde', explanation: 'Bu geçerlilik diliminin tutarı puantaj günlerinden bağımsız tam uygulanır.' },
  { value: 'FULL_IF_ELIGIBLE_DAY', label: 'Hak ediş varsa tam öde', explanation: 'Seçilen gün türlerinden en az biri bu politika diliminde varsa dilimin aylık payı ödenir. Dönemin tamamı için otomatik tam tutar anlamına gelmez.' },
  { value: 'PROPORTIONAL', label: 'Orantılı öde', explanation: 'Yalnız seçili hak günleri için, belirlenen paydaya göre kıst hesaplanır.' },
  { value: 'NO_PAYMENT', label: 'Tahakkuk ettirme', explanation: 'Bu geçerlilik diliminde bu gelir kalemi tahakkuk etmez.' },
];

const defaultBenefitRule = (method: BenefitPaymentMethod) => ({
  method,
  // Do not silently assume a collective agreement's eligibility list. Users
  // explicitly choose day classes for conditional and proportional rules.
  eligibleDayTypes: [] as BenefitDayType[],
  denominator: 'CALENDAR_DAYS' as const,
  allowMonthlyAmountExcess: false,
});

type BenefitPreview =
  | { kind: 'ready'; amount: string }
  | { kind: 'needs-day-types' }
  | { kind: 'invalid-amount' }
  | { kind: 'failed' };

function benefitAmountDraft(draft: Draft, key: keyof Draft['socialBenefitAccrualPolicies']): string {
  switch (key) {
    case 'birlestirilmisSosyalYardim': return draft.birlestirilmisSosyalYardim;
    case 'giyimYardimi': return draft.giyimYardimi;
    // The sample treats one service-year unit as its illustrative monthly amount.
    case 'hizmetZammi': return draft.hizmetZammiBirimi;
    case 'ekOdeme': return draft.ekOdeme.kind === 'VALUE' ? draft.ekOdeme.raw : '0';
    case 'digerGelir': return draft.digerGelirVarsayilan.kind === 'VALUE' ? draft.digerGelirVarsayilan.raw : '0';
  }
}

function inclusivePeriodDays(start: string, end: string): number | null {
  const datePattern = /^(\d{4})-(\d{2})-(\d{2})$/u;
  const startParts = datePattern.exec(start);
  const endParts = datePattern.exec(end);
  if (!startParts || !endParts) return null;
  const toUtc = (parts: RegExpExecArray) => Date.UTC(Number(parts[1]), Number(parts[2]) - 1, Number(parts[3]));
  const days = (toUtc(endParts) - toUtc(startParts)) / 86_400_000 + 1;
  return Number.isInteger(days) && days > 0 ? days : null;
}

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
}) => {
  const [percentageRaw, setPercentageRaw] = useState('');
  const [percentageFeedback, setPercentageFeedback] = useState<{ kind: 'success' | 'error'; message: string } | null>(null);
  const [benefitPreviews, setBenefitPreviews] = useState<Partial<Record<keyof Draft['socialBenefitAccrualPolicies'], BenefitPreview>>>({});
  const fieldErrors = getCompensationPolicyDraftFieldErrors(draft);
  const benefitRuleErrors = getCompensationPolicyBenefitRuleErrors(draft.socialBenefitAccrualPolicies);
  const required = (field: keyof CompensationPolicyStateDraft): string | undefined => showValidation ? fieldErrors[field] : undefined;
  const periodDays = inclusivePeriodDays(loadData.scope.periodStartInclusive, loadData.scope.periodEndInclusive);

  useEffect(() => {
    let current = true;
    setBenefitPreviews({});
    void (async () => {
      try {
        const runtime = await getWasmRuntime();
        const nextEntries = await Promise.all(BENEFIT_ITEMS.map(async ({ key }) => {
          const parsedAmount = parseCompensationPolicyDecimal(benefitAmountDraft(draft, key));
          if (parsedAmount.ok === false) {
            return [key, { kind: 'invalid-amount' } as BenefitPreview] as const;
          }
          if (periodDays === null) return [key, { kind: 'failed' } as BenefitPreview] as const;
          const policy = draft.socialBenefitAccrualPolicies[key];
          const requiresDayTypes = policy?.method === 'FULL_IF_ELIGIBLE_DAY' || policy?.method === 'PROPORTIONAL';
          if (requiresDayTypes && !policy?.eligibleDayTypes.length) {
            return [key, { kind: 'needs-day-types' } as BenefitPreview] as const;
          }
          try {
            const amount = runtime.calculate_social_benefit_preview_json(JSON.stringify({
              monthlyAmount: parsedAmount.value,
              periodDays,
              eligibleDays: requiresDayTypes ? Math.min(20, periodDays) : 0,
              policy: policy ?? null,
            }));
            return [key, { kind: 'ready', amount } as BenefitPreview] as const;
          } catch {
            return [key, { kind: 'failed' } as BenefitPreview] as const;
          }
        }));
        if (current) setBenefitPreviews(Object.fromEntries(nextEntries));
      } catch {
        if (current) setBenefitPreviews(Object.fromEntries(BENEFIT_ITEMS.map(({ key }) => [key, { kind: 'failed' }])));
      }
    })();
    return () => { current = false; };
  }, [draft, periodDays]);
  const effectiveDateError = showValidation ? validation.operationErrors.find((error) => error.includes('tarihi')) : undefined;
  const reasonError = showValidation ? validation.operationErrors.find((error) => error.includes('gerekçesi')) : undefined;
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
  const setBenefitPolicy = (
    key: keyof Draft['socialBenefitAccrualPolicies'],
    update: (current: Draft['socialBenefitAccrualPolicies'][typeof key]) => Draft['socialBenefitAccrualPolicies'][typeof key]
  ) => {
    const current = draft.socialBenefitAccrualPolicies[key];
    change({
      socialBenefitAccrualPolicies: {
        ...draft.socialBenefitAccrualPolicies,
        [key]: update(current),
      },
    });
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

        <section aria-labelledby="benefit-accrual-heading" className="space-y-3 border-t border-slate-200 pt-4">
          <div>
            <h3 id="benefit-accrual-heading" className="text-sm font-bold text-slate-900">Sosyal Yardım Hak Edişleri</h3>
            <p className="mt-1 text-xs text-slate-600">
              Her kalemin ödeme yöntemini ayrı seçin. Eski sürümlerde seçim yapılmamışsa mevcut hesaplama davranışı korunur.
            </p>
            <p className="mt-2 rounded-md border border-amber-300 bg-amber-50 p-2 text-xs text-amber-950">
              Hak ediş kurallarını kurumunuz için geçerli mevzuat ve toplu iş sözleşmesine göre belirleyin. Değişikliğin geçerlilik tarihi ve dayanağı bu ücret politikası sürümünde saklanır.
            </p>
          </div>
          <div className="space-y-3">
            {BENEFIT_ITEMS.map(({ key, label }) => {
              const policy = draft.socialBenefitAccrualPolicies[key];
              const method = policy?.method ?? null;
              const methodInfo = PAYMENT_METHODS.find((item) => item.value === method);
              return (
                <article key={key} className="rounded-md border border-slate-200 p-3">
                  <div className="grid gap-2 sm:grid-cols-[minmax(12rem,1fr)_minmax(16rem,1.2fr)] sm:items-end">
                    <div>
                      <label htmlFor={`benefit-policy-${key}`} className="mb-1 block text-sm font-semibold text-slate-800">{label}</label>
                      <select
                        id={`benefit-policy-${key}`}
                        className={inputClassName.replace('font-mono', '')}
                        value={method ?? 'LEGACY'}
                        onChange={(event) => {
                          const selected = event.target.value;
                          if (selected === 'LEGACY') {
                            setBenefitPolicy(key, () => null);
                          } else {
                            setBenefitPolicy(key, (current) => {
                              const selectedMethod = selected as BenefitPaymentMethod;
                              const next = {
                                ...(current ?? defaultBenefitRule(selectedMethod)),
                                method: selectedMethod,
                              };
                              if (selectedMethod === 'PROPORTIONAL' && current?.method !== 'PROPORTIONAL') {
                                next.allowMonthlyAmountExcess = current?.allowMonthlyAmountExcess ?? false;
                              }
                              return next;
                            });
                          }
                        }}
                      >
                        <option value="LEGACY">Eski hesaplama davranışını koru</option>
                        {PAYMENT_METHODS.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
                      </select>
                    </div>
                    <p className="text-xs leading-5 text-slate-600">
                      {methodInfo?.explanation ?? 'Bu kalem için henüz yeni bir hak ediş kuralı tanımlanmadı; önceki sürümdeki davranış kullanılır.'}
                    </p>
                  </div>
                  {(() => {
                    const preview = benefitPreviews[key];
                    const previewUsesEligibility = method === 'FULL_IF_ELIGIBLE_DAY' || method === 'PROPORTIONAL';
                    const denominatorLabel = policy?.denominator === 'THIRTY_DAY_MONTH'
                      ? 'sabit 30 gün'
                      : `dönemin ${periodDays ?? '—'} takvim günü`;
                    const selectedDayLabels = BENEFIT_DAY_TYPES
                      .filter(({ value }) => policy?.eligibleDayTypes.includes(value))
                      .map(({ label: dayLabel }) => dayLabel);
                    const monthlyCapLabel = method !== 'PROPORTIONAL'
                      ? null
                      : policy?.denominator !== 'THIRTY_DAY_MONTH'
                        ? 'Takvim günü paydası, hak günleri dönem günlerinden fazla olamayacağı için aylık tutarı aşmaz.'
                        : policy.allowMonthlyAmountExcess === undefined
                          ? 'Eski politika sürümü: tarihsel 31/30 aşımı önceki davranışla korunur.'
                          : policy.allowMonthlyAmountExcess
                            ? 'Aylık tutar aşılabilir; bu tercih yalnız sabit 30 gün paydasında etkilidir.'
                            : 'Aylık tutar, dönem içindeki politika dilimlerinin aylık payları toplamıyla sınırlandırılır.';
                    return (
                      <div className="mt-2 rounded bg-slate-50 px-2 py-2 text-xs text-slate-700" aria-live="polite">
                        <p><span className="font-semibold">Örnek brüt tahakkuk (vergi ve kesintiler hariç):</span>{' '}
                          {preview?.kind === 'ready' ? `${formatCompensationPolicyDecimalForUser(preview.amount)} TL` :
                            preview?.kind === 'needs-day-types' ? 'Gün türü seçilince hesaplanır.' :
                              preview?.kind === 'invalid-amount' ? 'Tutarı geçerli biçimde girin.' :
                                preview?.kind === 'failed' ? 'Önizleme hesaplanamadı.' : 'Hesaplanıyor…'}
                        </p>
                        <p className="mt-1">{loadData.scope.periodId} dönemi · {periodDays ?? '—'} takvim günü. Bu bir örnek senaryodur; mevcut personel puantajı kullanılmıyor.
                          {previewUsesEligibility ? ` Seçilen gün türlerinde ${Math.min(20, periodDays ?? 20)} örnek hak günü varsayılmıştır.` : ''}
                          {key === 'hizmetZammi' ? ' Hizmet zammı örneği bir hizmet yılı varsayar.' : ''}
                        </p>
                        {previewUsesEligibility && <p>Hak günü: {Math.min(20, periodDays ?? 20)} · Seçilen türler: {selectedDayLabels.join(', ')} · Payda: {denominatorLabel}.</p>}
                        {monthlyCapLabel && <p>{monthlyCapLabel}</p>}
                      </div>
                    );
                  })()}
                  {(method === 'FULL_IF_ELIGIBLE_DAY' || method === 'PROPORTIONAL') && (
                    <fieldset
                      id={`benefit-day-types-${key}`}
                      tabIndex={-1}
                      aria-invalid={showValidation && Boolean(benefitRuleErrors[key])}
                      aria-describedby={showValidation && benefitRuleErrors[key] ? `benefit-day-types-${key}-error` : undefined}
                      className="mt-3 space-y-2 border-t border-slate-100 pt-3 focus:outline-none"
                    >
                      <legend className="text-xs font-semibold text-slate-700">Hak kazandıran gün türleri</legend>
                      {policy?.eligibleDayTypes.length === 0 && (
                        <p className="rounded bg-amber-50 px-2 py-1.5 text-xs text-amber-900">
                          Bir gün türü otomatik seçilmez. Bu ödeme yöntemi için en az bir türü burada seçin.
                        </p>
                      )}
                      <div className="grid gap-1.5 sm:grid-cols-2">
                        {BENEFIT_DAY_TYPES.map(({ value, label: dayLabel }) => (
                          <label key={value} className="flex min-h-8 items-center gap-2 text-xs text-slate-700">
                            <input
                              type="checkbox"
                              className="h-4 w-4 accent-indigo-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
                              checked={policy?.eligibleDayTypes.includes(value) ?? false}
                              onChange={(event) => setBenefitPolicy(key, (current) => {
                                const rule = current ?? defaultBenefitRule(method);
                                return {
                                  ...rule,
                                  eligibleDayTypes: event.target.checked
                                    ? [...rule.eligibleDayTypes, value]
                                    : rule.eligibleDayTypes.filter((item) => item !== value),
                                };
                              })}
                            />
                            {dayLabel}
                          </label>
                        ))}
                      </div>
                      {showValidation && benefitRuleErrors[key] && (
                        <p id={`benefit-day-types-${key}-error`} role="alert" className={errorClassName}>{benefitRuleErrors[key]}</p>
                      )}
                      {method === 'PROPORTIONAL' && (
                        <div className="max-w-sm">
                          <label htmlFor={`benefit-denominator-${key}`} className="mb-1 block text-xs font-semibold text-slate-700">Oranlama paydası</label>
                          <select
                            id={`benefit-denominator-${key}`}
                            className={inputClassName.replace('font-mono', '')}
                            value={policy?.denominator ?? 'CALENDAR_DAYS'}
                            onChange={(event) => setBenefitPolicy(key, (current) => ({
                              ...(current ?? defaultBenefitRule(method)),
                              denominator: event.target.value as 'CALENDAR_DAYS' | 'THIRTY_DAY_MONTH',
                            }))}
                          >
                            <option value="CALENDAR_DAYS">Dönemin takvim günleri</option>
                            <option value="THIRTY_DAY_MONTH">Sabit 30 gün</option>
                          </select>
                        </div>
                      )}
                      {method === 'PROPORTIONAL' && policy?.denominator === 'THIRTY_DAY_MONTH' && (
                        <fieldset className="space-y-1 rounded border border-slate-200 p-2">
                          <legend className="px-1 text-xs font-semibold text-slate-700">Aylık tutar sınırı</legend>
                          <label className="flex items-start gap-2 text-xs text-slate-700">
                            <input
                              type="radio"
                              name={`benefit-monthly-cap-${key}`}
                              checked={policy.allowMonthlyAmountExcess === false}
                              onChange={() => setBenefitPolicy(key, (current) => ({
                                ...(current ?? defaultBenefitRule(method)), allowMonthlyAmountExcess: false,
                              }))}
                              className="mt-0.5 accent-indigo-600"
                            />
                            Aylık tanımlı tutarı aşma (önerilen)
                          </label>
                          <label className="flex items-start gap-2 text-xs text-slate-700">
                            <input
                              type="radio"
                              name={`benefit-monthly-cap-${key}`}
                              checked={policy.allowMonthlyAmountExcess !== false}
                              onChange={() => setBenefitPolicy(key, (current) => ({
                                ...(current ?? defaultBenefitRule(method)), allowMonthlyAmountExcess: true,
                              }))}
                              className="mt-0.5 accent-indigo-600"
                            />
                            Sözleşme gerektiriyorsa aylık tutarın aşılmasına izin ver
                          </label>
                          {policy.allowMonthlyAmountExcess === undefined && (
                            <p className="text-xs leading-5 text-amber-900">Bu eski politika sürümünde ayar bulunmuyor. Tarihsel 31/30 hesabını korumak için aşım davranışı açık gösteriliyor; yeni tercihi seçerek değiştirebilirsiniz.</p>
                          )}
                          <p className="text-xs leading-5 text-slate-500">31 günlük dönemde 31 hak günü, aşım açıkken aylık tutarın 31/30’u kadar tahakkuk oluşturabilir. Aşım kapalıysa politika dilimlerinin aylık payları toplamı üst sınır olur.</p>
                        </fieldset>
                      )}
                      <p className="text-xs leading-5 text-slate-500">
                        Tam ödeme, yalnız bu politika diliminin dönem içindeki payıdır; dilimde en az bir seçili gün varsa bu pay ödenir. Her takvim günü tek puantaj koduyla sınıflanır. Rapor günleri ücretli/ücretsiz rapor kayıtlarına göre ayrılır.
                      </p>
                    </fieldset>
                  )}
                </article>
              );
            })}
          </div>
        </section>

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
