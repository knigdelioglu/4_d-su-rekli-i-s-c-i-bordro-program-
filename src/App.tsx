/**
 * 4/D Sürekli İşçi Bordro Programı — Main App Component
 */

import { BulkPayrollProgress } from './components/BulkPayrollProgress';
import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Sidebar } from './components/Sidebar';
import { TopBar } from './components/TopBar';
import { AppViewportFrame } from './components/AppViewportFrame';
import {
  isKesintiTipi,
  isParametreSection,
  isPayrollViewType,
  isTabType,
  type KesintiTipi,
  type ParametreSection,
  type PayrollViewType,
  type TabType,
} from './types/navigation';
import {
  BACKUP_FORMAT_VERSION,
  BackupPayload,
  IsPrimiGrupItem,
  PayrollPeriodInstitutionSettings,
} from './types/payroll';
import { tauriBridge } from './services/tauriBridge';
import { getPayrollEngine } from './services/payrollEngine';
import type { PayrollDatasetSnapshot } from './services/payrollEngine';
import {
  serializePayrollStorage,
  toPayrollBoundaryDto,
  toPayrollUiModel,
  type PayrollStorageDto,
  type PayrollStorageFields,
} from './services/payrollEngine/decimalBoundary';
import { useBrowserPayrollPersistence } from './services/storage/useBrowserPayrollPersistence';
import {
  queueAuthoritativePayloadCommit,
  type AuthoritativePayloadCommitQueue,
} from './services/storage/authoritativePayloadCommit';
import { usePayrollMutationController } from './hooks/usePayrollMutationController';
import { useBackupController } from './hooks/useBackupController';
import { useCompensationPolicyController } from './hooks/useCompensationPolicyController';
import { getCompensationPolicyApi } from './services/compensationPolicy/api';
import { usePayrollNotices } from './components/PayrollNoticeCenter';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from './utils/payrollPresentation';
import { CheckCircle2, RotateCcw } from 'lucide-react';
import { formatPayrollError } from './components/useBordroCalculationController';
import { importConfirmedBackupFile } from './services/storage/confirmedBackupImport';
import { describeError } from './utils/errorMessage';
import { DEFAULT_IS_PRIMI_GRUPLARI } from './services/storage/payrollDefaults';

const PeriodSettingsPage = React.lazy(() =>
  import('./components/Settings/PeriodSettingsPage').then((module) => ({
    default: module.PeriodSettingsPage,
  }))
);
const CompensationPolicyPage = React.lazy(() =>
  import('./components/Settings/CompensationPolicyPage').then((module) => ({
    default: module.CompensationPolicyPage,
  }))
);
const PersonelList = React.lazy(() =>
  import('./components/PersonelList').then((module) => ({ default: module.PersonelList }))
);
const PuantajGrid = React.lazy(() =>
  import('./components/PuantajGrid').then((module) => ({ default: module.PuantajGrid }))
);
const BordroHesaplama = React.lazy(() =>
  import('./components/BordroHesaplama').then((module) => ({ default: module.BordroHesaplama }))
);
const GeriyeDonukFarklar = React.lazy(() =>
  import('./components/GeriyeDonukFarklar').then((module) => ({ default: module.GeriyeDonukFarklar }))
);
const BankaListesi = React.lazy(() =>
  import('./components/Listeler/BankaListesi').then((module) => ({ default: module.BankaListesi }))
);
const SgkPrimKontrolu = React.lazy(() =>
  import('./components/Listeler/SgkPrimKontrolu').then((module) => ({ default: module.SgkPrimKontrolu }))
);
const KesintiListesi = React.lazy(() =>
  import('./components/Listeler/KesintiListesi').then((module) => ({ default: module.KesintiListesi }))
);
const PeriodSummary = React.lazy(() =>
  import('./components/Dashboard/PeriodSummary').then((module) => ({ default: module.PeriodSummary }))
);
const DataBackupPage = React.lazy(() =>
  import('./components/DataBackupPage').then((module) => ({ default: module.DataBackupPage }))
);

const STORAGE_KEY = '4d_bordro_programi_mvp_v2';
const ACTIVE_TAB_STORAGE_KEY = '4d_bordro_active_tab';
const ACTIVE_KESINTI_STORAGE_KEY = '4d_bordro_active_kesinti';
const ACTIVE_PARAMETRE_STORAGE_KEY = '4d_bordro_active_parametre';
const ACTIVE_PAYROLL_VIEW_STORAGE_KEY = '4d_bordro_active_payroll_view';

type DatasetFields = PayrollStorageFields;
type UiDatasetFields = Omit<
  BackupPayload,
  | 'backupVersion'
  | 'exportedAt'
  | 'kurumDegerleriMap'
  | 'compensationRevisions'
  | 'compensationRevisionOverrides'
  | 'retroBatches'
  | 'retroAllocations'
> &
  { kurumDegerleriMap: Record<string, PayrollPeriodInstitutionSettings> } &
  Required<
    Pick<
      BackupPayload,
      | 'compensationRevisions'
      | 'compensationRevisionOverrides'
      | 'retroBatches'
      | 'retroAllocations'
      | 'isPrimiGruplari'
    >
  >;

function makeBackupPayload(data: DatasetFields): PayrollStorageDto {
  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    ...data,
    kurumDegerleriMap: toPayrollBoundaryDto(data.kurumDegerleriMap),
    isPrimiGruplari: toPayrollBoundaryDto(resolvePremiumGroups(data)),
    compensationPolicyVersions: data.compensationPolicyVersions ?? [],
  };
}

