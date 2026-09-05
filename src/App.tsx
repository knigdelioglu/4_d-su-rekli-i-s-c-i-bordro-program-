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
import { BankaListesi } from './components/Listeler/BankaListesi';
import { SgkPrimKontrolu } from './components/Listeler/SgkPrimKontrolu';
import { KesintiListesi } from './components/Listeler/KesintiListesi';
import {
  AnnualPayrollParameters,
  BACKUP_FORMAT_VERSION,
  BackupPayload,
  BordroDonemi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  SickLeaveRecord,
  ZAM_AYLARI_SETTING_KEY,
} from './types/payroll';
import { tauriBridge } from './services/tauriBridge';
import { browserPayrollStore } from './services/storage/browserPayrollStore';
import {
  applyBrowserPayrollImpact,
  assertBrowserMutationImpactAllowed,
} from './services/storage/browserPayrollPolicies';
import { getPayrollEngine } from './services/payrollEngine';
import type {
  MutationImpact,
  PayrollBoundaryPayroll,
  PayrollBoundaryPersonel,
  PayrollBoundaryTaxOpening,
  PayrollDatasetSnapshot,
  PayrollMutation,
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
  parseCurrentBrowserSnapshot,
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
type UiDatasetFields = Omit<BackupPayload, 'backupVersion' | 'exportedAt'>;

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

  const uiDataset = useMemo<UiDatasetFields>(() => {
    if (!authoritativePayload) return EMPTY_UI_DATASET;
    return toPayrollUiModel(authoritativePayload) as unknown as UiDatasetFields;
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

        const [fetchedPeriods, fetchedPersonnel, fetchedAttendance, fetchedPayrolls, fetchedSettings, fetchedTaxOpenings, fetchedSickLeaves, fetchedAnnualParameters, savedActivePeriodId, savedZamAylari] =
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
        }));
        setIsDataLoaded(true);
        return;
      }

      const saved = await browserPayrollStore.loadPayload();
      if (saved) {
        // IndexedDB may contain a pre-v3 backup. Route every persisted
        // snapshot through the import normalizer so legacy single-payroll
        // records become one NORMAL accrual without losing their snapshot.
        const payload = parseImportedBackup(saved);
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
        }));
      }
      setIsDataLoaded(true);
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
    void browserPayrollStore.savePayload(serializePayrollStorage(authoritativePayload)).catch((err) => {
      const browserError = formatBrowserStorageSaveError(err);
      console.error('Tarayıcı verisi kaydedilemedi.', browserError.technicalDetail, err);
      setLoadError(browserError.userMessage);
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
    for (const impact of impacts) {
      for (const key of impact.affectedPayrolls) {
        affected.set(`${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`, key);
      }
      for (const key of impact.blockedByFinalized) {
        blocked.set(`${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`, key);
      }
    }
    const merged = {
      affectedPayrolls: [...affected.values()],
      blockedByFinalized: [...blocked.values()],
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
    }));

    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
        return;
      }

      await evaluateBrowserMutations({ kind: 'ALL' });
      applyDataset(payload);
      setIsDataLoaded(true);
    } catch (err) {
      const message = `Örnek veriler yüklenemedi: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
      alert(message);
    }
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
        await evaluateBrowserMutations({ kind: 'ALL' });
        // Import is a user-visible commit point. Verify the IndexedDB write
        // before replacing the in-memory dataset or announcing success.
        await browserPayrollStore.savePayload(serializePayrollStorage(payload));
        applyDataset(payload);
        setIsDataLoaded(true);
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
    updateAuthoritativePayload((current) => ({
      ...current,
      personeller: current.personeller.filter((person) => person.id !== personelId),
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact).filter(
        (payroll) => payroll.personelId !== personelId
      ),
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
      if (index < 0) return { ...current, bordrolar: [...invalidated, updatedBordro] };
      const next = [...invalidated];
      next[index] = updatedBordro;
      return { ...current, bordrolar: next };
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
            {loadError && (
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
