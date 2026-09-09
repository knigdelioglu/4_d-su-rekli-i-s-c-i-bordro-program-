import { BACKUP_FORMAT_VERSION } from '../../types/payroll';
import { serializePayrollStorage } from '../payrollEngine/decimalBoundary';
import {
  isSupportedLegacyBackupPayload,
  parseCurrentBrowserSnapshot,
  parseImportedBackup,
  parseLegacyBackup,
} from './payrollPayload';

export { isSupportedLegacyBackupPayload as isMigratableBackupPayload } from './payrollPayload';

export type BrowserSnapshotSourceFormat = 'current' | 'legacy' | 'unknown';

const STORAGE_KEY = '4d_bordro_programi_mvp_v2';
const DATABASE_NAME = '4d-bordro-programi';
const DATABASE_VERSION = 1;
const OBJECT_STORE = 'snapshots';
const CURRENT_SNAPSHOT_KEY = 'current';

export interface BrowserPayrollSnapshot {
  payload: string;
  revision: number;
  /** `unknown` keeps pre-marker envelopes on the compatibility path. */
  sourceFormat?: BrowserSnapshotSourceFormat;
}

export interface BrowserRemoteSnapshotDecisionInput {
  currentRevision: number;
  remoteRevision: number;
  localDirty: boolean;
  pendingWrite: boolean;
}

/**
 * A remote snapshot is safe to adopt only when this tab has no local work in
 * flight. The revision comparison remains explicit so stale broadcasts never
 * move the local CAS baseline backwards.
 */
export function shouldAdoptRemoteSnapshot({
  currentRevision,
  remoteRevision,
  localDirty,
  pendingWrite,
}: BrowserRemoteSnapshotDecisionInput): boolean {
  return remoteRevision > currentRevision && !localDirty && !pendingWrite;
}

export class BrowserSnapshotConflictError extends Error {
  readonly expectedRevision: number;
  readonly actualRevision: number;

  constructor(expectedRevision: number, actualRevision: number) {
    super(
      `Tarayıcı snapshotı başka bir sekmede güncellendi (beklenen revizyon ${expectedRevision}, mevcut revizyon ${actualRevision}).`
    );
    this.name = 'BrowserSnapshotConflictError';
    this.expectedRevision = expectedRevision;
    this.actualRevision = actualRevision;
  }
}

function hasIndexedDb(): boolean {
  return typeof window !== 'undefined' && typeof window.indexedDB !== 'undefined';
}

function readLegacyLocalStorage(): string | null {
  if (typeof localStorage === 'undefined') return null;
  return localStorage.getItem(STORAGE_KEY);
}

/** Validates old JSON and returns its canonical exact-Decimal representation. */
export function canonicalizeLegacyBackupPayload(payload: string): string {
  return serializePayrollStorage(parseLegacyBackup(payload));
}

/** Serializes every IndexedDB write so an older async save cannot finish last. */
export class SerializedWriteQueue {
  private tail: Promise<unknown> = Promise.resolve();

  enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.tail.catch(() => undefined).then(operation);
    this.tail = next.catch(() => undefined);
    return next;
  }
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    if (!hasIndexedDb()) {
      reject(new Error('Tarayıcı IndexedDB desteği bulunamadı.'));
      return;
    }
    const request = window.indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB açılamadı.'));
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(OBJECT_STORE)) {
        database.createObjectStore(OBJECT_STORE);
      }
    };
    request.onsuccess = () => resolve(request.result);
  });
}

function decodeStoredSnapshot(value: unknown): BrowserPayrollSnapshot | null {
  if (value === undefined) return null;
  // V1 stored the raw payload string. Treat it as revision zero so existing
  // browser data gets an explicit CAS revision on its next successful write.
  if (typeof value === 'string') return { payload: value, revision: 0 };
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(
      'IndexedDB mevcut snapshotı geçersiz; kayıt payload/revision zarfı olmalıdır ve snapshot değiştirilmedi.'
    );
  }
  const record = value as {
    payload?: unknown;
    revision?: unknown;
    sourceFormat?: unknown;
  };
  if (
    typeof record.payload !== 'string' ||
    typeof record.revision !== 'number' ||
    !Number.isInteger(record.revision) ||
    record.revision < 0
  ) {
    throw new Error(
      'IndexedDB mevcut snapshotı geçersiz; payload string ve revision negatif olmayan tam sayı olmalıdır.'
    );
  }
  if (
    record.sourceFormat !== undefined
    && record.sourceFormat !== 'current'
    && record.sourceFormat !== 'legacy'
    && record.sourceFormat !== 'unknown'
  ) {
    throw new Error(
      'IndexedDB mevcut snapshotı geçersiz; sourceFormat current/legacy/unknown olmalıdır.'
    );
  }
  return {
    payload: record.payload,
    revision: record.revision,
    sourceFormat: (record.sourceFormat as BrowserSnapshotSourceFormat | undefined) ?? 'unknown',
  };
}

