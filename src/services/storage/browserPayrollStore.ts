import { serializePayrollStorage } from '../payrollEngine/decimalBoundary';
import {
  isSupportedLegacyBackupPayload,
  parseCurrentBrowserSnapshot,
  parseLegacyBackup,
} from './payrollPayload';

export { isSupportedLegacyBackupPayload as isMigratableBackupPayload } from './payrollPayload';

const STORAGE_KEY = '4d_bordro_programi_mvp_v2';
const DATABASE_NAME = '4d-bordro-programi';
const DATABASE_VERSION = 1;
const OBJECT_STORE = 'snapshots';
const CURRENT_SNAPSHOT_KEY = 'current';

export interface BrowserPayrollSnapshot {
  payload: string;
  revision: number;
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
  const record = value as { payload?: unknown; revision?: unknown };
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
  return { payload: record.payload, revision: record.revision };
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
  expectedRevision: number
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

        nextRevision = current?.payload === payload
          ? currentRevision
          : currentRevision + 1;
        if (nextRevision !== currentRevision) {
          const writeRequest = store.put(
            { payload, revision: nextRevision },
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
  expectedRevision: number
): Promise<number> {
  const revision = await writeSnapshotToDatabase(database, payload, expectedRevision);
  const readBack = await readSnapshotFromDatabase(database);
  if (!readBack || readBack.payload !== payload || readBack.revision < revision) {
    if (readBack && readBack.revision !== revision) {
      throw new BrowserSnapshotConflictError(revision, readBack.revision);
    }
    throw new Error('IndexedDB snapshot doğrulaması başarısız; yazılan veri geri okunamadı.');
  }
  return readBack.revision;
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
      // A present but malformed/empty snapshot is still authoritative. Let
      // App's version/shape validation surface it instead of silently
      // replacing it with a legacy localStorage copy.
      if (stored !== null) {
        this.knownRevision = stored.revision;
        return stored;
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
        const revision = await writeAndVerify(database, canonicalLegacy, 0);
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

  async savePayload(payload: string, expectedRevision?: number): Promise<number> {
    if (!hasIndexedDb()) {
      throw new Error(
        'Tarayıcı bordro verisi kaydedilemedi: IndexedDB desteği bulunamadı. Payroll persistence devre dışı bırakıldı.'
      );
    }

    // Do not let a caller turn an unvalidated string into authoritative state.
    // The same current V5 schema used on load protects every normal browser
    // write; legacy conversion writes only its already-validated canonical form.
    parseCurrentBrowserSnapshot(payload);

    return this.writeQueue.enqueue(async () => {
      const database = await openDatabase();
      try {
        const revision = await writeAndVerify(
          database,
          payload,
          expectedRevision ?? this.knownRevision ?? 0
        );
        this.knownRevision = revision;
        this.channel?.postMessage({ payload, revision });
        return revision;
      } finally {
        database.close();
      }
    });
  }
}

export const browserPayrollStore = new BrowserPayrollStore();
