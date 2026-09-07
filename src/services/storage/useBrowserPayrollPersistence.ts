import { useCallback, useEffect, useRef, useState } from 'react';
import {
  browserPayrollStore,
  BrowserSnapshotConflictError,
  shouldAdoptRemoteSnapshot,
  type BrowserPayrollSnapshot,
} from './browserPayrollStore';
import { parseImportedBackup } from './payrollPayload';
import {
  serializePayrollStorage,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';

interface UseBrowserPayrollPersistenceOptions {
  authoritativePayload: PayrollStorageDto | null;
  isDataLoaded: boolean;
  isNative: boolean;
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

/**
 * Owns browser snapshot revision/CAS state so App does not coordinate an
 * IndexedDB write queue, cross-tab reconciliation, and optimistic rollback.
 * Native persistence never enters this hook's effects.
 */
export function useBrowserPayrollPersistence({
  authoritativePayload,
  isDataLoaded,
  isNative,
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

  const loadSnapshot = useCallback(async (): Promise<string | null> => {
    const saved = await browserPayrollStore.loadSnapshot();
    return adoptSnapshot(saved);
  }, [adoptSnapshot]);

  const persistPayload = useCallback(
    async (payload: string, generation?: number): Promise<void> => {
      pendingWriteCount.current += 1;
      const persist = saveChain.current
        .catch(() => undefined)
        .then(async () => {
          try {
            const revision = await browserPayrollStore.savePayload(
              payload,
              snapshotRevision.current
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
    async (payload: string): Promise<void> => persistPayload(payload),
    [persistPayload]
  );

  const reloadExternalSnapshot = useCallback(async (): Promise<void> => {
    const saved = await browserPayrollStore.loadSnapshot();
    const payload = adoptSnapshot(saved);
    if (payload) {
      setAuthoritativePayload(parseImportedBackup(payload));
      setIsDataLoaded(true);
      setLoadError(null);
    }
  }, [adoptSnapshot, setAuthoritativePayload, setIsDataLoaded, setLoadError]);

  useEffect(() => {
    if (isNative) return undefined;
    return browserPayrollStore.subscribe((snapshot: BrowserPayrollSnapshot) => {
      try {
        const payload = parseImportedBackup(snapshot.payload);
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
      } catch (error) {
        console.error('Başka bir sekmeden gelen bordro snapshotı geçersiz.', error);
        setLoadError('Başka bir sekmedeki bordro verisi okunamadı. Mevcut kayıt korunuyor.');
      }
    });
  }, [isNative, recordExternalConflict, setAuthoritativePayload, setIsDataLoaded, setLoadError]);

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