function readSnapshotFromDatabase(database: IDBDatabase): Promise<BrowserPayrollSnapshot | null> {
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(OBJECT_STORE, 'readonly');
    const request = transaction.objectStore(OBJECT_STORE).get(CURRENT_SNAPSHOT_KEY);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB kaydı okunamadı.'));
    request.onsuccess = () => {
      try {
        resolve(decodeStoredSnapshot(request.result));
      } catch (error) {
        reject(error);
      }
    };
  });
}

function writeSnapshotToDatabase(
  database: IDBDatabase,
  payload: string,
  expectedRevision: number,
  sourceFormat: BrowserSnapshotSourceFormat
): Promise<number> {
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(OBJECT_STORE, 'readwrite');
    let nextRevision: number | null = null;
    let settled = false;
    const fail = (error: unknown) => {
      if (settled) return;
      settled = true;
      reject(error);
    };

    transaction.onerror = () =>
      fail(transaction.error ?? new Error('IndexedDB kaydı yazılamadı.'));
    transaction.onabort = () =>
      fail(transaction.error ?? new Error('IndexedDB kaydı yazılamadı.'));
    transaction.oncomplete = () => {
      if (nextRevision === null) {
        fail(new Error('IndexedDB snapshot revizyonu çözülemedi.'));
        return;
      }
      settled = true;
      resolve(nextRevision);
    };

    const store = transaction.objectStore(OBJECT_STORE);
    const readRequest = store.get(CURRENT_SNAPSHOT_KEY);
    readRequest.onerror = () => fail(readRequest.error ?? new Error('IndexedDB kaydı okunamadı.'));
    readRequest.onsuccess = () => {
      try {
        const current = decodeStoredSnapshot(readRequest.result);
        const currentRevision = current?.revision ?? 0;
        if (currentRevision !== expectedRevision) {
          const conflict = new BrowserSnapshotConflictError(expectedRevision, currentRevision);
          transaction.abort();
          fail(conflict);
          return;
        }

        const sameSnapshot = current?.payload === payload
          && current?.sourceFormat === sourceFormat;
        nextRevision = sameSnapshot
          ? currentRevision
          : currentRevision + 1;
        if (nextRevision !== currentRevision || !sameSnapshot) {
          const writeRequest = store.put(
            { payload, revision: nextRevision, sourceFormat },
            CURRENT_SNAPSHOT_KEY
          );
          writeRequest.onerror = () =>
            fail(writeRequest.error ?? new Error('IndexedDB kaydı yazılamadı.'));
        }
      } catch (error) {
        transaction.abort();
        fail(error);
      }
    };
  });
}

async function writeAndVerify(
  database: IDBDatabase,
  payload: string,
  expectedRevision: number,
  sourceFormat: BrowserSnapshotSourceFormat
): Promise<number> {
  const revision = await writeSnapshotToDatabase(
    database,
    payload,
    expectedRevision,
    sourceFormat
  );
  const readBack = await readSnapshotFromDatabase(database);
  if (
    !readBack
    || readBack.payload !== payload
    || readBack.revision < revision
    || readBack.sourceFormat !== sourceFormat
  ) {
    if (readBack && readBack.revision !== revision) {
      throw new BrowserSnapshotConflictError(revision, readBack.revision);
    }
    throw new Error('IndexedDB snapshot doğrulaması başarısız; yazılan veri geri okunamadı.');
  }
  return readBack.revision;
}

function parseStoredSnapshotPayload(
  stored: BrowserPayrollSnapshot
): { payload: string; sourceFormat: BrowserSnapshotSourceFormat } {
  const raw = JSON.parse(stored.payload) as { backupVersion?: unknown };
  if (stored.sourceFormat === 'current') {
    return {
      payload: serializePayrollStorage(parseCurrentBrowserSnapshot(stored.payload)),
      sourceFormat: 'current',
    };
  }
  if (raw.backupVersion === BACKUP_FORMAT_VERSION) {
    // V5 envelopes written before the source marker may still be either a
    // complete current snapshot or an older sparse shape. Promote only the
    // former after the strict parser succeeds; keep the latter on the
    // explicit legacy compatibility path.
    try {
      return {
        payload: serializePayrollStorage(parseCurrentBrowserSnapshot(stored.payload)),
        sourceFormat: 'current',
      };
    } catch {
      // The compatibility parser below is intentionally limited to the
      // missing persisted-GV authority that predates this marker. Any other
      // malformed/current financial data still fails there.
    }
    return {
      payload: serializePayrollStorage(parseCurrentBrowserSnapshot(stored.payload, {
        allowLegacyMissingGvBase: true,
      })),
      sourceFormat: 'legacy',
    };
  }
  return {
    payload: serializePayrollStorage(parseImportedBackup(stored.payload)),
    sourceFormat: 'legacy',
  };
}

/** Browser-only persistence. IndexedDB is the only authoritative payroll store. */
export class BrowserPayrollStore {
  private readonly writeQueue = new SerializedWriteQueue();
  private readonly listeners = new Set<(snapshot: BrowserPayrollSnapshot) => void>();
  private readonly channel: BroadcastChannel | null;
  private knownRevision: number | null = null;