function normalizePremiumGroups(value: unknown): IsPrimiGrupItem[] | null {
  if (!Array.isArray(value) || value.length === 0) return null;
  const groups: IsPrimiGrupItem[] = [];
  for (const item of value) {
    if (!item || typeof item !== 'object') return null;
    const candidate = item as Record<string, unknown>;
    const rate = typeof candidate.oran === 'number'
      ? candidate.oran
      : typeof candidate.oran === 'string' && /^(?:\d+)(?:\.\d+)?$/.test(candidate.oran)
        ? Number(candidate.oran)
        : NaN;
    if (typeof candidate.id !== 'string' || !candidate.id.trim()
      || typeof candidate.ad !== 'string' || !candidate.ad.trim()
      || !Number.isFinite(rate) || rate < 0 || rate > 100
      || (candidate.aktif !== undefined && typeof candidate.aktif !== 'boolean')) return null;
    groups.push({
      id: candidate.id,
      ad: candidate.ad,
      oran: rate,
      aktif: typeof candidate.aktif === 'boolean' ? candidate.aktif : true,
    });
  }
  return groups;
}

function resolvePremiumGroups(data: DatasetFields): IsPrimiGrupItem[] {
  const candidates: unknown[] = [data.isPrimiGruplari];
  const versions = [...(data.compensationPolicyVersions ?? [])].sort((left, right) =>
    left.effectiveFrom.localeCompare(right.effectiveFrom) || left.createdAt.localeCompare(right.createdAt)
  );
  candidates.push(...versions.reverse().map((version) => version.state.isPrimiGruplari));
  const archived = data.legacyInstitutionSettings ?? {};
  candidates.push(archived[data.aktifDonemId]?.isPrimiGruplari);
  candidates.push((data.kurumDegerleriMap as Record<string, unknown>)[data.aktifDonemId]
    && ((data.kurumDegerleriMap as Record<string, Record<string, unknown>>)[data.aktifDonemId]).isPrimiGruplari);
  candidates.push(...Object.values(archived).map((settings) => settings.isPrimiGruplari));
  candidates.push(...Object.values(data.kurumDegerleriMap as Record<string, Record<string, unknown>>)
    .map((settings) => settings?.isPrimiGruplari));
  for (const candidate of candidates) {
    const groups = normalizePremiumGroups(candidate);
    if (groups) return groups;
  }
  return DEFAULT_IS_PRIMI_GRUPLARI.map((group) => ({ ...group }));
}

function parsePremiumGroupsSetting(serialized: string | null): IsPrimiGrupItem[] | null {
  if (serialized === null) return null;
  let value: unknown;
  try {
    value = JSON.parse(serialized);
  } catch {
    throw new Error('İş primi grupları ayarı geçerli JSON değil.');
  }
  return normalizePremiumGroups(value);
}

function getErrorMessage(error: unknown): string {
  return describeError(error);
}

interface UserFacingStorageError {
  userMessage: string;
  technicalDetail: string;
}

function formatBrowserStorageLoadError(error: unknown): UserFacingStorageError {
  return {
    userMessage: 'Tarayıcıdaki bordro verisi okunamadı. Mevcut veriler değiştirilmedi.',
    technicalDetail: getErrorMessage(error),
  };
}

function getInitialActiveKesintiType(): KesintiTipi {
  try {
    const saved = localStorage.getItem(ACTIVE_KESINTI_STORAGE_KEY);
    if (isKesintiTipi(saved)) return saved;
  } catch {
    // localStorage may be unavailable in a restricted browser context.
  }
  return 'sendika';
}

export function getInitialActiveTab(storage?: Pick<Storage, 'getItem'>): TabType {
  try {
    const saved = (storage ?? localStorage).getItem(ACTIVE_TAB_STORAGE_KEY);
    if (isTabType(saved)) return saved;
  } catch {
    // localStorage may be unavailable in a restricted browser context.
  }
  return 'ozet';
}

function getInitialActiveParametreSection(): ParametreSection {
  try {
    const saved = localStorage.getItem(ACTIVE_PARAMETRE_STORAGE_KEY);
    if (isParametreSection(saved)) return saved;
  } catch {
    // localStorage may be unavailable in a restricted browser context.
  }
  return 'gelir';
}

function getInitialActivePayrollView(): PayrollViewType {
  try {
    const saved = localStorage.getItem(ACTIVE_PAYROLL_VIEW_STORAGE_KEY);
    if (isPayrollViewType(saved)) return saved;
  } catch {
    // localStorage may be unavailable in a restricted browser context.
  }
  return 'normal';
}

const EMPTY_UI_DATASET: UiDatasetFields = {
  donemler: [],
  aktifDonemId: '',
  personeller: [],
  kurumDegerleriMap: {},
  puantajlar: [],
  bordrolar: [],
  taxOpenings: [],
  sickLeaveRecords: [],
  annualPayrollParameters: [],
  isPrimiGruplari: DEFAULT_IS_PRIMI_GRUPLARI.map((group) => ({ ...group })),
  compensationRevisions: [],
  compensationRevisionOverrides: [],
  retroBatches: [],
  retroAllocations: [],
};

