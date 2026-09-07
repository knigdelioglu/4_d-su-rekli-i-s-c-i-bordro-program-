/**
 * 4/D Sürekli İşçi Bordro Programı — Main App Component
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
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
import { GeriyeDonukFarklar } from './components/GeriyeDonukFarklar';
import { BankaListesi } from './components/Listeler/BankaListesi';
import { SgkPrimKontrolu } from './components/Listeler/SgkPrimKontrolu';
import { KesintiListesi } from './components/Listeler/KesintiListesi';
import {
  BACKUP_FORMAT_VERSION,
  BackupPayload,
  ZAM_AYLARI_SETTING_KEY,
} from './types/payroll';
import { tauriBridge } from './services/tauriBridge';
import { getPayrollEngine } from './services/payrollEngine';
import type { PayrollDatasetSnapshot } from './services/payrollEngine';
import {
  toPayrollBoundaryDto,
  toPayrollUiModel,
  type PayrollStorageDto,
  type PayrollStorageFields,
} from './services/payrollEngine/decimalBoundary';
import {
  parseImportedBackup,
} from './services/storage/payrollPayload';
import { useBrowserPayrollPersistence } from './services/storage/useBrowserPayrollPersistence';
import { usePayrollMutationController } from './hooks/usePayrollMutationController';
import { useBackupController } from './hooks/useBackupController';
import { usePayrollNotices } from './components/PayrollNoticeCenter';
import { PeriodSummary } from './components/Dashboard/PeriodSummary';
import { DataBackupPage } from './components/DataBackupPage';

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

  const [targetPersonelIdForBordro, setTargetPersonelIdForBordro] = useState<
    string | undefined
  >(undefined);
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

  const browserPersistence = useBrowserPayrollPersistence({
    authoritativePayload,
    isDataLoaded,
    isNative: tauriBridge.isTauriAvailable(),
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
  });

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
    browserPersistence.markClean();
    setAuthoritativePayload(makeBackupPayload(data));
  }, [browserPersistence.markClean]);

  const updateAuthoritativePayload = useCallback(
    (update: (current: PayrollStorageDto) => PayrollStorageDto) => {
      browserPersistence.markDirty();
      setAuthoritativePayload((current) => {
        if (!current) return current;
        const next = update(current);
        return { ...next, exportedAt: new Date().toISOString() };
      });
    },
    [browserPersistence.markDirty]
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

      const saved = await browserPersistence.loadSnapshot();
      if (saved) {
        // Version-aware parsing keeps legacy compatibility explicit while a
        // current snapshot remains strict and never reaches repair logic.
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

  const mutationController = usePayrollMutationController({
    isNative: tauriBridge.isTauriAvailable(),
    payrollEngine,
    payrollDataset,
    authoritativePayload,
    donemler,
    bordrolar,
    sickLeaveRecords,
    compensationRevisions,
    compensationRevisionOverrides,
    retroBatches,
    updateAuthoritativePayload,
    loadData,
  });
  const {
    evaluateBrowserMutations,
    handleSavePersonel,
    handleDeletePersonel,
    handleCreateDonem,
    handleSaveKurumDegerleri,
    handleSavePuantaj,
    handleSaveTaxOpening,
    handleSaveSickLeaveRecord,
    handleSaveAnnualPayrollParameters,
    handleSaveZamAylari,
    handleDeleteSickLeaveRecord,
    handleDeleteBordro,
    handleSaveBordro,
    handleSaveCompensationRevision,
    handleCalculateRetroPreview,
    handleSaveRetroBatch,
    handleCreateRetroPayment,
  } = mutationController;

  const backupController = useBackupController({
    authoritativePayload,
    activePeriodId: aktifDonemId,
    isDataLoaded,
    browserPersistence,
    evaluateBrowserMutations,
    loadData,
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
  });
  const {
    handleResetSampleData,
    handleClearAndStartFresh,
    handleExportBackup,
    handleImportBackup,
    handleRecoveryFileImport,
  } = backupController;

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
