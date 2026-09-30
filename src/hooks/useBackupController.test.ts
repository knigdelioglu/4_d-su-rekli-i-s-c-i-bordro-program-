import { describe, expect, test } from 'bun:test';
import { useBackupController } from './useBackupController';
import { tauriBridge } from '../services/tauriBridge';

describe('useBackupController - sample data reset', () => {
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
        },
        setAuthoritativePayload: () => {},
        setIsDataLoaded: () => {},
        setLoadError: () => {},
        onSuccess: (msg) => {
          successMessage = msg;
        },
      });

      await controller.executeResetSampleData();

      expect(replacedBackupPayload !== null).toBe(true);
      expect((replacedBackupPayload ?? '').includes('"backupVersion":')).toBe(true);
      expect(loadDataCalled).toBe(true);
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
      expect(loadError).toBe('Örnek veriler yüklenemedi: Error: disk write failed');
      expect(successMessage).toBe(null);
    } finally {
      (tauriBridge as any).isTauriAvailable = originalIsTauri;
      (tauriBridge as any).replaceBackupPayload = originalReplaceBackup;
      (globalThis as any).window = originalWindow;
    }
  });
});
