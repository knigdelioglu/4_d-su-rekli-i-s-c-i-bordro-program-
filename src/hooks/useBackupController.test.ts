import { describe, expect, test } from 'bun:test';
import { useBackupController } from './useBackupController';
import { tauriBridge } from '../services/tauriBridge';
import { getInitialDataset } from '../utils/sampleData';
import { toPayrollBoundaryDto } from '../services/payrollEngine/decimalBoundary';

describe('useBackupController - sample data reset', () => {
  test('native backup export waits for file write, treats cancellation as no-op, and exposes write errors', async () => {
    const originalIsTauri = tauriBridge.isTauriAvailable;
    const originalExportBackup = (tauriBridge as any).exportBackup;
    (tauriBridge as any).isTauriAvailable = () => true;

    let exportResult: boolean | Error = true;
    const calls: Array<{ payload: string; fileName: string }> = [];
    const successMessages: string[] = [];
    const errors: Array<string | null> = [];
    (tauriBridge as any).exportBackup = async (payload: string, fileName: string) => {
      calls.push({ payload, fileName });
      if (exportResult instanceof Error) throw exportResult;
      return exportResult;
    };

    try {
      const controller = useBackupController({
        authoritativePayload: {
          backupVersion: 5,
          exportedAt: '2026-09-30T00:00:00.000Z',
          ...toPayrollBoundaryDto(getInitialDataset()),
        } as any,
        activePeriodId: '2026-09',
        isDataLoaded: true,
        browserPersistence: { markClean: () => {}, savePayload: async () => {} },
        evaluateBrowserMutations: async () => ({} as any),
        payrollEngine: {} as any,
        loadData: async () => {},
        setAuthoritativePayload: () => {},
        setIsDataLoaded: () => {},
        setLoadError: (message) => { errors.push(message); },
        onSuccess: (message) => { successMessages.push(message); },
      });

      await controller.handleExportBackup();
      expect(calls).toHaveLength(1);
      expect(JSON.parse(calls[0].payload).backupVersion).toBe(5);
      expect(calls[0].fileName).toMatch(/^4D_Bordro_Yedek_2026-09_.*\.json$/);
      expect(successMessages).toEqual(['Yedek dosyası başarıyla kaydedildi.']);

      exportResult = false;
      await controller.handleExportBackup();
      expect(successMessages).toHaveLength(1);

      exportResult = new Error('disk write failed');
      await controller.handleExportBackup();
      expect(errors.at(-1)).toBe('Yedek dosyası kaydedilemedi. disk write failed');
      expect(successMessages).toHaveLength(1);
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
      (tauriBridge as any).exportBackup = originalExportBackup;
    }
  });

  test('executeResetSampleData in browser persists sample data, commits state, and calls onSuccess', async () => {
    let savedPayload: string | null = null;
    let authoritativePayloadSet: unknown = null;
    let successMessage: string | null = null;
    let dataLoadedSet = false;

    // Force browser mode
    const originalIsTauri = tauriBridge.isTauriAvailable;
    (tauriBridge as any).isTauriAvailable = () => false;

    const controller = useBackupController({
      authoritativePayload: null,
      activePeriodId: '2026-09',
      isDataLoaded: true,
      browserPersistence: {
        markClean: () => {},
        savePayload: async (payload: string) => {
          savedPayload = payload;
        },
      },
      evaluateBrowserMutations: async () => ({
        affectedPayrolls: [],
        blockedByFinalized: [],
        affectedRetroBatches: [],
        blockedByFinalizedRetroBatches: [],
      }),
      payrollEngine: {} as any,
      loadData: async () => {},
      setAuthoritativePayload: (payload) => {
        authoritativePayloadSet = payload;
      },
      setIsDataLoaded: (loaded) => {
        dataLoadedSet = loaded;
      },
      setLoadError: () => {},
      onSuccess: (msg) => {
        successMessage = msg;
      },
    });

    try {
      await controller.executeResetSampleData();

      expect(savedPayload !== null).toBe(true);
      expect((savedPayload ?? '').includes('"backupVersion":')).toBe(true);
      expect(authoritativePayloadSet !== null).toBe(true);
      expect(dataLoadedSet).toBe(true);
      expect(successMessage).toBe('Örnek veriler başarıyla yüklendi ve kalıcı olarak kaydedildi.');
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
    }
  });

  test('executeResetSampleData in Tauri calls replaceBackupPayload, loadData, and onSuccess', async () => {
    let replacedBackupPayload: string | null = null;
    let loadDataCalled = false;
    let finishLoadData: (() => void) | undefined;
    let successMessage: string | null = null;

    const originalIsTauri = tauriBridge.isTauriAvailable;
    const originalReplaceBackup = (tauriBridge as any).replaceBackupPayload;
    (tauriBridge as any).isTauriAvailable = () => true;
    (tauriBridge as any).replaceBackupPayload = async (payloadJson: string) => {
      replacedBackupPayload = payloadJson;
    };

    try {
      const controller = useBackupController({
        authoritativePayload: null,
        activePeriodId: '2026-09',
        isDataLoaded: true,
        browserPersistence: {
          markClean: () => {},
          savePayload: async () => {},
        },
        evaluateBrowserMutations: async () => ({} as any),
        payrollEngine: {} as any,
        loadData: async () => {
          loadDataCalled = true;
          await new Promise<void>((resolve) => { finishLoadData = resolve; });
        },
        setAuthoritativePayload: () => {},
        setIsDataLoaded: () => {},
        setLoadError: () => {},
        onSuccess: (msg) => {
          successMessage = msg;
        },
      });

      const resetPromise = controller.executeResetSampleData();
      await Promise.resolve();

      expect(replacedBackupPayload !== null).toBe(true);
      expect((replacedBackupPayload ?? '').includes('"backupVersion":')).toBe(true);
      expect(loadDataCalled).toBe(true);
      expect(successMessage).toBe(null);
      finishLoadData?.();
      await resetPromise;
      expect(successMessage).toBe('Örnek veriler başarıyla yüklendi ve kalıcı olarak kaydedildi.');
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
      (tauriBridge as any).replaceBackupPayload = originalReplaceBackup;
    }
  });

  test('handleResetSampleData in Tauri runs without window.confirm and reports failure', async () => {
    let loadError: string | null = null;
    let successMessage: string | null = null;
    const originalIsTauri = tauriBridge.isTauriAvailable;
    const originalReplaceBackup = (tauriBridge as any).replaceBackupPayload;
    const originalWindow = globalThis.window;
    (tauriBridge as any).isTauriAvailable = () => true;
    (tauriBridge as any).replaceBackupPayload = async () => {
      throw new Error('disk write failed');
    };
    // Any accidental browser confirmation would fail this scenario. The app's
    // accessible dialog is the confirmation boundary for both browser and Tauri.
    (globalThis as any).window = {
      confirm: () => {
        throw new Error('window.confirm should not be called');
      },
    };

    try {
      const controller = useBackupController({
        authoritativePayload: null,
        activePeriodId: '2026-09',
        isDataLoaded: true,
        browserPersistence: { markClean: () => {}, savePayload: async () => {} },
        evaluateBrowserMutations: async () => ({} as any),
        payrollEngine: {} as any,
        loadData: async () => {},
        setAuthoritativePayload: () => {},
        setIsDataLoaded: () => {},
        setLoadError: (message) => {
          loadError = message;
        },
        onSuccess: (message) => {
          successMessage = message;
        },
      });

      let rejected = false;
      try {
        await controller.handleResetSampleData();
      } catch {
        rejected = true;
      }

      expect(rejected).toBe(true);
      expect(loadError).toBe('Örnek veriler yüklenemedi; mevcut kayıt korundu. disk write failed');
      expect(successMessage).toBe(null);
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
      (tauriBridge as any).replaceBackupPayload = originalReplaceBackup;
      (globalThis as any).window = originalWindow;
    }
  });

  test('native finalized reset failure preserves the authoritative payload and exposes the domain reason', async () => {
    const originalPayload = { personeller: [{ id: 'existing' }] } as any;
    let loadError: string | null = null;
    let successMessage: string | null = null;
    let loadDataCalled = false;
    const originalIsTauri = tauriBridge.isTauriAvailable;
    const originalReplaceBackup = (tauriBridge as any).replaceBackupPayload;
    (tauriBridge as any).isTauriAvailable = () => true;
    (tauriBridge as any).replaceBackupPayload = async () => {
      throw {
        type: 'PayrollFinalized',
        message: 'Kesinleştirilmiş (FINALIZED) bordro tarihçesini etkileyen veri değiştirilemez.',
      };
    };

    try {
      const controller = useBackupController({
        authoritativePayload: originalPayload,
        activePeriodId: '2026-09',
        isDataLoaded: true,
        browserPersistence: { markClean: () => {}, savePayload: async () => {} },
        evaluateBrowserMutations: async () => ({} as any),
        payrollEngine: {} as any,
        loadData: async () => { loadDataCalled = true; },
        setAuthoritativePayload: () => { throw new Error('failed reset must not replace UI state'); },
        setIsDataLoaded: () => {},
        setLoadError: (message) => { loadError = message; },
        onSuccess: (message) => { successMessage = message; },
      });

      let rejected = false;
      try { await controller.executeResetSampleData(); } catch { rejected = true; }
      expect(rejected).toBe(true);
      expect(originalPayload.personeller[0].id).toBe('existing');
      expect(loadDataCalled).toBe(false);
      expect(loadError).toBe(
        'Örnek veriler yüklenemedi; mevcut kayıt korundu. Kesinleştirilmiş (FINALIZED) bordro tarihçesini etkileyen veri değiştirilemez.'
      );
      expect(successMessage).toBe(null);
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
      (tauriBridge as any).replaceBackupPayload = originalReplaceBackup;
    }
  });

  test('native restore reports failure in UI state instead of using alert', async () => {
    let loadError: string | null = null;
    let successMessage: string | null = null;
    let loadDataCalled = false;
    const originalIsTauri = tauriBridge.isTauriAvailable;
    const originalReplaceBackup = (tauriBridge as any).replaceBackupPayload;
    const originalWindow = globalThis.window;
    (tauriBridge as any).isTauriAvailable = () => true;
    (tauriBridge as any).replaceBackupPayload = async () => {
      throw {
        type: 'PayrollFinalized',
        message: 'Kesinleştirilmiş (FINALIZED) bordro tarihçesini etkileyen veri değiştirilemez.',
      };
    };
    (globalThis as any).window = { alert: () => { throw new Error('window.alert should not be called'); } };

    try {
      const controller = useBackupController({
        authoritativePayload: null,
        activePeriodId: '2026-09',
        isDataLoaded: true,
        browserPersistence: { markClean: () => {}, savePayload: async () => {} },
        evaluateBrowserMutations: async () => ({} as any),
        payrollEngine: {} as any,
        loadData: async () => { loadDataCalled = true; },
        setAuthoritativePayload: () => {},
        setIsDataLoaded: () => {},
        setLoadError: (message) => { loadError = message; },
        onSuccess: (message) => { successMessage = message; },
      });

      const payload = {
        backupVersion: 5,
        exportedAt: new Date().toISOString(),
        ...toPayrollBoundaryDto(getInitialDataset()),
        compensationRevisions: [],
        compensationRevisionOverrides: [],
        retroBatches: [],
        retroAllocations: [],
      };
      await controller.handleImportBackup(JSON.stringify(payload));

      expect(loadDataCalled).toBe(false);
      expect((loadError ?? '').includes('mevcut kayıt korundu')).toBe(true);
      expect((loadError ?? '').includes('FINALIZED')).toBe(true);
      expect(successMessage).toBe(null);
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
      (tauriBridge as any).replaceBackupPayload = originalReplaceBackup;
      (globalThis as any).window = originalWindow;
    }
  });

  test('native restore parse failure is exposed through the shared UI error state', async () => {
    let loadError: string | null = null;
    let successMessage: string | null = null;
    const originalIsTauri = tauriBridge.isTauriAvailable;
    (tauriBridge as any).isTauriAvailable = () => true;

    try {
      const controller = useBackupController({
        authoritativePayload: null,
        activePeriodId: '2026-09',
        isDataLoaded: true,
        browserPersistence: { markClean: () => {}, savePayload: async () => {} },
        evaluateBrowserMutations: async () => ({} as any),
        payrollEngine: {} as any,
        loadData: async () => true,
        setAuthoritativePayload: () => {},
        setIsDataLoaded: () => {},
        setLoadError: (message) => { loadError = message; },
        onSuccess: (message) => { successMessage = message; },
      });

      await controller.handleImportBackup('{broken json');

      expect((loadError ?? '').includes('mevcut kayıt korundu')).toBe(true);
      expect((loadError ?? '').includes('JSON')).toBe(true);
      expect(successMessage).toBe(null);
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
    }
  });

  test('native persistence followed by reload failure is not reported as preserved data', async () => {
    let loadError: string | null = null;
    let successMessage: string | null = null;
    const originalIsTauri = tauriBridge.isTauriAvailable;
    const originalReplaceBackup = (tauriBridge as any).replaceBackupPayload;
    (tauriBridge as any).isTauriAvailable = () => true;
    (tauriBridge as any).replaceBackupPayload = async () => {};

    try {
      const controller = useBackupController({
        authoritativePayload: null,
        activePeriodId: '2026-09',
        isDataLoaded: true,
        browserPersistence: { markClean: () => {}, savePayload: async () => {} },
        evaluateBrowserMutations: async () => ({} as any),
        payrollEngine: {} as any,
        loadData: async () => false,
        setAuthoritativePayload: () => {},
        setIsDataLoaded: () => {},
        setLoadError: (message) => { loadError = message; },
        onSuccess: (message) => { successMessage = message; },
      });

      let rejected = false;
      try { await controller.executeResetSampleData(); } catch { rejected = true; }
      expect(rejected).toBe(true);

      expect((loadError ?? '').includes('Örnek veriler veritabanına kaydedildi ancak uygulamaya yeniden yüklenemedi')).toBe(true);
      expect((loadError ?? '').includes('Veritabanındaki kayıt korunuyor')).toBe(true);
      expect((loadError ?? '').includes('mevcut kayıt korundu')).toBe(false);
      expect(successMessage).toBe(null);
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
      (tauriBridge as any).replaceBackupPayload = originalReplaceBackup;
    }
  });
});
