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
        title: 'Kapsam içindeki bir ücret politikası okunamıyor.',
        message: 'Bu kayıt doğrulanana kadar ücret geçmişi önizlenemez veya değiştirilemez.',
        detail: `${error.policyVersion.effectiveFrom} tarihli kayıt · ${error.policyVersion.id} · ${quarantineReasonLabel(error.policyVersion.reason)}`,
      };
    case 'ROAD_ONLY_STATE_CONFLICT':
      return {
        title: 'Yalnız yol tutarı değişikliği doğrulanamadı.',
        message: error.reason === 'NON_ROAD_FIELDS_DIFFER'
          ? 'ROAD_ONLY seçildiğinde yol dışındaki tüm ücret alanlarının önceki tam durumla aynı kalması gerekir.'
          : 'Bu değişiklik sonraki ROAD_ONLY sürümlerinin yol dışı alan eşitliğini bozuyor. Sonraki kayıtlar otomatik değiştirilmedi.',
        detail: error.conflictingVersionId ? `Çakışan kayıt: ${error.conflictingVersionId} · önerilen tarih: ${error.proposedEffectiveFrom}` : `Önerilen tarih: ${error.proposedEffectiveFrom}`,
      };
    case 'FINALIZED_BLOCKER':
      return {
        title: 'Kesinleştirilmiş bordro bu değişikliği engelliyor.',
        message: 'Ücret politikası kaydedilmedi. Kesinleşmiş bordro burada yeniden hesaplanmaz; geçmiş düzeltmesi için ayrı retro işlemi gerekir.',
        detail: `${error.blockers.length} kesinleştirilmiş bordro engel oluşturuyor.`,
      };
    case 'RETRO_BLOCKER':
      return {
        title: 'Kesinleşmiş veya settlement uygulanmış retro kayıt değişikliği engelliyor.',
        message: 'Ücret politikası kaydedilmedi. Engelli retro batch veya ödeme olaylarını önce yetkili retro akışında çözümleyin.',
        detail: error.blockers.map((blocker) => `${blocker.batchId}: ${blocker.blockers.map(retroBlockerLabel).join(', ')}`).join(' · '),
      };
    case 'CONFLICT':
      return {
        title: 'Timeline önizlemeden sonra değişti.',
        message: 'Bu kaydetme yapılmadı. Yetkili ücret politikasını yeniden yükleyin ve etkileri tekrar önizleyin.',
        detail: error.message,
      };
    case 'UNSUPPORTED_HISTORICAL_REPLAY':
      return {
        title: 'Bu tarih için olağan ücret politikası değişikliği uygulanamaz.',
        message: 'Geçmiş bordro hakkı normal ileri tarihli timeline düzeltmesiyle değiştirilmez. Geçmiş düzeltmesi için retro işlemi gerekir.',
        detail: error.message,
      };
    case 'INVALID_REQUEST':
    case 'INVALID_POLICY':
      return { title: 'Ücret politikası doğrulanamadı.', message: 'Formdaki alanları ve değişiklik kapsamını gözden geçirin.', detail: error.message };
    case 'UNAUTHORIZED':
      return { title: 'Bu işlemi yapma yetkisi doğrulanamadı.', message: 'Ayarları yeniden yükleyin veya yetkili kullanıcıyla giriş yapın.', detail: error.message };
    case 'NOT_FOUND':
      return { title: 'Bordro hizmet dönemi bulunamadı.', message: 'Dönem seçimini yenileyip tekrar deneyin.', detail: error.message };
    case 'CURRENT_ENGINE_SNAPSHOT_REQUIRED':
      return { title: 'Güncel hesaplama kaynağı eksik.', message: 'Ücret politikası değişikliği bu bordro durumunda uygulanamadı.', detail: error.message };
    case 'PERSISTENCE':
    case 'UNAVAILABLE':
      return { title: 'Ücret politikası işlemi tamamlanamadı.', message: 'Bağlantıyı veya kaydetme durumunu kontrol edip yeniden deneyin.', detail: error.message };
  }
}

