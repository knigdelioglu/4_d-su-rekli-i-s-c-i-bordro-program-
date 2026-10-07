import type {
  CompensationPolicyApi,
  CompensationPolicyApiError,
  CompensationPolicyDependencyEffect,
  CompensationPolicyDirectEffect,
  CompensationPolicyField,
  CompensationPolicyLoadData,
  CompensationPolicyLoadRequest,
  CompensationPolicyOperationResult,
  CompensationPolicyPayrollReference,
  CompensationPolicyPayrollStateToken,
  CompensationPolicyPreviewData,
  CompensationPolicyPreviewRequest,
  CompensationPolicyPreviewToken,
  CompensationPolicyRetroDependencyStateToken,
  CompensationPolicyRetroEffect,
  CompensationPolicySaveReceipt,
  CompensationPolicySaveRequest,
  CompensationPolicyStateWire,
  CompensationPolicyVersionWire,
  ResolvedCompensationSegmentWire,
} from './contract';
import { compensationPolicyStateEqualsExceptRoad } from './contract';
import { tauriBridge, tauriCompensationPolicyApi } from '../tauriBridge';
import { assertExactDecimalDto, type PayrollStorageDto } from '../payrollEngine/decimalBoundary';
import type { PayrollDatasetSnapshotModel, PayrollKey, PayrollMutation } from '../payrollEngine/types';
import { getWasmRuntime } from '../payrollEngine/wasmRuntime';
import { BrowserSnapshotConflictError, browserPayrollStore } from '../storage/browserPayrollStore';
import { parseCurrentBrowserSnapshot } from '../storage/payrollPayload';

type BrowserPayload = PayrollStorageDto;
type BrowserPayroll = BrowserPayload['bordrolar'][number];
type CoreImpact = {
  affectedPayrolls: PayrollKey[];
  blockedByFinalized: PayrollKey[];
  affectedRetroBatches: string[];
  blockedByFinalizedRetroBatches: string[];
  payrollCauses?: Array<{ payroll: PayrollKey; causePeriodId: string }>;
  retroEffects?: CompensationPolicyRetroEffect[];
};

class PolicyApiFailure extends Error {
  constructor(readonly apiError: CompensationPolicyApiError) {
    super(apiError.message);
    this.name = 'PolicyApiFailure';
  }
}

function success<T>(value: T): CompensationPolicyOperationResult<T> {
  return { ok: true, value };
}

function failure<T>(error: CompensationPolicyApiError): CompensationPolicyOperationResult<T> {
  return { ok: false, error };
}

function stable(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stable);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.keys(value).sort().map((key) => [key, stable((value as Record<string, unknown>)[key])])
  );
}

function same(left: unknown, right: unknown): boolean {
  return JSON.stringify(stable(left)) === JSON.stringify(stable(right));
}

function errorResult<T>(error: unknown): CompensationPolicyOperationResult<T> {
  if (error instanceof PolicyApiFailure) return failure(error.apiError);
  if (error instanceof BrowserSnapshotConflictError) {
    return failure({
      code: 'CONFLICT',
      message: `Tarayıcı snapshotı değişti (beklenen revizyon ${error.expectedRevision}, mevcut ${error.actualRevision}); verileri yeniden yükleyin.`,
    });
  }
  return failure({ code: 'UNAVAILABLE', message: error instanceof Error ? error.message : String(error) });
}

function raise(code: CompensationPolicyApiError['code'], message: string, extra = {}): never {
  throw new PolicyApiFailure({ code, message, ...extra } as CompensationPolicyApiError);
}

function readSnapshot(payload: BrowserPayload) {
  return {
    personnel: payload.personeller,
    periods: payload.donemler,
    institutionSettings: payload.kurumDegerleriMap,
    attendances: payload.puantajlar,
    payrolls: payload.bordrolar,
    taxOpenings: payload.taxOpenings,
    sickLeaveRecords: payload.sickLeaveRecords,
    annualPayrollParameters: payload.annualPayrollParameters,
    compensationRevisions: payload.compensationRevisions,
    compensationRevisionOverrides: payload.compensationRevisionOverrides,
    retroBatches: payload.retroBatches,
    retroAllocations: payload.retroAllocations,
    compensationPolicyVersions: payload.compensationPolicyVersions,
  } as unknown as PayrollDatasetSnapshotModel;
}

async function loadBrowserPayload(): Promise<BrowserPayload> {
  const snapshot = await browserPayrollStore.loadSnapshot();
  if (!snapshot) raise('UNAVAILABLE', 'Yetkili tarayıcı bordro snapshotı bulunamadı.');
  return parseCurrentBrowserSnapshot(snapshot.payload);
}

