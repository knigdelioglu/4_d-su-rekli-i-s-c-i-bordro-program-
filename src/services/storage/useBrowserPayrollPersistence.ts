import { useCallback, useEffect, useRef, useState } from 'react';
import { BACKUP_FORMAT_VERSION } from '../../types/payroll';
import {
  browserPayrollStore,
  BrowserSnapshotConflictError,
  shouldAdoptRemoteSnapshot,
  type BrowserPayrollSnapshot,
} from './browserPayrollStore';
import {
  serializePayrollStorage,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';
import type { PayrollEngine } from '../payrollEngine/types';
import {
  parseCurrentBrowserSnapshot,
  parseImportedBackup,
  verifyCurrentPayrollBackupReplay,
} from './payrollPayload';

interface UseBrowserPayrollPersistenceOptions {
  authoritativePayload: PayrollStorageDto | null;
  isDataLoaded: boolean;
  isNative: boolean;
  payrollEngine: PayrollEngine;
  setAuthoritativePayload: (payload: PayrollStorageDto) => void;
  setIsDataLoaded: (loaded: boolean) => void;
  setLoadError: (message: string | null) => void;
}

interface UserFacingStorageError {
  userMessage: string;
  technicalDetail: string;
}

export const BROWSER_EXTERNAL_CONFLICT_MESSAGE =
  'Veriler başka bir sekmede değiştirildi. Bu sekmedeki kaydedilmemiş değişiklikler korunuyor.';

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatBrowserStorageSaveError(error: unknown): UserFacingStorageError {
  return {
    userMessage: 'Veriler kaydedilemedi. Mevcut kayıt korunuyor.',
    technicalDetail: getErrorMessage(error),
  };
}

function parseBrowserSnapshotPayload(snapshot: BrowserPayrollSnapshot): PayrollStorageDto {
  const raw = JSON.parse(snapshot.payload) as { backupVersion?: unknown };
  if (raw.backupVersion === BACKUP_FORMAT_VERSION) {
    return parseCurrentBrowserSnapshot(
      snapshot.payload,
      snapshot.sourceFormat === 'current'
        ? undefined
        : { allowLegacyMissingGvBase: true }
    );
  }
  return parseImportedBackup(snapshot.payload);
}

/**
 * Owns browser snapshot revision/CAS state so App does not coordinate an
 * IndexedDB write queue, cross-tab reconciliation, and optimistic rollback.
 * Native persistence never enters this hook's effects.
 */
export function useBrowserPayrollPersistence({
  authoritativePayload,
  isDataLoaded,
  isNative,
  payrollEngine,
  setAuthoritativePayload,
  setIsDataLoaded,
  setLoadError,
}: UseBrowserPayrollPersistenceOptions) {
  const snapshotRevision = useRef(0);
  const persistenceDirty = useRef(false);
  const persistenceGeneration = useRef(0);
  const pendingWriteCount = useRef(0);
  const externalSnapshot = useRef<BrowserPayrollSnapshot | null>(null);
  const saveChain = useRef<Promise<void>>(Promise.resolve());
  const [hasExternalConflict, setHasExternalConflict] = useState(false);
  const [externalRevision, setExternalRevision] = useState<number | null>(null);

  const markDirty = useCallback(() => {
    persistenceDirty.current = true;
    persistenceGeneration.current += 1;
  }, []);

  const markClean = useCallback(() => {
    persistenceDirty.current = false;
  }, []);

  const recordExternalConflict = useCallback(
    (snapshot: BrowserPayrollSnapshot | null, revision: number) => {
      if (snapshot) externalSnapshot.current = snapshot;
      setHasExternalConflict(true);
      setExternalRevision((previous) => Math.max(previous ?? -1, revision));
      setLoadError(BROWSER_EXTERNAL_CONFLICT_MESSAGE);
    },
    [setLoadError]
  );

  const adoptSnapshot = useCallback(
    (saved: BrowserPayrollSnapshot | null): string | null => {
      snapshotRevision.current = saved?.revision ?? 0;
      if (saved) browserPayrollStore.adoptSnapshot(saved);
      externalSnapshot.current = null;
      setHasExternalConflict(false);
      setExternalRevision(null);
      markClean();
      return saved?.payload ?? null;
    },
    [markClean]
  );

  const verifyStoredSnapshot = useCallback(async (saved: BrowserPayrollSnapshot | null) => {
    if (!saved) return;
    const payload = parseBrowserSnapshotPayload(saved);
    if (saved.sourceFormat === 'current') {
      await verifyCurrentPayrollBackupReplay(payload, payrollEngine);
    }
  }, [payrollEngine]);

  const loadSnapshot = useCallback(async (): Promise<string | null> => {
    const saved = await browserPayrollStore.loadSnapshot();
    await verifyStoredSnapshot(saved);
    return adoptSnapshot(saved);
  }, [adoptSnapshot, verifyStoredSnapshot]);

  const persistPayload = useCallback(
    async (
      payload: string,
      generation?: number,
      sourceFormat: 'current' | 'legacy' | 'unknown' = 'current'
    ): Promise<void> => {
      pendingWriteCount.current += 1;
      const persist = saveChain.current
        .catch(() => undefined)
        .then(async () => {
          try {
            const revision = await browserPayrollStore.savePayload(
              payload,
              snapshotRevision.current,
              sourceFormat
            );
            snapshotRevision.current = revision;
            if (generation === undefined || generation === persistenceGeneration.current) {
              markClean();
            }
          } catch (error) {
            if (error instanceof BrowserSnapshotConflictError) {
              recordExternalConflict(null, error.actualRevision);
            }
            throw error;
          }
        });
      saveChain.current = persist.then(() => undefined, () => undefined);
      try {
        await persist;
      } finally {
        pendingWriteCount.current = Math.max(0, pendingWriteCount.current - 1);
      }
    },
    [markClean, recordExternalConflict]
  );

  const savePayload = useCallback(
    async (
      payload: string,
      sourceFormat: 'current' | 'legacy' | 'unknown' = 'current'
    ): Promise<void> => persistPayload(payload, undefined, sourceFormat),
    [persistPayload]
  );

  const reloadExternalSnapshot = useCallback(async (): Promise<void> => {
    const saved = await browserPayrollStore.loadSnapshot();
    await verifyStoredSnapshot(saved);
    const payload = adoptSnapshot(saved);
    if (payload) {
      setAuthoritativePayload(parseImportedBackup(payload));
      setIsDataLoaded(true);
      setLoadError(null);
    }
  }, [
    adoptSnapshot,
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
    verifyStoredSnapshot,
  ]);

  useEffect(() => {
    if (isNative) return undefined;
    return browserPayrollStore.subscribe((snapshot: BrowserPayrollSnapshot) => {
      void (async () => {
        const payload = parseBrowserSnapshotPayload(snapshot);
        if (snapshot.sourceFormat === 'current') {
          await verifyCurrentPayrollBackupReplay(payload, payrollEngine);
        }
        const canAdopt = shouldAdoptRemoteSnapshot({
          currentRevision: snapshotRevision.current,
          remoteRevision: snapshot.revision,
          localDirty: persistenceDirty.current,
          pendingWrite: pendingWriteCount.current > 0,
        });
        if (!canAdopt) {
          if (snapshot.revision > snapshotRevision.current) {
            recordExternalConflict(snapshot, snapshot.revision);
          }
          return;
        }
        snapshotRevision.current = snapshot.revision;
        browserPayrollStore.adoptSnapshot(snapshot);
        setAuthoritativePayload(payload);
        setIsDataLoaded(true);
        setLoadError(null);
      })().catch((error) => {
        console.error('Başka bir sekmeden gelen bordro snapshotı geçersiz.', error);
        setLoadError('Başka bir sekmedeki bordro verisi okunamadı. Mevcut kayıt korunuyor.');
      });
    });
  }, [
    isNative,
    payrollEngine,
    recordExternalConflict,
    setAuthoritativePayload,
    setIsDataLoaded,
    setLoadError,
  ]);

  useEffect(() => {
    if (!isDataLoaded || isNative || !authoritativePayload || !persistenceDirty.current) {
      return;
    }

    const generation = persistenceGeneration.current;
    const serialized = serializePayrollStorage(authoritativePayload);
    void persistPayload(serialized, generation).catch(async (error) => {
      if (generation !== persistenceGeneration.current) return;
      if (error instanceof BrowserSnapshotConflictError) return;
      const browserError = formatBrowserStorageSaveError(error);
      console.error('Tarayıcı verisi kaydedilemedi.', browserError.technicalDetail, error);
      setLoadError(browserError.userMessage);
      try {
        const saved = await browserPayrollStore.loadSnapshot();
        if (!saved) return;
        const payload = adoptSnapshot(saved);
        if (payload) setAuthoritativePayload(parseImportedBackup(payload));
      } catch (reloadError) {
        console.error('Son başarılı tarayıcı snapshotı geri yüklenemedi.', reloadError);
        setIsDataLoaded(false);
      }
    });
  }, [adoptSnapshot, authoritativePayload, isDataLoaded, isNative, persistPayload, setAuthoritativePayload, setIsDataLoaded, setLoadError]);

  return {
    loadSnapshot,
    markClean,
    markDirty,
    savePayload,
    hasExternalConflict,
    externalRevision,
    reloadExternalSnapshot,
  };
}