function quarantineReasonLabel(reason: Extract<CompensationPolicyApiError, { code: 'QUARANTINED_POLICY' }>['policyVersion']['reason']): string {
  switch (reason) {
    case 'MISSING_BOUNDARY_SCOPE': return 'değişiklik kapsamı eksik';
    case 'MISSING_ROAD_ROUNDING_MODE': return 'yol yuvarlama biçimi eksik';
    case 'UNKNOWN_BOUNDARY_METADATA': return 'tanınmayan kapsam bilgisi';
    case 'INVALID_POLICY_STATE': return 'tam ücret durumu geçersiz';
    case 'INVALID_POLICY_GRAPH': return 'aynı tarihli sürüm bağlantısı geçersiz';
  }
}

function boundaryLabel(scope: CompensationBoundaryScope): string {
  return scope === 'ROAD_ONLY' ? 'Yalnız yol tutarı (ROAD_ONLY)' : 'Tam kazanç değişikliği (FULL_EARNING)';
}

function roundingLabel(mode: RoadRoundingMode): string {
  return mode === 'PER_COMPENSATION_SLICE' ? 'Her tam kazanç diliminde ayrı' : 'Bütün dönemi tek kez';
}

function moneyStateEntries(state: CompensationPolicyStateWire): Array<[string, string, string]> {
  const userValue = (value: string | null): string => value === null ? 'Yok (null)' : formatCompensationPolicyDecimalForUser(value);
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
    <div className="sm:col-span-2 xl:col-span-3">
      <dt className="mb-1 text-slate-600">İş primi grupları</dt>
      <dd>
        {state.isPrimiGruplari.map((group) => (
          <span key={`${group.id}:${group.ad}`} className="mr-3 inline-flex items-center gap-1 text-xs text-slate-800">
            <span className="font-semibold">{group.ad}</span>
            <span className="font-mono">{formatCompensationPolicyDecimalForUser(group.oran)}%</span>
            <span>{group.aktif ? 'etkin' : 'kapalı'}</span>
          </span>
        ))}
      </dd>
    </div>
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
    isPrimiGruplari: 'İş primi grupları',
    geceCalismaPrimiYuzde: 'Gece çalışma primi',
    geceCalismaTatiliPrimiYuzde: 'Gece çalışma tatili primi',
    ekOdeme: 'Ek ödeme',
    digerGelirVarsayilan: 'Diğer gelir varsayılanı',
  };
  return fields.map((field) => labels[field] ?? field);
}

function windowEndLabel(throughInclusive: string | null, nextDate: string | null): string {
  if (throughInclusive) return `${throughInclusive} tarihine kadar (dahil)`;
  if (nextDate) return `${nextDate} tarihinden önceki güne kadar`;
  return 'Dönem kapsamı içinde sonraki sınır görünmüyor';
}