function validateScope(payload: BrowserPayload, scope: CompensationPolicyLoadRequest['scope']) {
  if (scope.institutionScope !== 'CURRENT_INSTITUTION') {
    raise('INVALID_REQUEST', 'Yalnız CURRENT_INSTITUTION kapsamı desteklenir.');
  }
  const period = payload.donemler.find((item) => item.id === scope.periodId);
  if (!period) raise('NOT_FOUND', `Bordro dönemi bulunamadı: ${scope.periodId}`);
  if (
    period.baslangicTarihi !== scope.periodStartInclusive ||
    period.bitisTarihi !== scope.periodEndInclusive
  ) {
    raise('CONFLICT', 'Dönem tarihleri yetkili tarayıcı snapshotı ile eşleşmiyor.');
  }
  if (
    !isCanonicalDate(scope.projectionDate) ||
    scope.projectionDate < period.baslangicTarihi ||
    scope.projectionDate > period.bitisTarihi
  ) {
    raise('INVALID_REQUEST', 'Projection tarihi seçilen dönem aralığında olmalıdır.');
  }
  return period;
}

export function legacySettingsToPolicyPrefillOnly(
  payload: BrowserPayload,
  periodId: string
): CompensationPolicyLoadData['nonAuthoritativePrefill'] {
  // This named adapter is the only browser path that reads raw historical
  // compensation settings. It creates a user-visible draft only; it never
  // writes or promotes these values to policy authority.
  const settings = payload.legacyInstitutionSettings?.[periodId] as
    | Record<string, unknown>
    | undefined;
  if (!settings) return { kind: 'UNAVAILABLE', reason: 'NO_LEGACY_SETTINGS' };

  const requiredDecimalFields = [
    'gunlukTabanUcret',
    'gunlukYemek',
    'birlestirilmisSosyalYardim',
    'gunlukVasitaYol',
    'giyimYardimi',
    'hizmetZammiBirimi',
    'geceCalismaPrimiYuzde',
    'geceCalismaTatiliPrimiYuzde',
  ] as const;
  const nullableDecimalFields = ['isPrimiYuzde', 'ekOdeme', 'digerGelirVarsayilan'] as const;
  const hasOwn = (key: string) => Object.prototype.hasOwnProperty.call(settings, key);
  if (requiredDecimalFields.some((field) => !hasOwn(field) || typeof settings[field] !== 'string')) {
    return { kind: 'UNAVAILABLE', reason: 'INCOMPLETE_SETTINGS' };
  }
  if (nullableDecimalFields.some((field) =>
    !hasOwn(field) || (settings[field] !== null && typeof settings[field] !== 'string')
  )) {
    return { kind: 'UNAVAILABLE', reason: 'INCOMPLETE_SETTINGS' };
  }
  const state: Partial<CompensationPolicyStateWire> = {};
  for (const field of [...requiredDecimalFields, ...nullableDecimalFields]) {
    (state as Record<string, unknown>)[field] = settings[field];
  }
  if (!Array.isArray(settings.isPrimiGruplari) || settings.isPrimiGruplari.length === 0 || settings.isPrimiGruplari.some((group) => {
    if (!group || typeof group !== 'object') return true;
    const value = group as Record<string, unknown>;
    return typeof value.id !== 'string' || typeof value.ad !== 'string' || typeof value.oran !== 'string' || typeof value.aktif !== 'boolean';
  })) {
    return { kind: 'UNAVAILABLE', reason: 'INCOMPLETE_SETTINGS' };
  }
  state.isPrimiGruplari = settings.isPrimiGruplari.map((group) => {
    const value = group as Record<string, unknown>;
    return {
      id: value.id as string,
      ad: value.ad as string,
      oran: value.oran as string,
      aktif: value.aktif as boolean,
    };
  });
  return {
    kind: 'AVAILABLE',
    authority: 'PREFILL_ONLY',
    source: 'LEGACY_INSTITUTION_SETTINGS',
    sourcePeriodId: periodId,
    state,
  };
}

function isCanonicalDate(value: string): boolean {
  return /^\d{4}-\d{2}-\d{2}$/.test(value) && !Number.isNaN(Date.parse(`${value}T00:00:00Z`));
}

