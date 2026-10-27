import { createElement, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { useBackupController } from '../../hooks/useBackupController';
import { BordroHesaplama } from '../../components/BordroHesaplama';
import { BACKUP_FORMAT_VERSION } from '../../types/payroll';
import { getInitialDataset } from '../../utils/sampleData';
import { createBordroDonemi } from '../../utils/payrollPresentation';
import { getCompensationPolicyApi, previewRoadRoundingRepair, saveRoadRoundingRepair } from './api';
import type {
  CompensationPolicyPageProps,
  CompensationPolicySaveRequest,
  CompensationPolicyStateWire,
  CompensationPolicyVersionWire,
  CompensationPolicyWriteOperation,
} from './contract';
import {
  parseWasmPayrollBoundaryResult,
  serializePayrollRequestForWasm,
  serializePayrollStorage,
  toPayrollBoundaryDto,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';
import { getPayrollEngine } from '../payrollEngine';
import { browserPayrollStore } from '../storage/browserPayrollStore';
import type {
  PayrollBoundaryPayroll,
  PayrollCalculationRequest,
  PayrollDatasetSnapshot,
  RetroCalculationRequest,
  RetroCalculationResult,
} from '../payrollEngine/types';
import { getWasmRuntime } from '../payrollEngine/wasmRuntime';
import {
  validatePayrollMetadataThroughCore,
} from '../storage/payrollPayload';
import { getDefaultAnnualPayrollParameters } from '../storage/payrollDefaults';
import { useBrowserPayrollPersistence } from '../storage/useBrowserPayrollPersistence';
import { deletePeriodFromPayload } from '../storage/periodDeletion';

type Scope = CompensationPolicyPageProps['scope'];

function resetBrowserPayrollDatabase(): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = window.indexedDB.deleteDatabase('4d-bordro-programi');
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error ?? new Error('Test IndexedDB reset failed.'));
    request.onblocked = () => reject(new Error('Test IndexedDB reset was blocked.'));
  });
}

async function loadArchivedSeptemberPrefill() {
  await resetBrowserPayrollDatabase();
  const data = getInitialDataset();
  const period = createBordroDonemi(2026, 9, 2026, 10);
  const neighboringSettings = data.kurumDegerleriMap[data.aktifDonemId];
  if (!neighboringSettings) throw new Error('Browser archive fixture has no base institution settings.');
  const archivedSettings = {
    ...neighboringSettings,
    donemId: period.id,
    gunlukTabanUcret: '2616.26',
    gunlukYemek: '300.75',
    birlestirilmisSosyalYardim: '5089.7',
    gunlukVasitaYol: '128.93',
    giyimYardimi: '269.7',
    hizmetZammiBirimi: '24.67',
    isPrimiYuzde: '0',
    isPrimiGruplari: [
      { id: '1. Grup', ad: '1. Grup', oran: '9', aktif: true },
      { id: '2. Grup', ad: '2. Grup', oran: '8', aktif: true },
      { id: '3. Grup', ad: '3. Grup', oran: '7', aktif: true },
      { id: 'grup_1791176163314', ad: 'UX Deneme Grubu', oran: '6', aktif: true },
    ],
    geceCalismaPrimiYuzde: '0',
    geceCalismaTatiliPrimiYuzde: '0',
    ekOdeme: '0',
    digerGelirVarsayilan: '0',
  };
  const payload = toPayrollBoundaryDto({
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: '2026-10-07T00:00:00.000Z',
    ...data,
    aktifDonemId: period.id,
    donemler: [...data.donemler, period],
    compensationPolicyVersions: [],
    compensationRevisions: [],
    compensationRevisionOverrides: [],
    retroBatches: [],
    retroAllocations: [],
    legacyInstitutionSettings: { [period.id]: archivedSettings },
  }) as unknown as PayrollStorageDto;
  await browserPayrollStore.savePayload(serializePayrollStorage(payload));

  const scope: Scope = {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId: period.id,
    periodStartInclusive: period.baslangicTarihi,
    periodEndInclusive: period.bitisTarihi,
    projectionDate: period.baslangicTarihi,
  };
  const before = await browserPayrollStore.loadSnapshot();
  if (!before) throw new Error('Archived September fixture did not persist to the isolated browser store.');
  const result = await getCompensationPolicyApi().load({ scope });
  const after = await browserPayrollStore.loadSnapshot();
  return {
    scope,
    result,
    beforeRevision: before.revision,
    afterRevision: after?.revision ?? null,
  };
}

async function prepareBaseline() {
  await resetBrowserPayrollDatabase();
  const data = getInitialDataset();
  const activePeriod = data.donemler.find((period) => period.id === data.aktifDonemId);
  const personnel = data.personeller[0];
  if (!activePeriod || !personnel) throw new Error('Sample payroll calculation fixture is incomplete.');
  const seedVersion: CompensationPolicyVersionWire = {
    id: 'browser-legacy-seed-policy',
    effectiveFrom: activePeriod.baslangicTarihi,
    state: browserAcceptanceState(),
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PER_COMPENSATION_SLICE',
    createdAt: '2026-10-06T00:00:00Z',
    source: 'user-confirmed',
    reason: 'Temporary policy for a legacy payroll invalidation fixture',
    supersedesVersionId: null,
  };
  const dataset = toPayrollBoundaryDto({
    personnel: data.personeller,
    periods: data.donemler,
    institutionSettings: data.kurumDegerleriMap,
    attendances: data.puantajlar,
    payrolls: [],
    taxOpenings: data.taxOpenings ?? [],
    sickLeaveRecords: data.sickLeaveRecords ?? [],
    annualPayrollParameters: data.annualPayrollParameters ?? [],
    compensationRevisions: [],
    compensationRevisionOverrides: [],
    retroBatches: [],
    retroAllocations: [],
    compensationPolicyVersions: [seedVersion],
  }) as unknown as PayrollDatasetSnapshot;
  const request = {
    personnelId: personnel.id,
    periodId: activePeriod.id,
    calculatedAt: '2026-10-06T00:00:00.000Z',
    manualIncome: null,
    accrual: null,
    dataset,
  };
  const runtime = await getWasmRuntime();
  const calculated = parseWasmPayrollBoundaryResult<PayrollBoundaryPayroll>(
    runtime.calculate_payroll_json(serializePayrollRequestForWasm(request))
  );
  // This API test needs a pre-existing snapshotless legacy payroll so the
  // explicit baseline operation can prove it becomes stale. The temporary
  // seed policy is used only to calculate its amount through the default API.
  const legacyCalculated = { ...calculated, calculationMetadata: undefined };
  data.bordrolar = [legacyCalculated as unknown as typeof data.bordrolar[number]];
  const payload = toPayrollBoundaryDto({
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    ...data,
    taxOpenings: data.taxOpenings ?? [],
    sickLeaveRecords: data.sickLeaveRecords ?? [],
    annualPayrollParameters: data.annualPayrollParameters ?? [],
    compensationRevisions: [],
    compensationRevisionOverrides: [],
    retroBatches: [],
    retroAllocations: [],
    compensationPolicyVersions: [],
  });
  await browserPayrollStore.savePayload(serializePayrollStorage(payload));
  return await previewBaseline(data.aktifDonemId);
}

async function previewBaseline(periodId: string) {
  const snapshot = await browserPayrollStore.loadSnapshot();
  if (!snapshot) throw new Error('Seed browser snapshot is missing.');
  const parsed = JSON.parse(snapshot.payload) as {
    donemler: Array<{ id: string; baslangicTarihi: string; bitisTarihi: string }>;
  };
  const period = parsed.donemler.find((item) => item.id === periodId);
  if (!period) throw new Error(`Seed period ${periodId} is missing.`);
  const scope: Scope = {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId,
    periodStartInclusive: period.baslangicTarihi,
    periodEndInclusive: period.bitisTarihi,
    projectionDate: period.baslangicTarihi,
  };
  const api = getCompensationPolicyApi();
  const loaded = await api.load({ scope });
  if (loaded.ok === false) throw new Error(`Browser API load failed: ${loaded.error.message}`);
  if (!loaded.value.initialBaselineRequired) {
    throw new Error('The browser policy fixture unexpectedly already has an authoritative baseline.');
  }
  if (
    loaded.value.nonAuthoritativePrefill.kind !== 'UNAVAILABLE' ||
    loaded.value.nonAuthoritativePrefill.reason !== 'NO_LEGACY_SETTINGS'
  ) {
    throw new Error('The current-format fixture unexpectedly contains archived compensation settings.');
  }
  // Model values explicitly entered by a user; current period settings are
  // intentionally not used as compensation-policy authority or prefill.
  const state = browserAcceptanceState();
  const operation: CompensationPolicyWriteOperation = {
    kind: 'INITIAL_BASELINE',
    effectiveFrom: period.baslangicTarihi,
    state,
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PERIOD_TOTAL',
    reason: 'Browser API IndexedDB integration fixture',
  };
  const preview = await api.preview({ scope, operation });
  if (preview.ok === false) throw new Error(`Browser API preview failed: ${preview.error.message}`);
  return { scope, operation, preview: preview.value };
}

async function saveBaseline(prepared: Awaited<ReturnType<typeof previewBaseline>>) {
  const request: CompensationPolicySaveRequest = {
    scope: prepared.scope,
    operation: prepared.operation,
    previewToken: prepared.preview.previewToken,
    initialBaselineConfirmation: {
      confirmed: true,
      effectiveFrom: prepared.operation.effectiveFrom,
    },
  };
  return await getCompensationPolicyApi().save(request);
}

function rescaleDecimal(value: string | null): string | null {
  if (value === null) return null;
  return value.includes('.') ? `${value}0` : `${value}.00`;
}

