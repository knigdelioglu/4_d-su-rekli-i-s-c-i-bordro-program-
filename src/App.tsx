/**
 * 4/D Sürekli İşçi Bordro Programı — Main App Component
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Sidebar } from './components/Sidebar';
import { TopBar } from './components/TopBar';
import { PeriodSettingsPage } from './components/Settings/PeriodSettingsPage';
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
import { PersonelList } from './components/PersonelList';
import { PuantajGrid } from './components/PuantajGrid';
import { BordroHesaplama } from './components/BordroHesaplama';
import { GeriyeDonukFarklar, type RetroPreviewInput } from './components/GeriyeDonukFarklar';
import { BankaListesi } from './components/Listeler/BankaListesi';
import { SgkPrimKontrolu } from './components/Listeler/SgkPrimKontrolu';
import { KesintiListesi } from './components/Listeler/KesintiListesi';
import {
  AnnualPayrollParameters,
  BACKUP_FORMAT_VERSION,
  BackupPayload,
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  CompensationRevision,
  CompensationRevisionOverride,
  RetroAdjustmentBatch,
  RetroAllocation,
  SickLeaveRecord,
  PayrollAccrualInput,
  ZAM_AYLARI_SETTING_KEY,
} from './types/payroll';
import { tauriBridge } from './services/tauriBridge';
import { browserPayrollStore } from './services/storage/browserPayrollStore';
import {
  applyBrowserRetroBatchImpact,
  applyBrowserPayrollImpact,
  assertBrowserMutationImpactAllowed,
} from './services/storage/browserPayrollPolicies';
import { getPayrollEngine } from './services/payrollEngine';
import { nextPaymentSequence } from './services/payrollEngine/paymentEventOrder';
import type {
  MutationImpact,
  PayrollBoundaryPayroll,
  PayrollBoundaryPersonel,
  PayrollBoundaryTaxOpening,
  PayrollDatasetSnapshot,
  PayrollMutation,
  RetroCalculationRequest,
  RetroCalculationResult,
  RetroCalculationResultModel,
} from './services/payrollEngine';
import {
  mergePayrollUiIntoBoundary,
  serializePayrollStorage,
  toPayrollBoundaryDto,
  toPayrollUiModel,
  type PayrollStorageDto,
  type PayrollStorageFields,
} from './services/payrollEngine/decimalBoundary';
import {
  parseImportedBackup,
} from './services/storage/payrollPayload';
import { usePayrollNotices } from './components/PayrollNoticeCenter';
import { PeriodSummary } from './components/Dashboard/PeriodSummary';
import { DataBackupPage } from './components/DataBackupPage';
import { getInitialDataset } from './utils/sampleData';

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
  | 'compensationRevisions'
  | 'compensationRevisionOverrides'
  | 'retroBatches'
  | 'retroAllocations'
> &
  Required<
    Pick<
      BackupPayload,
      | 'compensationRevisions'
      | 'compensationRevisionOverrides'
      | 'retroBatches'
      | 'retroAllocations'
    >
  >;

function normalizeZamAylari(value: unknown): number[] {
  if (!Array.isArray(value)) return [];
  return [...new Set(
    value.filter(
      (month): month is number =>
        typeof month === 'number' && Number.isInteger(month) && month >= 1 && month <= 12
    )
  )].sort((a, b) => a - b);
}

function sameRevisionDefinition(
  left: CompensationRevision | undefined,
  right: CompensationRevision
): boolean {
  return Boolean(left) &&
    left!.id === right.id &&
    left!.reason === right.reason &&
    left!.title === right.title &&
    left!.effectiveFrom === right.effectiveFrom &&
    left!.effectiveTo === right.effectiveTo &&
    left!.decisionDate === right.decisionDate &&
    left!.signedAt === right.signedAt &&
    left!.description === right.description &&
    left!.scope === right.scope &&
    JSON.stringify(left!.personnelIds ?? []) === JSON.stringify(right.personnelIds ?? []) &&
    left!.personnelGroup === right.personnelGroup;
}

function sameRevisionOverrides(
  left: CompensationRevisionOverride[],
  right: CompensationRevisionOverride[]
): boolean {
  if (left.length !== right.length) return false;
  return left.every((candidate) => right.some((other) =>
    candidate.id === other.id &&
    candidate.revisionId === other.revisionId &&
    candidate.parameter === other.parameter &&
    candidate.value === other.value &&
    candidate.personnelId === other.personnelId
  ));
}

function parseZamAylariSetting(value: string | null): number[] {
  if (!value) return [];
  try {
    return normalizeZamAylari(JSON.parse(value));
  } catch {
    return [];
  }
}

function makeBackupPayload(data: DatasetFields): PayrollStorageDto {
  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    ...data,
  };
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
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

function formatBrowserStorageSaveError(error: unknown): UserFacingStorageError {
  return {
    userMessage: 'Veriler kaydedilemedi. Mevcut kayıt korunuyor.',
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
  zamAylari: [],
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

  const [authoritativePayload, setAuthoritativePayload] = useState<PayrollStorageDto | null>(null);
  const [isDataLoaded, setIsDataLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const browserPersistenceRevision = useRef(0);

  const [targetPersonelIdForBordro, setTargetPersonelIdForBordro] = useState<
    string | undefined
  >(undefined);
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

  const uiDataset = useMemo<UiDatasetFields>(() => {
    if (!authoritativePayload) return EMPTY_UI_DATASET;
    const decoded = toPayrollUiModel(authoritativePayload) as unknown as BackupPayload;
    return {
      ...decoded,
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
    zamAylari,
    compensationRevisions,
    compensationRevisionOverrides,
    retroBatches,
    retroAllocations,
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
    setAuthoritativePayload(makeBackupPayload(data));
  }, []);

  const updateAuthoritativePayload = useCallback(
    (update: (current: PayrollStorageDto) => PayrollStorageDto) => {
      setAuthoritativePayload((current) => {
        if (!current) return current;
        const next = update(current);
        return { ...next, exportedAt: new Date().toISOString() };
      });
    },
    []
  );

  const loadData = useCallback(async () => {
    setLoadError(null);
    const isNative = tauriBridge.isTauriAvailable();

    try {
      if (isNative) {
        const isMigrated = await tauriBridge.checkLegacyMigrated();
        if (!isMigrated) {
          const legacyStr = localStorage.getItem(STORAGE_KEY);
          if (legacyStr) {
            // Migration errors intentionally abort native loading. A failed
            // database write must never be presented as a browser-only success.
            await tauriBridge.migrateLegacyPayload(legacyStr);
          }
        }

        const [fetchedPeriods, fetchedPersonnel, fetchedAttendance, fetchedPayrolls, fetchedSettings, fetchedTaxOpenings, fetchedSickLeaves, fetchedAnnualParameters, savedActivePeriodId, savedZamAylari, fetchedRevisions, fetchedRevisionOverrides, fetchedRetroBatches, fetchedRetroAllocations] =
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
            tauriBridge.getAppSetting(ZAM_AYLARI_SETTING_KEY),
            tauriBridge.getCompensationRevisions(),
            tauriBridge.getCompensationRevisionOverrides(),
            tauriBridge.getRetroAdjustmentBatches(),
            tauriBridge.getRetroAdjustmentAllocations(),
          ]);

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
          zamAylari: parseZamAylariSetting(savedZamAylari),
          compensationRevisions: fetchedRevisions,
          compensationRevisionOverrides: fetchedRevisionOverrides,
          retroBatches: fetchedRetroBatches,
          retroAllocations: fetchedRetroAllocations,
        }));
        setIsDataLoaded(true);
        return;
      }

      const saved = await browserPayrollStore.loadPayload();
      if (saved) {
        // Version-aware parsing keeps legacy compatibility explicit while a
        // current V3 snapshot remains strict and never reaches repair logic.
        const payload: PayrollStorageDto = parseImportedBackup(saved);
        applyDataset(payload);
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
          zamAylari: [],
          compensationRevisions: [],
          compensationRevisionOverrides: [],
          retroBatches: [],
          retroAllocations: [],
        }));
      }
      setIsDataLoaded(true);
      setLoadError(null);
    } catch (err) {
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
    }
  }, [applyDataset]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  // Standalone browser mode has the same versioned backup contract as native
  // mode. The loaded guard prevents the initial empty state from overwriting a
  // real saved dataset before the first read completes.
  useEffect(() => {
    if (!isDataLoaded || tauriBridge.isTauriAvailable() || !authoritativePayload) return;
    const revision = ++browserPersistenceRevision.current;
    void browserPayrollStore
      .savePayload(serializePayrollStorage(authoritativePayload))
      .catch(async (err) => {
        // React state is optimistic, but a failed IndexedDB write must not
        // leave the UI presenting an unsaved payroll as authoritative. Ignore
        // an older failure when a newer complete snapshot is already queued.
        if (revision !== browserPersistenceRevision.current) return;
        const browserError = formatBrowserStorageSaveError(err);
        console.error('Tarayıcı verisi kaydedilemedi.', browserError.technicalDetail, err);
        setLoadError(browserError.userMessage);
        try {
          const saved = await browserPayrollStore.loadPayload();
          if (revision !== browserPersistenceRevision.current || !saved) return;
          setAuthoritativePayload(parseImportedBackup(saved));
        } catch (reloadError) {
          if (revision !== browserPersistenceRevision.current) return;
          console.error('Son başarılı tarayıcı snapshotı geri yüklenemedi.', reloadError);
          setIsDataLoaded(false);
        }
      });
  }, [authoritativePayload, isDataLoaded]);

  const handleSelectDonem = async (id: string) => {
    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.setAppSetting('active_period_id', id);
      }
      updateAuthoritativePayload((current) => ({ ...current, aktifDonemId: id }));
    } catch (err) {
      const message = `Aktif dönem kaydedilemedi: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
    }
  };

  const aktifDonem = donemler.find((d) => d.id === aktifDonemId) || donemler[0];
  const payrollEngine = getPayrollEngine();
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
        zamAylari: authoritativePayload.zamAylari,
        compensationRevisions: authoritativePayload.compensationRevisions ?? [],
        compensationRevisionOverrides: authoritativePayload.compensationRevisionOverrides ?? [],
        retroBatches: authoritativePayload.retroBatches ?? [],
        retroAllocations: authoritativePayload.retroAllocations ?? [],
      };
    }
    return toPayrollBoundaryDto(EMPTY_UI_DATASET) as unknown as PayrollDatasetSnapshot;
  }, [authoritativePayload]);
  const {
    notices: payrollNotices,
    counts: payrollNoticeCounts,
    isRefreshing: arePayrollNoticesRefreshing,
    loadError: payrollNoticeLoadError,
    refresh: refreshPayrollNotices,
  } = usePayrollNotices(isDataLoaded, aktifDonem?.id, payrollEngine, payrollDataset);
  const payrollNoticeCount =
    payrollNoticeCounts.critical + payrollNoticeCounts.warning + payrollNoticeCounts.info;

  /**
   * Browser mutations are authorized by the same Rust policy as native
   * mutations. This helper only merges the core response when a mutation has
   * both an old and a new source position (for example an edited period).
   */
  const evaluateBrowserMutations = async (
    mutation: PayrollMutation | PayrollMutation[]
  ): Promise<MutationImpact> => {
    const mutations = Array.isArray(mutation) ? mutation : [mutation];
    const impacts = await Promise.all(
      mutations.map((item) => payrollEngine.evaluateMutationPolicy(item, payrollDataset))
    );
    const affected = new Map<string, MutationImpact['affectedPayrolls'][number]>();
    const blocked = new Map<string, MutationImpact['blockedByFinalized'][number]>();
    const affectedRetroBatches = new Set<string>();
    const blockedByFinalizedRetroBatches = new Set<string>();
    for (const impact of impacts) {
      for (const key of impact.affectedPayrolls) {
        affected.set(`${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`, key);
      }
      for (const key of impact.blockedByFinalized) {
        blocked.set(`${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`, key);
      }
      for (const batchId of impact.affectedRetroBatches) affectedRetroBatches.add(batchId);
      for (const batchId of impact.blockedByFinalizedRetroBatches) {
        blockedByFinalizedRetroBatches.add(batchId);
      }
    }
    const merged = {
      affectedPayrolls: [...affected.values()],
      blockedByFinalized: [...blocked.values()],
      affectedRetroBatches: [...affectedRetroBatches],
      blockedByFinalizedRetroBatches: [...blockedByFinalizedRetroBatches],
    } satisfies MutationImpact;
    assertBrowserMutationImpactAllowed(merged);
    return merged;
  };

  const handleResetSampleData = async () => {
    if (
      !window.confirm(
        'Tüm mevcut veriler sıfırlanıp örnek 4/D bordro verileri yüklenecek. Emin misiniz?'
      )
    ) {
      return;
    }

    const initialData = getInitialDataset();
    const payload = makeBackupPayload(toPayrollBoundaryDto({
      ...initialData,
      taxOpenings: initialData.taxOpenings || [],
      sickLeaveRecords: initialData.sickLeaveRecords || [],
      annualPayrollParameters: initialData.annualPayrollParameters || [],
      zamAylari: initialData.zamAylari || [],
      compensationRevisions: [],
      compensationRevisionOverrides: [],
      retroBatches: [],
      retroAllocations: [],
    }));

    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
        return;
      }

      if (isDataLoaded) {
        await evaluateBrowserMutations({ kind: 'ALL' });
      }
      await browserPayrollStore.savePayload(serializePayrollStorage(payload));
      applyDataset(payload);
      setIsDataLoaded(true);
      setLoadError(null);
    } catch (err) {
      const message = `Örnek veriler yüklenemedi: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
      alert(message);
    }
  };

  const handleClearAndStartFresh = async () => {
    if (
      !window.confirm(
        'Tarayıcıdaki tüm yerel bordro verileri temizlenecek ve boş olarak başlatılacak. Emin misiniz?'
      )
    ) {
      return;
    }
    const empty = toPayrollBoundaryDto({
      donemler: [],
      aktifDonemId: '',
      personeller: [],
      kurumDegerleriMap: {},
      puantajlar: [],
      bordrolar: [],
      taxOpenings: [],
      sickLeaveRecords: [],
      annualPayrollParameters: [],
      zamAylari: [],
      compensationRevisions: [],
      compensationRevisionOverrides: [],
      retroBatches: [],
      retroAllocations: [],
    });
    const payload = makeBackupPayload(empty);
    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
        return;
      }
      await browserPayrollStore.savePayload(serializePayrollStorage(payload));
      applyDataset(payload);
      setIsDataLoaded(true);
      setLoadError(null);
    } catch (err) {
      const message = `Veriler sıfırlanamadı: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
      alert(message);
    }
  };

  const handleRecoveryFileImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async (event) => {
      const content = event.target?.result;
      if (typeof content === 'string') {
        await handleImportBackup(content);
      }
    };
    reader.readAsText(file);
    e.target.value = '';
  };

  const handleExportBackup = () => {
    if (!authoritativePayload) return;
    const jsonStr = serializePayrollStorage(authoritativePayload, 2);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `4D_Bordro_Yedek_${aktifDonemId || 'bos'}_${new Date()
      .toISOString()
      .slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImportBackup = async (jsonStr: string) => {
    try {
      const payload = parseImportedBackup(jsonStr);
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
      } else {
        if (isDataLoaded) {
          await evaluateBrowserMutations({ kind: 'ALL' });
        }
        // Import is a user-visible commit point. Verify the IndexedDB write
        // before replacing the in-memory dataset or announcing success.
        await browserPayrollStore.savePayload(serializePayrollStorage(payload));
        applyDataset(payload);
        setIsDataLoaded(true);
        setLoadError(null);
      }
      alert('Yedek başarıyla yüklendi!');
    } catch (err) {
      console.error('Yedek yükleme başarısız:', err);
      alert('Yedek yüklenemedi. Mevcut kayıt korunuyor.');
    }
  };

  const handleSavePersonel = async (newPersonel: Personel | PayrollBoundaryPersonel) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.savePersonnel(
        toPayrollBoundaryDto(newPersonel) as unknown as Personel
      );
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON',
      personnelId: newPersonel.id,
    });
    updateAuthoritativePayload((current) => {
      const existing = current.personeller.find((person) => person.id === newPersonel.id);
      const exactPersonel = mergePayrollUiIntoBoundary(existing, newPersonel);
      const personeller = existing
        ? current.personeller.map((person) =>
            person.id === newPersonel.id ? exactPersonel : person
          )
        : [...current.personeller, exactPersonel];
      return {
        ...current,
        personeller,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleDeletePersonel = async (personelId: string) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.deletePersonnel(personelId);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON',
      personnelId: personelId,
    });
    if (impact.affectedRetroBatches.length > 0) {
      throw new Error(
        'Retro batch tarihçesi bulunan personel silinemez; audit ledger korunmalıdır.'
      );
    }
    updateAuthoritativePayload((current) => ({
      ...current,
      personeller: current.personeller.filter((person) => person.id !== personelId),
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact).filter(
        (payroll) => payroll.personelId !== personelId
      ),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      puantajlar: current.puantajlar.filter((attendance) => attendance.personelId !== personelId),
      taxOpenings: current.taxOpenings.filter((opening) => opening.personnelId !== personelId),
      sickLeaveRecords: current.sickLeaveRecords.filter((record) => record.personnelId !== personelId),
    }));
  };

  const handleCreateDonem = async (
    newDonem: BordroDonemi,
    kurumDegerleri: DönemselKurumDegerleri
  ) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.savePeriodWithSettings(newDonem, kurumDegerleri);
      await loadData();
      return;
    }
    const existing = donemler.find((period) => period.id === newDonem.id);
    const positionMutations: PayrollMutation[] = [
      {
        kind: 'PERIOD_FROM_POSITION',
        startDate: newDonem.baslangicTarihi,
        taxYear: newDonem.taxYear,
        taxMonth: newDonem.taxMonth,
      },
    ];
    if (existing) {
      positionMutations.push({
        kind: 'PERIOD',
        periodId: newDonem.id,
      });
      const causalFieldsChanged =
        existing.yil !== newDonem.yil ||
        existing.ay !== newDonem.ay ||
        existing.baslangicTarihi !== newDonem.baslangicTarihi ||
        existing.bitisTarihi !== newDonem.bitisTarihi ||
        existing.taxYear !== newDonem.taxYear ||
        existing.taxMonth !== newDonem.taxMonth;
      if (causalFieldsChanged) {
        positionMutations.push({
          kind: 'PERIOD_FROM_POSITION',
          startDate: existing.baslangicTarihi,
          taxYear: existing.taxYear,
          taxMonth: existing.taxMonth,
        });
      }
    }
    const impact = await evaluateBrowserMutations(positionMutations);
    updateAuthoritativePayload((current) => {
      const exactPeriod = mergePayrollUiIntoBoundary(
        current.donemler.find((period) => period.id === newDonem.id),
        newDonem
      );
      const donemler = current.donemler.some((period) => period.id === newDonem.id)
        ? current.donemler.map((period) =>
            period.id === newDonem.id ? exactPeriod : period
          )
        : [...current.donemler, exactPeriod];
      const exactSettings = mergePayrollUiIntoBoundary(
        current.kurumDegerleriMap[newDonem.id],
        kurumDegerleri
      );
      return {
        ...current,
        donemler,
        kurumDegerleriMap: {
          ...current.kurumDegerleriMap,
          [newDonem.id]: exactSettings,
        },
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleSaveKurumDegerleri = async (settings: DönemselKurumDegerleri) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveInstitutionSettings(settings);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERIOD',
      periodId: settings.donemId,
    });
    updateAuthoritativePayload((current) => ({
      ...current,
      kurumDegerleriMap: {
        ...current.kurumDegerleriMap,
        [settings.donemId]: mergePayrollUiIntoBoundary(
          current.kurumDegerleriMap[settings.donemId],
          settings
        ),
      },
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleSavePuantaj = async (updatedPuantaj: PersonelPuantaj) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveAttendance(updatedPuantaj);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON_PERIOD',
      personnelId: updatedPuantaj.personelId,
      periodId: updatedPuantaj.donemId,
    });
    updateAuthoritativePayload((current) => {
      const index = current.puantajlar.findIndex((attendance) => attendance.id === updatedPuantaj.id);
      const puantajlar = [...current.puantajlar];
      const exactAttendance = mergePayrollUiIntoBoundary(
        index < 0 ? undefined : current.puantajlar[index],
        updatedPuantaj
      );
      if (index < 0) puantajlar.push(exactAttendance);
      else puantajlar[index] = exactAttendance;
      return {
        ...current,
        puantajlar,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleSaveTaxOpening = async (
    opening: PersonelTaxOpening | PayrollBoundaryTaxOpening
  ) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveTaxOpening(
        toPayrollBoundaryDto(opening) as unknown as PersonelTaxOpening
      );
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON_TAX_YEAR',
      personnelId: opening.personnelId,
      taxYear: opening.year,
    });
    updateAuthoritativePayload((current) => {
      const index = current.taxOpenings.findIndex((item) => item.id === opening.id);
      const taxOpenings = [...current.taxOpenings];
      const exactOpening = mergePayrollUiIntoBoundary(
        index < 0 ? undefined : current.taxOpenings[index],
        opening
      );
      if (index < 0) taxOpenings.push(exactOpening);
      else taxOpenings[index] = exactOpening;
      return {
        ...current,
        taxOpenings,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      };
    });
  };

  const handleSaveSickLeaveRecord = async (record: SickLeaveRecord) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveSickLeaveRecord(record);
      await loadData();
      return;
    }
    const existing = sickLeaveRecords.find((item) => item.id === record.id);
    const mutations: PayrollMutation[] = [
      { kind: 'PERSON_FROM_DATE', personnelId: record.personnelId, effectiveFrom: record.startDate },
    ];
    if (existing) {
      mutations.push({
        kind: 'PERSON_FROM_DATE',
        personnelId: existing.personnelId,
        effectiveFrom: existing.startDate,
      });
    }
    const impact = await evaluateBrowserMutations(mutations);
    updateAuthoritativePayload((current) => {
      const index = current.sickLeaveRecords.findIndex((item) => item.id === record.id);
      const sickLeaveRecords = [...current.sickLeaveRecords];
      if (index < 0) sickLeaveRecords.push(record);
      else sickLeaveRecords[index] = record;
      return {
        ...current,
        sickLeaveRecords,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleSaveAnnualPayrollParameters = async (parameters: AnnualPayrollParameters) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveAnnualPayrollParameters(parameters);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'TAX_YEAR',
      taxYear: parameters.year,
    });
    updateAuthoritativePayload((current) => {
      const index = current.annualPayrollParameters.findIndex(
        (item) => item.year === parameters.year
      );
      const annualPayrollParameters = [...current.annualPayrollParameters];
      const exactParameters = mergePayrollUiIntoBoundary(
        index < 0 ? undefined : current.annualPayrollParameters[index],
        parameters
      );
      if (index < 0) annualPayrollParameters.push(exactParameters);
      else annualPayrollParameters[index] = exactParameters;
      annualPayrollParameters.sort((a, b) => a.year - b.year);
      return {
        ...current,
        annualPayrollParameters,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      };
    });
  };

  const handleSaveZamAylari = async (months: number[]) => {
    const normalized = normalizeZamAylari(months);
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.setAppSetting(ZAM_AYLARI_SETTING_KEY, JSON.stringify(normalized));
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({ kind: 'ALL' });
    updateAuthoritativePayload((current) => ({
      ...current,
      zamAylari: normalized,
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleDeleteSickLeaveRecord = async (id: string) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.deleteSickLeaveRecord(id);
      await loadData();
      return;
    }
    const record = sickLeaveRecords.find((item) => item.id === id);
    const impact = record
      ? await evaluateBrowserMutations({
          kind: 'PERSON_FROM_DATE',
          personnelId: record.personnelId,
          effectiveFrom: record.startDate,
        })
      : null;
    updateAuthoritativePayload((current) => ({
      ...current,
      sickLeaveRecords: current.sickLeaveRecords.filter((item) => item.id !== id),
      bordrolar: record
        ? applyBrowserPayrollImpact(current.bordrolar, impact!)
        : current.bordrolar,
      retroBatches: record
        ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact!)
        : current.retroBatches,
    }));
  };

  const handleDeleteBordro = async (event: BordroKaydi) => {
    const eventId = event.accrualId || event.id;
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.deletePayrollAccrual(event.personelId, event.donemId, eventId);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'ACCRUAL_DELETE', personnelId: event.personelId,
      periodId: event.donemId, accrualId: eventId,
    });
    updateAuthoritativePayload((current) => ({
      ...current,
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact)
        .filter((item) => (item.accrualId || item.id) !== eventId),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleSaveBordro = async (updatedBordro: PayrollBoundaryPayroll) => {
    if (tauriBridge.isTauriAvailable()) {
      // Re-fetch the whole ledger: recalculating an earlier payroll may have marked
      // one or more downstream CALCULATED payrolls as STALE in the same transaction.
      await loadData();
      return;
    }
    const existing = bordrolar.find(
      (payroll) =>
        payroll.id === updatedBordro.id || payroll.accrualId === updatedBordro.accrualId
    );
    const mutation: PayrollMutation = existing
      ? {
          kind: 'ACCRUAL_CALCULATION',
          personnelId: updatedBordro.personelId,
          periodId: updatedBordro.donemId,
          accrualId: updatedBordro.accrualId,
        }
      : {
          kind: 'ACCRUAL_INSERT',
          personnelId: updatedBordro.personelId,
          periodId: updatedBordro.donemId,
          accrualId: updatedBordro.accrualId,
          paymentDate: updatedBordro.paymentDate,
          sequence: updatedBordro.sequence,
        };
    const impact = await evaluateBrowserMutations(mutation);
    updateAuthoritativePayload((current) => {
      const invalidated = applyBrowserPayrollImpact(current.bordrolar, impact);
      const index = invalidated.findIndex(
        (b) => b.id === updatedBordro.id || b.accrualId === updatedBordro.accrualId
      );
      if (index < 0) {
        return {
          ...current,
          bordrolar: [...invalidated, updatedBordro],
          retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
        };
      }
      const next = [...invalidated];
      next[index] = updatedBordro;
      return {
        ...current,
        bordrolar: next,
        retroBatches: updatedBordro.accrualType === 'RETRO_ADJUSTMENT' && updatedBordro.status === 'FINALIZED'
          ? (current.retroBatches ?? []).map((batch) =>
              batch.id === updatedBordro.accrualId
                ? {
                    ...batch,
                    status: 'FINALIZED' as const,
                    settlementStatus: 'PAID' as const,
                    finalizedAt: updatedBordro.sonGuncellemeTarihi,
                  }
                : batch
            )
          : current.retroBatches,
      };
    });
  };

  const handleSaveCompensationRevision = async (
    revision: CompensationRevision,
    overrides: CompensationRevisionOverride[]
  ) => {
    const existing = compensationRevisions.find((item) => item.id === revision.id);
    const revisionDefinitionChanged = !sameRevisionDefinition(existing, revision) ||
      !sameRevisionOverrides(
        compensationRevisionOverrides.filter((item) => item.revisionId === revision.id),
        overrides
      );
    if (existing?.status === 'FINALIZED') {
      if (revisionDefinitionChanged) {
        throw new Error('FINALIZED revision veya ona bağlı FINALIZED retro payment event’i değiştirilemez.');
      }
      // A status-only browser update must not downgrade a finalized revision.
      // Treat an identical definition as an idempotent no-op, matching native
      // persistence and keeping the revision state machine monotonic.
      return;
    }
    if (revisionDefinitionChanged && retroBatches.some(
      (batch) => batch.revisionId === revision.id && batch.status === 'FINALIZED'
    )) {
      throw new Error('FINALIZED revision veya ona bağlı FINALIZED retro payment event’i değiştirilemez.');
    }
    if (revision.status !== 'DRAFT') {
      throw new Error('Yeni veya değiştirilen compensation revision yalnızca DRAFT olabilir.');
    }
    const overrideKeys = new Set<string>();
    for (const override of overrides) {
      const key = `${override.parameter}\u0000${override.personnelId ?? ''}`;
      if (override.revisionId !== revision.id || overrideKeys.has(key)) {
        throw new Error('Revision override kayıtlarında duplicate veya yanlış revision ilişkisi var.');
      }
      overrideKeys.add(key);
    }

    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveCompensationRevision(revision, overrides);
      await loadData();
      return;
    }

    const retroMutations: PayrollMutation[] = revisionDefinitionChanged
      ? bordrolar
      .filter((payroll) =>
        payroll.accrualType === 'RETRO_ADJUSTMENT' &&
        retroBatches.some(
          (batch) => batch.revisionId === revision.id && batch.id === payroll.accrualId && batch.status !== 'FINALIZED'
        )
      )
      .map((payroll) => ({
        kind: 'ACCRUAL_CALCULATION' as const,
        personnelId: payroll.personelId,
        periodId: payroll.donemId,
        accrualId: payroll.accrualId,
      }))
      : [];
    const retroImpact = retroMutations.length
      ? await evaluateBrowserMutations(retroMutations)
      : null;

    const exactRevision = toPayrollBoundaryDto(revision) as unknown as NonNullable<PayrollStorageDto['compensationRevisions']>[number];
    const exactOverrides = toPayrollBoundaryDto(overrides) as unknown as NonNullable<PayrollStorageDto['compensationRevisionOverrides']>;
    updateAuthoritativePayload((current) => ({
      ...current,
      compensationRevisions: [
        ...(current.compensationRevisions ?? []).filter((item) => item.id !== revision.id),
        exactRevision,
      ],
      compensationRevisionOverrides: [
        ...(current.compensationRevisionOverrides ?? []).filter((item) => item.revisionId !== revision.id),
        ...exactOverrides,
      ],
      // Existing calculations remain in the audit store, but are no longer
      // authoritative after their revision input changes.
      retroBatches: revisionDefinitionChanged
        ? (current.retroBatches ?? []).map((batch) =>
            batch.revisionId === revision.id && batch.status !== 'FINALIZED'
              ? { ...batch, status: 'STALE' as const }
              : batch
          )
        : current.retroBatches,
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, retroImpact ?? {
        affectedPayrolls: [],
        blockedByFinalized: [],
        affectedRetroBatches: [],
        blockedByFinalizedRetroBatches: [],
      }).map((payroll) =>
        revisionDefinitionChanged && (current.retroBatches ?? []).some(
          (batch) => batch.revisionId === revision.id && batch.id === payroll.accrualId && batch.status !== 'FINALIZED'
        ) && payroll.status !== 'FINALIZED'
          ? { ...payroll, status: 'STALE' as const }
          : payroll
      ),
    }));
  };

  const handleCalculateRetroPreview = async (
    request: RetroPreviewInput
  ): Promise<RetroCalculationResultModel> => {
    const datasetForPreview: PayrollDatasetSnapshot = {
      ...payrollDataset,
      retroBatches: payrollDataset.retroBatches.map((batch) =>
        batch.id === request.batchId && batch.status !== 'FINALIZED'
          ? { ...batch, status: 'STALE' as const }
          : batch
      ),
    };
    const exactRequest = toPayrollBoundaryDto({
      ...request,
      dataset: datasetForPreview,
    }) as unknown as RetroCalculationRequest;
    const result: RetroCalculationResult = await payrollEngine.calculateRetroPreview(exactRequest);
    return toPayrollUiModel(result) as unknown as RetroCalculationResultModel;
  };

  const canonicalizeRetroResult = async (
    result: RetroCalculationResultModel
  ): Promise<RetroCalculationResultModel> => {
    const submittedBatch = result.batch;
    const existingBatch = payrollDataset.retroBatches.find(
      (batch) => batch.id === submittedBatch.id
    );
    if (existingBatch && (
      existingBatch.revisionId !== submittedBatch.revisionId ||
      existingBatch.personnelId !== submittedBatch.personnelId ||
      existingBatch.paymentDate !== submittedBatch.paymentDate
    )) {
      throw new Error(
        'Retro batch primary id’si farklı revision/personel/ödeme olayına ait; yeniden bağlanamaz.'
      );
    }
    if (existingBatch?.status === 'FINALIZED') {
      throw new Error(
        'FINALIZED retro batch yeniden hesaplanamaz; yeni bir correction batch’i oluşturulmalıdır.'
      );
    }
    const existingPayrollWithBatchIdentity = payrollDataset.payrolls.find(
      (payroll) => payroll.accrualId === submittedBatch.id || payroll.id === submittedBatch.id
    );
    if (existingPayrollWithBatchIdentity && (
      existingPayrollWithBatchIdentity.personelId !== submittedBatch.personnelId ||
      existingPayrollWithBatchIdentity.accrualType !== 'RETRO_ADJUSTMENT' ||
      existingPayrollWithBatchIdentity.paymentDate !== submittedBatch.paymentDate
    )) {
      throw new Error(
        'Retro batch kimliği mevcut bir farklı ödeme olayının kimliğiyle çakışıyor; yeni bir kimlik kullanın.'
      );
    }
    const persistedRevision = compensationRevisions.find(
      (revision) => revision.id === submittedBatch.revisionId
    );
    if (!persistedRevision) {
      throw new Error(`Retro revision persisted dataset'te bulunamadı: ${submittedBatch.revisionId}`);
    }
    const replayDataset: PayrollDatasetSnapshot = {
      ...payrollDataset,
      retroBatches: payrollDataset.retroBatches
        .filter((item) => item.id !== submittedBatch.id)
        .map((item) => item),
      retroAllocations: payrollDataset.retroAllocations.filter(
        (item) => item.batchId !== submittedBatch.id
      ),
    };
    const canonicalRequest = toPayrollBoundaryDto({
      batchId: submittedBatch.id,
      revision: persistedRevision,
      overrides: compensationRevisionOverrides.filter(
        (item) => item.revisionId === persistedRevision.id
      ),
      personnelId: submittedBatch.personnelId,
      paymentDate: submittedBatch.paymentDate,
      calculatedAt: submittedBatch.calculatedAt || submittedBatch.createdAt || new Date().toISOString(),
      description: submittedBatch.description || null,
      dataset: replayDataset,
    }) as unknown as RetroCalculationRequest;
    const canonicalResult = toPayrollUiModel(
      await payrollEngine.calculateRetroPreview(canonicalRequest)
    ) as unknown as RetroCalculationResultModel;
    const sortAllocations = (allocations: RetroAllocation[]) =>
      [...allocations].sort((left, right) => left.id.localeCompare(right.id));
    if (
      canonicalResult.batch.totalGrossDelta !== submittedBatch.totalGrossDelta ||
      JSON.stringify(toPayrollBoundaryDto(sortAllocations(canonicalResult.allocations))) !==
        JSON.stringify(toPayrollBoundaryDto(sortAllocations(result.allocations)))
    ) {
      throw new Error('Retro preview güncel veriyle eşleşmiyor; yeniden hesaplayın.');
    }
    return canonicalResult;
  };

  const handleSaveRetroBatch = async (result: RetroCalculationResultModel) => {
    if (!result.allocations.some((allocation) => allocation.deltaAmount < 0)) {
      throw new Error('Yalnız negatif farklar fazla tahakkuk batch’i olarak saklanabilir.');
    }
    const canonicalResult = await canonicalizeRetroResult(result);
    const batch = canonicalResult.batch;
    const activePayment = payrollDataset.payrolls.find(
      (payroll) =>
        payroll.accrualId === batch.id &&
        (payroll.status === 'DRAFT' || payroll.status === 'CALCULATED' || payroll.status === 'FINALIZED')
    );
    if (activePayment) {
      throw new Error(
        `${batch.id} retro payment event'i ${activePayment.status} durumunda; event silinmeden fazla tahakkuk batch'i saklanamaz.`
      );
    }
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveRetroAdjustmentBatch(batch, canonicalResult.allocations);
      await loadData();
      return;
    }
    if (!authoritativePayload) throw new Error('Yetkili veri snapshot’ı hazır değil.');
    const exactBatch = toPayrollBoundaryDto(batch) as unknown as NonNullable<PayrollStorageDto['retroBatches']>[number];
    const exactAllocations = toPayrollBoundaryDto(canonicalResult.allocations) as unknown as NonNullable<PayrollStorageDto['retroAllocations']>;
    updateAuthoritativePayload((current) => ({
      ...current,
      compensationRevisions: current.compensationRevisions ?? [],
      compensationRevisionOverrides: current.compensationRevisionOverrides ?? [],
      retroBatches: [...(current.retroBatches ?? []).filter((item) => item.id !== batch.id), exactBatch],
      retroAllocations: [
        ...(current.retroAllocations ?? []).filter((item) => item.batchId !== batch.id),
        ...exactAllocations,
      ],
    }));
  };

  const handleCreateRetroPayment = async (result: RetroCalculationResultModel) => {
    const submittedBatch = result.batch;
    if (submittedBatch.totalGrossDelta <= 0 || result.allocations.some((allocation) => allocation.deltaAmount < 0)) {
      throw new Error('Negatif veya sıfır retro delta otomatik payment event’ine dönüştürülemez.');
    }
    const canonicalResult = await canonicalizeRetroResult(result);
    const batch = canonicalResult.batch;
    const paymentParts = batch.paymentDate.split('-').map(Number);
    const paymentPeriod = donemler.find(
      (period) => period.taxYear === paymentParts[0] && period.taxMonth === paymentParts[1]
    );
    if (!paymentPeriod) {
      throw new Error(`${batch.paymentDate.slice(0, 7)} için payment/tax period bulunamadı.`);
    }
    const existingPayment = payrollDataset.payrolls.find(
      (payroll) =>
        payroll.personelId === batch.personnelId &&
        payroll.donemId === paymentPeriod.id &&
        payroll.accrualId === batch.id
    );
    const sequence = existingPayment?.sequence ??
      nextPaymentSequence(payrollDataset, batch.personnelId, paymentPeriod, batch.paymentDate);
    const accrual: PayrollAccrualInput = {
      accrualId: batch.id,
      accrualType: 'RETRO_ADJUSTMENT',
      paymentDate: batch.paymentDate,
      sequence,
      grossAmount: batch.totalGrossDelta,
      description: batch.description || 'Geriye dönük hakediş farkı',
    };

    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.createRetroPayment(
        batch,
        canonicalResult.allocations,
        paymentPeriod.id,
        sequence
      );
      await loadData();
      return;
    }

    if (!authoritativePayload) throw new Error('Yetkili veri snapshot’ı hazır değil.');
    const exactBatch = toPayrollBoundaryDto(batch) as unknown as NonNullable<PayrollStorageDto['retroBatches']>[number];
    const exactAllocations = toPayrollBoundaryDto(canonicalResult.allocations) as unknown as NonNullable<PayrollStorageDto['retroAllocations']>;
    const datasetWithBatch: PayrollDatasetSnapshot = {
      ...payrollDataset,
      retroBatches: [...payrollDataset.retroBatches.filter((item) => item.id !== batch.id), exactBatch],
      retroAllocations: [
        ...payrollDataset.retroAllocations.filter((item) => item.batchId !== batch.id),
        ...exactAllocations,
      ],
    };
    const mutation: PayrollMutation = existingPayment
      ? {
          kind: 'ACCRUAL_CALCULATION',
          personnelId: batch.personnelId,
          periodId: paymentPeriod.id,
          accrualId: batch.id,
        }
      : {
      kind: 'ACCRUAL_INSERT',
      personnelId: batch.personnelId,
      periodId: paymentPeriod.id,
      accrualId: batch.id,
      paymentDate: batch.paymentDate,
      sequence,
    };
    const impact = await payrollEngine.evaluateMutationPolicy(mutation, datasetWithBatch);
    assertBrowserMutationImpactAllowed(impact);
    const paymentRequest = {
      personnelId: batch.personnelId,
      periodId: paymentPeriod.id,
      calculatedAt: new Date().toISOString(),
      manualIncome: null,
      accrual: toPayrollBoundaryDto(accrual) as unknown as Parameters<typeof payrollEngine.calculatePayroll>[0]['accrual'],
      dataset: datasetWithBatch,
    };
    // `calculatePayroll` is a pure formula call in the browser adapter. Run
    // the same strict cross-period/tax-month preflight that the native retro
    // service runs before accepting the result as a payment event.
    await payrollEngine.validatePayroll(paymentRequest);
    const calculated = await payrollEngine.calculatePayroll(paymentRequest);
    updateAuthoritativePayload((current) => {
      const invalidated = applyBrowserPayrollImpact(current.bordrolar, impact);
      const existingPayrollIndex = invalidated.findIndex((item) => item.accrualId === batch.id || item.id === batch.id);
      const nextPayrolls = [...invalidated];
      if (existingPayrollIndex < 0) nextPayrolls.push(calculated);
      else nextPayrolls[existingPayrollIndex] = calculated;
      return {
        ...current,
        compensationRevisions: current.compensationRevisions ?? [],
        compensationRevisionOverrides: current.compensationRevisionOverrides ?? [],
        retroBatches: [...(current.retroBatches ?? []).filter((item) => item.id !== batch.id), exactBatch],
        retroAllocations: [
          ...(current.retroAllocations ?? []).filter((item) => item.batchId !== batch.id),
          ...exactAllocations,
        ],
        bordrolar: nextPayrolls,
      };
    });
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
    <div className="min-h-screen bg-slate-100 text-slate-900 flex flex-col font-sans">
      {isDataLoaded && (
        <TopBar
          donemler={donemler}
          aktifDonemId={aktifDonemId}
          onSelectDonem={handleSelectDonem}
          onExportBackup={handleExportBackup}
          onImportBackup={handleImportBackup}
          onResetSampleData={handleResetSampleData}
          noticeCount={payrollNoticeCount}
          onOpenNoticeSummary={() => setActiveTab('ozet')}
          isSidebarOpen={isSidebarOpen}
          onToggleSidebar={() => setIsSidebarOpen((current) => !current)}
        />
      )}

      <div className="flex min-h-0 flex-1">
        {isDataLoaded && (
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

        <main className="min-w-0 flex-1 px-4 py-6 sm:px-6 lg:px-8">
          <div className="mx-auto w-full max-w-[1600px]">
            {loadError && isDataLoaded && (
              <div
                role="alert"
                data-testid="storage-error"
                className="mb-5 rounded-xl border border-rose-300 bg-rose-50 p-4 text-xs font-semibold text-rose-900"
              >
                {loadError}
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
                    onClick={handleResetSampleData}
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
                      onChange={handleRecoveryFileImport}
                    />
                  </label>
                </div>
              </div>
            )}

            {isDataLoaded && (
              <>
                {activeTab === 'personel' && (
                  <PersonelList
                    personeller={personeller}
                    onSavePersonel={handleSavePersonel}
                    onDeletePersonel={handleDeletePersonel}
                    onSelectPersonelForBordro={handleSelectPersonelForBordro}
                    isPrimiGruplari={aktifDonemId ? kurumDegerleriMap[aktifDonemId]?.isPrimiGruplari : undefined}
                  />
                )}

                {!aktifDonem &&
                  activeTab !== 'personel' &&
                  activeTab !== 'parametrelar' &&
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
                      <button type="button" onClick={handleResetSampleData} className="rounded-xl bg-slate-100 px-4 py-2 text-xs font-semibold text-slate-700 transition-colors hover:bg-slate-200">Örnek Verileri Yükle</button>
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
                  />
                )}

                {aktifDonem && activeTab === 'puantaj' && (
                  <PuantajGrid
                    aktifDonem={aktifDonem}
                    personeller={personeller}
                    puantajlar={puantajlar}
                    onSavePuantaj={handleSavePuantaj}
                    onSelectPersonelForBordro={handleSelectPersonelForBordro}
                  />
                )}

                {aktifDonem && activeTab === 'bordro' && (
                  <BordroHesaplama
                    aktifDonem={aktifDonem}
                    donemler={donemler}
                    personeller={personeller}
                    kurumDegerleriMap={kurumDegerleriMap}
                    puantajlar={puantajlar}
                    bordrolar={bordrolar}
                    taxOpenings={taxOpenings}
                    sickLeaveRecords={sickLeaveRecords}
                    annualPayrollParameters={annualPayrollParameters}
                    zamAylari={zamAylari}
                    activePayrollView={activePayrollView}
                    authoritativeDataset={payrollDataset}
                    onSaveBordro={handleSaveBordro}
                    onDeleteBordro={handleDeleteBordro}
                    onSavePersonel={handleSavePersonel}
                    onSaveTaxOpening={handleSaveTaxOpening}
                    initialPersonelId={targetPersonelIdForBordro}
                    onGoToPuantaj={(personelId) => {
                      if (personelId) setTargetPersonelIdForBordro(personelId);
                      setActiveTab('puantaj');
                    }}
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
                    kurumDegerleriMap={kurumDegerleriMap}
                    onSaveKurumDegerleri={handleSaveKurumDegerleri}
                    personeller={personeller}
                    annualPayrollParameters={annualPayrollParameters}
                    onSaveAnnualPayrollParameters={handleSaveAnnualPayrollParameters}
                    sickLeaveRecords={sickLeaveRecords}
                    onSaveSickLeaveRecord={handleSaveSickLeaveRecord}
                    onDeleteSickLeaveRecord={handleDeleteSickLeaveRecord}
                    zamAylari={zamAylari}
                    onSaveZamAylari={handleSaveZamAylari}
                  />
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
                    onImportBackup={handleImportBackup}
                    onResetSampleData={handleResetSampleData}
                  />
                )}
              </>
            )}
          </div>
        </main>
      </div>

    </div>
  );
}