function orderedVersions(versions: CompensationPolicyVersionWire[]): CompensationPolicyVersionWire[] {
  const byDate = new Map<string, CompensationPolicyVersionWire[]>();
  for (const version of versions) {
    const group = byDate.get(version.effectiveFrom) ?? [];
    group.push(version);
    byDate.set(version.effectiveFrom, group);
  }
  const output: CompensationPolicyVersionWire[] = [];
  for (const [, group] of [...byDate].sort(([left], [right]) => left.localeCompare(right))) {
    const child = new Map<string, CompensationPolicyVersionWire>();
    const roots = group.filter((version) => version.supersedesVersionId === null);
    if (roots.length !== 1) raise('INVALID_POLICY', 'Policy supersession grafiğinde tek kök bulunamadı.');
    for (const version of group) {
      if (version.supersedesVersionId) child.set(version.supersedesVersionId, version);
    }
    const seen = new Set<string>();
    let cursor: CompensationPolicyVersionWire | undefined = roots[0];
    while (cursor) {
      if (seen.has(cursor.id)) raise('INVALID_POLICY', 'Policy supersession grafiği döngülü.');
      seen.add(cursor.id);
      output.push(cursor);
      cursor = child.get(cursor.id);
    }
    if (seen.size !== group.length) raise('INVALID_POLICY', 'Policy supersession grafiği kopuk.');
  }
  return output;
}

function activeVersions(versions: CompensationPolicyVersionWire[]): CompensationPolicyVersionWire[] {
  const ordered = orderedVersions(versions);
  const superseded = new Set(ordered.flatMap((version) => version.supersedesVersionId ?? []));
  return ordered.filter((version) => !superseded.has(version.id));
}

function targetState(
  versions: CompensationPolicyVersionWire[],
  effectiveFrom: string
): CompensationPolicyPreviewToken['targetState'] {
  const active = activeVersions(versions);
  return {
    effectiveFrom,
    activeVersionIdAtDate: active.filter((version) => version.effectiveFrom <= effectiveFrom).at(-1)?.id ?? null,
    sameDateActiveLeafVersionId: active.find((version) => version.effectiveFrom === effectiveFrom)?.id ?? null,
  };
}

function sameExceptRoad(left: CompensationPolicyStateWire, right: CompensationPolicyStateWire): boolean {
  return compensationPolicyStateEqualsExceptRoad(left, right);
}

function validateOperation(
  payload: BrowserPayload,
  period: BrowserPayload['donemler'][number],
  operation: CompensationPolicyPreviewRequest['operation'],
  runtime: Awaited<ReturnType<typeof getWasmRuntime>>
): CompensationPolicyVersionWire[] {
  const versions = orderedVersions(payload.compensationPolicyVersions);
  if (!isCanonicalDate(operation.effectiveFrom) ||
      operation.effectiveFrom < period.baslangicTarihi ||
      operation.effectiveFrom > period.bitisTarihi) {
    raise('INVALID_REQUEST', 'effectiveFrom yetkili dönem içinde olmalıdır.');
  }
  if (!operation.reason.trim()) raise('INVALID_REQUEST', 'Değişiklik gerekçesi zorunludur.');
  if (
    operation.kind !== 'INITIAL_BASELINE' &&
    ((operation.boundaryScope === 'ROAD_ONLY' && operation.changeIntent !== 'ROAD_AMOUNT_ONLY') ||
      (operation.boundaryScope === 'FULL_EARNING' && operation.changeIntent !== 'FULL_STATE_CHANGE'))
  ) {
    raise('INVALID_REQUEST', 'boundaryScope ile changeIntent eşleşmiyor.');
  }
  const active = activeVersions(versions);
  if (operation.kind === 'INITIAL_BASELINE' && versions.length > 0) {
    raise('CONFLICT', 'Başlangıç baseline yalnız boş policy geçmişine eklenebilir.', {
      actualTargetState: targetState(versions, operation.effectiveFrom),
    });
  }
  if (operation.kind === 'NEW_EFFECTIVE_DATE' && active.some((item) => item.effectiveFrom === operation.effectiveFrom)) {
    raise('CONFLICT', 'Bu tarihte zaten etkin policy var; aynı tarih düzeltmesi seçilmelidir.', {
      actualTargetState: targetState(versions, operation.effectiveFrom),
    });
  }
  if (operation.kind === 'SAME_DATE_CORRECTION') {
    const leaf = active.find((item) => item.effectiveFrom === operation.effectiveFrom);
    if (!leaf || leaf.id !== operation.supersedesVersionId) {
      raise('CONFLICT', 'Düzeltme isteği aynı tarihin güncel etkin yaprağını hedeflemiyor.', {
        actualTargetState: targetState(versions, operation.effectiveFrom),
      });
    }
  }
  if (operation.boundaryScope === 'ROAD_ONLY') {
    const previous = active.filter((version) => version.effectiveFrom < operation.effectiveFrom).at(-1);
    if (previous && !sameExceptRoad(previous.state, operation.state)) {
      raise('ROAD_ONLY_STATE_CONFLICT', 'ROAD_ONLY yalnız yol tutarını değiştirebilir.', {
        proposedEffectiveFrom: operation.effectiveFrom,
        conflictingVersionId: previous.id,
        reason: 'NON_ROAD_FIELDS_DIFFER',
      });
    }
  }
  const candidate: CompensationPolicyVersionWire = {
    id: '__compensation-policy-preview__',
    effectiveFrom: operation.effectiveFrom,
    state: operation.state,
    boundaryScope: operation.boundaryScope,
    roadRoundingMode: operation.roadRoundingMode,
    createdAt: '2000-01-01T00:00:00Z',
    source: 'COMPENSATION_POLICY_PREVIEW',
    reason: operation.reason.trim(),
    supersedesVersionId: operation.kind === 'SAME_DATE_CORRECTION' ? operation.supersedesVersionId : null,
  };
  try {
    runtime.resolve_compensation_policy_timeline_json(JSON.stringify({
      startInclusive: period.baslangicTarihi,
      endInclusive: period.bitisTarihi,
      versions: [...versions, candidate],
    }));
  } catch (error) {
    const descendantRoadOnly = operation.kind === 'SAME_DATE_CORRECTION'
      ? active.find((version) => version.effectiveFrom > operation.effectiveFrom && version.boundaryScope === 'ROAD_ONLY')
      : undefined;
    if (descendantRoadOnly) {
      raise('ROAD_ONLY_STATE_CONFLICT', String(error), {
        proposedEffectiveFrom: operation.effectiveFrom,
        conflictingVersionId: descendantRoadOnly.id,
        reason: 'DESCENDANT_ROAD_ONLY_INVALIDATED',
      });
    }
    raise('INVALID_POLICY', String(error));
  }
  return versions;
}