function addDays(iso: string, days: number): string {
  const date = new Date(`${iso}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

async function exerciseLinkedRoundingRepair() {
  await resetBrowserPayrollDatabase();
  const data = getInitialDataset();
  const periods = [createBordroDonemi(2027, 10), createBordroDonemi(2027, 11), createBordroDonemi(2027, 12), createBordroDonemi(2028, 1)];
  const versions: CompensationPolicyVersionWire[] = [
    ['base', '2027-10-15', 'PERIOD_TOTAL'], ['nov', '2027-11-01', 'PER_COMPENSATION_SLICE'],
    ['jan', '2028-01-01', 'PER_COMPENSATION_SLICE'], ['feb', '2028-02-01', 'PERIOD_TOTAL'],
  ].map(([id, effectiveFrom, mode], index) => ({ id, effectiveFrom, roadRoundingMode: mode as CompensationPolicyVersionWire['roadRoundingMode'],
    state: { ...browserAcceptanceState(), gunlukTabanUcret: `${2100 + index * 10}.005` }, boundaryScope: 'FULL_EARNING',
    createdAt: '2026-01-01T00:00:00Z', source: 'TEST', reason: 'Legacy policy', supersedesVersionId: null }));
  const payload = toPayrollBoundaryDto({ backupVersion: BACKUP_FORMAT_VERSION, exportedAt: '2026-01-01T00:00:00Z', ...data,
    personeller: [], puantajlar: [], bordrolar: [], sickLeaveRecords: [], taxOpenings: [], donemler: [...data.donemler, ...periods], compensationPolicyVersions: versions,
    compensationRevisions: [], compensationRevisionOverrides: [], retroBatches: [], retroAllocations: [], aktifDonemId: periods[0].id,
  }) as unknown as PayrollStorageDto;
  await browserPayrollStore.savePayload(serializePayrollStorage(payload));
  const scope: Scope = { institutionScope: 'CURRENT_INSTITUTION', periodId: periods[0].id, periodStartInclusive: periods[0].baslangicTarihi,
    periodEndInclusive: periods[0].bitisTarihi, projectionDate: periods[0].baslangicTarihi };
  const preview = await previewRoadRoundingRepair(scope);
  if (preview.ok === false) throw new Error(JSON.stringify(preview.error));
  const rejected = await saveRoadRoundingRepair(scope, { previews: preview.value.previews.slice(0, 1) });
  const before = await browserPayrollStore.loadPayload();
  const saved = await saveRoadRoundingRepair(scope, preview.value);
  const after = await browserPayrollStore.loadPayload();
  const retry = await saveRoadRoundingRepair(scope, preview.value);
  const resolutions = [];
  for (const period of periods) {
    const loaded = await getCompensationPolicyApi().load({ scope: { ...scope, periodId: period.id,
      periodStartInclusive: period.baslangicTarihi, periodEndInclusive: period.bitisTarihi, projectionDate: period.baslangicTarihi } });
    resolutions.push(loaded.ok === true ? loaded.value.timeline.resolution : loaded.error.code);
  }
  return { count: preview.value.previews.length, rejected: !rejected.ok, saved: saved.ok, retryRejected: !retry.ok,
    originalsPreserved: (JSON.parse(before!) as PayrollStorageDto).compensationPolicyVersions.every((source) => JSON.stringify((JSON.parse(after!) as PayrollStorageDto).compensationPolicyVersions.find((v) => v.id === source.id)) === JSON.stringify(source)),
    beforeCount: (JSON.parse(before!) as PayrollStorageDto).compensationPolicyVersions.length,
    afterCount: (JSON.parse(after!) as PayrollStorageDto).compensationPolicyVersions.length, resolutions };
}

async function exerciseRoadRoundingRecovery() {
  await resetBrowserPayrollDatabase();
  const data = getInitialDataset();
  const period = createBordroDonemi(2027, 12);
  const sourceSettings = data.kurumDegerleriMap[data.aktifDonemId];
  if (!sourceSettings) throw new Error('Road rounding fixture has no institution settings.');
  const scope: Scope = {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId: period.id,
    periodStartInclusive: period.baslangicTarihi,
    periodEndInclusive: period.bitisTarihi,
    projectionDate: period.baslangicTarihi,
  };
  const state: CompensationPolicyStateWire = {
    ...browserAcceptanceState(),
    gunlukVasitaYol: '136.44',
  };
  const baseline: CompensationPolicyVersionWire = {
    id: 'road-rounding-december-baseline',
    effectiveFrom: period.baslangicTarihi,
    state,
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PERIOD_TOTAL',
    createdAt: '2027-12-01T00:00:00.000Z',
    source: 'TEST',
    reason: 'December service-period baseline',
    supersedesVersionId: null,
  };
  const seed = async (versions: CompensationPolicyVersionWire[]) => {
    const payload = toPayrollBoundaryDto({
      backupVersion: BACKUP_FORMAT_VERSION,
      exportedAt: '2027-12-01T00:00:00.000Z',
      ...data,
      donemler: [...data.donemler.filter((item) => item.id !== period.id), period],
      kurumDegerleriMap: {
        ...data.kurumDegerleriMap,
        [period.id]: { ...sourceSettings, donemId: period.id },
      },
      compensationPolicyVersions: versions,
      compensationRevisions: [],
      compensationRevisionOverrides: [],
      retroBatches: [],
      retroAllocations: [],
    }) as unknown as PayrollStorageDto;
    await browserPayrollStore.savePayload(serializePayrollStorage(payload));
  };

  await seed([baseline]);
  const midPeriodModeChange: CompensationPolicyWriteOperation = {
    kind: 'NEW_EFFECTIVE_DATE',
    effectiveFrom: '2028-01-01',
    state: { ...state, gunlukTabanUcret: '2125.13' },
    boundaryScope: 'FULL_EARNING',
    changeIntent: 'FULL_STATE_CHANGE',
    roadRoundingMode: 'PER_COMPENSATION_SLICE',
    reason: 'Rejected mid-service-period road mode change',
  };
  const rejected = await getCompensationPolicyApi().preview({ scope, operation: midPeriodModeChange });

  const invalid = {
    ...baseline,
    id: 'road-rounding-invalid-january-policy',
    effectiveFrom: '2028-01-01',
    state: { ...state, gunlukTabanUcret: '2125.13' },
    roadRoundingMode: 'PER_COMPENSATION_SLICE' as const,
    createdAt: '2027-12-15T00:00:00.000Z',
    reason: 'Legacy mid-service-period rounding mismatch',
  };
  await seed([baseline, invalid]);
  const loadedInvalid = await getCompensationPolicyApi().load({ scope });
  const correction: CompensationPolicyWriteOperation = {
    kind: 'SAME_DATE_CORRECTION',
    effectiveFrom: invalid.effectiveFrom,
    state: invalid.state,
    boundaryScope: 'FULL_EARNING',
    changeIntent: 'FULL_STATE_CHANGE',
    roadRoundingMode: baseline.roadRoundingMode,
    reason: 'Restore the established rounding mode without changing the effective date',
    supersedesVersionId: invalid.id,
  };
  const correctionPreview = await getCompensationPolicyApi().preview({ scope, operation: correction });
  let saved: unknown;
  if (correctionPreview.ok === true) {
    const saveResult = await getCompensationPolicyApi().save({
      scope,
      operation: correction,
      previewToken: correctionPreview.value.previewToken,
    });
    saved = saveResult.ok === true
      ? { version: {
        id: saveResult.value.version.id,
        effectiveFrom: saveResult.value.version.effectiveFrom,
        roadRoundingMode: saveResult.value.version.roadRoundingMode,
        supersedesVersionId: saveResult.value.version.supersedesVersionId,
      }, error: null }
      : { version: null, error: saveResult.error };
  } else {
    saved = { version: null, error: correctionPreview.error };
  }
  const reloaded = await getCompensationPolicyApi().load({ scope });
  const rejectedError = rejected.ok === true ? null : rejected.error;
  const invalidSummary = {
    resolution: loadedInvalid.ok === true ? loadedInvalid.value.timeline.resolution : null,
    roadRoundingIssues: loadedInvalid.ok === true ? loadedInvalid.value.timeline.roadRoundingIssues : [],
    error: loadedInvalid.ok === true ? null : loadedInvalid.error,
  };
  const correctionError = correctionPreview.ok === true ? null : correctionPreview.error;
  const reloadSummary = {
    resolution: reloaded.ok === true ? reloaded.value.timeline.resolution : null,
    roadRoundingIssues: reloaded.ok === true ? reloaded.value.timeline.roadRoundingIssues : [],
    resolvedRoadRoundingModes: reloaded.ok === true
      ? reloaded.value.timeline.resolvedSegments.map((segment) => segment.roadRoundingMode)
      : [],
    versions: reloaded.ok === true
      ? reloaded.value.timeline.versions.map((version) => ({
        id: version.id,
        effectiveFrom: version.effectiveFrom,
        roadRoundingMode: version.roadRoundingMode,
        supersedesVersionId: version.supersedesVersionId,
      }))
      : [],
    error: reloaded.ok === true ? null : reloaded.error,
  };
  return {
    scope,
    rejected: rejectedError,
    loadedInvalid: invalidSummary,
    correctionPreview: correctionError,
    saved,
    reloaded: reloadSummary,
  };
}

async function exerciseFuturePeriodRoundingConflict() {
  await resetBrowserPayrollDatabase();
  const data = getInitialDataset();
  const december = createBordroDonemi(2026, 12);
  const january = createBordroDonemi(2027, 1);
  const february = createBordroDonemi(2027, 2);
  const periods = [december, january, february];
  const sourceSettings = data.kurumDegerleriMap[data.aktifDonemId];
  if (!sourceSettings) throw new Error('Future rounding fixture has no institution settings.');
  const scope: Scope = {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId: january.id,
    periodStartInclusive: january.baslangicTarihi,
    periodEndInclusive: january.bitisTarihi,
    projectionDate: january.baslangicTarihi,
  };
  const state: CompensationPolicyStateWire = {
    ...browserAcceptanceState(),
    gunlukVasitaYol: '136.44',
  };
  const baseline: CompensationPolicyVersionWire = {
    id: 'future-rounding-december-baseline',
    effectiveFrom: '2026-12-15',
    state,
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PERIOD_TOTAL',
    createdAt: '2026-12-01T00:00:00.000Z',
    source: 'TEST',
    reason: 'December service-period baseline',
    supersedesVersionId: null,
  };
  const marchVersion: CompensationPolicyVersionWire = {
    ...baseline,
    id: 'future-rounding-march-policy',
    effectiveFrom: '2027-03-01',
    createdAt: '2027-02-01T00:00:00.000Z',
    reason: 'March policy remains on the established mode',
  };
  const seed = async (versions: CompensationPolicyVersionWire[]) => {
    const institutionSettings = { ...data.kurumDegerleriMap };
    for (const period of periods) {
      institutionSettings[period.id] = { ...sourceSettings, donemId: period.id };
    }
    const payload = toPayrollBoundaryDto({
      backupVersion: BACKUP_FORMAT_VERSION,
      exportedAt: '2026-12-01T00:00:00.000Z',
      ...data,
      donemler: [
        ...data.donemler.filter((item) => !periods.some((candidate) => candidate.id === item.id)),
        ...periods,
      ],
      kurumDegerleriMap: institutionSettings,
      compensationPolicyVersions: versions,
      compensationRevisions: [],
      compensationRevisionOverrides: [],
      retroBatches: [],
      retroAllocations: [],
    }) as unknown as PayrollStorageDto;
    await browserPayrollStore.savePayload(serializePayrollStorage(payload));
  };

  await seed([baseline, marchVersion]);
  const invalidOperation: CompensationPolicyWriteOperation = {
    kind: 'NEW_EFFECTIVE_DATE',
    effectiveFrom: '2027-01-15',
    state: { ...state, gunlukTabanUcret: '2125.13' },
    boundaryScope: 'FULL_EARNING',
    changeIntent: 'FULL_STATE_CHANGE',
    roadRoundingMode: 'PER_COMPENSATION_SLICE',
    reason: 'Regression: this creates a future service-period conflict',
  };
  const rejectedPreview = await getCompensationPolicyApi().preview({ scope, operation: invalidOperation });

  const validOperation: CompensationPolicyWriteOperation = {
    ...invalidOperation,
    roadRoundingMode: 'PERIOD_TOTAL',
    reason: 'Same mode is valid in the affected service periods',
  };
  const validPreview = await getCompensationPolicyApi().preview({ scope, operation: validOperation });
  if (validPreview.ok === false) throw new Error(`Valid same-mode preview failed: ${validPreview.error.message}`);

  // Model a newer authoritative policy arriving after preview. Save must run
  // the shared validator again against the snapshot inside its IndexedDB write.
  await browserPayrollStore.mutateSnapshot((currentPayload) => ({
    ...currentPayload,
    compensationPolicyVersions: [
      ...currentPayload.compensationPolicyVersions,
      {
        ...marchVersion,
        id: 'future-rounding-march-correction',
        roadRoundingMode: 'PER_COMPENSATION_SLICE',
        createdAt: '2027-02-02T00:00:00.000Z',
        reason: 'Concurrent immutable correction changes the future mode',
        supersedesVersionId: marchVersion.id,
      },
    ],
  }));
  const rejectedSave = await getCompensationPolicyApi().save({
    scope,
    operation: validOperation,
    previewToken: validPreview.value.previewToken,
  });
  const after = await browserPayrollStore.loadSnapshot();
  const stored = after ? JSON.parse(after.payload) as PayrollStorageDto : null;
  return {
    rejectedPreview: rejectedPreview.ok === false ? rejectedPreview.error : null,
    validPreview: validPreview.ok,
    rejectedSave: rejectedSave.ok === false ? rejectedSave.error : null,
    storedPolicyIds: stored?.compensationPolicyVersions.map((version) => version.id) ?? [],
    expectedPeriod: { id: february.id, start: february.baslangicTarihi, end: february.bitisTarihi },
  };
}

async function exerciseSyntheticPolicySchedule() {
  await resetBrowserPayrollDatabase();
  const source = getInitialDataset();
  const sourceSettings = source.kurumDegerleriMap[source.aktifDonemId];
  const annual2026 = getDefaultAnnualPayrollParameters(2026);
  if (!source.personeller[0] || !sourceSettings || !annual2026) {
    throw new Error('Synthetic policy schedule fixture is incomplete.');
  }
  const periods = [
    ...[9, 10, 11, 12].map((month) => createBordroDonemi(2026, month, 2026, month)),
    ...Array.from({ length: 12 }, (_, index) => createBordroDonemi(2027, index + 1, 2027, index + 1)),
    ...Array.from({ length: 9 }, (_, index) => createBordroDonemi(2028, index + 1, 2028, index + 1)),
  ];
  const periodByDate = (effectiveFrom: string) => {
    const period = periods.find((item) => item.baslangicTarihi <= effectiveFrom && item.bitisTarihi >= effectiveFrom);
    if (!period) throw new Error(`No synthetic service period contains ${effectiveFrom}.`);
    return period;
  };
  const settings = Object.fromEntries(periods.map((period) => [period.id, {
    ...sourceSettings,
    donemId: period.id,
  }]));
  const annual2027 = { ...annual2026, year: 2027, sigortaGvYillikBrutAsgariUcretTavani: 180000 };
  const annual2028 = { ...annual2026, year: 2028, sigortaGvYillikBrutAsgariUcretTavani: 190000 };
  const payload = toPayrollBoundaryDto({
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: '2026-10-11T00:00:00Z',
    ...source,
    aktifDonemId: '2026-09',
    donemler: periods,
    kurumDegerleriMap: settings,
    puantajlar: [],
    bordrolar: [],
    taxOpenings: [],
    sickLeaveRecords: [],
    annualPayrollParameters: [annual2026, annual2027, annual2028],
    compensationRevisions: [],
    compensationRevisionOverrides: [],
    retroBatches: [],
    retroAllocations: [],
    compensationPolicyVersions: [],
  }) as unknown as PayrollStorageDto;
  await browserPayrollStore.savePayload(serializePayrollStorage(payload));

  const api = getCompensationPolicyApi();
  let state: CompensationPolicyStateWire = {
    ...browserAcceptanceState(),
    gunlukTabanUcret: '1000.00',
    gunlukVasitaYol: '75.125',
  };
  const savedVersions: CompensationPolicyVersionWire[] = [];
  const save = async (operation: CompensationPolicyWriteOperation) => {
    const scope = acceptanceScope(periodByDate(operation.effectiveFrom));
    const preview = await api.preview({ scope, operation });
    if (preview.ok === false) throw new Error(`Preview ${operation.effectiveFrom} failed: ${preview.error.code}: ${preview.error.message}`);
    const result = operation.kind === 'INITIAL_BASELINE'
      ? await api.save({
        scope,
        operation,
        previewToken: preview.value.previewToken,
        initialBaselineConfirmation: { confirmed: true, effectiveFrom: operation.effectiveFrom },
      })
      : await api.save({ scope, operation, previewToken: preview.value.previewToken });
    if (result.ok === false) throw new Error(`Save ${operation.effectiveFrom} failed: ${result.error.code}: ${result.error.message}`);
    savedVersions.push(result.value.version);
    return result.value.version;
  };

  await save({
    kind: 'INITIAL_BASELINE',
    effectiveFrom: '2027-01-15',
    state,
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PERIOD_TOTAL',
    reason: 'Synthetic confirmed starting policy',
  });

  const addPercent = (value: string, percentageBasisPoints: bigint) => {
    const [whole, fraction = ''] = value.split('.');
    const cents = BigInt(whole) * 100n + BigInt(`${fraction}00`.slice(0, 2));
    const increase = (cents * percentageBasisPoints + 5000n) / 10000n;
    const updated = cents + increase;
    return `${updated / 100n}.${String(updated % 100n).padStart(2, '0')}`;
  };
  const salaryRaises = [
    { date: '2027-02-01', basisPoints: 708n },
    { date: '2027-09-01', basisPoints: 1135n },
    { date: '2028-02-01', basisPoints: 625n },
    { date: '2028-09-01', basisPoints: 950n },
  ];
  const roadDates = [
    '2027-03-15', '2027-04-15', '2027-05-15', '2027-06-15', '2027-07-15',
    '2027-10-15', '2027-11-15', '2028-03-15', '2028-05-15', '2028-07-15',
  ];
  const schedule = [
    ...salaryRaises.map((item) => ({ date: item.date, kind: 'SALARY' as const, basisPoints: item.basisPoints })),
    ...roadDates.map((date) => ({ date, kind: 'ROAD' as const })),
  ].sort((left, right) => left.date.localeCompare(right.date));
  const salaryValues: Array<{ date: string; value: string }> = [];
  const roadValues: Array<{ date: string; value: string }> = [];
  let roadMilliunits = 75125n;

  for (const item of schedule) {
    if (item.kind === 'SALARY') {
      state = { ...state, gunlukTabanUcret: addPercent(state.gunlukTabanUcret, item.basisPoints) };
      salaryValues.push({ date: item.date, value: state.gunlukTabanUcret });
      const saved = await save({
        kind: 'NEW_EFFECTIVE_DATE',
        effectiveFrom: item.date,
        state,
        boundaryScope: 'FULL_EARNING',
        changeIntent: 'FULL_STATE_CHANGE',
        roadRoundingMode: 'PERIOD_TOTAL',
        reason: `Synthetic ${item.basisPoints.toString()} basis-point salary increase`,
      });
      if (item.date === '2027-02-01') {
        const [whole, fraction = ''] = state.gunlukTabanUcret.split('.');
        const correctedCents = BigInt(whole) * 100n + BigInt(`${fraction}00`.slice(0, 2)) + 100n;
        state = { ...state, gunlukTabanUcret: `${correctedCents / 100n}.${String(correctedCents % 100n).padStart(2, '0')}` };
        await save({
          kind: 'SAME_DATE_CORRECTION',
          effectiveFrom: item.date,
          state,
          boundaryScope: 'FULL_EARNING',
          changeIntent: 'FULL_STATE_CHANGE',
          roadRoundingMode: 'PERIOD_TOTAL',
          reason: 'Synthetic same-date salary correction',
          supersedesVersionId: saved.id,
        });
      }
    } else {
      roadMilliunits += 125n;
      state = {
        ...state,
        gunlukVasitaYol: `${roadMilliunits / 1000n}.${String(roadMilliunits % 1000n).padStart(3, '0')}`,
      };
      roadValues.push({ date: item.date, value: state.gunlukVasitaYol });
      await save({
        kind: 'NEW_EFFECTIVE_DATE',
        effectiveFrom: item.date,
        state,
        boundaryScope: 'ROAD_ONLY',
        changeIntent: 'ROAD_AMOUNT_ONLY',
        roadRoundingMode: 'PERIOD_TOTAL',
        reason: 'Synthetic road allowance change',
      });
    }
  }

  const septemberLeaf = [...savedVersions].reverse().find((version) => version.effectiveFrom === '2027-09-01');
  if (!septemberLeaf) throw new Error('Synthetic September policy correction fixture is missing.');
  await save({
    kind: 'SAME_DATE_CORRECTION',
    effectiveFrom: septemberLeaf.effectiveFrom,
    state: septemberLeaf.state,
    boundaryScope: 'FULL_EARNING',
    changeIntent: 'FULL_STATE_CHANGE',
    roadRoundingMode: septemberLeaf.roadRoundingMode,
    reason: 'Synthetic late same-date correction with future policy versions',
    supersedesVersionId: septemberLeaf.id,
  });

  const finalPeriod = periodByDate('2028-09-15');
  const finalScope = acceptanceScope(finalPeriod);
  const loaded = await api.load({ scope: finalScope });
  if (loaded.ok === false) throw new Error(`Final policy reload failed: ${loaded.error.code}: ${loaded.error.message}`);
  const stored = await browserPayrollStore.loadSnapshot();
  if (!stored) throw new Error('Synthetic policy snapshot disappeared after successful saves.');
  const storedPayload = JSON.parse(stored.payload) as PayrollStorageDto;
  return {
    salaryValues,
    roadValues,
    savedVersionCount: savedVersions.length,
    storedVersionCount: storedPayload.compensationPolicyVersions.length,
    resolution: loaded.value.timeline.resolution,
    finalRoadMode: loaded.value.timeline.resolvedSegments.at(-1)?.roadRoundingMode ?? null,
    finalRoadAmount: loaded.value.timeline.resolvedSegments.at(-1)?.state.gunlukVasitaYol ?? null,
    previewSaveRoutes: ['INITIAL_BASELINE', 'NEW_EFFECTIVE_DATE', 'SAME_DATE_CORRECTION'],
  };
}

async function previewRoadOnlyScaleChange(periodId: string) {
  const snapshot = await browserPayrollStore.loadSnapshot();
  if (!snapshot) throw new Error('Seed browser snapshot is missing.');
  const parsed = JSON.parse(snapshot.payload) as PayrollStorageDto;
  const period = parsed.donemler.find((item) => item.id === periodId);
  const previous = parsed.compensationPolicyVersions.find((item) => item.effectiveFrom === period?.baslangicTarihi);
  if (!period || !previous) throw new Error('Road-only fixture needs a saved baseline policy.');
  const state: CompensationPolicyStateWire = {
    ...previous.state,
    gunlukTabanUcret: rescaleDecimal(previous.state.gunlukTabanUcret)!,
    gunlukYemek: rescaleDecimal(previous.state.gunlukYemek)!,
    birlestirilmisSosyalYardim: rescaleDecimal(previous.state.birlestirilmisSosyalYardim)!,
    gunlukVasitaYol: previous.state.gunlukVasitaYol === '987.654' ? '987.655' : '987.654',
    giyimYardimi: rescaleDecimal(previous.state.giyimYardimi)!,
    hizmetZammiBirimi: rescaleDecimal(previous.state.hizmetZammiBirimi)!,
    isPrimiYuzde: rescaleDecimal(previous.state.isPrimiYuzde),
    isPrimiGruplari: previous.state.isPrimiGruplari.map((group) => ({
      ...group,
      oran: rescaleDecimal(group.oran)!,
    })),
    geceCalismaPrimiYuzde: rescaleDecimal(previous.state.geceCalismaPrimiYuzde)!,
    geceCalismaTatiliPrimiYuzde: rescaleDecimal(previous.state.geceCalismaTatiliPrimiYuzde)!,
    ekOdeme: rescaleDecimal(previous.state.ekOdeme),
    digerGelirVarsayilan: rescaleDecimal(previous.state.digerGelirVarsayilan),
  };
  const scope: Scope = {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId,
    periodStartInclusive: period.baslangicTarihi,
    periodEndInclusive: period.bitisTarihi,
    projectionDate: period.baslangicTarihi,
  };
  const operation: CompensationPolicyWriteOperation = {
    kind: 'NEW_EFFECTIVE_DATE',
    effectiveFrom: addDays(period.baslangicTarihi, 1),
    state,
    boundaryScope: 'ROAD_ONLY',
    changeIntent: 'ROAD_AMOUNT_ONLY',
    roadRoundingMode: 'PERIOD_TOTAL',
    reason: 'Browser ROAD_ONLY decimal scale regression',
  };
  const preview = await getCompensationPolicyApi().preview({ scope, operation });
  if (preview.ok === false) throw new Error(`ROAD_ONLY preview failed: ${preview.error.code}: ${preview.error.message}`);
  return { scope, operation, preview: preview.value };
}

async function savePreparedOperation(
  prepared: Awaited<ReturnType<typeof previewRoadOnlyScaleChange>>
) {
  const request: CompensationPolicySaveRequest = {
    scope: prepared.scope,
    operation: prepared.operation,
    previewToken: prepared.preview.previewToken,
  };
  return await getCompensationPolicyApi().save(request);
}

async function saveOperationWithAbortedPut(
  prepared: Awaited<ReturnType<typeof previewRoadOnlyScaleChange>>
) {
  const prototype = window.IDBObjectStore.prototype;
  const originalPut = prototype.put;
  let didAbort = false;
  prototype.put = function (this: IDBObjectStore, ...args: Parameters<typeof originalPut>) {
    const transaction = this.transaction;
    const request = Reflect.apply(originalPut, this, args) as IDBRequest<IDBValidKey>;
    if (!didAbort) {
      didAbort = true;
      request.addEventListener('success', () => {
        try {
          transaction.abort();
        } catch {
          // The request is the last write in the API transaction; already-aborted is safe.
        }
      }, { once: true });
    }
    return request;
  };
  try {
    const result = await savePreparedOperation(prepared);
    return { result, didAbort };
  } finally {
    prototype.put = originalPut;
  }
}

async function prepareRetroBaseline() {
  const prepared = await prepareBaseline();
  const baselineSave = await saveBaseline(prepared);
  if (baselineSave.ok === false) {
    throw new Error(`Policy baseline save failed: ${baselineSave.error.code}: ${baselineSave.error.message}`);
  }

  let snapshot = await browserPayrollStore.loadSnapshot();
  if (!snapshot) throw new Error('Saved policy baseline snapshot is missing.');
  let stored = JSON.parse(snapshot.payload) as PayrollStorageDto;
  const sourcePeriod = stored.donemler.find((period) => period.id === prepared.scope.periodId);
  const legacyNormal = stored.bordrolar.find((payroll) =>
    payroll.personelId === stored.personeller[0]?.id &&
    payroll.donemId === prepared.scope.periodId &&
    payroll.accrualType === 'NORMAL'
  );
  if (!sourcePeriod || !legacyNormal) throw new Error('Schema-2 lifecycle needs one source NORMAL payroll.');

  const runtime = await getWasmRuntime();
  const sourceDataset = (payload: PayrollStorageDto): PayrollDatasetSnapshot => ({
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
  }) as unknown as PayrollDatasetSnapshot;
  const normalDataset = sourceDataset(stored);
  normalDataset.payrolls = normalDataset.payrolls.filter((payroll) => payroll.id !== legacyNormal.id);
  const normalRequest: PayrollCalculationRequest = {
    personnelId: legacyNormal.personelId,
    periodId: legacyNormal.donemId,
    calculatedAt: new Date().toISOString(),
    manualIncome: {
      tediye: legacyNormal.gelirler.tediye ?? null,
      tisIkramiyesi: legacyNormal.gelirler.tisIkramiyesi ?? null,
    },
    accrual: {
      accrualId: legacyNormal.accrualId,
      accrualType: 'NORMAL',
      paymentDate: legacyNormal.paymentDate,
      sequence: legacyNormal.sequence,
      grossAmount: null,
      description: legacyNormal.accrualDescription ?? null,
    },
    dataset: normalDataset,
  };
  const schema2Normal = parseWasmPayrollBoundaryResult<PayrollBoundaryPayroll>(
    runtime.calculate_compensation_policy_payroll_json(serializePayrollRequestForWasm(normalRequest))
  );
  await browserPayrollStore.mutateSnapshot((current) => ({
    ...current,
    bordrolar: [
      ...current.bordrolar.filter((payroll) => payroll.id !== schema2Normal.id),
      schema2Normal as unknown as PayrollStorageDto['bordrolar'][number],
    ],
  }));

  snapshot = await browserPayrollStore.loadSnapshot();
  if (!snapshot) throw new Error('Schema-2 NORMAL snapshot is missing.');
  stored = JSON.parse(snapshot.payload) as PayrollStorageDto;
  const savedNormal = stored.bordrolar.find((payroll) =>
    payroll.personelId === legacyNormal.personelId &&
    payroll.donemId === sourcePeriod.id &&
    payroll.accrualType === 'NORMAL' &&
    payroll.status === 'CALCULATED'
  );
  if (!savedNormal) throw new Error('Schema-2 NORMAL did not persist as CALCULATED.');

  const baselineVersion = stored.compensationPolicyVersions.find(
    (version) => version.effectiveFrom === sourcePeriod.baslangicTarihi
  );
  if (!baselineVersion) throw new Error('Persisted baseline policy version is missing.');
  const revisionId = 'browser-retro-revision';
  const revision: PayrollStorageDto['compensationRevisions'][number] = {
    id: revisionId,
    reason: 'OTHER',
    title: 'IndexedDB calculated retro lifecycle',
    effectiveFrom: sourcePeriod.baslangicTarihi,
    effectiveTo: null,
    decisionDate: null,
    signedAt: null,
    description: 'WASM-calculated source entitlement and payment event',
    status: 'DRAFT',
    scope: 'SELECTED_PERSONNEL',
    personnelIds: [savedNormal.personelId],
    personnelGroup: null,
    createdAt: new Date().toISOString(),
    updatedAt: null,
  };
  const [wholeWage, fractionalWage] = baselineVersion.state.gunlukTabanUcret.split('.');
  const increasedWage = `${(BigInt(wholeWage) + 1n).toString()}${fractionalWage ? `.${fractionalWage}` : ''}`;
  const override: PayrollStorageDto['compensationRevisionOverrides'][number] = {
    id: 'browser-retro-wage-override',
    revisionId,
    parameter: 'GUNLUK_TABAN_UCRET',
    value: increasedWage,
    personnelId: savedNormal.personelId,
  };
  const batchId = 'browser-retro-batch';
  const retroRequest: RetroCalculationRequest = {
    batchId,
    revision,
    overrides: [override],
    personnelId: savedNormal.personelId,
    paymentDate: sourcePeriod.bitisTarihi,
    calculatedAt: new Date().toISOString(),
    description: 'Calculated IndexedDB lifecycle fixture',
    dataset: sourceDataset(stored),
  };
  const retro = parseWasmPayrollBoundaryResult<RetroCalculationResult>(
    runtime.calculate_retro_preview_json(
      serializePayrollRequestForWasm(retroRequest as unknown as PayrollCalculationRequest)
    )
  );
  if (retro.batch.status !== 'CALCULATED' || retro.batch.payableSettlementAmount === '0') {
    throw new Error('WASM retro preview did not produce a payable CALCULATED batch.');
  }
  const calculatedRevision = { ...revision, status: 'CALCULATED' as const };
  const paymentDataset = sourceDataset(stored);
  paymentDataset.compensationRevisions = [...paymentDataset.compensationRevisions, calculatedRevision];
  paymentDataset.compensationRevisionOverrides = [...paymentDataset.compensationRevisionOverrides, override];
  paymentDataset.retroBatches = [...paymentDataset.retroBatches, retro.batch];
  paymentDataset.retroAllocations = [...paymentDataset.retroAllocations, ...retro.allocations];
  const paymentRequest: PayrollCalculationRequest = {
    personnelId: savedNormal.personelId,
    periodId: sourcePeriod.id,
    calculatedAt: new Date().toISOString(),
    manualIncome: null,
    accrual: {
      accrualId: batchId,
      accrualType: 'RETRO_ADJUSTMENT',
      paymentDate: sourcePeriod.bitisTarihi,
      sequence: savedNormal.sequence + 1,
      grossAmount: retro.batch.payableSettlementAmount,
      description: 'Calculated IndexedDB lifecycle payment',
    },
    dataset: paymentDataset,
  };
  const paymentEvent = parseWasmPayrollBoundaryResult<PayrollBoundaryPayroll>(
    runtime.calculate_payroll_json(serializePayrollRequestForWasm(paymentRequest))
  );
  await browserPayrollStore.mutateSnapshot((current) => ({
    ...current,
    compensationRevisions: [...current.compensationRevisions, calculatedRevision],
    compensationRevisionOverrides: [...current.compensationRevisionOverrides, override],
    retroBatches: [...current.retroBatches, retro.batch as unknown as PayrollStorageDto['retroBatches'][number]],
    retroAllocations: [
      ...current.retroAllocations,
      ...retro.allocations as unknown as PayrollStorageDto['retroAllocations'],
    ],
    bordrolar: [
      ...current.bordrolar,
      paymentEvent as unknown as PayrollStorageDto['bordrolar'][number],
    ],
  }));

  const policyChange = await previewRoadOnlyScaleChange(prepared.scope.periodId);
  return {
    ...policyChange,
    lifecycle: {
      sourcePayrollId: savedNormal.id,
      batchId: retro.batch.id,
      allocationIds: retro.allocations.map((allocation) => allocation.id),
      paymentEventId: paymentEvent.id,
      sourcePeriodId: sourcePeriod.id,
    },
  };
}

async function exerciseWorkPremiumGroupSave() {
  const lifecycle = await prepareRetroBaseline();
  const beforeSnapshot = await browserPayrollStore.loadSnapshot();
  if (!beforeSnapshot) throw new Error('Work-premium group fixture snapshot is missing.');
  const before = JSON.parse(beforeSnapshot.payload) as PayrollStorageDto;
  const original = before.compensationPolicyVersions[0];
  if (!original) throw new Error('Work-premium group fixture policy is missing.');
  const groups: CompensationPolicyStateWire['isPrimiGruplari'] = [
    { id: '1. Grup', ad: 'Grup 1', oran: '11', aktif: true },
  ];
  const result = await getCompensationPolicyApi().saveWorkPremiumGroups(groups);
  const afterSnapshot = await browserPayrollStore.loadSnapshot();
  if (!afterSnapshot) throw new Error('Work-premium group save removed the browser snapshot.');
  const after = JSON.parse(afterSnapshot.payload) as PayrollStorageDto;
  return {
    result,
    original,
    versions: after.compensationPolicyVersions,
    groups: after.isPrimiGruplari,
    payrollStatuses: after.bordrolar.map((payroll) => payroll.status),
    retroStatuses: after.retroBatches.map((batch) => batch.status),
    lifecycle: lifecycle.lifecycle,
  };
}

async function readAuthoritativeSnapshot() {
  const snapshot = await browserPayrollStore.loadSnapshot();
  if (!snapshot) return null;
  const payload = JSON.parse(snapshot.payload) as {
    compensationPolicyVersions: unknown[];
    bordrolar: Array<{ status: string }>;
    retroBatches: Array<{ id: string; status: string }>;
  };
  return {
    revision: snapshot.revision,
    policyVersionCount: payload.compensationPolicyVersions.length,
    payrollStatuses: payload.bordrolar.map((payroll) => payroll.status),
    retroBatchStatuses: payload.retroBatches.map((batch) => ({ id: batch.id, status: batch.status })),
  };
}

async function readRawSnapshotState() {
  const snapshot = await browserPayrollStore.loadSnapshot();
  return snapshot ? { revision: snapshot.revision, payload: snapshot.payload } : null;
}

function acceptancePeriods() {
  return [
    ...[9, 10, 11, 12].map((month) => createBordroDonemi(2026, month, 2026, month)),
    ...[1, 2, 3, 4, 5, 6, 7, 8].map((month) => createBordroDonemi(2027, month, 2027, month)),
  ];
}

function allWorkingDays(personnelId: string, period: PayrollStorageDto['donemler'][number]) {
  const days: Record<string, string> = {};
  const date = new Date(period.baslangicTarihi + 'T00:00:00Z');
  const end = new Date(period.bitisTarihi + 'T00:00:00Z');
  while (date <= end) {
    days[date.toISOString().slice(0, 10)] = 'Ç';
    date.setUTCDate(date.getUTCDate() + 1);
  }
  return {
    id: personnelId + '_' + period.id,
    personelId: personnelId,
    donemId: period.id,
    gunler: days,
  };
}

function makeBrowserAcceptancePayload(): PayrollStorageDto {
  const sample = getInitialDataset();
  const personnel = sample.personeller[0];
  const sourceSettings = sample.kurumDegerleriMap[sample.aktifDonemId];
  const annual2026 = getDefaultAnnualPayrollParameters(2026);
  if (!personnel || !sourceSettings || !annual2026) {
    throw new Error('Browser acceptance seed inputs are incomplete.');
  }
  const periods = acceptancePeriods();
  const settings = Object.fromEntries(periods.map((period) => [period.id, {
    ...sourceSettings,
    donemId: period.id,
    gunlukAsgariUcret: period.taxYear === 2026 ? 1000 : 1200,
    pekTavanKatsayisi: 7.5,
    gunlukYemekIstisnasiSGK: 100,
    gunlukYemekIstisnasiGV: 100,
    statutoryParameterSnapshot: null,
    statutoryParameterSegments: [],
  }]));
  const annual2027 = {
    ...annual2026,
    year: 2027,
    sigortaGvYillikBrutAsgariUcretTavani: 180000,
  };
  return toPayrollBoundaryDto({
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: '2026-10-06T00:00:00Z',
    donemler: periods,
    aktifDonemId: '2026-09',
    personeller: [personnel],
    kurumDegerleriMap: settings,
    puantajlar: periods.map((period) => allWorkingDays(personnel.id, period)),
    bordrolar: [],
    taxOpenings: [
      {
        id: personnel.id + '_2026',
        personnelId: personnel.id,
        year: 2026,
        gvCumulativeOpening: 1234.567,
        effectiveFromPeriodId: '2026-09',
        asgariGvCumulativeOpening: 456.789,
        asgariGvEffectiveFromPeriodId: '2026-09',
        createdAt: null,
        updatedAt: null,
      },
      {
        id: personnel.id + '_2027',
        personnelId: personnel.id,
        year: 2027,
        gvCumulativeOpening: 2345.678,
        effectiveFromPeriodId: '2027-01',
        asgariGvCumulativeOpening: 567.891,
        asgariGvEffectiveFromPeriodId: '2027-01',
        createdAt: null,
        updatedAt: null,
      },
    ],
    sickLeaveRecords: [],
    annualPayrollParameters: [annual2026, annual2027],
    compensationRevisions: [],
    compensationRevisionOverrides: [],
    retroBatches: [],
    retroAllocations: [],
    compensationPolicyVersions: [],
  }) as unknown as PayrollStorageDto;
}

function browserAcceptanceState(): CompensationPolicyStateWire {
  return {
    gunlukTabanUcret: '2000.125',
    gunlukYemek: '250.25',
    birlestirilmisSosyalYardim: '3000.50',
    gunlukVasitaYol: '75.125',
    giyimYardimi: '125',
    hizmetZammiBirimi: '10.125',
    isPrimiYuzde: null,
    isPrimiGruplari: [{ id: '1. Grup', ad: 'Grup 1', oran: '9', aktif: true }],
    geceCalismaPrimiYuzde: '0',
    geceCalismaTatiliPrimiYuzde: '0',
    ekOdeme: '0',
    digerGelirVarsayilan: '0',
  };
}

function acceptanceScope(period: PayrollStorageDto['donemler'][number]): Scope {
  return {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId: period.id,
    periodStartInclusive: period.baslangicTarihi,
    periodEndInclusive: period.bitisTarihi,
    projectionDate: period.baslangicTarihi,
  };
}

function payrollDatasetFromPayload(payload: PayrollStorageDto): PayrollDatasetSnapshot {
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
  } as unknown as PayrollDatasetSnapshot;
}

function acceptanceNormalRequest(
  payload: PayrollStorageDto,
  periodId: string
): PayrollCalculationRequest {
  const period = payload.donemler.find((item) => item.id === periodId);
  const personnel = payload.personeller[0];
  if (!period || !personnel) throw new Error('Acceptance period or personnel is missing.');
  return {
    personnelId: personnel.id,
    periodId,
    calculatedAt: period.bitisTarihi + 'T12:00:00Z',
    manualIncome: null,
    accrual: {
      accrualId: personnel.id + '-' + period.id,
      accrualType: 'NORMAL',
      paymentDate: period.id + '-25',
      sequence: 0,
      grossAmount: null,
      description: 'Chromium 2026-2027 history-free acceptance',
    },
    dataset: payrollDatasetFromPayload(payload),
  };
}

function browserAcceptanceSummary(payload: PayrollStorageDto | null, isDataLoaded: boolean, loadError: string | null) {
  return {
    isDataLoaded,
    loadError,
    activePeriodId: payload?.aktifDonemId ?? null,
    personName: payload?.personeller[0]?.ad ?? null,
    policyVersionIds: payload?.compensationPolicyVersions.map((version) => version.id) ?? [],
    payrollStatuses: payload?.bordrolar.map((payroll) => payroll.status) ?? [],
    payrollPeriods: payload?.bordrolar.map((payroll) => payroll.donemId) ?? [],
  };
}

type BrowserAcceptanceSegment = { startDate: string; endDate: string; versionId: string };
type BrowserAcceptanceTwelvePeriodResult = {
  payrollCount: number;
  periods: string[];
  statuses: string[];
  versionIds: string[];
  marchSegments: BrowserAcceptanceSegment[];
  septemberSegments: BrowserAcceptanceSegment[];
  runtimeRaiseCalendarAbsent: boolean;
  annual2027Cap: unknown;
  taxOpening2026: unknown;
};
type BrowserAcceptanceRestoreResult = ReturnType<typeof browserAcceptanceSummary> & {
  sentinelReplaced: boolean;
  hasLoadedBackup: boolean;
};
type BrowserAcceptancePostRestoreResult = {
  freshEngineKind: string;
  normalStatus: string;
  normalPolicyVersionIds: string[];
  retroStatus: string;
  retroPayable: string;
  retroAllocationCount: number;
  deletedPeriodId: string;
  policyTimeline: PayrollStorageDto['compensationPolicyVersions'];
  policyTimelineUnchanged: boolean;
};
type BrowserAcceptanceV5Result = {
  missingBaselineError: string;
  policyVersionCount: number;
  baselineVersionId: string;
  calculatedStatus: string;
  calculatedVersionIds: string[];
};

function isPositiveDecimalText(value: string): boolean {
  return !value.trim().startsWith('-') && /[1-9]/.test(value.replace(/[^0-9]/g, ''));
}

function browserAppBaselineOperation(effectiveFrom: string): CompensationPolicyWriteOperation {
  return {
    kind: 'INITIAL_BASELINE',
    effectiveFrom,
    state: browserAcceptanceState(),
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PER_COMPENSATION_SLICE',
    reason: 'Baseline after browser acceptance restore',
  };
}

async function saveBrowserPolicyOperation(
  scope: Scope,
  operation: CompensationPolicyWriteOperation,
  baseline: boolean
) {
  const api = getCompensationPolicyApi();
  const preview = await api.preview({ scope, operation });
  if (preview.ok === false) {
    throw new Error('Browser policy preview failed: ' + preview.error.code + ': ' + preview.error.message);
  }
  let request: CompensationPolicySaveRequest;
  if (baseline) {
    if (operation.kind !== 'INITIAL_BASELINE') {
      throw new Error('Initial baseline save requires an INITIAL_BASELINE operation.');
    }
    request = {
      scope,
      operation,
      previewToken: preview.value.previewToken,
      initialBaselineConfirmation: { confirmed: true, effectiveFrom: operation.effectiveFrom },
    };
  } else {
    if (operation.kind === 'INITIAL_BASELINE') {
      throw new Error('A baseline operation requires explicit baseline confirmation.');
    }
    request = { scope, operation, previewToken: preview.value.previewToken };
  }
  const saved = await api.save(request);
  if (saved.ok === false) {
    throw new Error('Browser policy save failed: ' + saved.error.code + ': ' + saved.error.message);
  }
  return saved.value;
}

let acceptanceApplicationRoot: Root | null = null;
let missingBaselineApplicationRoot: Root | null = null;

function BrowserAcceptanceApplication() {
  const payrollEngine = getPayrollEngine();
  const [authoritativePayload, setAuthoritativePayload] = useState<PayrollStorageDto | null>(null);
  const [isDataLoaded, setIsDataLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const browserPersistence = useBrowserPayrollPersistence({
    authoritativePayload,
    isDataLoaded,
    isNative: false,
    payrollEngine,
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
  });
  const loadData = useCallback(async () => {
    const loaded = await browserPersistence.loadSnapshot();
    setAuthoritativePayload(loaded);
    setIsDataLoaded(loaded !== null);
    return loaded !== null;
  }, [browserPersistence.loadSnapshot]);
  useEffect(() => {
    void loadData().catch((error) => setLoadError(String(error)));
  }, [loadData]);
  const evaluateBrowserMutations = useCallback(async (mutation: Parameters<typeof payrollEngine.evaluateMutationPolicy>[0]) => {
    if (!authoritativePayload) throw new Error('Application dataset is not loaded.');
    return await payrollEngine.evaluateMutationPolicy(
      mutation,
      payrollDatasetFromPayload(authoritativePayload)
    );
  }, [authoritativePayload, payrollEngine]);
  const backupController = useBackupController({
    authoritativePayload,
    activePeriodId: authoritativePayload?.aktifDonemId ?? '',
    isDataLoaded,
    browserPersistence: {
      markClean: browserPersistence.markClean,
      replacePayload: browserPersistence.replacePayload,
    },
    evaluateBrowserMutations,
    payrollEngine,
    loadData,
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
  });
  const backupRef = useRef<string | null>(null);

  const persistApplicationPayload = useCallback(async (payload: PayrollStorageDto) => {
    const serialized = serializePayrollStorage(payload);
    await browserPersistence.savePayload(serialized, 'current');
    browserPersistence.markClean();
    setAuthoritativePayload(payload);
    setIsDataLoaded(true);
    setLoadError(null);
  }, [browserPersistence.markClean, browserPersistence.savePayload]);

  const loadApplicationPayload = useCallback(async () => {
    const payload = await browserPersistence.loadSnapshot();
    if (!payload) throw new Error('Application IndexedDB snapshot is missing.');
    setAuthoritativePayload(payload);
    setIsDataLoaded(true);
    return payload;
  }, [browserPersistence.loadSnapshot]);

  useEffect(() => {
    window.__compensationAcceptanceBrowserApp = {
      mount: mountAcceptanceApplication,
      mountMissingBaselineGate,
      read: () => browserAcceptanceSummary(authoritativePayload, isDataLoaded, loadError),
      runTwelvePeriodSeries: async () => {
        if (payrollEngine.kind !== 'wasm') throw new Error('Browser acceptance did not select generated WASM.');
        if (await browserPersistence.loadSnapshot()) {
          throw new Error('Isolated browser acceptance database was expected to be empty.');
        }
        const seed = makeBrowserAcceptancePayload();
        await persistApplicationPayload(seed);
        let current = await loadApplicationPayload();

        const baselineOperation: CompensationPolicyWriteOperation = {
          kind: 'INITIAL_BASELINE',
          effectiveFrom: '2026-09-15',
          state: browserAcceptanceState(),
          boundaryScope: 'FULL_EARNING',
          roadRoundingMode: 'PER_COMPENSATION_SLICE',
          reason: 'Synthetic history-free browser acceptance baseline',
        };
        const baseline = await saveBrowserPolicyOperation(
          acceptanceScope(current.donemler[0]),
          baselineOperation,
          true
        );
        current = await loadApplicationPayload();
        const marchState: CompensationPolicyStateWire = {
          ...browserAcceptanceState(),
          gunlukTabanUcret: '2015.125',
          gunlukYemek: '252.25',
        };
        const march = await saveBrowserPolicyOperation(
          acceptanceScope(current.donemler[5]),
          {
            kind: 'NEW_EFFECTIVE_DATE',
            effectiveFrom: '2027-03-01',
            state: marchState,
            boundaryScope: 'FULL_EARNING',
            changeIntent: 'FULL_STATE_CHANGE',
            roadRoundingMode: 'PER_COMPENSATION_SLICE',
            reason: 'Synthetic 2027-03-01 state change',
          },
          false
        );
        current = await loadApplicationPayload();
        const septemberState: CompensationPolicyStateWire = {
          ...marchState,
          gunlukVasitaYol: '78.125',
        };
        const september = await saveBrowserPolicyOperation(
          acceptanceScope(current.donemler[11]),
          {
            kind: 'NEW_EFFECTIVE_DATE',
            effectiveFrom: '2027-09-01',
            state: septemberState,
            boundaryScope: 'FULL_EARNING',
            changeIntent: 'FULL_STATE_CHANGE',
            roadRoundingMode: 'PER_COMPENSATION_SLICE',
            reason: 'Synthetic 2027-09-01 state change',
          },
          false
        );
        current = await loadApplicationPayload();
        for (const period of current.donemler) {
          const request = acceptanceNormalRequest(current, period.id);
          const calculated = await payrollEngine.calculatePayroll(request);
          const withCalculated: PayrollStorageDto = {
            ...current,
            bordrolar: [...current.bordrolar, calculated],
          };
          await persistApplicationPayload(withCalculated);
          const finalized = await payrollEngine.finalizePayroll(
            request.personnelId,
            period.id,
            payrollDatasetFromPayload(withCalculated),
            calculated.accrualId
          );
          current = {
            ...withCalculated,
            bordrolar: withCalculated.bordrolar.map((payroll) =>
              payroll.id === finalized.id ? finalized : payroll
            ),
          };
          await persistApplicationPayload(current);
        }
        const februaryPayroll = current.bordrolar.find((payroll) => payroll.donemId === '2027-02');
        const augustPayroll = current.bordrolar.find((payroll) => payroll.donemId === '2027-08');
        if (!februaryPayroll || !augustPayroll) throw new Error('Internal-boundary payroll records are missing.');
        const marchSegments = februaryPayroll.calculationMetadata?.snapshot?.compensation.segments ?? [];
        const septemberSegments = augustPayroll.calculationMetadata?.snapshot?.compensation.segments ?? [];
        if (
          marchSegments.length !== 2 ||
          marchSegments[0].startDate !== '2027-02-15' ||
          marchSegments[0].endDate !== '2027-02-28' ||
          marchSegments[0].versionId !== baseline.version.id ||
          marchSegments[1].startDate !== '2027-03-01' ||
          marchSegments[1].endDate !== '2027-03-14' ||
          marchSegments[1].versionId !== march.version.id
        ) {
          throw new Error('Chromium March boundary segments do not match the saved policy version IDs.');
        }
        if (
          septemberSegments.length !== 2 ||
          septemberSegments[0].startDate !== '2027-08-15' ||
          septemberSegments[0].endDate !== '2027-08-31' ||
          septemberSegments[0].versionId !== march.version.id ||
          septemberSegments[1].startDate !== '2027-09-01' ||
          septemberSegments[1].endDate !== '2027-09-14' ||
          septemberSegments[1].versionId !== september.version.id
        ) {
          throw new Error('Chromium September boundary segments do not match the saved policy version IDs.');
        }
        await validatePayrollMetadataThroughCore(current);
        backupRef.current = serializePayrollStorage(current);
        return {
          payrollCount: current.bordrolar.length,
          periods: current.donemler.map((period) => period.id),
          statuses: current.bordrolar.map((payroll) => payroll.status),
          versionIds: [baseline.version.id, march.version.id, september.version.id],
          marchSegments,
          septemberSegments,
          runtimeRaiseCalendarAbsent: !('zamAylari' in payrollDatasetFromPayload(current)),
          annual2027Cap: current.annualPayrollParameters.find((item) => item.year === 2027)
            ?.sigortaGvYillikBrutAsgariUcretTavani,
          taxOpening2026: current.taxOpenings.find((opening) => opening.year === 2026)
            ?.gvCumulativeOpening,
        };
      },
      persistSentinelState: async () => {
        if (!authoritativePayload || !backupRef.current) throw new Error('V6 acceptance backup is not ready.');
        const sentinel: PayrollStorageDto = {
          ...authoritativePayload,
          aktifDonemId: '2027-08',
          personeller: authoritativePayload.personeller.map((personnel) => ({
            ...personnel,
            ad: 'SENTINEL BEFORE RESTORE',
          })),
          bordrolar: [],
          compensationRevisions: [],
          compensationRevisionOverrides: [],
          retroBatches: [],
          retroAllocations: [],
          compensationPolicyVersions: authoritativePayload.compensationPolicyVersions.length > 0
            ? [{
              ...authoritativePayload.compensationPolicyVersions[0],
              id: 'sentinel-unrelated-policy-history',
              effectiveFrom: '2028-01-15',
              supersedesVersionId: null,
            }]
            : [],
        };
        await browserPersistence.replacePayload(serializePayrollStorage(sentinel), 'current');
        browserPersistence.markClean();
        setAuthoritativePayload(sentinel);
        setIsDataLoaded(true);
        return browserAcceptanceSummary(sentinel, true, null);
      },
      importV6BackupOverSentinel: async () => {
        if (!backupRef.current) throw new Error('V6 acceptance backup is missing.');
        await backupController.handleImportBackup(backupRef.current);
        const restored = await loadApplicationPayload();
        return {
          ...browserAcceptanceSummary(restored, true, null),
          sentinelReplaced: restored.personeller[0]?.ad !== 'SENTINEL BEFORE RESTORE',
          hasLoadedBackup: restored.bordrolar.length === 12,
        };
      },
      runAfterFreshRestore: async () => {
        let current = await loadApplicationPayload();
        const personnel = current.personeller[0];
        if (!personnel) throw new Error('Restored personnel is missing.');
        const nextPeriod = createBordroDonemi(2027, 9, 2027, 9);
        const sourceSettings = current.kurumDegerleriMap['2027-08'];
        if (!sourceSettings) throw new Error('Restored 2027 settings are missing.');
        current = {
          ...current,
          aktifDonemId: nextPeriod.id,
          donemler: [...current.donemler, nextPeriod],
          kurumDegerleriMap: {
            ...current.kurumDegerleriMap,
            [nextPeriod.id]: {
              ...sourceSettings,
              donemId: nextPeriod.id,
              gunlukAsgariUcret: '1200',
              statutoryParameterSnapshot: null,
              statutoryParameterSegments: [],
            },
          },
          puantajlar: [...current.puantajlar, allWorkingDays(personnel.id, nextPeriod)],
        };
        await persistApplicationPayload(current);
        const request = acceptanceNormalRequest(current, nextPeriod.id);
        const calculated = await payrollEngine.calculatePayroll(request);
        const withCalculated = { ...current, bordrolar: [...current.bordrolar, calculated] };
        await persistApplicationPayload(withCalculated);
        const finalized = await payrollEngine.finalizePayroll(
          personnel.id,
          nextPeriod.id,
          payrollDatasetFromPayload(withCalculated),
          calculated.accrualId
        );
        current = {
          ...withCalculated,
          bordrolar: withCalculated.bordrolar.map((payroll) =>
            payroll.id === finalized.id ? finalized : payroll
          ),
        };
        await persistApplicationPayload(current);

        const restoredSeptemberPolicy = current.compensationPolicyVersions.find(
          (version) => version.effectiveFrom === '2027-09-01'
        );
        const normalSegments = finalized.calculationMetadata?.snapshot?.compensation.segments ?? [];
        if (
          !restoredSeptemberPolicy ||
          normalSegments.length !== 1 ||
          normalSegments[0].startDate !== nextPeriod.baslangicTarihi ||
          normalSegments[0].endDate !== nextPeriod.bitisTarihi ||
          normalSegments[0].versionId !== restoredSeptemberPolicy.id
        ) {
          throw new Error('Fresh browser runtime did not use the restored September policy version.');
        }

        const revision: PayrollStorageDto['compensationRevisions'][number] = {
          id: 'browser-post-restore-retro-revision',
          reason: 'COLLECTIVE_AGREEMENT',
          title: 'Post-restore synthetic entitlement revision',
          effectiveFrom: '2027-08-15',
          effectiveTo: null,
          decisionDate: '2027-09-20',
          signedAt: '2027-09-20',
          description: 'Replay from the restored IndexedDB ledger',
          status: 'DRAFT',
          scope: 'SELECTED_PERSONNEL',
          personnelIds: [personnel.id],
          personnelGroup: null,
          createdAt: '2026-10-06T00:00:00Z',
          updatedAt: null,
        };
        const override: PayrollStorageDto['compensationRevisionOverrides'][number] = {
          id: 'browser-post-restore-retro-wage',
          revisionId: revision.id,
          parameter: 'GUNLUK_TABAN_UCRET',
          value: '2016.13',
          personnelId: personnel.id,
        };
        current = {
          ...current,
          compensationRevisions: [...current.compensationRevisions, revision],
          compensationRevisionOverrides: [...current.compensationRevisionOverrides, override],
        };
        await persistApplicationPayload(current);
        current = await loadApplicationPayload();
        const retro = await payrollEngine.calculateRetroPreview({
          batchId: 'browser-post-restore-retro-batch',
          revision,
          overrides: [override],
          personnelId: personnel.id,
          paymentDate: '2027-09-25',
          calculatedAt: '2026-10-06T00:00:00Z',
          description: 'Post-restore retro calculation',
          dataset: payrollDatasetFromPayload(current),
        });
        if (retro.batch.status !== 'CALCULATED' || !isPositiveDecimalText(retro.batch.payableSettlementAmount)) {
          throw new Error('Fresh browser runtime did not calculate a payable retro batch.');
        }

        const deletable = createBordroDonemi(2028, 1, 2028, 1);
        const deletionSettingsSource = current.kurumDegerleriMap[nextPeriod.id];
        if (!deletionSettingsSource) throw new Error('New-period settings are missing for deletion fixture.');
        current = {
          ...current,
          donemler: [...current.donemler, deletable],
          kurumDegerleriMap: {
            ...current.kurumDegerleriMap,
            [deletable.id]: { ...deletionSettingsSource, donemId: deletable.id },
          },
          puantajlar: [...current.puantajlar, allWorkingDays(personnel.id, deletable)],
        };
        await persistApplicationPayload(current);
        const beforeTimeline = JSON.stringify(current.compensationPolicyVersions);
        const impact = await payrollEngine.evaluateMutationPolicy(
          {
            kind: 'PERIOD_FROM_POSITION',
            startDate: deletable.baslangicTarihi,
            taxYear: deletable.taxYear,
            taxMonth: deletable.taxMonth,
          },
          payrollDatasetFromPayload(current)
        );
        const deleted = deletePeriodFromPayload(current, deletable.id, impact);
        if (JSON.stringify(deleted.compensationPolicyVersions) !== beforeTimeline) {
          throw new Error('Browser period deletion changed the immutable policy timeline.');
        }
        await persistApplicationPayload(deleted);
        const readBack = await loadApplicationPayload();
        if (
          readBack.donemler.some((period) => period.id === deletable.id) ||
          JSON.stringify(readBack.compensationPolicyVersions) !== beforeTimeline
        ) {
          throw new Error('IndexedDB period deletion readback did not preserve the exact policy timeline.');
        }
        return {
          freshEngineKind: payrollEngine.kind,
          normalStatus: finalized.status,
          normalPolicyVersionIds: finalized.calculationMetadata?.snapshot?.compensation.segments
            .map((segment) => segment.versionId) ?? [],
          retroStatus: retro.batch.status,
          retroPayable: retro.batch.payableSettlementAmount,
          retroAllocationCount: retro.allocations.length,
          deletedPeriodId: deletable.id,
          policyTimeline: readBack.compensationPolicyVersions,
          policyTimelineUnchanged: true,
        };
      },
      runV5RestoreBaselineGate: async () => {
        const currentPayload = makeBrowserAcceptancePayload();
        const preexistingV6: PayrollStorageDto = {
          ...currentPayload,
          bordrolar: [],
          compensationPolicyVersions: [{
            id: 'unrelated-v6-history-before-v5-restore',
            effectiveFrom: currentPayload.donemler[0].baslangicTarihi,
            state: browserAcceptanceState(),
            boundaryScope: 'FULL_EARNING',
            roadRoundingMode: 'PER_COMPENSATION_SLICE',
            createdAt: '2026-10-06T00:00:00.000Z',
            source: 'RESTORE_FIXTURE',
            reason: 'V5 restore replacement fixture',
            supersedesVersionId: null,
          }],
        };
        await browserPersistence.replacePayload(serializePayrollStorage(preexistingV6), 'current');
        browserPersistence.markClean();
        const v5Object: Record<string, unknown> = {
          ...currentPayload,
          backupVersion: 5,
          kurumDegerleriMap: Object.fromEntries(
            Object.entries(currentPayload.kurumDegerleriMap).map(([periodId, settings]) => [
              periodId,
              { ...settings, ...browserAcceptanceState() },
            ])
          ),
          zamAylari: [],
        };
        delete v5Object.compensationPolicyVersions;
        await backupController.handleImportBackup(JSON.stringify(v5Object));
        let current = await loadApplicationPayload();
        if (current.compensationPolicyVersions.length !== 0) {
          throw new Error('V5 restore unexpectedly imported V6 compensation history.');
        }
        let missingBaselineError = '';
        try {
          await payrollEngine.calculatePayroll(
            acceptanceNormalRequest(current, current.donemler[0].id)
          );
        } catch (error) {
          missingBaselineError = error instanceof Error
            ? error.message
            : typeof error === 'string'
              ? error
              : JSON.stringify(error);
        }
        if (!missingBaselineError || !/ücret politikası|policy/i.test(missingBaselineError)) {
          throw new Error('V5 policy calculation did not fail closed without a baseline: ' + missingBaselineError);
        }
        if (current.bordrolar.length !== 0) {
          throw new Error('Failed V5 calculation mutated the restored payroll ledger.');
        }
        const first = current.donemler[0];
        const baseline = await saveBrowserPolicyOperation(
          acceptanceScope(first),
          browserAppBaselineOperation(first.baslangicTarihi),
          true
        );
        current = await loadApplicationPayload();
        const calculated = await payrollEngine.calculatePayroll(
          acceptanceNormalRequest(current, first.id)
        );
        if (calculated.status !== 'CALCULATED') {
          throw new Error('V5 policy calculation did not succeed after the baseline API save.');
        }
        const calculatedSegments = calculated.calculationMetadata?.snapshot?.compensation.segments ?? [];
        if (
          calculatedSegments.length !== 1 ||
          calculatedSegments[0].versionId !== baseline.version.id
        ) {
          throw new Error('V5 post-baseline calculation did not use the API-saved version.');
        }
        current = { ...current, bordrolar: [...current.bordrolar, calculated] };
        await persistApplicationPayload(current);
        return {
          missingBaselineError,
          policyVersionCount: current.compensationPolicyVersions.length,
          baselineVersionId: baseline.version.id,
          calculatedStatus: calculated.status,
          calculatedVersionIds: calculated.calculationMetadata?.snapshot?.compensation.segments
            .map((segment) => segment.versionId) ?? [],
        };
      },
    };
  }, [
    authoritativePayload,
    backupController.handleImportBackup,
    browserPersistence,
    isDataLoaded,
    loadError,
    loadApplicationPayload,
    payrollEngine,
    persistApplicationPayload,
  ]);

  return createElement('div', { 'data-compensation-acceptance-app': 'mounted' });
}

function mountAcceptanceApplication() {
  let rootElement = document.getElementById('compensation-acceptance-root');
  if (!rootElement) {
    rootElement = document.createElement('div');
    rootElement.id = 'compensation-acceptance-root';
    document.body.appendChild(rootElement);
  }
  acceptanceApplicationRoot?.unmount();
  acceptanceApplicationRoot = createRoot(rootElement);
  acceptanceApplicationRoot.render(createElement(BrowserAcceptanceApplication));
}

function MissingBaselineUiAcceptanceApplication() {
  const data = useMemo(() => getInitialDataset(), []);
  const [route, setRoute] = useState<'payroll' | 'policy'>('payroll');
  const [savedCount, setSavedCount] = useState(0);
  const activePeriod = data.donemler.find((period) => period.id === data.aktifDonemId);
  const personnel = data.personeller.find((person) =>
    data.puantajlar.some((item) => item.personelId === person.id && item.donemId === data.aktifDonemId)
  );
  const dataset = useMemo<PayrollDatasetSnapshot>(() => toPayrollBoundaryDto({
    personnel: data.personeller,
    periods: data.donemler,
    institutionSettings: data.kurumDegerleriMap,
    attendances: data.puantajlar,
    payrolls: [],
    taxOpenings: data.taxOpenings,
    sickLeaveRecords: data.sickLeaveRecords,
    annualPayrollParameters: data.annualPayrollParameters,
    compensationRevisions: [],
    compensationRevisionOverrides: [],
    retroBatches: [],
    retroAllocations: [],
    compensationPolicyVersions: [],
  }) as PayrollDatasetSnapshot, [data]);

  if (!activePeriod || !personnel) {
    return createElement('p', { 'data-testid': 'missing-baseline-ui-error' }, 'Sample UI fixture is incomplete.');
  }
  if (route === 'policy') {
    return createElement('div', {
      'data-testid': 'missing-baseline-ui-app',
      'data-route': 'policy',
      'data-person-id': personnel.id,
      'data-saved-count': savedCount,
    }, createElement('h1', { 'data-testid': 'compensation-policy-screen' }, 'Ücret Politikası'));
  }
  return createElement('div', {
    'data-testid': 'missing-baseline-ui-app',
    'data-route': 'payroll',
    'data-person-id': personnel.id,
    'data-saved-count': savedCount,
  }, createElement(BordroHesaplama, {
    aktifDonem: activePeriod,
    donemler: data.donemler,
    personeller: [personnel],
    kurumDegerleriMap: data.kurumDegerleriMap,
    puantajlar: data.puantajlar.filter((item) => item.personelId === personnel.id),
    bordrolar: [],
    taxOpenings: data.taxOpenings,
    sickLeaveRecords: data.sickLeaveRecords,
    annualPayrollParameters: data.annualPayrollParameters,
    activePayrollView: 'normal',
    authoritativeDataset: dataset,
    onDeleteBordro: async () => undefined,
    onSaveBordro: async () => setSavedCount((count) => count + 1),
    onSavePersonelAndTaxOpening: async () => undefined,
    initialPersonelId: personnel.id,
    onOpenCompensationPolicy: () => setRoute('policy'),
  }));
}

function mountMissingBaselineGate() {
  let rootElement = document.getElementById('missing-baseline-ui-root');
  if (!rootElement) {
    rootElement = document.createElement('div');
    rootElement.id = 'missing-baseline-ui-root';
    document.body.appendChild(rootElement);
  }
  missingBaselineApplicationRoot?.unmount();
  missingBaselineApplicationRoot = createRoot(rootElement);
  missingBaselineApplicationRoot.render(createElement(MissingBaselineUiAcceptanceApplication));
}

declare global {
  interface Window {
    __compensationAcceptanceBrowserApp: {
      mount(): void;
      mountMissingBaselineGate(): void;
      read(): ReturnType<typeof browserAcceptanceSummary>;
      runTwelvePeriodSeries(): Promise<BrowserAcceptanceTwelvePeriodResult>;
      persistSentinelState(): Promise<ReturnType<typeof browserAcceptanceSummary>>;
      importV6BackupOverSentinel(): Promise<BrowserAcceptanceRestoreResult>;
      runAfterFreshRestore(): Promise<BrowserAcceptancePostRestoreResult>;
      runV5RestoreBaselineGate(): Promise<BrowserAcceptanceV5Result>;
    };
    __compensationPolicyBrowserApiTest: {
      loadArchivedSeptemberPrefill(): ReturnType<typeof loadArchivedSeptemberPrefill>;
      exerciseLinkedRoundingRepair(): ReturnType<typeof exerciseLinkedRoundingRepair>;
      exerciseRoadRoundingRecovery(): ReturnType<typeof exerciseRoadRoundingRecovery>;
      exerciseFuturePeriodRoundingConflict(): ReturnType<typeof exerciseFuturePeriodRoundingConflict>;
      exerciseSyntheticPolicySchedule(): ReturnType<typeof exerciseSyntheticPolicySchedule>;
      prepareBaseline(): ReturnType<typeof prepareBaseline>;
      prepareRetroBaseline(): ReturnType<typeof prepareRetroBaseline>;
      exerciseWorkPremiumGroupSave(): ReturnType<typeof exerciseWorkPremiumGroupSave>;
      previewBaseline(periodId: string): ReturnType<typeof previewBaseline>;
      saveBaseline(prepared: Awaited<ReturnType<typeof previewBaseline>>): ReturnType<typeof saveBaseline>;
      readAuthoritativeSnapshot(): ReturnType<typeof readAuthoritativeSnapshot>;
      previewRoadOnlyScaleChange(periodId: string): ReturnType<typeof previewRoadOnlyScaleChange>;
      savePreparedOperation(prepared: Awaited<ReturnType<typeof previewRoadOnlyScaleChange>>): ReturnType<typeof savePreparedOperation>;
      saveOperationWithAbortedPut(prepared: Awaited<ReturnType<typeof previewRoadOnlyScaleChange>>): ReturnType<typeof saveOperationWithAbortedPut>;
      readRawSnapshotState(): ReturnType<typeof readRawSnapshotState>;
    };
  }
}

window.__compensationAcceptanceBrowserApp = {
  mount: mountAcceptanceApplication,
  mountMissingBaselineGate,
  read: () => browserAcceptanceSummary(null, false, 'Application has not mounted.'),
  runTwelvePeriodSeries: async () => {
    throw new Error('Application has not mounted.');
  },
  persistSentinelState: async () => {
    throw new Error('Application has not mounted.');
  },
  importV6BackupOverSentinel: async () => {
    throw new Error('Application has not mounted.');
  },
  runAfterFreshRestore: async () => {
    throw new Error('Application has not mounted.');
  },
  runV5RestoreBaselineGate: async () => {
    throw new Error('Application has not mounted.');
  },
};

window.__compensationPolicyBrowserApiTest = {
  loadArchivedSeptemberPrefill,
  exerciseLinkedRoundingRepair,
  exerciseRoadRoundingRecovery,
  exerciseFuturePeriodRoundingConflict,
  exerciseSyntheticPolicySchedule,
  prepareBaseline,
  prepareRetroBaseline,
  exerciseWorkPremiumGroupSave,
  previewBaseline,
  saveBaseline,
  readAuthoritativeSnapshot,
  previewRoadOnlyScaleChange,
  savePreparedOperation,
  saveOperationWithAbortedPut,
  readRawSnapshotState,
};
