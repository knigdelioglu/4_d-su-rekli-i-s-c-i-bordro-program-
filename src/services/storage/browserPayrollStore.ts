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
  private tail: Promise<void> = Promise.resolve();

  enqueue(operation: () => Promise<void>): Promise<void> {
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

function readFromDatabase(database: IDBDatabase): Promise<string | null> {
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(OBJECT_STORE, 'readonly');
    const request = transaction.objectStore(OBJECT_STORE).get(CURRENT_SNAPSHOT_KEY);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB kaydı okunamadı.'));
    request.onsuccess = () => {
      const value = request.result;
      if (value === undefined) {
        resolve(null);
        return;
      }
      if (typeof value !== 'string') {
        reject(
          new Error(
            'IndexedDB mevcut snapshotı geçersiz; kayıt JSON string olmalıdır ve snapshot değiştirilmedi.'
          )
        );
        return;
      }
      resolve(value);
    };
  });
}

function writeToDatabase(database: IDBDatabase, payload: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(OBJECT_STORE, 'readwrite');
    transaction.onerror = () =>
      reject(transaction.error ?? new Error('IndexedDB kaydı yazılamadı.'));
    transaction.oncomplete = () => resolve();
    const request = transaction.objectStore(OBJECT_STORE).put(payload, CURRENT_SNAPSHOT_KEY);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB kaydı yazılamadı.'));
  });
}

async function writeAndVerify(database: IDBDatabase, payload: string): Promise<void> {
  await writeToDatabase(database, payload);
  const readBack = await readFromDatabase(database);
  if (readBack !== payload) {
    throw new Error('IndexedDB snapshot doğrulaması başarısız; yazılan veri geri okunamadı.');
  }
}

/** Browser-only persistence. IndexedDB is the only authoritative payroll store. */
export class BrowserPayrollStore {
  private readonly writeQueue = new SerializedWriteQueue();

  async loadPayload(): Promise<string | null> {
    const database = await openDatabase();
    try {
      const stored = await readFromDatabase(database);
      // A present but malformed/empty snapshot is still authoritative. Let
      // App's version/shape validation surface it instead of silently
      // replacing it with a legacy localStorage copy.
      if (stored !== null) return stored;

      // Migrate only after the versioned payload has been read successfully.
      // The legacy key is intentionally retained as a recovery copy.
      const legacy = readLegacyLocalStorage();
      if (!legacy) return null;
      if (!isSupportedLegacyBackupPayload(legacy)) {
        throw new Error(
          'Eski localStorage yedeği geçersiz veya desteklenmiyor; IndexedDB snapshotı değiştirilmedi.'
        );
      }
      // Validate and canonicalize Decimal fields before the first write. A
      // structurally valid but malformed Decimal must not become the new
      // authoritative IndexedDB snapshot.
      const canonicalLegacy = canonicalizeLegacyBackupPayload(legacy);
      await writeAndVerify(database, canonicalLegacy);
      return await readFromDatabase(database);
    } finally {
      database.close();
    }
  }

  async savePayload(payload: string): Promise<void> {
    if (!hasIndexedDb()) {
      throw new Error(
        'Tarayıcı bordro verisi kaydedilemedi: IndexedDB desteği bulunamadı. Payroll persistence devre dışı bırakıldı.'
      );
    }

    // Do not let a caller turn an unvalidated string into authoritative state.
    // The same current V2 schema used on load protects every normal browser
    // write; legacy conversion writes only its already-validated canonical form.
    parseCurrentBrowserSnapshot(payload);

    return this.writeQueue.enqueue(async () => {
      const database = await openDatabase();
      try {
        await writeAndVerify(database, payload);
      } finally {
        database.close();
      }
    });
  }
}

export const browserPayrollStore = new BrowserPayrollStore();