function directEffects(
  payload: BrowserPayload,
  operation: CompensationPolicyPreviewRequest['operation'],
  versions: CompensationPolicyVersionWire[]
): CompensationPolicyDirectEffect[] {
  const date = operation.effectiveFrom;
  const active = activeVersions(versions);
  const next = active.find((version) => version.effectiveFrom > date);
  const through = next ? addDays(next.effectiveFrom, -1) : null;
  const nextFull = active.find((version) =>
    version.effectiveFrom > date && version.boundaryScope === 'FULL_EARNING'
  );
  const previous = operation.kind === 'SAME_DATE_CORRECTION'
    ? active.find((version) => version.effectiveFrom === date)
    : active.filter((version) => version.effectiveFrom < date).at(-1);
  const allFields = Object.keys(operation.state) as Array<keyof CompensationPolicyStateWire>;
  const changedFields: CompensationPolicyField[] = operation.boundaryScope === 'ROAD_ONLY'
    && previous
    && compensationPolicyStateEqualsExceptRoad(previous.state, operation.state)
    ? ['gunlukVasitaYol']
    : allFields.filter((field) => !previous || !same(previous.state[field], operation.state[field]));
  const roundingThrough = operation.kind === 'INITIAL_BASELINE'
    ? (nextFull ? addDays(nextFull.effectiveFrom, -1) : null)
    : (nextFull ? addDays(nextFull.effectiveFrom, -1) : null);
  const maximum = through ?? '9999-12-31';
  const affectedPeriods = payload.donemler
    .filter((period) => period.bitisTarihi >= date && period.baslangicTarihi <= maximum)
    .map((period) => ({
      periodId: period.id,
      startInclusive: period.baslangicTarihi,
      endInclusive: period.bitisTarihi,
    }));
  return [{
    effectiveFrom: date,
    policyStateWindow: { throughInclusive: through, nextEffectiveDate: next?.effectiveFrom ?? null },
    earningRoundingWindow: {
      kind: operation.kind === 'INITIAL_BASELINE'
        ? 'PERIOD_BASELINE'
        : operation.boundaryScope === 'FULL_EARNING'
          ? 'OPENS_FULL_EARNING_SLICE'
          : 'ROAD_ONLY_WITHIN_ACTIVE_SLICE',
      throughInclusive: roundingThrough,
      nextFullEarningDate: nextFull?.effectiveFrom ?? null,
    },
    boundaryScope: operation.boundaryScope,
    roadRoundingMode: operation.roadRoundingMode,
    changedFields,
    affectedPeriods,
  }];
}

