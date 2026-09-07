import { useCallback, useEffect, useRef } from 'react';
import {
  browserPayrollStore,
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
  const saveChain = useRef<Promise<void>>(Promise.resolve());

  const markDirty = useCallback(() => {
    persistenceDirty.current = true;
    persistenceGeneration.current += 1;
  }, []);

  const markClean = useCallback(() => {
    persistenceDirty.current = false;
  }, []);

  const loadSnapshot = useCallback(async (): Promise<string | null> => {
    const saved = await browserPayrollStore.loadSnapshot();
    snapshotRevision.current = saved?.revision ?? 0;
    markClean();
    return saved?.payload ?? null;
  }, [markClean]);

  const savePayload = useCallback(async (payload: string): Promise<void> => {
    const persist = saveChain.current
      .catch(() => undefined)
      .then(async () => {
        const revision = await browserPayrollStore.savePayload(
          payload,
          snapshotRevision.current
        );
        snapshotRevision.current = revision;
        markClean();
      });
    saveChain.current = persist.then(() => undefined, () => undefined);
    await persist;
  }, [markClean]);

  useEffect(() => {
    if (isNative) return undefined;
    return browserPayrollStore.subscribe((snapshot: BrowserPayrollSnapshot) => {
      if (snapshot.revision <= snapshotRevision.current) return;
      try {
        const payload = parseImportedBackup(snapshot.payload);
        snapshotRevision.current = snapshot.revision;
        browserPayrollStore.adoptSnapshot(snapshot);
        markClean();
        setAuthoritativePayload(payload);
        setIsDataLoaded(true);
        setLoadError(null);
      } catch (error) {
        console.error('Başka bir sekmeden gelen bordro snapshotı geçersiz.', error);
        setLoadError('Başka bir sekmedeki bordro verisi okunamadı. Mevcut kayıt korunuyor.');
      }
    });
  }, [isNative, markClean, setAuthoritativePayload, setIsDataLoaded, setLoadError]);

  useEffect(() => {
    if (!isDataLoaded || isNative || !authoritativePayload || !persistenceDirty.current) {
      return;
    }

    const generation = persistenceGeneration.current;
    const serialized = serializePayrollStorage(authoritativePayload);
    const persist = saveChain.current
      .catch(() => undefined)
      .then(async () => {
        try {
          const revision = await browserPayrollStore.savePayload(
            serialized,
            snapshotRevision.current
          );
          snapshotRevision.current = revision;
          if (generation === persistenceGeneration.current) {
            persistenceDirty.current = false;
          }
        } catch (error) {
          if (generation !== persistenceGeneration.current) return;
          const browserError = formatBrowserStorageSaveError(error);
          console.error('Tarayıcı verisi kaydedilemedi.', browserError.technicalDetail, error);
          setLoadError(browserError.userMessage);
          try {
            const saved = await browserPayrollStore.loadSnapshot();
            if (!saved) return;
            snapshotRevision.current = saved.revision;
            markClean();
            setAuthoritativePayload(parseImportedBackup(saved.payload));
          } catch (reloadError) {
            console.error('Son başarılı tarayıcı snapshotı geri yüklenemedi.', reloadError);
            setIsDataLoaded(false);
          }
        }
      });
    saveChain.current = persist.then(() => undefined, () => undefined);
  }, [authoritativePayload, isDataLoaded, isNative, markClean, setAuthoritativePayload, setIsDataLoaded, setLoadError]);

  return {
    loadSnapshot,
    markClean,
    markDirty,
    savePayload,
  };
}