  constructor() {
    this.channel = typeof BroadcastChannel === 'undefined'
      ? null
      : new BroadcastChannel('4d-bordro-programi-snapshot');
    this.channel?.addEventListener('message', (event: MessageEvent<unknown>) => {
      try {
        const snapshot = decodeStoredSnapshot(event.data);
        if (!snapshot || snapshot.revision <= (this.knownRevision ?? -1)) return;
        // Keep the local CAS baseline unchanged until the caller explicitly
        // adopts the remote snapshot. A stale tab must fail its next write
        // against the revision it originally loaded, never overwrite the
        // remote update merely because the broadcast arrived first.
        this.listeners.forEach((listener) => listener(snapshot));
      } catch {
        // Other app versions may share the channel. Ignore messages that do
        // not match this store's snapshot envelope.
      }
    });
  }

  subscribe(listener: (snapshot: BrowserPayrollSnapshot) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  adoptSnapshot(snapshot: BrowserPayrollSnapshot): void {
    this.knownRevision = snapshot.revision;
  }

  async loadSnapshot(): Promise<BrowserPayrollSnapshot | null> {
    const database = await openDatabase();
    try {
      const stored = await readSnapshotFromDatabase(database);
      // A present but malformed/empty snapshot is still authoritative. The
      // current parser validates it and throws instead of silently replacing
      // it with a legacy localStorage copy.
      if (stored !== null) {
        // Current V5 snapshots may still contain person-level legacy opening
        // fields from before taxOpenings became authoritative. The parser
        // repairs only an exact, unambiguous period match; persist that repair
        // under the same CAS revision before exposing the snapshot to App.
        const parsedStored = parseStoredSnapshotPayload(stored);
        const repairedPayload = parsedStored.payload;
        if (repairedPayload !== stored.payload) {
          try {
            const revision = await writeAndVerify(
              database,
              repairedPayload,
              stored.revision,
              parsedStored.sourceFormat
            );
            this.knownRevision = revision;
            return {
              payload: repairedPayload,
              revision,
              sourceFormat: parsedStored.sourceFormat,
            };
          } catch (error) {
            if (!(error instanceof BrowserSnapshotConflictError)) throw error;
            const concurrent = await readSnapshotFromDatabase(database);
            if (!concurrent) throw error;
            this.knownRevision = concurrent.revision;
            return concurrent;
          }
        }
        this.knownRevision = stored.revision;
        return {
          ...stored,
          sourceFormat: parsedStored.sourceFormat,
        };
      }

      // Migrate only after the versioned payload has been read successfully.
      // The legacy key is intentionally retained as a recovery copy.
      const legacy = readLegacyLocalStorage();
      if (!legacy) {
        this.knownRevision = 0;
        return null;
      }
      if (!isSupportedLegacyBackupPayload(legacy)) {
        throw new Error(
          'Eski localStorage yedeği geçersiz veya desteklenmiyor; IndexedDB snapshotı değiştirilmedi.'
        );
      }
      // Validate and canonicalize Decimal fields before the first write. A
      // structurally valid but malformed Decimal must not become the new
      // authoritative IndexedDB snapshot.
      const canonicalLegacy = canonicalizeLegacyBackupPayload(legacy);
      try {
        const revision = await writeAndVerify(database, canonicalLegacy, 0, 'legacy');
        this.knownRevision = revision;
      } catch (error) {
        if (!(error instanceof BrowserSnapshotConflictError)) throw error;
        const concurrent = await readSnapshotFromDatabase(database);
        if (!concurrent) throw error;
        this.knownRevision = concurrent.revision;
        return concurrent;
      }
      return await readSnapshotFromDatabase(database);
    } finally {
      database.close();
    }
  }

  async loadPayload(): Promise<string | null> {
    return (await this.loadSnapshot())?.payload ?? null;
  }

  async savePayload(
    payload: string,
    expectedRevision?: number,
    sourceFormat: BrowserSnapshotSourceFormat = 'current'
  ): Promise<number> {
    if (!hasIndexedDb()) {
      throw new Error(
        'Tarayıcı bordro verisi kaydedilemedi: IndexedDB desteği bulunamadı. Payroll persistence devre dışı bırakıldı.'
      );
    }

    // Do not let a caller turn an unvalidated string into authoritative state.
    // The same current V5 schema used on load protects every normal browser
    // write; legacy conversion writes only its already-validated canonical form.
    parseCurrentBrowserSnapshot(
      payload,
      sourceFormat === 'current' ? undefined : { allowLegacyMissingGvBase: true }
    );

    return this.writeQueue.enqueue(async () => {
      const database = await openDatabase();
      try {
        const revision = await writeAndVerify(
          database,
          payload,
          expectedRevision ?? this.knownRevision ?? 0,
          sourceFormat
        );
        this.knownRevision = revision;
        this.channel?.postMessage({ payload, revision, sourceFormat });
        return revision;
      } finally {
        database.close();
      }
    });
  }
}

export const browserPayrollStore = new BrowserPayrollStore();