function addDays(iso: string, days: number): string {
  const date = new Date(`${iso}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

function payrollKey(payroll: BrowserPayroll): PayrollKey {
  return { personnelId: payroll.personelId, periodId: payroll.donemId, accrualId: payroll.accrualId };
}

function keyString(key: PayrollKey): string {
  return `${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`;
}

function payrollReference(payroll: BrowserPayroll): CompensationPolicyPayrollReference {
  return {
    payrollId: payroll.id,
    personnelId: payroll.personelId,
    periodId: payroll.donemId,
    accrualId: payroll.accrualId,
  };
}

function stateToken(payroll: BrowserPayroll): CompensationPolicyPayrollStateToken {
  return {
    payrollId: payroll.id,
    personnelId: payroll.personelId,
    periodId: payroll.donemId,
    accrualId: payroll.accrualId,
    status: payroll.status,
    updatedAt: payroll.sonGuncellemeTarihi,
    calculationProvenance: payroll.calculationMetadata ? 'COMPENSATION_POLICY_ENGINE' : 'LEGACY',
    snapshotSchemaVersion: payroll.calculationMetadata?.snapshot?.snapshotSchemaVersion ?? null,
  };
}

function retroDependencyStateTokens(
  payload: BrowserPayload,
  effects: CompensationPolicyRetroEffect[]
): CompensationPolicyRetroDependencyStateToken[] {
  const orderedEffects = [...effects].sort((left, right) => left.batchId.localeCompare(right.batchId));
  return orderedEffects.map((effect) => {
    const batch = payload.retroBatches.find((item) => item.id === effect.batchId);
    if (!batch) raise('UNSUPPORTED_HISTORICAL_REPLAY', effect.batchId + ' retro batch bulunamadı.');
    if (batch.status !== 'CALCULATED') {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' retro batch CALCULATED değil; replay token üretilemedi.');
    }
    const allocations = payload.retroAllocations
      .filter((item) => item.batchId === batch.id)
      .sort((left, right) => left.id.localeCompare(right.id));
    if (!allocations.length) {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' retro batch allocations bulunamadı.');
    }
    if (allocations.some((item) => item.personnelId !== batch.personnelId)) {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' retro allocation personel kimliği tutarsız.');
    }
    const sourcePeriodIds = [...new Set(allocations.map((item) => item.sourcePeriodId))].sort();
    if (!same(sourcePeriodIds, effect.sourcePeriodIds)) {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' source period tokenı kapanışla eşleşmiyor.');
    }
    const revision = payload.compensationRevisions.find((item) => item.id === batch.revisionId);
    if (!revision) raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' retro revision kaydı bulunamadı.');
    const overrides = payload.compensationRevisionOverrides
      .filter((item) => item.revisionId === revision.id)
      .sort((left, right) => left.id.localeCompare(right.id));
    const sourcePeriods = sourcePeriodIds
      .map((periodId) => payload.donemler.find((period) => period.id === periodId))
      .sort((left, right) => (left?.id ?? '').localeCompare(right?.id ?? ''));
    if (sourcePeriods.some((period) => !period)) {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' kaynak dönemi bulunamadı.');
    }
    const concreteSourcePeriods = sourcePeriods as NonNullable<(typeof sourcePeriods)[number]>[];
    const sourcePeriodSet = new Set(sourcePeriodIds);
    const sourcePayrolls = payload.bordrolar
      .filter((payroll) => payroll.personelId === batch.personnelId && sourcePeriodSet.has(payroll.donemId))
      .sort((left, right) =>
        left.donemId.localeCompare(right.donemId) ||
        left.accrualId.localeCompare(right.accrualId) ||
        left.id.localeCompare(right.id)
      );
    const legacySourcePeriodIds = new Set<string>();
    for (const period of concreteSourcePeriods) {
      const normals = sourcePayrolls.filter((payroll) =>
        payroll.donemId === period.id &&
        payroll.accrualType === 'NORMAL' &&
        (payroll.status === 'CALCULATED' || payroll.status === 'FINALIZED')
      );
      if (normals.length !== 1) {
        raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' ' + period.id + ' için tek authoritative NORMAL kaydı bulunamadı.');
      }
      const normal = normals[0];
      if (normal.status !== 'CALCULATED') {
        raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' ' + period.id + ' NORMAL kaydı CALCULATED değil.');
      }
      if (!normal.calculationMetadata) {
        legacySourcePeriodIds.add(period.id);
      } else if (
        normal.calculationMetadata.snapshot?.snapshotSchemaVersion !== 2 ||
        normal.calculationMetadata.snapshot.compensation.segments.length === 0
      ) {
        raise('UNSUPPORTED_HISTORICAL_REPLAY', normal.accrualId + ' için geçerli schema-2 snapshot bulunamadı.');
      }
    }
    const sourceSettings: CompensationPolicyRetroDependencyStateToken['persistedState']['sourceSettings'] = {};
    let sourcePersonnel: BrowserPayload['personeller'][number] | null = null;
    let sourceAttendances: BrowserPayload['puantajlar'] = [];
    let sourceSickLeaveRecords: BrowserPayload['sickLeaveRecords'] = [];
    let legacyHistoryPeriods: BrowserPayload['donemler'] = [];
    if (legacySourcePeriodIds.size) {
      raise(
        'UNSUPPORTED_HISTORICAL_REPLAY',
        batch.id + ' snapshotless NORMAL source cannot be replayed from mutable settings or attendance.'
      );
    }
    const linkedPaymentEvents = payload.bordrolar
      .filter((payroll) => effect.linkedPaymentEventIds.includes(payroll.id))
      .sort((left, right) => left.id.localeCompare(right.id));
    if (linkedPaymentEvents.length !== effect.linkedPaymentEventIds.length) {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' linked payment event tokenı eksik.');
    }
    const paymentPeriodIds = new Set(linkedPaymentEvents.map((event) => event.donemId));
    const paymentPeriods = payload.donemler
      .filter((period) => paymentPeriodIds.has(period.id))
      .sort((left, right) => left.id.localeCompare(right.id));
    if (paymentPeriods.length !== paymentPeriodIds.size) {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', batch.id + ' payment period tokenı eksik.');
    }
    return {
      batchId: batch.id,
      sourcePeriodIds,
      linkedPaymentEventIds: [...effect.linkedPaymentEventIds].sort(),
      persistedState: {
        batch,
        allocations,
        revision,
        overrides,
        sourcePayrolls,
        sourcePeriods: concreteSourcePeriods,
        paymentEvents: linkedPaymentEvents,
        paymentPeriods,
        sourceSettings,
        sourcePersonnel,
        sourceAttendances,
        sourceSickLeaveRecords,
        legacyHistoryPeriods,
        sourceZamAylari: [],
      },
    };
  });
}

function calculatePreview(
  payload: BrowserPayload,
  scope: CompensationPolicyPreviewRequest['scope'],
  operation: CompensationPolicyPreviewRequest['operation'],
  runtime: Awaited<ReturnType<typeof getWasmRuntime>>
): CompensationPolicyPreviewData {
  const period = validateScope(payload, scope);
  const versions = validateOperation(payload, period, operation, runtime);
  const directCompensationEffects = directEffects(payload, operation, versions);
  const directPeriodIds = new Set(directCompensationEffects.flatMap((effect) => effect.affectedPeriods.map((item) => item.periodId)));
  const dataset = readSnapshot(payload);
  assertExactDecimalDto(dataset);
  const payrollByKey = new Map(payload.bordrolar.map((payroll) => [keyString(payrollKey(payroll)), payroll]));
  const causes = new Map<string, Set<string>>();
  const affected = new Map<string, PayrollKey>();
  const blocked = new Map<string, PayrollKey>();
  const effect = directCompensationEffects[0];
  if (!effect) raise('INVALID_POLICY', 'Politika doğrudan etki aralığı üretmedi.');
  const mutation: PayrollMutation = {
    kind: 'COMPENSATION_POLICY',
    effectiveFrom: effect.effectiveFrom,
    throughInclusive: effect.policyStateWindow.throughInclusive,
  };
  let impact: CoreImpact;
  try {
    impact = JSON.parse(runtime.evaluate_mutation_policy_json(JSON.stringify({ dataset, mutation }))) as CoreImpact;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.includes('UNSUPPORTED_HISTORICAL_REPLAY')) {
      raise('UNSUPPORTED_HISTORICAL_REPLAY', message);
    }
    throw error;
  }
  if (!Array.isArray(impact.payrollCauses)) {
    raise('INVALID_POLICY', 'Paylaşılan payroll-core compensation dependency nedenlerini sağlamadı.');
  }
  if (!Array.isArray(impact.retroEffects)) {
    raise('INVALID_POLICY', 'Paylaşılan payroll-core retro dependency etkilerini sağlamadı.');
  }
  impact.affectedPayrolls.forEach((key) => affected.set(keyString(key), key));
  impact.payrollCauses.forEach(({ payroll, causePeriodId }) => {
    const id = keyString(payroll);
    const roots = causes.get(id) ?? new Set<string>();
    roots.add(causePeriodId);
    causes.set(id, roots);
  });
  impact.blockedByFinalized.forEach((key) => blocked.set(keyString(key), key));
  const retroEffects = impact.retroEffects;
  const retroBlockers = retroEffects.filter((item) => item.action === 'IMMUTABLE_BLOCKER');
  if (retroBlockers.length) {
    raise('RETRO_BLOCKER', 'Değişiklik kesinleşmiş veya settlement uygulanmış retro tarihçesini etkiliyor.', {
      blockers: retroBlockers,
    });
  }
  if (blocked.size) {
    const blockers = [...blocked.values()].flatMap((key) => {
      const payroll = payrollByKey.get(keyString(key));
      return payroll ? [payrollReference(payroll)] : [];
    });
    raise('FINALIZED_BLOCKER', 'Değişiklik kesinleştirilmiş bordro tarihçesini etkiliyor.', { blockers });
  }
  const orderedAffected = [...affected.values()].sort((left, right) => keyString(left).localeCompare(keyString(right)));
  const dependencyPayrollStates = orderedAffected.flatMap((key) => {
    const payroll = payrollByKey.get(keyString(key));
    return payroll ? [stateToken(payroll)] : [];
  });
  const retroDependencyStates = retroDependencyStateTokens(payload, retroEffects);
  const periodById = new Map(payload.donemler.map((item) => [item.id, item]));
  const dependencyClosureEffects: CompensationPolicyDependencyEffect[] = [];
  for (const key of orderedAffected) {
    const payroll = payrollByKey.get(keyString(key));
    if (!payroll) continue;
    const action = payroll.status === 'CALCULATED'
      ? { action: 'CALCULATED_TO_STALE' as const, toStatus: 'STALE' as const }
      : payroll.status === 'STALE'
        ? { action: 'ALREADY_STALE' as const, toStatus: null }
        : { action: 'NO_MUTATION' as const, toStatus: null };
    for (const causePeriodId of causes.get(keyString(key)) ?? []) {
      const cause = periodById.get(causePeriodId);
      const candidate = periodById.get(payroll.donemId);
      const kind = payroll.donemId === causePeriodId || directPeriodIds.has(payroll.donemId)
        ? 'PAYMENT'
        : payroll.devredenPekGelen?.some((item) => item.kaynakDonemId === causePeriodId)
          ? 'CARRY'
          : cause && candidate && cause.taxYear === candidate.taxYear
            ? 'GV'
            : 'PEK';
      dependencyClosureEffects.push({
        kind,
        causePeriodId,
        affectedPayroll: payrollReference(payroll),
        fromStatus: payroll.status,
        toStatus: action.toStatus,
        action: action.action,
      });
    }
  }
  const ordered = orderedVersions(payload.compensationPolicyVersions);
  return {
    directCompensationEffects,
    dependencyClosureEffects,
    retroEffects,
    finalizedBlockers: [],
    previewToken: {
      operation,
      targetState: targetState(versions, operation.effectiveFrom),
      policyTimelineVersionIds: ordered.map((version) => version.id),
      dependencyPayrollStates,
      retroDependencyStates,
    },
  };
}

async function loadBrowserPolicy(
  request: CompensationPolicyLoadRequest
): Promise<CompensationPolicyOperationResult<CompensationPolicyLoadData>> {
  try {
    const payload = await loadBrowserPayload();
    const period = validateScope(payload, request.scope);
    const ordered = orderedVersions(payload.compensationPolicyVersions);
    const active = activeVersions(ordered);
    const baselineDate = active.filter((version) => version.effectiveFrom <= period.baslangicTarihi).at(-1)?.effectiveFrom;
    const scoped = ordered.filter((version) =>
      version.effectiveFrom === baselineDate ||
      (version.effectiveFrom > period.baslangicTarihi && version.effectiveFrom <= period.bitisTarihi)
    );
    let resolvedSegments: ResolvedCompensationSegmentWire[] = [];
    let resolution: CompensationPolicyLoadData['timeline']['resolution'] = 'RESOLVED';
    if (!baselineDate) {
      resolution = 'MISSING_START_POLICY';
    } else {
      const runtime = await getWasmRuntime();
      try {
        resolvedSegments = JSON.parse(runtime.resolve_compensation_policy_timeline_json(JSON.stringify({
          startInclusive: period.baslangicTarihi,
          endInclusive: period.bitisTarihi,
          versions: scoped,
        }))) as ResolvedCompensationSegmentWire[];
      } catch (error) {
        raise('INVALID_POLICY', String(error));
      }
    }
    const projection = active.filter((version) => version.effectiveFrom <= request.scope.projectionDate).at(-1);
    return success({
      scope: request.scope,
      timeline: { versions: scoped, resolvedSegments, resolution },
      activePolicyProjection: projection
        ? { kind: 'ACTIVE', asOfDate: request.scope.projectionDate, version: projection }
        : { kind: 'MISSING', asOfDate: request.scope.projectionDate },
      initialBaselineRequired: ordered.length === 0,
      nonAuthoritativePrefill: legacySettingsToPolicyPrefillOnly(payload, period.id),
    });
  } catch (error) {
    return errorResult(error);
  }
}

const browserCompensationPolicyApi: CompensationPolicyApi = {
  load: loadBrowserPolicy,

  async preview(request: CompensationPolicyPreviewRequest) {
    try {
      const [payload, runtime] = await Promise.all([loadBrowserPayload(), getWasmRuntime()]);
      return success(calculatePreview(payload, request.scope, request.operation, runtime));
    } catch (error) {
      return errorResult(error);
    }
  },

  async save(request: CompensationPolicySaveRequest) {
    try {
      if (!same(request.operation, request.previewToken.operation)) {
        raise('CONFLICT', 'Kaydetme işlemi önizlenen işlemle eşleşmiyor.');
      }
      const initial = request.operation.kind === 'INITIAL_BASELINE';
      if (initial && (!request.initialBaselineConfirmation?.confirmed ||
        request.initialBaselineConfirmation.effectiveFrom !== request.operation.effectiveFrom)) {
        raise('INVALID_REQUEST', 'Başlangıç baseline tarihi açıkça onaylanmalıdır.');
      }
      if (!initial && request.initialBaselineConfirmation !== undefined) {
        raise('INVALID_REQUEST', 'Başlangıç baseline onayı yalnız ilk sürümde kullanılabilir.');
      }
      const runtime = await getWasmRuntime();
      let receipt: CompensationPolicySaveReceipt | undefined;
      await browserPayrollStore.mutateSnapshot((current) => {
        const preview = calculatePreview(current, request.scope, request.operation, runtime);
        if (!same(preview.previewToken, request.previewToken)) {
          raise('CONFLICT', 'Ücret politikası önizleme tokenı güncel snapshot ile eşleşmiyor.', {
            actualTargetState: preview.previewToken.targetState,
          });
        }
        const now = new Date().toISOString();
        const version: CompensationPolicyVersionWire = {
          id: globalThis.crypto.randomUUID(),
          effectiveFrom: request.operation.effectiveFrom,
          state: request.operation.state,
          boundaryScope: request.operation.boundaryScope,
          roadRoundingMode: request.operation.roadRoundingMode,
          createdAt: now,
          source: 'SETTINGS_COMPENSATION_POLICY',
          reason: request.operation.reason.trim(),
          supersedesVersionId: request.operation.kind === 'SAME_DATE_CORRECTION'
            ? request.operation.supersedesVersionId
            : null,
        };
        const calculated = calculatePreview(current, request.scope, request.operation, runtime);
        const staleKeys = new Set(
          calculated.dependencyClosureEffects
            .filter((effect) => effect.action === 'CALCULATED_TO_STALE')
            .map((effect) => `${effect.affectedPayroll.personnelId}\u0000${effect.affectedPayroll.periodId}\u0000${effect.affectedPayroll.accrualId}`)
        );
        const staleRetroEffects = calculated.retroEffects.filter((effect) => effect.action === 'STALE_ELIGIBLE');
        const staleBatchIds = new Set(staleRetroEffects.map((effect) => effect.batchId));
        const stalePaymentEventIds = new Set(staleRetroEffects.flatMap((effect) => effect.linkedPaymentEventIds));
        const staleNow = new Date().toISOString();
        receipt = {
          version,
          invalidatedPayrolls: current.bordrolar.flatMap((payroll) => {
            const key = keyString(payrollKey(payroll));
            if (!staleKeys.has(key) || payroll.status !== 'CALCULATED') return [];
            return [{ payroll: payrollReference(payroll), fromStatus: 'CALCULATED', toStatus: 'STALE' }];
          }),
          invalidatedRetroBatches: current.retroBatches.flatMap((batch) => {
            if (!staleBatchIds.has(batch.id) || batch.status !== 'CALCULATED') return [];
            return [{ batchId: batch.id, fromStatus: 'CALCULATED', toStatus: 'STALE' }];
          }),
          invalidatedRetroPaymentEvents: current.bordrolar.flatMap((payroll) => {
            if (!stalePaymentEventIds.has(payroll.id) || payroll.status !== 'CALCULATED') return [];
            return [{ payroll: payrollReference(payroll), fromStatus: 'CALCULATED', toStatus: 'STALE' }];
          }),
          authoritativeReloadRequired: true,
        };
        return {
          ...current,
          compensationPolicyVersions: [...current.compensationPolicyVersions, version],
          retroBatches: current.retroBatches.map((batch) =>
            staleBatchIds.has(batch.id) && batch.status === 'CALCULATED'
              ? { ...batch, status: 'STALE' }
              : batch
          ),
          bordrolar: current.bordrolar.map((payroll) =>
            (staleKeys.has(keyString(payrollKey(payroll))) || stalePaymentEventIds.has(payroll.id))
              && payroll.status === 'CALCULATED'
              ? { ...payroll, status: 'STALE', sonGuncellemeTarihi: staleNow }
              : payroll
          ),
        };
      });
      if (!receipt) raise('PERSISTENCE', 'Kaydetme sonucu oluşturulamadı.');
      return success({ ...receipt, authoritativeReloadRequired: true as const });
    } catch (error) {
      return errorResult(error);
    }
  },
};

/** Native and browser APIs share the frozen DTO and fail-closed result union. */
export function getCompensationPolicyApi(): CompensationPolicyApi {
  return tauriBridge.isTauriAvailable() ? tauriCompensationPolicyApi : browserCompensationPolicyApi;
}
