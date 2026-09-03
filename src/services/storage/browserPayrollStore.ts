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

function isMigratableBackupPayload(payload: string): boolean {
  try {
    const parsed = JSON.parse(payload) as Record<string, unknown>;
    const version = typeof parsed.backupVersion === 'number' ? parsed.backupVersion : 1;
    if (version <= 0 || version > 2) return false;
    if (!Array.isArray(parsed.donemler) || !Array.isArray(parsed.personeller)) return false;
    return (
      version < 2 ||
      (Array.isArray(parsed.puantajlar) &&
        Array.isArray(parsed.bordrolar) &&
        Array.isArray(parsed.taxOpenings) &&
        Array.isArray(parsed.sickLeaveRecords) &&
        Array.isArray(parsed.annualPayrollParameters))
    );
  } catch {
    return false;
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
      resolve(typeof value === 'string' ? value : null);
    };
  });
}

function writeToDatabase(database: IDBDatabase, payload: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(OBJECT_STORE, 'readwrite');
    transaction.onerror = () =>
      reject(transaction.error ?? new Error('IndexedDB kaydı yazılamadı.'));
    transaction.oncomplete = () => resolve();
    transaction.objectStore(OBJECT_STORE).put(payload, CURRENT_SNAPSHOT_KEY);
  });
}

/** Browser-only persistence. IndexedDB is preferred; localStorage is a legacy/fallback boundary. */
export class BrowserPayrollStore {
  async loadPayload(): Promise<string | null> {
    if (!hasIndexedDb()) return readLegacyLocalStorage();

    const database = await openDatabase();
    try {
      const stored = await readFromDatabase(database);
      if (stored) return stored;

      // Migrate only after the versioned payload has been read successfully.
      // The legacy key is intentionally retained as a recovery copy.
      const legacy = readLegacyLocalStorage();
      if (legacy && isMigratableBackupPayload(legacy)) {
        await writeToDatabase(database, legacy);
      }
      return legacy;
    } finally {
      database.close();
    }
  }

  async savePayload(payload: string): Promise<void> {
    if (!hasIndexedDb()) {
      if (typeof localStorage === 'undefined') {
        throw new Error('Tarayıcı kalıcı depolama desteği bulunamadı.');
      }
      localStorage.setItem(STORAGE_KEY, payload);
      return;
    }

    const database = await openDatabase();
    try {
      await writeToDatabase(database, payload);
    } finally {
      database.close();
    }
  }
}

export const browserPayrollStore = new BrowserPayrollStore();