const DirectEffectDetails: React.FC<{ effect: CompensationPolicyDirectEffect }> = ({ effect }) => {
  const earningKind = effect.earningRoundingWindow.kind === 'PERIOD_BASELINE'
    ? 'Dönem başlangıç baseline yuvarlama sınırı'
    : effect.earningRoundingWindow.kind === 'OPENS_FULL_EARNING_SLICE'
      ? 'Bu sürüm yeni kazanç yuvarlama dilimi açıyor'
      : 'Yalnız yol güncelleniyor; mevcut kazanç yuvarlama dilimi sürüyor';
  return (
    <article className="space-y-2 border-t border-slate-200 pt-3 first:border-t-0 first:pt-0">
      <h4 className="text-sm font-bold text-slate-900">{effect.effectiveFrom} tarihinde doğrudan ücret etkisi</h4>
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
  return `${reference.periodId} · bordro ${reference.payrollId} · tahakkuk ${reference.accrualId}`;
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
    case 'CALCULATED_TO_STALE': return 'Hesaplandı → yeniden hesaplanmalı';
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
  return status;
}

function retroBlockerLabel(reason: string): string {
  switch (reason) {
    case 'FINALIZED_BATCH': return 'batch kesinleştirilmiş';
    case 'SETTLED_BATCH': return 'settlement uygulanmış';
    case 'FINALIZED_SOURCE': return 'kaynak bordro kesinleştirilmiş';
    case 'FINALIZED_PAYMENT_EVENT': return 'ödeme olayı kesinleştirilmiş';
    default: return reason;
  }
}

const PreviewPanel: React.FC<{ data: CompensationPolicyPreviewData }> = ({ data }) => (
  <section aria-labelledby="policy-preview-heading" className="space-y-4 border-t border-slate-200 pt-4">
    <div>
      <h3 id="policy-preview-heading" className="text-base font-bold text-slate-900">Önizleme etkileri</h3>
      <p className="mt-1 text-xs text-slate-600">Ücret tutarı penceresi ile kazanç yuvarlama penceresi ayrıdır. Doğrudan dönemler ve bordro bağımlılıkları ayrı listelenir.</p>
    </div>
    {data.finalizedBlockers.length > 0 && (
      <div role="alert" className={errorBoxClass}>
        <h4 className="font-bold">Kesinleştirilmiş bordro değişikliği engelliyor</h4>
        <p className="mt-1">Politika kaydedilemez. Kesinleşmiş bordro burada yeniden hesaplanmaz; geçmiş düzeltmesi için retro işlemi gerekir.</p>
        <ul className="mt-2 list-disc pl-5 text-xs">
          {data.finalizedBlockers.map((reference) => <li key={`${reference.payrollId}:${reference.accrualId}`}>{payrollReferenceLabel(reference)}</li>)}
        </ul>
      </div>
    )}

    {data.retroEffects.length > 0 && (
      <div className="rounded-md border border-indigo-200 bg-indigo-50 p-3">
        <h4 className="font-bold text-indigo-950">Etkilenen retro kayıtları</h4>
        <p className="mt-1 text-xs text-indigo-900">Politika kaydedilirse bu hesaplanmış, henüz settlement uygulanmamış kayıtlar ve bağlı ödeme olayları eski durum olarak korunup STALE yapılır.</p>
        <ul className="mt-2 divide-y divide-indigo-100 text-xs text-indigo-950">
          {data.retroEffects.map((effect) => (
            <li key={effect.batchId} className="py-2">
              <span className="font-semibold">Batch {effect.batchId}</span>
              <span className="block">Kaynak dönemler: {effect.sourcePeriodIds.join(', ') || '—'} · bağlı ödeme olayı: {effect.linkedPaymentEventIds.length}</span>
              <span className="block">Durum: {effect.action === 'STALE_ELIGIBLE' ? 'Politika kaydıyla birlikte STALE yapılacak' : effect.blockers.map(retroBlockerLabel).join(', ')}</span>
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
      ) : <p className="text-sm text-slate-600">Transitif bordro bağımlılığı gösterilmedi.</p>}
    </div>
  </section>
);

const ScopedHistory: React.FC<{ data: CompensationPolicyLoadData }> = ({ data }) => {
  const projection = data.activePolicyProjection;
  return (
    <section aria-labelledby="policy-history-heading" className="space-y-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <div>
          <h2 id="policy-history-heading" className="text-lg font-bold text-slate-900">Bu hizmet dönemi kapsamındaki ücret geçmişi</h2>
          <p className="mt-1 text-xs text-slate-600">Dönem: {data.scope.periodId} · {data.scope.periodStartInclusive}–{data.scope.periodEndInclusive} · as-of: {data.scope.projectionDate}</p>
        </div>
        <p className="text-xs text-slate-500">Bu görünüm yalnız kapsamlı sürümlerdir; kurumun tüm geçmiş timeline'ı değildir.</p>
      </div>

      {data.timeline.resolution === 'MISSING_START_POLICY' && (
        <div className={warningBoxClass} role="status">
          <strong>Dönem başlangıç tarihini kapsayan ücret politikası bulunamadı.</strong>
          <span className="block mt-1">Yeni ücret kaynağı eksik. Önce kullanıcı onaylı tam durumlu başlangıç politikası oluşturulmalıdır.</span>
        </div>
      )}

      <div className="border-l-2 border-slate-200 pl-3">
        <h3 className="text-sm font-bold text-slate-900">As-of günündeki etkin ücret politikası</h3>
        {projection.kind === 'ACTIVE' ? (
          <div className="mt-2 space-y-2">
            <p className="text-xs text-slate-700">{projection.asOfDate} · geçerlilik başlangıcı {projection.version.effectiveFrom} · {boundaryLabel(projection.version.boundaryScope)} · {roundingLabel(projection.version.roadRoundingMode)}</p>
            <PolicyStateDetails state={projection.version.state} />
          </div>
        ) : <p className="mt-1 text-sm text-slate-600">{projection.asOfDate} tarihinde etkin politika yok.</p>}
      </div>

      <div className="space-y-2">
        <h3 className="text-sm font-bold text-slate-900">Scoped sürümler</h3>
        {data.timeline.versions.length === 0 ? (
          <p className="text-sm text-slate-600">Bu dönem kapsamına giren kayıtlı sürüm yok. İlk kullanımda bu boş timeline normaldir; başlangıç sürümü otomatik oluşturulmaz.</p>
        ) : (
          <ol className="divide-y divide-slate-200 border-y border-slate-200">
            {data.timeline.versions.map((version) => (
              <li key={version.id} className="py-3">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div>
                    <h4 className="text-sm font-semibold text-slate-900">{version.effectiveFrom} · {version.reason}</h4>
                    <p className="mt-1 text-xs text-slate-600">{boundaryLabel(version.boundaryScope)} · yol yuvarlama: {roundingLabel(version.roadRoundingMode)}</p>
                    {version.supersedesVersionId && <p className="mt-1 text-xs text-slate-500">Aynı tarihli önceki sürümü düzeltiyor.</p>}
                  </div>
                  <span className="text-xs text-slate-500">{version.createdAt}</span>
                </div>
                <details className="mt-2">
                  <summary className="cursor-pointer text-xs font-semibold text-indigo-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">Tam ücret durumunu göster</summary>
                  <div className="mt-2"><PolicyStateDetails state={version.state} /></div>
                </details>
              </li>
            ))}
          </ol>
        )}
      </div>

      <div className="space-y-2 border-t border-slate-200 pt-3">
        <h3 className="text-sm font-bold text-slate-900">Çözümlenmiş dönem dilimleri</h3>
        {data.timeline.resolvedSegments.length === 0 ? (
          <p className="text-sm text-slate-600">Bu dönem için ücret dilimi çözümlenmedi.</p>
        ) : (
          <ul className="space-y-1 text-xs text-slate-700">
            {data.timeline.resolvedSegments.map((segment, index) => (
              <li key={`${segment.versionId}:${index}`} className="flex flex-wrap justify-between gap-2">
                <span>{segment.startDate}–{segment.endDate} · {boundaryLabel(segment.boundaryScope)} · yol {roundingLabel(segment.roadRoundingMode)}</span>
                <span className="font-mono text-slate-500">{segment.versionId}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
};

function mergedValidation(local: CompensationPolicyDraftValidation, upstream: CompensationPolicyDraftValidation): CompensationPolicyDraftValidation {
  if (local.fullState.status !== 'VALID') return local;
  if (upstream.fullState.status !== 'VALID') return upstream;
  const operationErrors = [...new Set([...local.operationErrors, ...upstream.operationErrors])];
  return { draft: operationErrors.length === 0 ? 'VALID' : 'INVALID', fullState: local.fullState, operationErrors };
}

export const CompensationPolicyPage: React.FC<Props> = (props) => {
  const [scopeSelection, setScopeSelection] = useState<Props['draft']['boundaryScope'] | null>(null);
  const [roundingSelection, setRoundingSelection] = useState<Props['draft']['roadRoundingMode'] | null>(null);
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

  const succeededLoad = props.saveState.status === 'succeeded' ? props.saveState.authoritativeLoad : null;
  const readyLoad = props.loadState.status === 'ready' ? props.loadState.value : null;
  const candidateAuthoritativeLoad = succeededLoad && stableValue(succeededLoad.scope) === stableValue(props.scope)
    ? succeededLoad
    : readyLoad ?? succeededLoad;
  const scopeMatches = Boolean(candidateAuthoritativeLoad && stableValue(candidateAuthoritativeLoad.scope) === stableValue(props.scope));
  const authoritativeLoad = scopeMatches ? candidateAuthoritativeLoad : null;
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
  const scopeSelectionFresh = scopeSelection !== null && scopeSelection === props.draft.boundaryScope;
  const roundingSelectionFresh = roundingSelection !== null && roundingSelection === props.draft.roadRoundingMode;
  const initialBaselineAllowed = Boolean(authoritativeLoad?.initialBaselineRequired && props.draft.operationKind === 'INITIAL_BASELINE');
  const baselineConfirmed = !initialBaselineAllowed || baselineConfirmationMatches;
  const isSaving = props.saveState.status === 'saving' || props.saveState.status === 'reloading-authoritative' || savePending;
  const operationUnchangedSinceSuccess = props.saveState.status === 'succeeded'
    && (savedOperationFingerprint === null || savedOperationFingerprint === currentFingerprint);

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
    if (next.operationKind !== props.draft.operationKind) {
      setScopeSelection(null);
      setRoundingSelection(null);
    }
    setBaselineConfirmation(null);
    setCallbackError(null);
    setShowValidation(false);
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
    if (!correctionTargetFresh) add('Aynı tarih düzeltmesi için mevcut scoped timeline içindeki etkin leaf hedefi eşleşmiyor. Yetkili timeline yüklemesini yenileyin.');
    if (props.draft.operationKind === 'INITIAL_BASELINE' && !initialBaselineAllowed) add('Başlangıç politikası yalnız yetkili yükleme başlangıç policy’si gerektiğini bildirdiğinde oluşturulabilir.');
    if (initialBaselineAllowed && !baselineConfirmationMatches) add('Başlangıç ücret politikası için bu tam durum ve geçerlilik tarihini ayrıca onaylayın.');
    return messages;
  }, [validation, scopeSelectionFresh, roundingSelectionFresh, correctionTargetFresh, props.draft.operationKind, initialBaselineAllowed, baselineConfirmationMatches]);

  const formReady = validation.draft === 'VALID'
    && scopeSelectionFresh
    && roundingSelectionFresh
    && correctionTargetFresh
    && (props.draft.operationKind !== 'INITIAL_BASELINE' || (initialBaselineAllowed && baselineConfirmationMatches));
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
  const canPreview = Boolean(authoritativeLoad)
    && !isSaving
    && !previewBusy
    && props.saveState.status !== 'persisted-reload-failed'
    && !operationUnchangedSinceSuccess;
  const canSave = Boolean(activePreview)
    && previewFresh
    && formReady
    && finalizedBlockers.length === 0
    && !isSaving
    && !previewBusy
    && !operationUnchangedSinceSuccess
    && props.saveState.status !== 'persisted-reload-failed';

  const validateAndFocus = (): boolean => {
    props.onValidateDraft(props.draft);
    if (formReady && currentOperation) return true;
    setShowValidation(true);
    requestAnimationFrame(() => validationSummaryRef.current?.focus());
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
    if (!initialBaselineAllowed || validation.fullState.status !== 'VALID' || !scopeSelectionFresh || !roundingSelectionFresh || !currentOperation) {
      setShowValidation(true);
      requestAnimationFrame(() => validationSummaryRef.current?.focus());
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

  const prefillDraft = () => {
    if (!authoritativeLoad || authoritativeLoad.nonAuthoritativePrefill.kind !== 'AVAILABLE') return;
    try {
      const stateDraft = compensationPolicyStateToDraft(authoritativeLoad.nonAuthoritativePrefill.state);
      changeDraft({ ...props.draft, ...stateDraft, operationKind: 'INITIAL_BASELINE', supersedesVersionId: null });
    } catch (error) {
      setCallbackError(error instanceof Error ? `Taslak ön bilgisi kullanılamadı: ${error.message}` : 'Taslak ön bilgisi exact ücret alanı içermiyor.');
    }
  };

  const copyActivePolicyDraft = () => {
    if (!authoritativeLoad || authoritativeLoad.activePolicyProjection.kind !== 'ACTIVE') return;
    try {
      const stateDraft = compensationPolicyStateToDraft(authoritativeLoad.activePolicyProjection.version.state);
      changeDraft({ ...props.draft, ...stateDraft });
    } catch (error) {
      setCallbackError(error instanceof Error ? `Etkin ücret durumu forma kopyalanamadı: ${error.message}` : 'Etkin ücret durumu exact string içermiyor.');
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
        {error.code === 'CONFLICT' && error.actualTargetState && <p className="mt-1 text-xs">Güncel hedef leaf: {error.actualTargetState.sameDateActiveLeafVersionId ?? 'yok'}.</p>}
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
  const operationValidForConfirm = validation.draft === 'VALID'
    && scopeSelectionFresh
    && roundingSelectionFresh
    && correctionTargetFresh;

  return (
    <main className="mx-auto w-full max-w-7xl space-y-5 p-4 text-slate-900 sm:p-6" aria-labelledby="compensation-policy-title">
      <header className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 pb-4">
        <div>
          <h1 id="compensation-policy-title" className="text-2xl font-bold tracking-tight">Ücret Politikası / Ücret Geçmişi</h1>
          <p className="mt-1 text-sm text-slate-600">Geçerlilik tarihine göre tam ücret durumunu inceleyin, etkileri önizleyin ve açık onayla kaydedin.</p>
          <p className="mt-2 text-xs font-medium text-slate-700">Hizmet dönemi: {props.scope.periodId} · {props.scope.periodStartInclusive}–{props.scope.periodEndInclusive} · as-of {props.scope.projectionDate}</p>
        </div>
        {props.loadState.status === 'idle' && (
          <button type="button" className={secondaryButtonClass} onClick={handleLoad} disabled={loadBusy}>
            {loadBusy ? 'Yükleniyor…' : 'Ücret geçmişini yükle'}
          </button>
        )}
      </header>

      {loadBusy && <p role="status" className="text-sm text-slate-600">Bu hizmet dönemi için ücret politikası yükleniyor…</p>}
      {props.loadState.status === 'error' && props.saveState.status !== 'persisted-reload-failed' && showApiError(props.loadState.error, 'load')}
      {candidateAuthoritativeLoad && !scopeMatches && (
        <div role="alert" className={warningBoxClass}>
          Bu sayfa farklı bir hizmet dönemi bağlamına geçti. Önceki dönemin timeline'ı gösterilmiyor veya değişiklik önizlemesinde kullanılmıyor.
          <button type="button" className={`${secondaryButtonClass} mt-3`} onClick={handleReloadAuthoritative} disabled={reloadPending || loadBusy}>{reloadPending ? 'Yeniden yükleniyor…' : 'Yeni dönem ücret geçmişini yükle'}</button>
        </div>
      )}
      {callbackError && <p role="alert" className={errorBoxClass}>{callbackError}</p>}

      {props.saveState.status === 'saving' && <p role="status" className={warningBoxClass}>Ücret politikası ve etkilenen bordro bağımlılıkları kaydediliyor. Tekrar gönderim engellendi.</p>}
      {props.saveState.status === 'reloading-authoritative' && <p role="status" className={warningBoxClass}>Kayıt tamamlandı; başarıyı göstermeden önce yetkili ücret geçmişi yeniden yükleniyor.</p>}
      {props.saveState.status === 'persisted-reload-failed' && (
        <div className={errorBoxClass} role="alert">
          <h2 className="font-bold">Politika kaydedildi; yetkili görünüm yeniden yüklenemedi.</h2>
          <p className="mt-1">Sürüm {props.saveState.versionId} kalıcı olarak yazıldı. Form taslağı başarı olarak gösterilmez.</p>
          <p className="mt-1 text-xs">{titleCaseApiError(props.saveState.error).message}</p>
          <button type="button" className={`${secondaryButtonClass} mt-3`} onClick={handleReloadAuthoritative} disabled={reloadPending || props.loadState.status === 'loading'}>{reloadPending ? 'Yeniden yükleniyor…' : 'Yetkili ücret geçmişini yeniden yükle'}</button>
        </div>
      )}
      {props.saveState.status === 'succeeded' && operationUnchangedSinceSuccess && (
        <div role="status" className={successBoxClass}>
          <strong>Ücret politikası kaydedildi ve yetkili geçmiş yeniden yüklendi.</strong>
          <span className="block mt-1">{props.saveState.receipt.version.effectiveFrom} · sürüm {props.saveState.receipt.version.id}.</span>
          {props.saveState.receipt.invalidatedRetroBatches.length > 0 && (
            <span className="block mt-1">STALE yapılan retro batch’ler: {props.saveState.receipt.invalidatedRetroBatches.map((item) => item.batchId).join(', ')}.</span>
          )}
          {props.saveState.receipt.invalidatedRetroPaymentEvents.length > 0 && (
            <span className="block mt-1">STALE yapılan bağlı ödeme olayları: {props.saveState.receipt.invalidatedRetroPaymentEvents.map((item) => item.payroll.accrualId).join(', ')}.</span>
          )}
        </div>
      )}
      {props.saveState.status === 'succeeded' && !operationUnchangedSinceSuccess && (
        <p role="status" className="rounded-sm bg-slate-50 p-2 text-sm text-slate-700">Önceki ücret sürümü kaydedildi. Mevcut taslak bu kayıttan sonra değişti; yeni taslağı önizlemeden ve kaydetmeden önceki başarı mesajı güncel taslak için geçerli değildir.</p>
      )}
      {props.saveState.status === 'conflict' && showApiError(props.saveState.error, 'reload')}
      {props.saveState.status === 'blocked' && showApiError(props.saveState.error)}
      {props.saveState.status === 'error' && showApiError(props.saveState.error, props.saveState.error.code === 'CONFLICT' ? 'reload' : 'preview')}

      {authoritativeLoad && <ScopedHistory data={authoritativeLoad} />}

      {showInitialBaselineCard && authoritativeLoad && (
        <section aria-labelledby="initial-baseline-heading" className={warningBoxClass}>
          <h2 id="initial-baseline-heading" className="font-bold">Başlangıç ücret politikası gerekli</h2>
          <p className="mt-1">Dönem başlangıcını kapsayan yetkili ücret politikası yok. Eski ayar arşivi yalnız taslak ön bilgisi olabilir; bu sayfa yüklenirken baseline oluşturulmadı.</p>
          <p className="mt-1 text-xs">{authoritativeLoad.nonAuthoritativePrefill.kind === 'AVAILABLE'
            ? `Taslak ön bilgisi eski dönem ayarları arşivinden (${authoritativeLoad.nonAuthoritativePrefill.sourcePeriodId}) geliyor ve authoritative değildir.`
            : authoritativeLoad.nonAuthoritativePrefill.reason === 'INCOMPLETE_SETTINGS'
              ? 'Mevcut dönem ayarları eksik; eksik ücret alanları otomatik sıfırlanmayacak.'
              : authoritativeLoad.nonAuthoritativePrefill.reason === 'NO_LEGACY_SETTINGS'
                ? 'Eski ücret ayarları bulunamadı; tam durumu kullanıcı girmelidir.'
                : 'Mevcut dönem ücret ayarları prefill olarak verilmedi.'}</p>
          {authoritativeLoad.nonAuthoritativePrefill.kind === 'AVAILABLE' && (
            <button type="button" className={`${secondaryButtonClass} mt-3`} onClick={prefillDraft} disabled={isSaving}>Eski ayar arşivinden taslak hazırla</button>
          )}
          {initialBaselineAllowed && operationValidForConfirm && !baselineConfirmationMatches && (
            <div className="mt-3 border-t border-amber-300 pt-3">
              <p className="text-sm font-semibold">{props.draft.effectiveFrom} tarihli ve aşağıda görünen tam ücret durumunu başlangıç politikası olarak kaydetmek istediğinizi açıkça onaylayın. {props.initialBaselineConfirmationRequired ? 'Bu onay kaydetme işleminden önce zorunludur.' : 'Başlangıç sürümü her durumda ayrı kullanıcı onayı ister.'}</p>
              <button type="button" className={`${primaryButtonClass} mt-2 bg-amber-900 hover:bg-amber-950 focus-visible:outline-amber-800`} onClick={handleConfirmBaseline} disabled={isSaving}>Bu tarihten itibaren geçerli başlangıç ücret politikası olarak onayla</button>
            </div>
          )}
          {initialBaselineAllowed && baselineConfirmationMatches && (
            <div className="mt-3 flex flex-wrap items-center gap-3 border-t border-amber-300 pt-3">
              <p className="text-sm font-semibold">{props.draft.effectiveFrom} tarihi ve bu tam ücret durumu için başlangıç onayı geçerli. Herhangi bir alan veya tarih değişirse yeniden onay gerekir.</p>
              <button type="button" className={secondaryButtonClass} onClick={handleCancelBaseline} disabled={isSaving}>Başlangıç onayını iptal et</button>
            </div>
          )}
        </section>
      )}

      {authoritativeLoad && (
        <section aria-labelledby="policy-edit-heading" className="space-y-4 border-t-2 border-slate-300 pt-4">
          <div>
            <h2 id="policy-edit-heading" className="text-lg font-bold text-slate-900">Ücret politikasını düzenle</h2>
            <p className="mt-1 text-xs text-slate-600">Yeni sürüm tam ücret durumunu taşır. Aynı tarih düzeltmesi mevcut scoped etkin leaf'i belirtir; gelecek tarihteki kaynak state backend önizlemesinde yetkili olarak belirlenir.</p>
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
            scopeSelection={scopeSelection}
            roundingSelection={roundingSelection}
            onDraftChange={changeDraft}
            onSelectBoundaryScope={selectBoundaryScope}
            onSelectRoadRoundingMode={selectRoadMode}
            onCopyActiveState={copyActivePolicyDraft}
            canApplyPercentageIncrease={validation.fullState.status === 'VALID' && !disabledForm}
          />

          {props.previewState.status === 'error' && showApiError(props.previewState.error, 'preview')}
          {activePreview && previewStale && (
            <p role="status" className={warningBoxClass}>Bu önizleme başka bir taslak, tarih veya kapsam içindi. Kaydetme kapalı; güncel taslağın etkilerini yeniden önizleyin.</p>
          )}
          {activePreview && previewFresh && <PreviewPanel data={activePreview} />}

          {props.previewState.status === 'loading' && <p role="status" className={warningBoxClass}>Etkiler, tarih pencereleri ve bordro bağımlılıkları yetkili timeline üzerinden hesaplanıyor…</p>}
          {props.previewState.status === 'ready' && !activePreview && <p className="text-sm text-slate-600">Önizleme sonucu bulunmuyor.</p>}

          <div className="flex flex-wrap items-center gap-3 border-t border-slate-200 pt-4">
            <button type="button" className={primaryButtonClass} onClick={handlePreview} disabled={!canPreview} title={!formReady ? 'Tüm alanları tamamlayın, kapsamları açıkça seçin ve gerekiyorsa baseline onayı verin.' : undefined}>
              {previewBusy ? 'Etkiler yükleniyor…' : 'Değişikliği ve etkileri önizle'}
            </button>
            <button type="button" className={secondaryButtonClass} onClick={handleSave} disabled={!canSave} title={finalizedBlockers.length > 0 ? 'Kesinleştirilmiş bordro engeli varken kaydedilemez.' : !previewFresh ? 'Önce bu tam taslak için güncel önizleme alın.' : undefined}>
              {isSaving ? 'Kaydediliyor…' : 'Önizlenen ücret politikasını kaydet'}
            </button>
            {!formReady && <p className="text-xs text-slate-600">Kaydetme için tam ve geçerli state, açık kapsam seçimleri, güncel baseline onayı ve geçerli hedef gerekir.</p>}
            {finalizedBlockers.length > 0 && <p className="text-xs font-semibold text-rose-800">Kesinleştirilmiş bordro engeli kaydetmeyi kapattı. Normal yeniden hesaplama seçeneği yoktur.</p>}
            {operationUnchangedSinceSuccess && <p className="text-xs text-emerald-800">Bu değişiklik zaten kaydedildi. Yeni değişiklik için tarihi veya tam durumu güncelleyin ve yeniden önizleyin.</p>}
          </div>
        </section>
      )}
    </main>
  );
};