export default function App() {
  const [activeTab, setActiveTab] = useState<TabType>(getInitialActiveTab);
  const [activeKesintiType, setActiveKesintiType] = useState<KesintiTipi>(
    getInitialActiveKesintiType
  );
  const [activeParametreSection, setActiveParametreSection] = useState<ParametreSection>(
    getInitialActiveParametreSection
  );
  const [activePayrollView, setActivePayrollView] = useState<PayrollViewType>(
    getInitialActivePayrollView
  );

  const [authoritativePayload, setAuthoritativePayloadState] = useState<PayrollStorageDto | null>(null);
  const authoritativePayloadRef = useRef<PayrollStorageDto | null>(null);
  const payloadMutationGeneration = useRef(0);
  const payloadCommitQueue = useRef<AuthoritativePayloadCommitQueue>({ current: Promise.resolve() });
  const setAuthoritativePayload = useCallback((payload: PayrollStorageDto) => {
    authoritativePayloadRef.current = payload;
    setAuthoritativePayloadState(payload);
  }, []);
  const dataLoadGeneration = useRef(0);
  const [isDataLoaded, setIsDataLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [globalSuccess, setGlobalSuccess] = useState<string | null>(null);
  const [isResetConfirmOpen, setIsResetConfirmOpen] = useState(false);
  const [isResettingSampleData, setIsResettingSampleData] = useState(false);
  const [pendingBackupFile, setPendingBackupFile] = useState<File | null>(null);
  const [isImportingBackup, setIsImportingBackup] = useState(false);
  const backupImportCancelRef = useRef<HTMLButtonElement>(null);
  const payrollEngine = getPayrollEngine();

  useEffect(() => {
    if (pendingBackupFile) backupImportCancelRef.current?.focus();
  }, [pendingBackupFile]);

  useEffect(() => {
    if (!pendingBackupFile) return;
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImportingBackup) {
        event.preventDefault();
        setPendingBackupFile(null);
      }
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [pendingBackupFile, isImportingBackup]);

  const [targetPersonelIdForBordro, setTargetPersonelIdForBordro] = useState<
    string | undefined
  >(undefined);
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

  const browserPersistence = useBrowserPayrollPersistence({
    authoritativePayload,
    isDataLoaded,
    isNative: tauriBridge.isTauriAvailable(),
    payrollEngine,
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
  });

  const uiDataset = useMemo<UiDatasetFields>(() => {
    if (!authoritativePayload) return EMPTY_UI_DATASET;
    const decoded = toPayrollUiModel(authoritativePayload) as unknown as BackupPayload;
    return {
      ...decoded,
      kurumDegerleriMap: Object.fromEntries(
        Object.entries(decoded.kurumDegerleriMap).map(([periodId, saved]) => [periodId, {
          ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
          ...saved,
          donemId: periodId,
        }])
      ),
      compensationRevisions: decoded.compensationRevisions ?? [],
      compensationRevisionOverrides: decoded.compensationRevisionOverrides ?? [],
      retroBatches: decoded.retroBatches ?? [],
      retroAllocations: decoded.retroAllocations ?? [],
    } as UiDatasetFields;
  }, [authoritativePayload]);

  const {
    donemler,
    aktifDonemId,
    personeller,
    kurumDegerleriMap,
    puantajlar,
    bordrolar,
    taxOpenings,
    sickLeaveRecords,
    annualPayrollParameters,
    compensationRevisions,
    compensationRevisionOverrides,
    retroBatches,
    retroAllocations,
    isPrimiGruplari,
  } = uiDataset;

  useEffect(() => {
    try {
      localStorage.setItem(ACTIVE_TAB_STORAGE_KEY, activeTab);
    } catch {
      // Ignore navigation preference write errors.
    }
  }, [activeTab]);

  useEffect(() => {
    try {
      localStorage.setItem(ACTIVE_KESINTI_STORAGE_KEY, activeKesintiType);
    } catch {
      // Ignore deduction navigation preference write errors.
    }
  }, [activeKesintiType]);

  useEffect(() => {
    try {
      localStorage.setItem(ACTIVE_PARAMETRE_STORAGE_KEY, activeParametreSection);
    } catch {
      // Ignore settings navigation preference write errors.
    }
  }, [activeParametreSection]);

  useEffect(() => {
    try {
      localStorage.setItem(ACTIVE_PAYROLL_VIEW_STORAGE_KEY, activePayrollView);
    } catch {
      // Ignore payroll child navigation preference write errors.
    }
  }, [activePayrollView]);

  const applyDataset = useCallback((data: DatasetFields) => {
    browserPersistence.markClean();
    payloadMutationGeneration.current += 1;
    setAuthoritativePayload(makeBackupPayload(data));
  }, [browserPersistence.markClean, setAuthoritativePayload]);

  const updateAuthoritativePayload = useCallback(
    (update: (current: PayrollStorageDto) => PayrollStorageDto) => {
      browserPersistence.markDirty();
      payloadMutationGeneration.current += 1;
      const current = authoritativePayloadRef.current;
      if (!current) return;
      const next = { ...update(current), exportedAt: new Date().toISOString() };
      setAuthoritativePayload(next);
    },
    [browserPersistence.markDirty, setAuthoritativePayload]
  );

  const commitAuthoritativePayload = useCallback(
    (update: (current: PayrollStorageDto) => PayrollStorageDto | Promise<PayrollStorageDto>) => {
      return queueAuthoritativePayloadCommit(
        payloadCommitQueue.current,
        {
          getCurrent: () => authoritativePayloadRef.current,
          getGeneration: () => payloadMutationGeneration.current,
          markDirty: () => {
            browserPersistence.markDirty();
            payloadMutationGeneration.current += 1;
          },
          persist: (payload) => browserPersistence.savePayload(serializePayrollStorage(payload)),
          publish: setAuthoritativePayload,
          missingPayloadMessage: 'Puantaj kaydedilemedi: geçerli veri snapshotı bulunamadı.',
        },
        update
      );
    },
    [browserPersistence.markDirty, browserPersistence.savePayload, setAuthoritativePayload]
  );

  const loadData = useCallback(async (): Promise<boolean> => {
    const generation = ++dataLoadGeneration.current;
    const isLatestLoad = () => generation === dataLoadGeneration.current;
    setLoadError(null);
    const isNative = tauriBridge.isTauriAvailable();

    try {
      if (isNative) {
        const isMigrated = await tauriBridge.checkLegacyMigrated();
        if (!isLatestLoad()) return false;
        if (!isMigrated) {
          const legacyStr = localStorage.getItem(STORAGE_KEY);
          if (legacyStr) {
            // Migration errors intentionally abort native loading. A failed
            // database write must never be presented as a browser-only success.
            await tauriBridge.migrateLegacyPayload(legacyStr);
          }
        }
        if (!isLatestLoad()) return false;

        const [fetchedPeriods, fetchedPersonnel, fetchedAttendance, fetchedPayrolls, fetchedSettings, fetchedTaxOpenings, fetchedSickLeaves, fetchedAnnualParameters, savedActivePeriodId, fetchedRevisions, fetchedRevisionOverrides, fetchedRetroBatches, fetchedRetroAllocations, savedPremiumGroups] =
          await Promise.all([
            tauriBridge.getPeriods(),
            tauriBridge.getPersonnelList(),
            tauriBridge.getAttendanceList(),
            tauriBridge.getPayrollList(),
            tauriBridge.getInstitutionSettings(),
            tauriBridge.getTaxOpenings(),
            tauriBridge.getSickLeaveRecords(),
            tauriBridge.getAnnualPayrollParameters(),
            tauriBridge.getAppSetting('active_period_id'),
            tauriBridge.getCompensationRevisions(),
            tauriBridge.getCompensationRevisionOverrides(),
            tauriBridge.getRetroAdjustmentBatches(),
            tauriBridge.getRetroAdjustmentAllocations(),
            tauriBridge.getAppSetting('is_primi_gruplari'),
          ]);

        if (!isLatestLoad()) return false;
        const configuredPremiumGroups = parsePremiumGroupsSetting(savedPremiumGroups);
        const globalPremiumGroups = configuredPremiumGroups
          ?? DEFAULT_IS_PRIMI_GRUPLARI.map((group) => ({ ...group }));
        if (!configuredPremiumGroups) {
          await tauriBridge.setAppSetting('is_primi_gruplari', JSON.stringify(toPayrollBoundaryDto(globalPremiumGroups)));
        }
        if (!isLatestLoad()) return false;
        applyDataset(toPayrollBoundaryDto({
          donemler: fetchedPeriods,
          aktifDonemId: savedActivePeriodId || fetchedPeriods[0]?.id || '',
          personeller: fetchedPersonnel,
          kurumDegerleriMap: fetchedSettings,
          puantajlar: fetchedAttendance,
          bordrolar: fetchedPayrolls,
          taxOpenings: fetchedTaxOpenings,
          sickLeaveRecords: fetchedSickLeaves,
          annualPayrollParameters: fetchedAnnualParameters,
          compensationRevisions: fetchedRevisions,
          compensationRevisionOverrides: fetchedRevisionOverrides,
          retroBatches: fetchedRetroBatches,
          retroAllocations: fetchedRetroAllocations,
          isPrimiGruplari: globalPremiumGroups,
        }));
        setIsDataLoaded(true);
        return true;
      }

      const saved = await browserPersistence.loadSnapshot();
      if (!isLatestLoad()) return false;
      if (saved) {
        applyDataset(saved);
      } else {
        applyDataset(toPayrollBoundaryDto({
          donemler: [],
          aktifDonemId: '',
          personeller: [],
          kurumDegerleriMap: {},
          puantajlar: [],
          bordrolar: [],
          taxOpenings: [],
          sickLeaveRecords: [],
          annualPayrollParameters: [],
          compensationRevisions: [],
          compensationRevisionOverrides: [],
          retroBatches: [],
          retroAllocations: [],
          isPrimiGruplari: DEFAULT_IS_PRIMI_GRUPLARI.map((group) => ({ ...group })),
        }));
      }
      setIsDataLoaded(true);
      setLoadError(null);
      return true;
    } catch (err) {
      if (!isLatestLoad()) return false;
      const browserError = isNative ? null : formatBrowserStorageLoadError(err);
      const message = isNative
        ? `Veri yüklenemedi: ${getErrorMessage(err)}`
        : browserError!.userMessage;
      console.error(
        isNative ? 'Veri yüklenemedi.' : 'Tarayıcıdaki bordro verisi okunamadı.',
        browserError?.technicalDetail,
        err
      );
      setLoadError(message);
      // Native failures stop here. Browser failures also remain visible rather
      // than being replaced with an empty, apparently valid dataset.
      setIsDataLoaded(false);
      return false;
    }
  }, [applyDataset, browserPersistence.loadSnapshot]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleSelectDonem = async (id: string) => {
    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.setAppSetting('active_period_id', id);
      }
      updateAuthoritativePayload((current) => ({ ...current, aktifDonemId: id }));
    } catch (err) {
      const message = `Aktif dönem kaydedilemedi: ${describeError(err)}`;
      console.error(message, err);
      setLoadError(message);
    }
  };

  const handleSaveIsPrimiGruplari = async (groups: IsPrimiGrupItem[]) => {
    const normalized = normalizePremiumGroups(groups);
    if (!normalized) throw new Error('İş primi grupları geçerli değil.');
    const exactGroups = toPayrollBoundaryDto(normalized).map((group) => ({
      ...group,
      aktif: group.aktif ?? true,
    }));
    const result = await getCompensationPolicyApi().saveWorkPremiumGroups(exactGroups);
    if (result.ok === false) throw new Error(result.error.message);
    await loadData();
  };

  const aktifDonem = donemler.find((d) => d.id === aktifDonemId) || donemler[0];
  const compensationPolicyProps = useCompensationPolicyController(aktifDonem, loadData);
  const payrollDataset = useMemo<PayrollDatasetSnapshot>(() => {
    if (authoritativePayload) {
      return {
        personnel: authoritativePayload.personeller,
        periods: authoritativePayload.donemler,
        institutionSettings: authoritativePayload.kurumDegerleriMap,
        attendances: authoritativePayload.puantajlar,
        payrolls: authoritativePayload.bordrolar,
        taxOpenings: authoritativePayload.taxOpenings,
        sickLeaveRecords: authoritativePayload.sickLeaveRecords,
        annualPayrollParameters: authoritativePayload.annualPayrollParameters,
        compensationRevisions: authoritativePayload.compensationRevisions ?? [],
        compensationRevisionOverrides: authoritativePayload.compensationRevisionOverrides ?? [],
        retroBatches: authoritativePayload.retroBatches ?? [],
        retroAllocations: authoritativePayload.retroAllocations ?? [],
        compensationPolicyVersions: authoritativePayload.compensationPolicyVersions ?? [],
      };
    }
    return toPayrollBoundaryDto(EMPTY_UI_DATASET) as unknown as PayrollDatasetSnapshot;
  }, [authoritativePayload]);
  const payrollDatasetRef = useRef(payrollDataset);
  useLayoutEffect(() => { payrollDatasetRef.current = payrollDataset; }, [payrollDataset]);
  const getAuthoritativePayrollDataset = useCallback(() => payrollDatasetRef.current, []);
  const {
    notices: payrollNotices,
    counts: payrollNoticeCounts,
    isRefreshing: arePayrollNoticesRefreshing,
    loadError: payrollNoticeLoadError,
    refresh: refreshPayrollNotices,
  } = usePayrollNotices(isDataLoaded, aktifDonem?.id, payrollEngine, payrollDataset);
  const payrollNoticeCount =
    payrollNoticeCounts.critical + payrollNoticeCounts.warning + payrollNoticeCounts.info;

  const mutationController = usePayrollMutationController({
    isNative: tauriBridge.isTauriAvailable(),
    payrollEngine,
    payrollDataset,
    authoritativePayload,
    donemler,
    puantajlar,
    bordrolar,
    sickLeaveRecords,
    compensationRevisions,
    compensationRevisionOverrides,
    retroBatches,
    updateAuthoritativePayload,
    commitAuthoritativePayload,
    loadData,
  });
  const {
    evaluateBrowserMutations,
    handleSavePersonel,
    handleSavePersonelAndTaxOpening,
    handleDeletePersonel,
    handleCreateDonem,
    handleDeleteDonem,
    handleSaveKurumDegerleri,
    handleSavePuantaj,
    handleSavePuantajBatch,
    handleSaveSickLeaveRecord,
    handleSaveAnnualPayrollParameters,
    handleDeleteSickLeaveRecord,
    handleDeleteBordro,
    handleSaveBordro,
    handleSaveCompensationRevision,
    handleCalculateRetroPreview,
    handleSaveRetroBatch,
    handleCreateRetroPayment,
    handleRecoverRetroBatch,
    handleRecoverRetroChain,
  } = mutationController;

  const backupController = useBackupController({
    authoritativePayload,
    activePeriodId: aktifDonemId,
    isDataLoaded,
    browserPersistence,
    evaluateBrowserMutations,
    payrollEngine,
    loadData,
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
    onSuccess: (msg) => {
      setGlobalSuccess(msg);
      setTimeout(() => setGlobalSuccess(null), 4000);
    },
  });
  const {
    executeResetSampleData,
    handleResetSampleData,
    handleClearAndStartFresh,
    handleExportBackup,
    handleImportBackup,
  } = backupController;

  const handleBackupFileSelected = (file: File) => setPendingBackupFile(file);

  const handleConfirmBackupImport = async () => {
    if (!pendingBackupFile) return;
    setIsImportingBackup(true);
    try {
      await importConfirmedBackupFile(pendingBackupFile, true, handleImportBackup);
      setPendingBackupFile(null);
    } catch (error) {
      setLoadError(`Yedek dosyası okunamadı; kayıt değiştirilmedi. ${formatPayrollError(error)}`);
      setPendingBackupFile(null);
    } finally {
      setIsImportingBackup(false);
    }
  };

  const handleConfirmResetSampleData = async () => {
    setIsResettingSampleData(true);
    try {
      await executeResetSampleData();
      setIsResetConfirmOpen(false);
    } catch {
      // Hata zaten useBackupController içinde setLoadError ile yakalanıyor
    } finally {
      setIsResettingSampleData(false);
    }
  };

  const handleSelectPersonelForBordro = (personelId: string) => {
    setTargetPersonelIdForBordro(personelId);
    setActiveTab('bordro');
    setActivePayrollView('normal');
  };

  const handleTabChange = (tab: TabType) => {
    setActiveTab(tab);
  };

  const handlePayrollViewChange = (view: PayrollViewType) => {
    setActivePayrollView(view);
    setActiveTab('bordro');
  };

  const handleKesintiTypeChange = (type: KesintiTipi) => {
    setActiveTab('kesintiler');
    setActiveKesintiType(type);
  };

  const handleParametreSectionChange = (section: ParametreSection) => {
    setActiveTab('parametrelar');
    setActiveParametreSection(section);
  };

  const handleOpenNewPeriodSettings = () => {
    setActiveTab('parametrelar');
    setActiveParametreSection('newPeriod');
  };

  return (
    <>
      <AppViewportFrame
        viewportBounded={activeTab === 'compensationPolicy'}
        header={isDataLoaded && (
        <TopBar
          donemler={donemler}
          aktifDonemId={aktifDonemId}
          onSelectDonem={handleSelectDonem}
          onExportBackup={handleExportBackup}
          onImportFileSelected={handleBackupFileSelected}
          onResetSampleData={() => setIsResetConfirmOpen(true)}
          noticeCount={payrollNoticeCount}
          onOpenNoticeSummary={() => setActiveTab('ozet')}
          isSidebarOpen={isSidebarOpen}
          onToggleSidebar={() => setIsSidebarOpen((current) => !current)}
        />
        )}
        navigation={isDataLoaded && (
          <Sidebar
            activeTab={activeTab}
            activeKesintiType={activeKesintiType}
            activeParametreSection={activeParametreSection}
            activePayrollView={activePayrollView}
            onTabChange={handleTabChange}
            onKesintiTypeChange={handleKesintiTypeChange}
            onParametreSectionChange={handleParametreSectionChange}
            onPayrollViewChange={handlePayrollViewChange}
            isOpen={isSidebarOpen}
            onClose={() => setIsSidebarOpen(false)}
          />
        )}
      >
            {globalSuccess && (
              <div
                role="status"
                data-testid="global-success-message"
                className="mb-5 flex items-center justify-between rounded-xl border border-emerald-300 bg-emerald-50 p-4 text-xs font-semibold text-emerald-900 shadow-xs"
              >
                <div className="flex items-center gap-2">
                  <CheckCircle2 className="h-4 w-4 text-emerald-600 shrink-0" />
                  <span>{globalSuccess}</span>
                </div>
                <button
                  type="button"
                  onClick={() => setGlobalSuccess(null)}
                  className="text-emerald-700 hover:text-emerald-900 ml-3 cursor-pointer"
                >
                  ✕
                </button>
              </div>
            )}

            {loadError && isDataLoaded && (
              <div
                role="alert"
                data-testid="storage-error"
                className="mb-5 rounded-xl border border-rose-300 bg-rose-50 p-4 text-xs font-semibold text-rose-900"
              >
                <div>{loadError}</div>
                {browserPersistence.hasExternalConflict && (
                  <button
                    type="button"
                    className="mt-3 rounded-lg border border-rose-400 bg-white px-3 py-2 text-xs font-semibold text-rose-900 hover:bg-rose-100"
                    onClick={() => {
                      void browserPersistence.reloadExternalSnapshot().catch((error) => {
                        setLoadError(`Son kayıtlı tarayıcı snapshotı yüklenemedi: ${getErrorMessage(error)}`);
                      });
                    }}
                  >
                    Başka sekmedeki kayıtlı veriyi yükle
                  </button>
                )}
              </div>
            )}

            {!isDataLoaded && !loadError && (
              <div
                role="status"
                data-testid="data-loading-state"
                className="mx-auto my-16 max-w-xl rounded-2xl border border-slate-200 bg-white p-8 text-center text-sm font-semibold text-slate-600 shadow-sm"
              >
                Veriler yükleniyor…
              </div>
            )}

            {!isDataLoaded && loadError && (
              <div className="mx-auto my-12 max-w-xl space-y-4 rounded-2xl border border-rose-200 bg-white p-8 text-center shadow-sm">
                <div
                  role="alert"
                  data-testid="storage-error"
                  className="rounded-xl border border-rose-300 bg-rose-50 p-4 text-xs font-semibold text-rose-900 text-left"
                >
                  {loadError}
                </div>
                <p className="text-xs leading-relaxed text-slate-600">
                  Tarayıcınızda kayıtlı veriler yeni sürümle tam uyumlu olmayabilir veya veri okuma hatası oluştu. İşlemlere devam etmek için örnek verileri yükleyebilir veya temiz bir başlangıç yapabilirsiniz.
                </p>
                <div className="flex flex-col sm:flex-row items-center justify-center gap-3 pt-2">
                  <button
                    type="button"
                    onClick={() => setIsResetConfirmOpen(true)}
                    className="w-full sm:w-auto rounded-xl bg-indigo-600 px-4 py-2.5 text-xs font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700 cursor-pointer"
                  >
                    Örnek Verileri Yükle
                  </button>
                  <button
                    type="button"
                    onClick={handleClearAndStartFresh}
                    className="w-full sm:w-auto rounded-xl bg-slate-100 px-4 py-2.5 text-xs font-semibold text-slate-700 transition-colors hover:bg-slate-200 cursor-pointer"
                  >
                    Verileri Sıfırla (Temiz Başla)
                  </button>
                </div>
                <div className="pt-2 border-t border-slate-100">
                  <label className="inline-flex items-center gap-2 cursor-pointer text-xs font-medium text-indigo-600 hover:text-indigo-700">
                    <span>veya Yedek Dosyası (.json) Yükle</span>
                    <input
                      type="file"
                      accept=".json,application/json"
                      className="hidden"
                      onChange={(event) => {
                        const file = event.target.files?.[0];
                        if (file) handleBackupFileSelected(file);
                        event.target.value = '';
                      }}
                    />
                  </label>
                </div>
              </div>
            )}

            <React.Suspense
              fallback={(
                <div
                  role="status"
                  aria-live="polite"
                  className="mx-auto my-16 max-w-xl rounded-2xl border border-slate-200 bg-white p-8 text-center text-sm font-semibold text-slate-600 shadow-sm"
                >
                  Ekran yükleniyor…
                </div>
              )}
            >
              {isDataLoaded && (
                <>
                {activeTab === 'personel' && (
                  <PersonelList
                    personeller={personeller}
                    onSavePersonel={handleSavePersonel}
                    onDeletePersonel={handleDeletePersonel}
                    onSelectPersonelForBordro={handleSelectPersonelForBordro}
                    isPrimiGruplari={isPrimiGruplari}
                    gunlukTabanUcret={compensationPolicyProps?.loadState.status === 'ready'
                      ? compensationPolicyProps.loadState.value.activePolicyProjection.kind === 'ACTIVE'
                        ? compensationPolicyProps.loadState.value.activePolicyProjection.version.state.gunlukTabanUcret
                        : undefined
                      : undefined}
                    sendikaAidatiYuzde={aktifDonemId ? kurumDegerleriMap[aktifDonemId]?.sendikaAidatiYuzde : undefined}
                  />
                )}

                {!aktifDonem &&
                  activeTab !== 'personel' &&
                  activeTab !== 'parametrelar' &&
                  activeTab !== 'compensationPolicy' &&
                  activeTab !== 'ozet' &&
                  activeTab !== 'veri' && (
                  <div className="mx-auto my-12 max-w-xl space-y-4 rounded-2xl border border-slate-200 bg-white p-8 text-center shadow-sm">
                    <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-indigo-50 text-xl font-bold text-indigo-600">!</div>
                    <h3 className="text-lg font-bold text-slate-800">Henüz Dönem Bulunmamaktadır</h3>
                    <p className="text-xs leading-relaxed text-slate-600">
                      İşlemlere başlamak için yeni bir dönem tanımlayabilir veya örnek verileri yükleyebilirsiniz.
                    </p>
                    <div className="flex items-center justify-center gap-3 pt-2">
                      <button type="button" onClick={handleOpenNewPeriodSettings} className="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700">Yeni Dönem Aç</button>
                      <button type="button" onClick={() => setIsResetConfirmOpen(true)} className="rounded-xl bg-slate-100 px-4 py-2 text-xs font-semibold text-slate-700 transition-colors hover:bg-slate-200">Örnek Verileri Yükle</button>
                    </div>
                  </div>
                )}

                {activeTab === 'ozet' && (
                  <PeriodSummary
                    aktifDonem={aktifDonem}
                    personeller={personeller}
                    puantajlar={puantajlar}
                    bordrolar={bordrolar}
                    activeKurumDegerleri={aktifDonem ? kurumDegerleriMap[aktifDonem.id] : undefined}
                    compensationPolicyReady={Boolean(
                      compensationPolicyProps?.loadState.status === 'ready' &&
                      compensationPolicyProps.loadState.value.timeline.resolution === 'RESOLVED'
                    )}
                    annualPayrollParameters={annualPayrollParameters}
                    payrollNotices={payrollNotices}
                    isRefreshingNotices={arePayrollNoticesRefreshing}
                    noticeLoadError={payrollNoticeLoadError}
                    onRefreshNotices={refreshPayrollNotices}
                    onNavigate={(tab, payrollView, parametreSection) => {
                      if (payrollView) setActivePayrollView(payrollView);
                      if (parametreSection) setActiveParametreSection(parametreSection);
                      setActiveTab(tab);
                    }}
                  />
                )}

                {activeTab === 'retro' && (
                  <GeriyeDonukFarklar
                    donemler={donemler}
                    personeller={personeller}
                    revisions={compensationRevisions}
                    overrides={compensationRevisionOverrides}
                    batches={retroBatches}
                    allocations={retroAllocations}
                    bordrolar={bordrolar}
                    onSaveRevision={handleSaveCompensationRevision}
                    onCalculatePreview={handleCalculateRetroPreview}
                    onSaveBatch={handleSaveRetroBatch}
                    onCreatePayment={handleCreateRetroPayment}
                    onRecoverChain={handleRecoverRetroChain}
                  />
                )}

                {activeTab !== 'bordro' && <BulkPayrollProgress periods={donemler} personnel={personeller}
                  onOpen={(periodId) => { void handleSelectDonem(periodId); setActiveTab('bordro'); }} />}

                {aktifDonem && activeTab === 'puantaj' && (
                  <PuantajGrid
                    aktifDonem={aktifDonem}
                    personeller={personeller}
                    puantajlar={puantajlar}
                    sickLeaveRecords={sickLeaveRecords}
                    authoritativeDataset={payrollDataset}
                    onSavePuantaj={handleSavePuantaj}
                    onSavePuantajBatch={handleSavePuantajBatch}
                    onSelectPersonelForBordro={handleSelectPersonelForBordro}
                  />
                )}

                {aktifDonem && activeTab === 'bordro' && (
                  <BordroHesaplama
                    aktifDonem={aktifDonem}
                    donemler={donemler}
                    personeller={personeller}
                    isPrimiGruplari={isPrimiGruplari}
                    kurumDegerleriMap={kurumDegerleriMap}
                    puantajlar={puantajlar}
                    bordrolar={bordrolar}
                    taxOpenings={taxOpenings}
                    sickLeaveRecords={sickLeaveRecords}
                    annualPayrollParameters={annualPayrollParameters}
                    activePayrollView={activePayrollView}
                    authoritativeDataset={payrollDataset}
                    getAuthoritativeDataset={getAuthoritativePayrollDataset}
                    onSaveBordro={handleSaveBordro}
                    onRecoverRetroBatch={handleRecoverRetroBatch}
                    onDeleteBordro={handleDeleteBordro}
                    onSavePersonelAndTaxOpening={handleSavePersonelAndTaxOpening}
                    initialPersonelId={targetPersonelIdForBordro}
                    onGoToPuantaj={(personelId) => {
                      if (personelId) setTargetPersonelIdForBordro(personelId);
                      setActiveTab('puantaj');
                    }}
                    onOpenCompensationPolicy={() => setActiveTab('compensationPolicy')}
                  />
                )}

                {aktifDonem && activeTab === 'banka' && <BankaListesi aktifDonem={aktifDonem} personeller={personeller} bordrolar={bordrolar} />}
                {aktifDonem && activeTab === 'sgkKontrol' && (
                  <SgkPrimKontrolu
                    aktifDonem={aktifDonem}
                    personeller={personeller}
                    bordrolar={bordrolar}
                    retroBatches={retroBatches}
                    retroAllocations={retroAllocations}
                    kurumDegerleri={kurumDegerleriMap[aktifDonem.id]}
                  />
                )}
                {aktifDonem && activeTab === 'kesintiler' && (
                  <KesintiListesi
                    aktifDonem={aktifDonem}
                    personeller={personeller}
                    bordrolar={bordrolar}
                    activeType={activeKesintiType}
                  />
                )}

                {activeTab === 'parametrelar' && (
                  <PeriodSettingsPage
                    activeSection={activeParametreSection}
                    onSectionChange={handleParametreSectionChange}
                    donemler={donemler}
                    aktifDonem={aktifDonem}
                    aktifDonemId={aktifDonemId}
                    onSelectDonem={handleSelectDonem}
                    onCreateDonem={handleCreateDonem}
                    onDeleteDonem={handleDeleteDonem}
                    kurumDegerleriMap={kurumDegerleriMap}
                    onSaveKurumDegerleri={handleSaveKurumDegerleri}
                    personeller={personeller}
                    annualPayrollParameters={annualPayrollParameters}
                    onSaveAnnualPayrollParameters={handleSaveAnnualPayrollParameters}
                    puantajlar={puantajlar}
                    sickLeaveRecords={sickLeaveRecords}
                    onSaveSickLeaveRecord={handleSaveSickLeaveRecord}
                    onDeleteSickLeaveRecord={handleDeleteSickLeaveRecord}
                    onOpenCompensationPolicy={() => setActiveTab('compensationPolicy')}
                    isPrimiGruplari={isPrimiGruplari}
                    onSaveIsPrimiGruplari={handleSaveIsPrimiGruplari}
                  />
                )}

                {activeTab === 'compensationPolicy' && compensationPolicyProps && (
                  <CompensationPolicyPage {...compensationPolicyProps} />
                )}
                {activeTab === 'compensationPolicy' && !compensationPolicyProps && (
                  <div className="mx-auto my-12 max-w-xl rounded-2xl border border-slate-200 bg-white p-8 text-center shadow-sm">
                    <h2 className="text-lg font-bold text-slate-800">Ücret politikası için dönem seçin</h2>
                    <p className="mt-2 text-sm text-slate-600">Yetkili ücret geçmişini görüntülemek için önce bir bordro dönemi oluşturun.</p>
                    <button type="button" onClick={handleOpenNewPeriodSettings} className="mt-4 rounded-xl bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700">Yeni dönem aç</button>
                  </div>
                )}

                {activeTab === 'veri' && authoritativePayload && (
                  <DataBackupPage
                    lastSavedAt={authoritativePayload.exportedAt}
                    hasData={personeller.length > 0 || donemler.length > 0 || bordrolar.length > 0}
                    storageLabel={tauriBridge.isTauriAvailable() ? 'Bu cihazda yerel kayıt' : 'Bu tarayıcıda yerel kayıt'}
                    storageDetail={tauriBridge.isTauriAvailable()
                      ? 'Veriler bu cihazdaki yerel uygulama veritabanında tutulur; düzenli JSON yedeği almanız önerilir.'
                      : 'Veriler bu tarayıcıda yerel olarak tutulur; düzenli JSON yedeği almanız önerilir.'}
                    onExportBackup={handleExportBackup}
                    onImportFileSelected={handleBackupFileSelected}
                    onResetSampleData={() => setIsResetConfirmOpen(true)}
                  />
                )}
                </>
              )}
            </React.Suspense>
      </AppViewportFrame>

      {pendingBackupFile && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="backup-import-confirm-title"
          className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 p-4 backdrop-blur-xs"
        >
          <div className="w-full max-w-md space-y-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-xl">
            <div>
              <h3 id="backup-import-confirm-title" className="text-base font-bold text-slate-900">
                Yedekten geri yükleme onayı
              </h3>
              <p className="mt-1 break-all text-xs text-slate-500">{pendingBackupFile.name}</p>
            </div>
            <p className="text-xs leading-relaxed text-slate-600">
              Bu işlem mevcut personel, dönem, puantaj, bordro, vergi açılışı, raporlar, yıllık parametreler ve geriye dönük fark kayıtlarını yedekteki verilerle değiştirecek. İşlem geri alınamaz. Devam etmek istiyor musunuz?
            </p>
            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                type="button"
                ref={backupImportCancelRef}
                disabled={isImportingBackup}
                onClick={() => setPendingBackupFile(null)}
                className="rounded-xl border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 disabled:opacity-50"
              >
                Vazgeç
              </button>
              <button
                type="button"
                disabled={isImportingBackup}
                onClick={() => void handleConfirmBackupImport()}
                className="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-bold text-white hover:bg-indigo-700 disabled:opacity-50"
              >
                {isImportingBackup ? 'Yedek yükleniyor…' : 'Yedeği yükle'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Örnek Veri Reset Onay Modalı */}
      {isResetConfirmOpen && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="reset-sample-data-modal-title"
          className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4"
        >
          <div className="w-full max-w-md rounded-2xl border border-slate-200 bg-white p-6 shadow-xl space-y-4">
            <div className="flex items-center gap-3">
              <div className="rounded-xl bg-amber-100 p-2.5 text-amber-700 shrink-0">
                <RotateCcw className="h-5 w-5" />
              </div>
              <div>
                <h3 id="reset-sample-data-modal-title" className="text-base font-bold text-slate-900">
                  Örnek Verileri Yeniden Yükle
                </h3>
                <p className="text-xs text-slate-500">Mevcut veriler sıfırlanacaktır</p>
              </div>
            </div>

            <p className="text-xs leading-relaxed text-slate-600">
              Tüm personel, dönem, puantaj, bordro ve ayar verileri sıfırlanarak standart örnek 4/D bordro verileri kalıcı olarak yüklenecektir. Bu işlem geri alınamaz. Devam etmek istiyor musunuz?
            </p>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                type="button"
                disabled={isResettingSampleData}
                onClick={() => setIsResetConfirmOpen(false)}
                className="rounded-xl border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 disabled:opacity-50 cursor-pointer"
              >
                Vazgeç
              </button>
              <button
                type="button"
                disabled={isResettingSampleData}
                onClick={() => void handleConfirmResetSampleData()}
                className="rounded-xl bg-amber-600 hover:bg-amber-700 px-4 py-2 text-xs font-bold text-white shadow-xs disabled:opacity-50 flex items-center gap-1.5 cursor-pointer"
              >
                <RotateCcw className={`h-3.5 w-3.5 ${isResettingSampleData ? 'animate-spin' : ''}`} />
                <span>{isResettingSampleData ? 'Yükleniyor…' : 'Evet, Örnek Verileri Yükle'}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
