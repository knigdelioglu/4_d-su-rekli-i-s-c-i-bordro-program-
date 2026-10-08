import React, { useEffect, useMemo, useRef, useState } from 'react';
import type {
  CompensationBoundaryScope,
  CompensationPolicyApiError,
  CompensationPolicyDirectEffect,
  CompensationPolicyDraftValidation,
  CompensationPolicyLoadData,
  CompensationPolicyPageProps,
  CompensationPolicyPayrollReference,
  CompensationPolicyPreviewData,
  CompensationPolicySaveRequest,
  CompensationPolicyStateWire,
  CompensationPolicyWriteOperation,
  RoadRoundingMode,
} from '../../services/compensationPolicy/contract';
import { CompensationPolicyForm } from './CompensationPolicyForm';
import {
  compensationPolicyStateToDraft,
  draftToCompensationPolicyOperation,
  formatCompensationPolicyDecimalForUser,
  findScopedSameDateActiveLeaf,
  validateCompensationPolicyDraft,
} from './compensationPolicyDecimal';

const primaryButtonClass = 'min-h-10 rounded-md bg-indigo-700 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600 disabled:cursor-not-allowed disabled:bg-slate-300';
const secondaryButtonClass = 'min-h-10 rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-semibold text-slate-800 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600 disabled:cursor-not-allowed disabled:bg-slate-100 disabled:text-slate-500';
const errorBoxClass = 'rounded-md border border-rose-300 bg-rose-50 p-3 text-sm text-rose-900';
const warningBoxClass = 'rounded-md border border-amber-300 bg-amber-50 p-3 text-sm text-amber-950';
const successBoxClass = 'rounded-md border border-emerald-300 bg-emerald-50 p-3 text-sm text-emerald-900';

type Props = CompensationPolicyPageProps;
type Draft = Props['draft'];

