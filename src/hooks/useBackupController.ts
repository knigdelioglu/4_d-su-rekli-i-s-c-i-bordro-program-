import type { ChangeEvent } from 'react';
import { BACKUP_FORMAT_VERSION } from '../types/payroll';
import { getInitialDataset } from '../utils/sampleData';
import { tauriBridge } from '../services/tauriBridge';
import type { PayrollMutation } from '../services/payrollEngine';
import {
  serializePayrollStorage,
  toPayrollBoundaryDto,
  type PayrollStorageDto,
} from '../services/payrollEngine/decimalBoundary';
import { parseImportedBackup } from '../services/storage/payrollPayload';

interface BrowserPersistenceAdapter {
  markClean: () => void;
  savePayload: (payload: string) => Promise<void>;
}

export interface BackupControllerOptions {
  authoritativePayload: PayrollStorageDto | null;
  activePeriodId: string;
  isDataLoaded: boolean;
  browserPersistence: BrowserPersistenceAdapter;
  evaluateBrowserMutations: (mutation: PayrollMutation) => Promise<unknown>;
  loadData: () => Promise<void>;
  setAuthoritativePayload: (payload: PayrollStorageDto) => void;
  setIsDataLoaded: (loaded: boolean) => void;
  setLoadError: (message: string | null) => void;
}

function makeBackupPayload<T extends object>(dataset: T): PayrollStorageDto {
  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    ...toPayrollBoundaryDto(dataset),
  } as unknown as PayrollStorageDto;
}

/**
 * Coordinates backup/reset persistence. Validation and authoritative writes
 * stay in the existing adapters; this controller only owns the application
 * flow around those commits.
 */
export function useBackupController({
  authoritativePayload,
  activePeriodId,
  isDataLoaded,
  browserPersistence,
  evaluateBrowserMutations,
  loadData,
  setAuthoritativePayload,
  setIsDataLoaded,
  setLoadError,
}: BackupControllerOptions) {
  const commitBrowserPayload = (payload: PayrollStorageDto) => {
    browserPersistence.markClean();
    setAuthoritativePayload(payload);
    setIsDataLoaded(true);
    setLoadError(null);
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
    const payload = makeBackupPayload({
      ...initialData,
      taxOpenings: initialData.taxOpenings || [],
      sickLeaveRecords: initialData.sickLeaveRecords || [],
      annualPayrollParameters: initialData.annualPayrollParameters || [],
      zamAylari: initialData.zamAylari || [],
      compensationRevisions: [],
      compensationRevisionOverrides: [],
      retroBatches: [],
      retroAllocations: [],
    });

    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
        return;
      }

      if (isDataLoaded) {
        await evaluateBrowserMutations({ kind: 'ALL' });
      }
      await browserPersistence.savePayload(serializePayrollStorage(payload));
      commitBrowserPayload(payload);
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
    const payload = makeBackupPayload({
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
    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
        return;
      }
      await browserPersistence.savePayload(serializePayrollStorage(payload));
      commitBrowserPayload(payload);
    } catch (err) {
      const message = `Veriler sıfırlanamadı: ${String(err)}`;
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
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `4D_Bordro_Yedek_${activePeriodId || 'bos'}_${new Date()
      .toISOString()
      .slice(0, 10)}.json`;
    anchor.click();
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
        await browserPersistence.savePayload(serializePayrollStorage(payload));
        commitBrowserPayload(payload);
      }
      alert('Yedek başarıyla yüklendi!');
    } catch (err) {
      console.error('Yedek yükleme başarısız:', err);
      alert('Yedek yüklenemedi. Mevcut kayıt korunuyor.');
    }
  };

  const handleRecoveryFileImport = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async (loadEvent) => {
      const content = loadEvent.target?.result;
      if (typeof content === 'string') {
        await handleImportBackup(content);
      }
    };
    reader.readAsText(file);
    event.target.value = '';
  };

  return {
    handleResetSampleData,
    handleClearAndStartFresh,
    handleExportBackup,
    handleImportBackup,
    handleRecoveryFileImport,
  };
}
