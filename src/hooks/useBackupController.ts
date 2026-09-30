import type { ChangeEvent } from 'react';
import { BACKUP_FORMAT_VERSION } from '../types/payroll';
import { getInitialDataset } from '../utils/sampleData';
import { tauriBridge } from '../services/tauriBridge';
import type { PayrollEngine, PayrollMutation } from '../services/payrollEngine';
import { formatPayrollError } from '../components/useBordroCalculationController';
import {
  serializePayrollStorage,
  toPayrollBoundaryDto,
  type PayrollStorageDto,
} from '../services/payrollEngine/decimalBoundary';
import {
  parseImportedBackup,
  verifyCurrentPayrollBackupReplay,
} from '../services/storage/payrollPayload';

interface BrowserPersistenceAdapter {
  markClean: () => void;
  savePayload: (
    payload: string,
    sourceFormat?: 'current' | 'legacy' | 'unknown'
  ) => Promise<void>;
}

export interface BackupControllerOptions {
  authoritativePayload: PayrollStorageDto | null;
  activePeriodId: string;
  isDataLoaded: boolean;
  browserPersistence: BrowserPersistenceAdapter;
  evaluateBrowserMutations: (mutation: PayrollMutation) => Promise<unknown>;
  payrollEngine: PayrollEngine;
  loadData: () => Promise<void | boolean>;
  setAuthoritativePayload: (payload: PayrollStorageDto) => void;
  setIsDataLoaded: (loaded: boolean) => void;
  setLoadError: (message: string | null) => void;
  onSuccess?: (message: string) => void;
}

function makeBackupPayload<T extends object>(dataset: T): PayrollStorageDto {
  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    ...toPayrollBoundaryDto(dataset),
  } as unknown as PayrollStorageDto;
}

function formatPreservedDataFailure(action: string, error: unknown): string {
  return `${action}; mevcut kayıt korundu. ${formatPayrollError(error)}`;
}

class BackupReloadFailure extends Error {}

async function reloadPersistedBackup(loadData: () => Promise<void | boolean>): Promise<void> {
  try {
    const loaded = await loadData();
    if (loaded === false) throw new Error('Kaydedilen yedek uygulamaya yeniden yüklenemedi.');
  } catch (error) {
    throw new BackupReloadFailure(formatPayrollError(error));
  }
}

function formatBackupOperationFailure(action: string, error: unknown): string {
  if (error instanceof BackupReloadFailure) {
    const subject = action.includes('Örnek')
      ? 'Örnek veriler'
      : action.includes('Veriler')
        ? 'Veriler'
        : 'Yedek';
    return `${subject} veritabanına kaydedildi ancak uygulamaya yeniden yüklenemedi. Veritabanındaki kayıt korunuyor. ${error.message}`;
  }
  return formatPreservedDataFailure(action, error);
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
  payrollEngine,
  loadData,
  setAuthoritativePayload,
  setIsDataLoaded,
  setLoadError,
  onSuccess,
}: BackupControllerOptions) {
  const commitBrowserPayload = (payload: PayrollStorageDto) => {
    browserPersistence.markClean();
    setAuthoritativePayload(payload);
    setIsDataLoaded(true);
    setLoadError(null);
  };

  const executeResetSampleData = async () => {
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
        await reloadPersistedBackup(loadData);
        onSuccess?.('Örnek veriler başarıyla yüklendi ve kalıcı olarak kaydedildi.');
        return;
      }

      if (isDataLoaded) {
        await evaluateBrowserMutations({ kind: 'ALL' });
      }
      await browserPersistence.savePayload(serializePayrollStorage(payload));
      commitBrowserPayload(payload);
      onSuccess?.('Örnek veriler başarıyla yüklendi ve kalıcı olarak kaydedildi.');
    } catch (err) {
      const message = formatBackupOperationFailure('Örnek veriler yüklenemedi', err);
      console.error(message, err);
      setLoadError(message);
      throw err;
    }
  };

  // Confirmation belongs to the app UI so the same accessible dialog works in
  // both the browser and Tauri webview. Keep this handler for existing callers.
  const handleResetSampleData = async () => executeResetSampleData();

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
      if (isDataLoaded) {
        await evaluateBrowserMutations({ kind: 'ALL' });
      }
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await reloadPersistedBackup(loadData);
        return;
      }
      await browserPersistence.savePayload(serializePayrollStorage(payload));
      commitBrowserPayload(payload);
    } catch (err) {
      const message = formatBackupOperationFailure('Veriler sıfırlanamadı', err);
      console.error(message, err);
      setLoadError(message);
      alert(message);
    }
  };

  const handleExportBackup = async () => {
    if (!authoritativePayload) return;
    const jsonStr = serializePayrollStorage(authoritativePayload, 2);
    const fileName = `4D_Bordro_Yedek_${activePeriodId || 'bos'}_${new Date()
      .toISOString()
      .slice(0, 10)}.json`;

    if (tauriBridge.isTauriAvailable()) {
      try {
        const saved = await tauriBridge.exportBackup(jsonStr, fileName);
        if (saved) {
          setLoadError(null);
          onSuccess?.('Yedek dosyası başarıyla kaydedildi.');
        }
      } catch (error) {
        const message = `Yedek dosyası kaydedilemedi. ${formatPayrollError(error)}`;
        setLoadError(message);
      }
      return;
    }

    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = fileName;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const handleImportBackup = async (jsonStr: string) => {
    try {
      const raw: unknown = JSON.parse(jsonStr);
      const isCurrentBackup = Boolean(
        raw && typeof raw === 'object' && !Array.isArray(raw)
          && (raw as { backupVersion?: unknown }).backupVersion === BACKUP_FORMAT_VERSION
      );
      const payload = parseImportedBackup(jsonStr);
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await reloadPersistedBackup(loadData);
      } else {
        if (isCurrentBackup) {
          await verifyCurrentPayrollBackupReplay(payload, payrollEngine);
        }
        if (isDataLoaded) {
          await evaluateBrowserMutations({ kind: 'ALL' });
        }
        // Import is a user-visible commit point. Verify the IndexedDB write
        // before replacing the in-memory dataset or announcing success.
        await browserPersistence.savePayload(
          serializePayrollStorage(payload),
          isCurrentBackup ? 'current' : 'legacy'
        );
        commitBrowserPayload(payload);
      }
      setLoadError(null);
      onSuccess?.('Yedek başarıyla yüklendi ve kalıcı olarak kaydedildi.');
    } catch (err) {
      console.error('Yedek yükleme başarısız:', err);
      setLoadError(formatBackupOperationFailure('Yedek yüklenemedi', err));
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
    executeResetSampleData,
    handleResetSampleData,
    handleClearAndStartFresh,
    handleExportBackup,
    handleImportBackup,
    handleRecoveryFileImport,
  };
}