function stableValue(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableValue).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.entries(value as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => `${JSON.stringify(key)}:${stableValue(item)}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function sameOperation(left: CompensationPolicyWriteOperation, right: CompensationPolicyWriteOperation): boolean {
  return stableValue(left) === stableValue(right);
}

function titleCaseApiError(error: CompensationPolicyApiError): { title: string; message: string; detail?: string } {
  switch (error.code) {
    case 'QUARANTINED_POLICY':
      return {
        title: 'Ücret geçmişindeki bir kayıt doğrulanamıyor.',
        message: 'Kayıt incelenene kadar bu dönem önizlenemez veya değiştirilemez.',
        detail: `${error.policyVersion.effectiveFrom} tarihli kayıt · ${quarantineReasonLabel(error.policyVersion.reason)}`,
      };
    case 'ROAD_ONLY_STATE_CONFLICT':
      return {
        title: 'Yalnız yol tutarı değişikliği doğrulanamadı.',
        message: error.reason === 'NON_ROAD_FIELDS_DIFFER'
          ? 'Yalnız yol tutarı seçildiğinde diğer ücret kalemleri önceki kayıtla aynı kalmalıdır.'
          : 'Bu tarih, daha sonraki yol tutarı değişiklikleriyle çakışıyor. Sonraki kayıtlar değiştirilmedi.',
        detail: `Önerilen geçerlilik tarihi: ${error.proposedEffectiveFrom}`,
      };
    case 'FINALIZED_BLOCKER':
      return {
        title: 'Kesinleştirilmiş bordro bu değişikliği engelliyor.',
        message: 'Ücret politikası kaydedilmedi. Kesinleşmiş bordro burada yeniden hesaplanmaz; geçmiş düzeltmesi için geçmiş farkı işlemi gerekir.',
        detail: `${error.blockers.length} kesinleştirilmiş bordro engel oluşturuyor.`,
      };
    case 'RETRO_BLOCKER':
      return {
        title: 'Geçmiş farkı kaydı bu değişikliği engelliyor.',
        message: 'Ücret değişikliği kaydedilmedi. Engeli geçmiş farkı kayıtlarında çözümleyip yeniden önizleyin.',
        detail: error.blockers.map((blocker) => blocker.blockers.map(retroBlockerLabel).join(', ')).join(' · '),
      };
    case 'CONFLICT':
      return {
        title: 'Önizlemeden sonra ücret geçmişi değişti.',
        message: 'Bu kayıt yapılmadı. Ücret geçmişini yenileyip etkileri tekrar önizleyin.',
      };
    case 'UNSUPPORTED_HISTORICAL_REPLAY':
      return {
        title: 'Bu tarih için olağan ücret politikası değişikliği uygulanamaz.',
        message: 'Geçmiş bordro hakkı ileri tarihli ücret değişikliğiyle düzeltilemez. Geçmiş farkı işlemini kullanın.',
      };
    case 'INVALID_POLICY':
      if (/road rounding mode|yol yuvarlama/i.test(error.message)) {
        return {
          title: 'Yol yuvarlama biçimi dönem ortasında değiştirilemez.',
          message: 'Yöntemi mevcut ücret politikasından koruyun. Değiştirmek gerekiyorsa geçerlilik tarihini ayın 15’ine alın; kayıtlı yanlış bir sürüm varsa aynı tarihte düzeltin.',
        };
      }
      return { title: 'Ücret değişikliği doğrulanamadı.', message: 'Formdaki tutarları, tarih ve açıklamayı gözden geçirin.' };
    case 'INVALID_REQUEST':
      return { title: 'Ücret değişikliği doğrulanamadı.', message: 'Formdaki tutarları, tarih ve açıklamayı gözden geçirin.' };
    case 'UNAUTHORIZED':
      return { title: 'Bu işlemi yapma yetkisi doğrulanamadı.', message: 'Ayarları yenileyin veya yetkili kullanıcıyla giriş yapın.' };
    case 'NOT_FOUND':
      return { title: 'Bordro dönemi bulunamadı.', message: 'Dönem seçimini yenileyip tekrar deneyin.' };
    case 'CURRENT_ENGINE_SNAPSHOT_REQUIRED':
      return { title: 'Güncel bordro bilgisi eksik.', message: 'Ücret değişikliği bu bordro durumunda uygulanamadı.' };
    case 'PERSISTENCE':
    case 'UNAVAILABLE':
      return { title: 'Ücret değişikliği tamamlanamadı.', message: 'Bağlantıyı veya kaydetme durumunu kontrol edip yeniden deneyin.' };
  }
}

function quarantineReasonLabel(reason: Extract<CompensationPolicyApiError, { code: 'QUARANTINED_POLICY' }>['policyVersion']['reason']): string {
  switch (reason) {
    case 'MISSING_BOUNDARY_SCOPE': return 'değişiklik kapsamı eksik';
    case 'MISSING_ROAD_ROUNDING_MODE': return 'yol yuvarlama biçimi eksik';
    case 'UNKNOWN_BOUNDARY_METADATA': return 'tanınmayan kapsam bilgisi';
    case 'INVALID_POLICY_STATE': return 'kayıtlı ücret bilgileri geçersiz';
    case 'INVALID_POLICY_GRAPH': return 'aynı tarihli kayıt bağlantısı geçersiz';
  }
}

function boundaryLabel(scope: CompensationBoundaryScope): string {
  return scope === 'ROAD_ONLY' ? 'Yalnız yol tutarı' : 'Tüm ücret kalemleri';
}

function roundingLabel(mode: RoadRoundingMode): string {
  return mode === 'PER_COMPENSATION_SLICE' ? 'Her tam kazanç diliminde ayrı' : 'Bütün dönemi tek kez';
}

function periodName(periodId: string): string {
  const [year, month] = periodId.split('-');
  const monthNames = ['Ocak', 'Şubat', 'Mart', 'Nisan', 'Mayıs', 'Haziran', 'Temmuz', 'Ağustos', 'Eylül', 'Ekim', 'Kasım', 'Aralık'];
  const index = Number(month) - 1;
  return Number.isInteger(Number(year)) && monthNames[index] ? `${monthNames[index]} ${year}` : periodId;
}

function shortDate(date: string): string {
  return /^\d{4}-\d{2}-\d{2}$/.test(date) ? date.split('-').reverse().join('.') : date;
}

function moneyStateEntries(state: CompensationPolicyStateWire): Array<[string, string, string]> {
  const userValue = (value: string | null): string => value === null ? 'Uygulanmıyor' : formatCompensationPolicyDecimalForUser(value);
  return [
    ['Günlük taban ücret', userValue(state.gunlukTabanUcret), 'TL'],
    ['Günlük yemek', userValue(state.gunlukYemek), 'TL'],
    ['Birleştirilmiş sosyal yardım', userValue(state.birlestirilmisSosyalYardim), 'TL'],
    ['Günlük vasıta/yol', userValue(state.gunlukVasitaYol), 'TL'],
    ['Giyim yardımı', userValue(state.giyimYardimi), 'TL'],
    ['Hizmet zammı birimi', userValue(state.hizmetZammiBirimi), 'TL'],
    ['İş primi yüzdesi', userValue(state.isPrimiYuzde), state.isPrimiYuzde === null ? '' : '%'],
    ['Gece çalışma primi', userValue(state.geceCalismaPrimiYuzde), '%'],
    ['Gece çalışma tatili primi', userValue(state.geceCalismaTatiliPrimiYuzde), '%'],
    ['Ek ödeme', userValue(state.ekOdeme), state.ekOdeme === null ? '' : 'TL'],
    ['Diğer gelir varsayılanı', userValue(state.digerGelirVarsayilan), state.digerGelirVarsayilan === null ? '' : 'TL'],
  ];
}

const PolicyStateDetails: React.FC<{ state: CompensationPolicyStateWire }> = ({ state }) => (
  <dl className="grid gap-x-4 gap-y-1 text-xs sm:grid-cols-2 xl:grid-cols-3">
    {moneyStateEntries(state).map(([label, value, unit]) => (
      <div key={label} className="flex min-w-0 items-baseline justify-between gap-2 border-b border-slate-100 py-1">
        <dt className="text-slate-600">{label}</dt>
        <dd className="break-all text-right font-mono font-semibold text-slate-900">{value}{unit ? ` ${unit}` : ''}</dd>
      </div>
    ))}
  </dl>
);

function operationFieldLabels(fields: CompensationPolicyDirectEffect['changedFields']): string[] {
  const labels: Record<string, string> = {
    gunlukTabanUcret: 'Günlük taban ücret',
    gunlukYemek: 'Günlük yemek',
    birlestirilmisSosyalYardim: 'Birleştirilmiş sosyal yardım',
    gunlukVasitaYol: 'Günlük vasıta/yol',
    giyimYardimi: 'Giyim yardımı',
    hizmetZammiBirimi: 'Hizmet zammı birimi',
    isPrimiYuzde: 'İş primi yüzdesi',
    geceCalismaPrimiYuzde: 'Gece çalışma primi',
    geceCalismaTatiliPrimiYuzde: 'Gece çalışma tatili primi',
    ekOdeme: 'Ek ödeme',
    digerGelirVarsayilan: 'Diğer gelir varsayılanı',
  };
  return fields
    .filter((field) => field !== 'isPrimiGruplari')
    .map((field) => labels[field] ?? field);
}

function windowEndLabel(throughInclusive: string | null, nextDate: string | null): string {
  if (throughInclusive) return `${shortDate(throughInclusive)} tarihine kadar (dahil)`;
  if (nextDate) return `${shortDate(nextDate)} tarihinden önceki güne kadar`;
  return 'Dönem kapsamı içinde sonraki sınır görünmüyor';
}

const DirectEffectDetails: React.FC<{ effect: CompensationPolicyDirectEffect }> = ({ effect }) => {
  const earningKind = effect.earningRoundingWindow.kind === 'PERIOD_BASELINE'
    ? 'Dönem başlangıcındaki yuvarlama sınırı'
    : effect.earningRoundingWindow.kind === 'OPENS_FULL_EARNING_SLICE'
      ? 'Bu sürüm yeni kazanç yuvarlama dilimi açıyor'
      : 'Yalnız yol güncelleniyor; mevcut kazanç yuvarlama dilimi sürüyor';
  return (
    <article className="space-y-2 border-t border-slate-200 pt-3 first:border-t-0 first:pt-0">
      <h4 className="text-sm font-bold text-slate-900">{shortDate(effect.effectiveFrom)} tarihinde doğrudan ücret etkisi</h4>
      <dl className="grid gap-2 sm:grid-cols-2">
        <div className="rounded-sm bg-slate-50 p-2">
          <dt className="text-xs font-semibold text-slate-700">Ücret durumu penceresi</dt>
          <dd className="mt-1 text-xs text-slate-700">{windowEndLabel(effect.policyStateWindow.throughInclusive, effect.policyStateWindow.nextEffectiveDate)}</dd>
        </div>
        <div className="rounded-sm bg-slate-50 p-2">
          <dt className="text-xs font-semibold text-slate-700">Kazanç yuvarlama penceresi</dt>
          <dd className="mt-1 text-xs text-slate-700">{earningKind}. {windowEndLabel(effect.earningRoundingWindow.throughInclusive, effect.earningRoundingWindow.nextFullEarningDate)}</dd>
        </div>
      </dl>
      <p className="text-xs text-slate-700"><span className="font-semibold">Kapsam:</span> {boundaryLabel(effect.boundaryScope)} · <span className="font-semibold">Yol yuvarlama:</span> {roundingLabel(effect.roadRoundingMode)}</p>
      <p className="text-xs text-slate-700"><span className="font-semibold">Değişen alanlar:</span> {operationFieldLabels(effect.changedFields).join(', ') || 'Aynı değerlerle yeni tarihli sürüm'}</p>
      {effect.affectedPeriods.length > 0 ? (
        <ul className="list-disc pl-5 text-xs text-slate-700">
          {effect.affectedPeriods.map((period) => <li key={`${period.periodId}:${period.startInclusive}`}>{period.periodId} · {period.startInclusive}–{period.endInclusive}</li>)}
        </ul>
      ) : <p className="text-xs text-slate-600">Doğrudan etkilenen bordro dönemi gösterilmedi.</p>}
    </article>
  );
};

function payrollReferenceLabel(reference: CompensationPolicyPayrollReference): string {
  return `${periodName(reference.periodId)} dönemindeki bordro kaydı`;
}

function dependencyLabel(kind: CompensationPolicyPreviewData['dependencyClosureEffects'][number]['kind']): string {
  switch (kind) {
    case 'GV': return 'Gelir vergisi zinciri';
    case 'PEK': return 'Prime esas kazanç zinciri';
    case 'PAYMENT': return 'Ödeme bağlantısı';
    case 'CARRY': return 'Devreden tutar bağlantısı';
  }
}

function dependencyActionLabel(action: CompensationPolicyPreviewData['dependencyClosureEffects'][number]['action']): string {
  switch (action) {
    case 'CALCULATED_TO_STALE': return 'Hesaplandı → yeniden hesaplanması gerekecek';
    case 'ALREADY_STALE': return 'Zaten yeniden hesaplanmalı';
    case 'FINALIZED_BLOCKER': return 'Kesinleştirilmiş · engel';
    case 'NO_MUTATION': return 'Kayıt değiştirilmeyecek';
  }
}

function payrollStatusLabel(status: CompensationPolicyPreviewData['dependencyClosureEffects'][number]['fromStatus']): string {
  switch (status) {
    case 'CALCULATED': return 'Hesaplandı';
    case 'STALE': return 'Yeniden hesaplanmalı';
    case 'FINALIZED': return 'Kesinleştirildi';
    case 'DRAFT': return 'Taslak';
  }
  return 'Durum bilinmiyor';
}

function retroBlockerLabel(reason: string): string {
  switch (reason) {
    case 'FINALIZED_BATCH': return 'geçmiş farkı kaydı kesinleştirilmiş';
    case 'SETTLED_BATCH': return 'ödeme tamamlanmış';
    case 'FINALIZED_SOURCE': return 'kaynak bordro kesinleştirilmiş';
    case 'FINALIZED_PAYMENT_EVENT': return 'ödeme kaydı kesinleştirilmiş';
    default: return 'çözümlenmemiş bir engel var';
  }
}

const PreviewPanel: React.FC<{ data: CompensationPolicyPreviewData }> = ({ data }) => (
  <section aria-labelledby="policy-preview-heading" className="space-y-4 border-t border-slate-200 pt-4">
    <div>
      <h3 id="policy-preview-heading" className="text-base font-bold text-slate-900">3. Etkileri inceleyin</h3>
      <p className="mt-1 text-xs text-slate-600">Ücret tutarı penceresi ile kazanç yuvarlama penceresi ayrıdır. Doğrudan dönemler ve bordro bağımlılıkları ayrı listelenir.</p>
    </div>
    {data.finalizedBlockers.length > 0 && (
      <div role="alert" className={errorBoxClass}>
        <h4 className="font-bold">Kesinleştirilmiş bordro değişikliği engelliyor</h4>
        <p className="mt-1">Politika kaydedilemez. Kesinleşmiş bordro burada yeniden hesaplanmaz; geçmiş düzeltmesi için geçmiş farkı işlemi gerekir.</p>
        <ul className="mt-2 list-disc pl-5 text-xs">
          {data.finalizedBlockers.map((reference) => <li key={`${reference.payrollId}:${reference.accrualId}`}>{payrollReferenceLabel(reference)}</li>)}
        </ul>
      </div>
    )}

    {data.retroEffects.length > 0 && (
      <div className="rounded-md border border-indigo-200 bg-indigo-50 p-3">
        <h4 className="font-bold text-indigo-950">Etkilenen geçmiş farkı kayıtları</h4>
        <p className="mt-1 text-xs text-indigo-900">Kaydetme tamamlanırsa hesaplanmış ve ödemesi tamamlanmamış kayıtlar korunur, yeniden hesaplanması gereken duruma alınır.</p>
        <ul className="mt-2 divide-y divide-indigo-100 text-xs text-indigo-950">
          {data.retroEffects.map((effect) => (
            <li key={effect.batchId} className="py-2">
              <span className="font-semibold">Geçmiş farkı kaydı</span>
              <span className="block">Kaynak dönemler: {effect.sourcePeriodIds.map(periodName).join(', ') || '—'} · bağlı ödeme kaydı: {effect.linkedPaymentEventIds.length}</span>
              <span className="block">{effect.action === 'STALE_ELIGIBLE' ? 'Kaydetme tamamlanırsa yeniden hesaplanması gerekecek.' : effect.blockers.map(retroBlockerLabel).join(', ')}</span>
            </li>
          ))}
        </ul>
      </div>
    )}

    <div className="space-y-3">
      <h4 className="text-sm font-bold text-slate-900">Doğrudan ücret etkisi</h4>
      {data.directCompensationEffects.length > 0
        ? data.directCompensationEffects.map((effect, index) => <DirectEffectDetails key={`${effect.effectiveFrom}:${index}`} effect={effect} />)
        : <p className="text-sm text-slate-600">Doğrudan ücret etkisi bulunmadı.</p>}
    </div>

    <div className="space-y-2 border-t border-slate-200 pt-3">
      <h4 className="text-sm font-bold text-slate-900">Bordro bağımlılık etkileri</h4>
      <p className="text-xs text-slate-600">Gelir vergisi, PEK, ödeme ve devreden tutar zincirleri ücret etkilerinden ayrı gösterilir.</p>
      {data.dependencyClosureEffects.length > 0 ? (
        <ul className="divide-y divide-slate-100">
          {data.dependencyClosureEffects.map((effect, index) => (
            <li key={`${effect.kind}:${effect.affectedPayroll.payrollId}:${index}`} className="py-2 text-xs text-slate-700">
              <span className="font-semibold">{dependencyLabel(effect.kind)}</span> · {payrollReferenceLabel(effect.affectedPayroll)}
              <span className="block">{payrollStatusLabel(effect.fromStatus)} → {effect.toStatus ? payrollStatusLabel(effect.toStatus) : 'Değiştirilmeyecek'} · {dependencyActionLabel(effect.action)}</span>
            </li>
          ))}
        </ul>
      ) : <p className="text-sm text-slate-600">Bağlı başka bordro etkisi gösterilmedi.</p>}
    </div>
  </section>
);

const ScopedHistory: React.FC<{ data: CompensationPolicyLoadData }> = ({ data }) => {
  if (data.timeline.versions.length === 0) return null;
  return (
    <details className="border-t border-slate-200 pt-3">
      <summary className="cursor-pointer text-sm font-semibold text-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">
        Bu dönem için geçerli ücret kayıtları ({data.timeline.versions.length})
      </summary>
      <p className="mt-1 text-xs text-slate-600">Liste, seçili dönem başlangıcında geçerli kayıtla bu dönem içindeki değişiklikleri kapsar.</p>
      <ol className="mt-2 divide-y divide-slate-200">
        {data.timeline.versions.map((version) => (
          <li key={version.id} className="py-2">
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <div>
                <h3 className="text-sm font-semibold text-slate-900">{shortDate(version.effectiveFrom)} · {version.reason}</h3>
                <p className="mt-1 text-xs text-slate-600">{boundaryLabel(version.boundaryScope)} · yol tutarı: {roundingLabel(version.roadRoundingMode)}</p>
                {version.supersedesVersionId && <p className="mt-1 text-xs text-slate-500">Aynı tarihteki önceki kayıt düzeltildi.</p>}
              </div>
              <time className="text-xs text-slate-500" dateTime={version.createdAt}>{version.createdAt.slice(0, 10)}</time>
            </div>
            <details className="mt-2">
              <summary className="cursor-pointer text-xs font-semibold text-indigo-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">Ücret ayrıntılarını göster</summary>
              <div className="mt-2"><PolicyStateDetails state={version.state} /></div>
            </details>
          </li>
        ))}
      </ol>
    </details>
  );
};

function mergedValidation(local: CompensationPolicyDraftValidation, upstream: CompensationPolicyDraftValidation): CompensationPolicyDraftValidation {
  if (local.fullState.status !== 'VALID') return local;
  if (upstream.fullState.status !== 'VALID') return upstream;
  const operationErrors = [...new Set([...local.operationErrors, ...upstream.operationErrors])];
  return { draft: operationErrors.length === 0 ? 'VALID' : 'INVALID', fullState: local.fullState, operationErrors };
}

function policySelectionSource(
  load: CompensationPolicyLoadData | null,
  draft: Draft
): CompensationPolicyLoadData['timeline']['versions'][number] | null {
  if (!load || draft.operationKind === 'INITIAL_BASELINE') return null;
  if (draft.operationKind === 'SAME_DATE_CORRECTION') {
    const targetId = findScopedSameDateActiveLeaf(load.timeline.versions, draft.effectiveFrom);
    return targetId ? load.timeline.versions.find((version) => version.id === targetId) ?? null : null;
  }
  const sourceDate = [...new Set(load.timeline.versions
    .map((version) => version.effectiveFrom)
    .filter((date) => date <= draft.effectiveFrom))]
    .sort()
    .reverse()
    .find((date) => findScopedSameDateActiveLeaf(load.timeline.versions, date) !== null);
  const sourceId = sourceDate
    ? findScopedSameDateActiveLeaf(load.timeline.versions, sourceDate)
    : null;
  return sourceId
    ? load.timeline.versions.find((version) => version.id === sourceId) ?? null
    : load.activePolicyProjection.kind === 'ACTIVE' ? load.activePolicyProjection.version : null;
}

function rebaseChangedDraftFields(
  draft: Draft,
  previous: CompensationPolicyLoadData['timeline']['versions'][number] | null,
  next: CompensationPolicyLoadData['timeline']['versions'][number]
): Partial<Draft> {
  const nextState = compensationPolicyStateToDraft(next.state);
  if (!previous) return nextState;
  const previousState = compensationPolicyStateToDraft(previous.state);
  const current = draft as unknown as Record<string, unknown>;
  const before = previousState as unknown as Record<string, unknown>;
  const rebased = { ...nextState } as unknown as Record<string, unknown>;
  for (const key of Object.keys(rebased)) {
    if (stableValue(current[key]) !== stableValue(before[key])) rebased[key] = current[key];
  }
  return rebased as Partial<Draft>;
}

export const CompensationPolicyPage: React.FC<Props> = (props) => {
  const succeededLoad = props.saveState.status === 'succeeded' ? props.saveState.authoritativeLoad : null;
  const readyLoad = props.loadState.status === 'ready' ? props.loadState.value : null;
  const candidateAuthoritativeLoad = succeededLoad && stableValue(succeededLoad.scope) === stableValue(props.scope)
    ? succeededLoad
    : readyLoad ?? succeededLoad;
  const scopeMatches = Boolean(candidateAuthoritativeLoad && stableValue(candidateAuthoritativeLoad.scope) === stableValue(props.scope));
  const authoritativeLoad = scopeMatches ? candidateAuthoritativeLoad : null;
  const selectionSource = policySelectionSource(authoritativeLoad, props.draft);
  const initialBaselineSelections = props.draft.operationKind === 'INITIAL_BASELINE'
    ? { boundaryScope: props.draft.boundaryScope, roadRoundingMode: props.draft.roadRoundingMode }
    : null;
  const selectionSourceKey = stableValue({
    scope: props.scope,
    operationKind: props.draft.operationKind,
    selectionSource: selectionSource
      ? { id: selectionSource.id, boundaryScope: selectionSource.boundaryScope, roadRoundingMode: selectionSource.roadRoundingMode, state: selectionSource.state }
      : null,
    initialBaselineSelections,
  });
  const initialScopeSelection = selectionSource?.boundaryScope ?? initialBaselineSelections?.boundaryScope ?? null;
  const initialRoundingSelection = selectionSource?.roadRoundingMode ?? initialBaselineSelections?.roadRoundingMode ?? null;
  const [scopeSelection, setScopeSelection] = useState<Props['draft']['boundaryScope'] | null>(initialScopeSelection);
  const [roundingSelection, setRoundingSelection] = useState<Props['draft']['roadRoundingMode'] | null>(initialRoundingSelection);
  const [baselineConfirmation, setBaselineConfirmation] = useState<string | null>(null);
  const [showValidation, setShowValidation] = useState(false);
  const [callbackError, setCallbackError] = useState<string | null>(null);
  const [loadPending, setLoadPending] = useState(false);
  const [previewPending, setPreviewPending] = useState(false);
  const [savePending, setSavePending] = useState(false);
  const [reloadPending, setReloadPending] = useState(false);
  const [savedOperationFingerprint, setSavedOperationFingerprint] = useState<string | null>(null);
  const [lastPreviewRequest, setLastPreviewRequest] = useState<{ operation: string; scope: string } | null>(null);
  const previewInFlightRef = useRef(false);
  const saveInFlightRef = useRef(false);
  const validationSummaryRef = useRef<HTMLDivElement>(null);

  const localValidation = useMemo(() => validateCompensationPolicyDraft(props.draft), [props.draft]);
  const validation = mergedValidation(localValidation, props.draftValidation);
  const operationResult = useMemo(() => draftToCompensationPolicyOperation(props.draft), [props.draft]);
  const currentOperation = operationResult.ok ? operationResult.operation : null;
  const currentFingerprint = currentOperation ? stableValue(currentOperation) : null;
  const baselineConfirmationMatches = currentFingerprint !== null && baselineConfirmation === currentFingerprint;
  const sameDateLeaf = authoritativeLoad && props.draft.operationKind === 'SAME_DATE_CORRECTION'
    ? findScopedSameDateActiveLeaf(authoritativeLoad.timeline.versions, props.draft.effectiveFrom)
    : null;
  const correctionTargetFresh = props.draft.operationKind !== 'SAME_DATE_CORRECTION'
    || (sameDateLeaf !== null && props.draft.supersedesVersionId === sameDateLeaf);
  const visibleScopeSelection = scopeSelection ?? selectionSource?.boundaryScope ?? null;
  const visibleRoundingSelection = roundingSelection ?? selectionSource?.roadRoundingMode ?? null;
  const scopeSelectionFresh = visibleScopeSelection !== null && visibleScopeSelection === props.draft.boundaryScope;
  const roundingSelectionFresh = visibleRoundingSelection !== null && visibleRoundingSelection === props.draft.roadRoundingMode;
  const initialBaselineAllowed = Boolean(authoritativeLoad?.initialBaselineRequired && props.draft.operationKind === 'INITIAL_BASELINE');
  const isSaving = props.saveState.status === 'saving' || props.saveState.status === 'reloading-authoritative' || savePending;
  const operationUnchangedSinceSuccess = props.saveState.status === 'succeeded'
    && (savedOperationFingerprint === null || savedOperationFingerprint === currentFingerprint);

  useEffect(() => {
    setScopeSelection(initialScopeSelection);
    setRoundingSelection(initialRoundingSelection);
  }, [selectionSourceKey]);
  useEffect(() => {
    if (props.loadState.status !== 'idle' && props.loadState.status !== 'error') setLoadPending(false);
  }, [props.loadState.status]);
  useEffect(() => {
    if (props.previewState.status !== 'idle') {
      previewInFlightRef.current = false;
      setPreviewPending(false);
    }
  }, [props.previewState.status]);
  useEffect(() => {
    if (props.saveState.status !== 'idle') {
      saveInFlightRef.current = false;
      setSavePending(false);
    }
    if (props.saveState.status === 'succeeded' && currentFingerprint !== null) setSavedOperationFingerprint(currentFingerprint);
  }, [props.saveState.status]);
  useEffect(() => {
    if (props.loadState.status !== 'loading') setReloadPending(false);
  }, [props.loadState.status]);

  const changeDraft = (next: Draft) => {
    const operationChanged = next.operationKind !== props.draft.operationKind;
    const correctionTargetChanged = next.operationKind === 'SAME_DATE_CORRECTION'
      && (next.effectiveFrom !== props.draft.effectiveFrom || next.supersedesVersionId !== props.draft.supersedesVersionId);
    const effectiveDateChanged = next.operationKind === 'NEW_EFFECTIVE_DATE'
      && next.effectiveFrom !== props.draft.effectiveFrom;
    if (operationChanged || correctionTargetChanged || effectiveDateChanged) {
      const previousSource = effectiveDateChanged
        ? policySelectionSource(authoritativeLoad, props.draft)
        : null;
      const nextSource = policySelectionSource(authoritativeLoad, next);
      setScopeSelection(nextSource?.boundaryScope ?? null);
      setRoundingSelection(nextSource?.roadRoundingMode ?? null);
      if (nextSource && next.operationKind !== 'INITIAL_BASELINE') {
        next = {
          ...next,
          ...(operationChanged || next.operationKind === 'SAME_DATE_CORRECTION'
            ? compensationPolicyStateToDraft(nextSource.state)
            : effectiveDateChanged
              ? rebaseChangedDraftFields(props.draft, previousSource, nextSource)
              : {}),
          boundaryScope: nextSource.boundaryScope,
          roadRoundingMode: nextSource.roadRoundingMode,
          supersedesVersionId: next.operationKind === 'SAME_DATE_CORRECTION' ? nextSource.id : null,
        };
      }
    }
    setBaselineConfirmation(null);
    setCallbackError(null);
    props.onDraftChange(next);
    props.onValidateDraft(next);
  };

  const selectBoundaryScope = (scope: Draft['boundaryScope']) => {
    setScopeSelection(scope);
    if (props.draft.boundaryScope === scope) props.onValidateDraft(props.draft);
    else changeDraft({ ...props.draft, boundaryScope: scope });
  };

  const selectRoadMode = (mode: Draft['roadRoundingMode']) => {
    setRoundingSelection(mode);
    if (props.draft.roadRoundingMode === mode) props.onValidateDraft(props.draft);
    else changeDraft({ ...props.draft, roadRoundingMode: mode });
  };

  const validationMessages = useMemo(() => {
    const messages: string[] = [];
    const add = (message: string) => { if (message && !messages.includes(message)) messages.push(message); };
    if (validation.fullState.status === 'INCOMPLETE') Object.values(validation.fullState.fieldErrors).forEach((message) => message && add(message));
    if (validation.fullState.status === 'INVALID') validation.fullState.errors.forEach(add);
    validation.operationErrors.forEach(add);
    if (!scopeSelectionFresh) add('Ücret değişikliği kapsamını açıkça seçin.');
    if (!roundingSelectionFresh) add('Yol tutarı yuvarlama biçimini açıkça seçin.');
    if (!correctionTargetFresh) add('Aynı tarih düzeltmesi için kayıtlı etkin ücret bulunamadı. Ücret geçmişini yenileyin.');
    if (props.draft.operationKind === 'INITIAL_BASELINE' && !initialBaselineAllowed) add('Başlangıç ücreti yalnız yeni bir başlangıç kaydı gerektiğinde oluşturulabilir.');
    return messages;
  }, [validation, scopeSelectionFresh, roundingSelectionFresh, correctionTargetFresh, props.draft.operationKind, initialBaselineAllowed]);

  const formReady = validation.draft === 'VALID'
    && scopeSelectionFresh
    && roundingSelectionFresh
    && correctionTargetFresh
    && (props.draft.operationKind !== 'INITIAL_BASELINE' || initialBaselineAllowed);
  const activePreview = props.previewState.status === 'ready' ? props.previewState.value : null;
  const currentScopeFingerprint = stableValue(props.scope);
  const previewRequestFresh = currentFingerprint !== null
    && lastPreviewRequest?.operation === currentFingerprint
    && lastPreviewRequest.scope === currentScopeFingerprint;
  const previewFresh = Boolean(activePreview && currentOperation && previewRequestFresh && sameOperation(activePreview.previewToken.operation, currentOperation));
  const previewStale = Boolean(activePreview && !previewFresh);
  const finalizedBlockers = activePreview?.finalizedBlockers ?? [];
  const previewBusy = props.previewState.status === 'loading' || previewPending;
  const loadBusy = props.loadState.status === 'loading' || loadPending;
  const canConfirmBaseline = initialBaselineAllowed
    && validation.fullState.status === 'VALID'
    && scopeSelectionFresh
    && roundingSelectionFresh
    && correctionTargetFresh
    && previewFresh
    && finalizedBlockers.length === 0
    && !isSaving
    && !previewBusy;
  const canPreview = Boolean(authoritativeLoad)
    && !isSaving
    && !previewBusy
    && props.saveState.status !== 'persisted-reload-failed'
    && !operationUnchangedSinceSuccess;
  const canSave = Boolean(activePreview)
    && previewFresh
    && formReady
    && (props.draft.operationKind !== 'INITIAL_BASELINE' || baselineConfirmationMatches)
    && finalizedBlockers.length === 0
    && !isSaving
    && !previewBusy
    && !operationUnchangedSinceSuccess
    && props.saveState.status !== 'persisted-reload-failed';
  const saveHint = finalizedBlockers.length > 0
    ? 'Kesinleştirilmiş bordro engeli çözülmeden bu değişiklik kaydedilemez.'
    : !formReady
      ? 'Tutarları ve gerekli politika bilgilerini tamamlayın.'
      : !previewFresh
        ? 'Kaydetmeden önce bu taslağın etkilerini önizleyin.'
        : null;

  const validateAndFocus = (): boolean => {
    props.onValidateDraft(props.draft);
    if (formReady && currentOperation) return true;
    setShowValidation(true);
    if (!scopeSelectionFresh || !roundingSelectionFresh) {
      const targetId = !scopeSelectionFresh ? 'policy-boundary-scope-full' : 'policy-road-rounding-slice';
      requestAnimationFrame(() => {
        const details = document.getElementById('policy-calculation-details') as HTMLDetailsElement | null;
        if (details) details.open = true;
        requestAnimationFrame(() => {
          const target = document.getElementById(targetId);
          target?.scrollIntoView({ block: 'center' });
          target?.focus();
        });
      });
    } else {
      requestAnimationFrame(() => validationSummaryRef.current?.focus());
    }
    return false;
  };

  const handleLoad = () => {
    if (loadBusy) return;
    setLoadPending(true);
    setCallbackError(null);
    try {
      const result = props.onLoad();
      if (result && typeof result.then === 'function') {
        void Promise.resolve(result).catch(() => setCallbackError('Ücret politikası yüklenemedi. Tekrar deneyin.')).finally(() => setLoadPending(false));
      } else {
        window.setTimeout(() => setLoadPending(false), 0);
      }
    } catch {
      setLoadPending(false);
      setCallbackError('Ücret politikası yüklenemedi. Tekrar deneyin.');
    }
  };

  const handlePreview = () => {
    if (previewInFlightRef.current || !canPreview || !validateAndFocus() || !currentOperation) return;
    setShowValidation(false);
    setCallbackError(null);
    previewInFlightRef.current = true;
    setPreviewPending(true);
    setLastPreviewRequest({ operation: stableValue(currentOperation), scope: stableValue(props.scope) });
    try {
      const result = props.onPreview(currentOperation);
      if (result && typeof result.then === 'function') {
        void Promise.resolve(result).catch(() => setCallbackError('Etki önizlemesi alınamadı. Tekrar deneyin.')).finally(() => {
          previewInFlightRef.current = false;
          setPreviewPending(false);
        });
      } else {
        window.setTimeout(() => {
          if (props.previewState.status === 'idle') previewInFlightRef.current = false;
          setPreviewPending(false);
        }, 0);
      }
    } catch {
      previewInFlightRef.current = false;
      setPreviewPending(false);
      setCallbackError('Etki önizlemesi alınamadı. Tekrar deneyin.');
    }
  };

  const handleSave = () => {
    if (saveInFlightRef.current || !canSave || !validateAndFocus() || !currentOperation || !activePreview) return;
    const isInitial = currentOperation.kind === 'INITIAL_BASELINE';
    const request: CompensationPolicySaveRequest = isInitial
      ? {
          scope: props.scope,
          previewToken: activePreview.previewToken,
          operation: currentOperation,
          initialBaselineConfirmation: { confirmed: true, effectiveFrom: currentOperation.effectiveFrom },
        }
      : { scope: props.scope, previewToken: activePreview.previewToken, operation: currentOperation };
    setSavePending(true);
    saveInFlightRef.current = true;
    setCallbackError(null);
    try {
      const result = props.onSave(request);
      if (result && typeof result.then === 'function') {
        void Promise.resolve(result).catch(() => setCallbackError('Ücret politikası kaydedilemedi. Tekrar önizleyin.')).finally(() => {
          saveInFlightRef.current = false;
          setSavePending(false);
        });
      } else {
        // A synchronous controller updates saveState to `saving`; the prop transition releases this guard.
        window.setTimeout(() => {
          if (props.saveState.status === 'idle') {
            saveInFlightRef.current = false;
            setSavePending(false);
          }
        }, 0);
      }
    } catch {
      saveInFlightRef.current = false;
      setSavePending(false);
      setCallbackError('Ücret politikası kaydedilemedi. Tekrar önizleyin.');
    }
  };

  const handleReloadAuthoritative = () => {
    if (reloadPending || props.loadState.status === 'loading') return;
    setReloadPending(true);
    setCallbackError(null);
    try {
      const result = props.onReloadAuthoritative();
      if (result && typeof result.then === 'function') {
        void Promise.resolve(result).catch(() => setCallbackError('Yetkili ücret politikası yeniden yüklenemedi.')).finally(() => setReloadPending(false));
      } else {
        window.setTimeout(() => setReloadPending(false), 0);
      }
    } catch {
      setReloadPending(false);
      setCallbackError('Yetkili ücret politikası yeniden yüklenemedi.');
    }
  };

  const handleConfirmBaseline = () => {
    if (!canConfirmBaseline || !currentOperation) {
      if (!formReady) {
        setShowValidation(true);
        requestAnimationFrame(() => validationSummaryRef.current?.focus());
      } else if (!previewFresh) {
        setCallbackError('Önce güncel etkileri inceleyin.');
      }
      return;
    }
    props.onConfirmInitialBaseline(currentOperation.effectiveFrom);
    setBaselineConfirmation(currentFingerprint);
    props.onValidateDraft(props.draft);
  };

  const handleCancelBaseline = () => {
    setBaselineConfirmation(null);
    props.onCancelInitialBaseline();
  };

  const copyActivePolicyDraft = () => {
    const source = policySelectionSource(authoritativeLoad, props.draft);
    if (!source) return;
    try {
      const stateDraft = compensationPolicyStateToDraft(source.state);
      setScopeSelection(source.boundaryScope);
      setRoundingSelection(source.roadRoundingMode);
      changeDraft({
        ...props.draft,
        ...stateDraft,
        boundaryScope: source.boundaryScope,
        roadRoundingMode: source.roadRoundingMode,
        supersedesVersionId: props.draft.operationKind === 'SAME_DATE_CORRECTION' ? source.id : null,
      });
    } catch {
      setCallbackError('Kayıtlı ücret tutarları taslağa taşınamadı; tutarları gözden geçirip yeniden deneyin.');
    }
  };

  const showApiError = (error: CompensationPolicyApiError, retry?: 'load' | 'reload' | 'preview') => {
    const formatted = titleCaseApiError(error);
    return (
      <div role="alert" className={errorBoxClass}>
        <h3 className="font-bold">{formatted.title}</h3>
        <p className="mt-1">{formatted.message}</p>
        {formatted.detail && <p className="mt-1 text-xs text-rose-800">{formatted.detail}</p>}
        {error.code === 'FINALIZED_BLOCKER' && (
          <ul className="mt-2 list-disc pl-5 text-xs">
            {error.blockers.map((reference) => <li key={`${reference.payrollId}:${reference.accrualId}`}>{payrollReferenceLabel(reference)}</li>)}
          </ul>
        )}
        {error.code === 'CONFLICT' && error.actualTargetState?.sameDateActiveLeafVersionId && <p className="mt-1 text-xs">Aynı tarihte başka bir değişiklik kaydı bulundu.</p>}
        {retry && (
          <button type="button" className={`${secondaryButtonClass} mt-3`} onClick={retry === 'load' ? handleLoad : retry === 'reload' ? handleReloadAuthoritative : handlePreview}>
            {retry === 'preview' ? 'Etkileri yeniden önizle' : retry === 'load' ? 'Ücret geçmişini yeniden yükle' : 'Yetkili durumu yeniden yükle'}
          </button>
        )}
      </div>
    );
  };

  const disabledForm = !authoritativeLoad || loadBusy || isSaving;
  const showInitialBaselineCard = authoritativeLoad?.initialBaselineRequired ?? false;
  return (
    <section className="mx-auto flex h-full min-h-0 w-full max-w-7xl flex-1 flex-col text-slate-900" aria-labelledby="compensation-policy-title">
      <div data-testid="compensation-policy-scroll-region" className="min-h-0 flex-1 space-y-5 overflow-y-auto p-4 sm:p-6">
      <header className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 pb-4">
        <div>
          <h1 id="compensation-policy-title" className="text-xl font-bold tracking-tight">{showInitialBaselineCard ? 'Başlangıç ücretlerini oluştur' : 'Ücret politikası'}</h1>
          <p className="mt-1 text-sm text-slate-600">{showInitialBaselineCard ? 'Dönem başlangıç ücretlerini kontrol edin ve etkileri inceleyin.' : 'Ücretleri gözden geçirin, etkileri önizleyin ve kaydedin.'}</p>
          <p className="mt-2 text-xs font-medium text-slate-700">{periodName(props.scope.periodId)} · {shortDate(props.scope.periodStartInclusive)}–{shortDate(props.scope.periodEndInclusive)}</p>
        </div>
        {props.loadState.status === 'idle' && (
          <button type="button" className={secondaryButtonClass} onClick={handleLoad} disabled={loadBusy}>
            {loadBusy ? 'Yükleniyor…' : 'Ücret geçmişini yükle'}
          </button>
        )}
      </header>

      {loadBusy && <p role="status" className="text-sm text-slate-600">Bu dönem için ücret bilgileri yükleniyor…</p>}
      {props.loadState.status === 'error' && props.saveState.status !== 'persisted-reload-failed' && showApiError(props.loadState.error, 'load')}
      {candidateAuthoritativeLoad && !scopeMatches && (
        <div role="alert" className={warningBoxClass}>
          Dönem değişti. Yeni dönemin ücret bilgilerini yeniden yükleyin.
          <button type="button" className={`${secondaryButtonClass} mt-3`} onClick={handleReloadAuthoritative} disabled={reloadPending || loadBusy}>{reloadPending ? 'Yeniden yükleniyor…' : 'Yeni dönemin bilgilerini yükle'}</button>
        </div>
      )}
      {callbackError && <p role="alert" className={errorBoxClass}>{callbackError}</p>}

      {props.saveState.status === 'saving' && <p role="status" className={warningBoxClass}>Ücret değişikliği kaydediliyor…</p>}
      {props.saveState.status === 'reloading-authoritative' && <p role="status" className={warningBoxClass}>Kayıt tamamlandı; ücret bilgileri yeniden yükleniyor…</p>}
      {props.saveState.status === 'persisted-reload-failed' && (
        <div className={errorBoxClass} role="alert">
          <h2 className="font-bold">Kayıt tamamlandı ancak güncel ücret bilgileri yeniden yüklenemedi.</h2>
          <p className="mt-1">Tekrar yükleyerek kaydedilen değişikliği doğrulayın.</p>
          <p className="mt-1 text-xs">{titleCaseApiError(props.saveState.error).message}</p>
          <button type="button" className={`${secondaryButtonClass} mt-3`} onClick={handleReloadAuthoritative} disabled={reloadPending || props.loadState.status === 'loading'}>{reloadPending ? 'Yeniden yükleniyor…' : 'Ücret bilgilerini yeniden yükle'}</button>
        </div>
      )}
      {props.saveState.status === 'succeeded' && operationUnchangedSinceSuccess && (
        <div role="status" className={successBoxClass}>
          <strong>Ücret değişikliği kaydedildi.</strong>
          <span className="block mt-1">Geçerlilik tarihi: {shortDate(props.saveState.receipt.version.effectiveFrom)}.</span>
          {props.saveState.receipt.invalidatedRetroBatches.length > 0 && (
            <span className="block mt-1">Yeniden hesaplanması gereken geçmiş farkı kaydı: {props.saveState.receipt.invalidatedRetroBatches.length}.</span>
          )}
          {props.saveState.receipt.invalidatedRetroPaymentEvents.length > 0 && (
            <span className="block mt-1">Yeniden incelenmesi gereken bağlı ödeme kaydı: {props.saveState.receipt.invalidatedRetroPaymentEvents.length}.</span>
          )}
        </div>
      )}
      {props.saveState.status === 'succeeded' && !operationUnchangedSinceSuccess && (
        <p role="status" className="rounded-sm bg-slate-50 p-2 text-sm text-slate-700">Önceki değişiklik kaydedildi. Taslak sonradan değişti; yeni taslağı önizleyip tekrar kaydedin.</p>
      )}
      {props.saveState.status === 'conflict' && showApiError(props.saveState.error, 'reload')}
      {props.saveState.status === 'blocked' && showApiError(props.saveState.error)}
      {props.saveState.status === 'error' && showApiError(props.saveState.error, props.saveState.error.code === 'CONFLICT' ? 'reload' : 'preview')}

      {authoritativeLoad && <ScopedHistory data={authoritativeLoad} />}

      {authoritativeLoad?.timeline.resolution === 'INVALID_ROAD_ROUNDING' && (
        <div data-testid="policy-road-rounding-recovery" role="alert" className={errorBoxClass}>
          <h2 className="font-bold">Bu hizmet döneminde yol yuvarlama biçimleri uyuşmuyor.</h2>
          <p className="mt-1">Bordro hesaplaması güvenli sonuç üretemediği için durduruldu. Her farklı modu, önceki etkin sürümle aynı yapacak şekilde aynı tarih düzeltmesiyle onarın; mod değişikliği gerekiyorsa yeni hizmet dönemi ayın 15’inde başlamalıdır.</p>
          <ul className="mt-2 list-disc pl-5 text-xs">
            {(authoritativeLoad.timeline.roadRoundingIssues ?? []).map((issue) => (
              <li key={`${issue.effectiveFrom}:${issue.actualMode}`}>
                {shortDate(issue.effectiveFrom)} tarihli sürüm: {roundingLabel(issue.actualMode)} · önceki etkin sürüm: {roundingLabel(issue.expectedMode)}
              </li>
            ))}
          </ul>
          <p className="mt-2 text-xs">Düzeltme için işlem türünü “Aynı tarihte düzeltme” seçin, listedeki tarihi açın ve yuvarlama biçimini önceki sürümle eşleştirin. Kesinleşmiş bordrolar için mevcut retro engelleri ayrıca uygulanır.</p>
        </div>
      )}

      {showInitialBaselineCard && authoritativeLoad && (
        <section aria-labelledby="initial-baseline-heading" className={`${warningBoxClass} space-y-0.5 px-3 py-2`}>
          <h2 id="initial-baseline-heading" className="text-sm font-bold">Başlangıç kaydı henüz yok.</h2>
          <p className="text-sm">{authoritativeLoad.nonAuthoritativePrefill.kind === 'AVAILABLE'
            ? `${periodName(authoritativeLoad.nonAuthoritativePrefill.sourcePeriodId)} tutarları aşağıdaki forma aktarıldı. Bu işlem kayıt oluşturmadı.`
            : authoritativeLoad.nonAuthoritativePrefill.reason === 'INCOMPLETE_SETTINGS'
              ? 'Eski dönem ayarlarında eksik alanlar var. Eksik tutarları doğrulayarak tamamlayın.'
              : authoritativeLoad.nonAuthoritativePrefill.reason === 'NO_LEGACY_SETTINGS'
                ? 'Kullanılabilir eski ücret kaydı bulunamadı. Başlangıç tutarlarını girin.'
                : 'Eski dönem ücret ayarlarına erişilemiyor. Başlangıç tutarlarını girin.'}</p>
        </section>
      )}

      {authoritativeLoad && (
        <section aria-labelledby="policy-edit-heading" className="space-y-4 border-t-2 border-slate-300 pt-4">
          <div>
            <h2 id="policy-edit-heading" className="text-base font-bold text-slate-900">{showInitialBaselineCard ? '1. Tutarları ve grup oranlarını gözden geçirin' : 'Değişikliği hazırlayın'}</h2>
          </div>

          {showValidation && validationMessages.length > 0 && (
            <div ref={validationSummaryRef} tabIndex={-1} role="alert" className={errorBoxClass}>
              <h3 className="font-bold">Önizleme için formu tamamlayın</h3>
              <ul className="mt-1 list-disc pl-5">
                {validationMessages.slice(0, 12).map((message) => <li key={message}>{message}</li>)}
                {validationMessages.length > 12 && <li>Diğer alanlardaki hataları ilgili bölümde inceleyin.</li>}
              </ul>
            </div>
          )}

          <CompensationPolicyForm
            draft={props.draft}
            loadData={authoritativeLoad}
            validation={validation}
            disabled={disabledForm}
            showValidation={showValidation}
            scopeSelection={visibleScopeSelection}
            roundingSelection={visibleRoundingSelection}
            onDraftChange={changeDraft}
            onSelectBoundaryScope={selectBoundaryScope}
            onSelectRoadRoundingMode={selectRoadMode}
            onCopyActiveState={copyActivePolicyDraft}
            canApplyPercentageIncrease={validation.fullState.status === 'VALID' && !disabledForm}
          />

          {props.previewState.status === 'error' && showApiError(props.previewState.error, 'preview')}
          {activePreview && previewStale && (
            <p role="status" className={warningBoxClass}>Taslak önizlemeden sonra değişti. Kaydetmeden önce güncel etkileri yeniden önizleyin.</p>
          )}
          {activePreview && previewFresh && <PreviewPanel data={activePreview} />}

          {props.previewState.status === 'loading' && <p role="status" className={warningBoxClass}>Ücret değişikliğinin dönem ve bordro etkileri hesaplanıyor…</p>}
          {props.previewState.status === 'ready' && !activePreview && <p className="text-sm text-slate-600">Önizleme sonucu bulunmuyor.</p>}

        </section>
      )}
      <div data-testid="compensation-policy-content-end" aria-hidden="true" className="h-px" />
      </div>
      {authoritativeLoad && (
        <footer data-testid="compensation-policy-action-bar" className="shrink-0 border-t border-slate-200 bg-white px-4 py-3 shadow-[0_-4px_10px_rgba(15,23,42,0.06)] sm:px-6">
          <div className="flex flex-wrap items-center gap-2 sm:gap-3">
            {initialBaselineAllowed ? (
              !previewFresh || finalizedBlockers.length > 0 ? (
                <button type="button" className={primaryButtonClass} onClick={handlePreview} disabled={!canPreview} title={!formReady ? saveHint ?? undefined : undefined}>
                  {previewBusy ? 'Etkiler hesaplanıyor…' : previewFresh ? 'Önizlemeyi yenile' : previewStale ? 'Önizlemeyi yenile' : 'Etkileri önizle'}
                </button>
              ) : !baselineConfirmationMatches ? (
                <button type="button" className={primaryButtonClass} onClick={handleConfirmBaseline} disabled={!canConfirmBaseline}>
                  Başlangıç ücretini onayla
                </button>
              ) : (
                <>
                  <button type="button" className={primaryButtonClass} onClick={handleSave} disabled={!canSave} title={!canSave ? saveHint ?? undefined : undefined}>
                    {isSaving ? 'Kaydediliyor…' : 'Başlangıç ücretini kaydet'}
                  </button>
                  <button type="button" className={secondaryButtonClass} onClick={handleCancelBaseline} disabled={isSaving}>Onayı kaldır</button>
                </>
              )
            ) : !previewFresh || finalizedBlockers.length > 0 ? (
              <button type="button" className={primaryButtonClass} onClick={handlePreview} disabled={!canPreview} title={!formReady ? saveHint ?? undefined : undefined}>
                {previewBusy ? 'Etkiler hesaplanıyor…' : previewStale ? 'Önizlemeyi yenile' : 'Etkileri önizle'}
              </button>
            ) : (
              <>
                <button type="button" className={secondaryButtonClass} onClick={handlePreview} disabled={!canPreview}>Önizlemeyi yenile</button>
                <button type="button" className={primaryButtonClass} onClick={handleSave} disabled={!canSave} title={!canSave ? saveHint ?? undefined : undefined}>
                  {isSaving ? 'Kaydediliyor…' : 'Önizlenen değişikliği kaydet'}
                </button>
              </>
            )}
          </div>
          {initialBaselineAllowed && previewFresh && !baselineConfirmationMatches && finalizedBlockers.length === 0 && (
            <p className="mt-2 text-xs text-slate-600">Önizlemeyi inceledikten sonra {shortDate(props.draft.effectiveFrom)} tarihli başlangıç ücretini ayrıca onaylayın.</p>
          )}
          {initialBaselineAllowed && baselineConfirmationMatches && (
            <p role="status" className="mt-2 text-xs text-emerald-800">Bu tarih ve ücretler için ayrı onay alındı. Kaydetmek son adımdır; tutar veya tarih değişirse onay yenilenir.</p>
          )}
          {saveHint && <p className="mt-1 text-xs text-slate-600">{saveHint}</p>}
          {operationUnchangedSinceSuccess && <p className="mt-1 text-xs text-emerald-800">Bu değişiklik zaten kaydedildi. Yeni değişiklik için tarihi veya tutarları güncelleyin ve yeniden önizleyin.</p>}
        </footer>
      )}
    </section>
  );
};
