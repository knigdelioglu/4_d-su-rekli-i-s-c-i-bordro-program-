import { BACKUP_FORMAT_VERSION } from '../../types/payroll';
import {
  serializePayrollStorage,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';
import {
  isSupportedLegacyBackupPayload,
  canonicalizeV5BackupRecord,
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

export type BrowserSnapshotMutation = (
  current: PayrollStorageDto
) => PayrollStorageDto;

/** A delayed load must never move a tab behind a snapshot it has already seen. */
export function meetsSnapshotRevisionFloor(
  snapshotRevision: number | null,
  minimumRevision: number
): boolean {
  return (snapshotRevision ?? 0) >= minimumRevision;
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

function stableJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableJsonValue);
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, stableJsonValue((value as Record<string, unknown>)[key])])
    );
  }
  return value;
}

function assertPolicyHistoryPreserved(
  existing: PayrollStorageDto,
  incoming: PayrollStorageDto
): void {
  const incomingById = new Map(
    incoming.compensationPolicyVersions.map((version) => [version.id, version])
  );
  for (const version of existing.compensationPolicyVersions) {
    const restored = incomingById.get(version.id);
    if (!restored) {
      throw new Error(
        `Immutable policy restore çakışması: mevcut sürüm ${version.id} silinemez.`
      );
    }
    if (JSON.stringify(stableJsonValue(version)) !== JSON.stringify(stableJsonValue(restored))) {
      throw new Error(
        `Immutable policy restore çakışması: ${version.id} farklı full-state, boundary flag veya supersession içeriyor.`
      );
    }
  }
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
  sourceFormat: BrowserSnapshotSourceFormat,
  allowPolicyHistoryReplacement = false
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

        if (current && !allowPolicyHistoryReplacement) {
          const existing = JSON.parse(parseStoredSnapshotPayload(current).payload) as PayrollStorageDto;
          const incoming = parseCurrentBrowserSnapshot(
            payload,
            sourceFormat === 'current' ? undefined : { allowLegacyMissingGvBase: true }
          );
          assertPolicyHistoryPreserved(existing, incoming);
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
          writeRequest.onsuccess = () => {
            const verifyRequest = store.get(CURRENT_SNAPSHOT_KEY);
            verifyRequest.onerror = () =>
              fail(verifyRequest.error ?? new Error('IndexedDB geri yükleme doğrulaması okunamadı.'));
            verifyRequest.onsuccess = () => {
              try {
                const verified = decodeStoredSnapshot(verifyRequest.result);
                if (
                  !verified ||
                  verified.payload !== payload ||
                  verified.revision !== nextRevision ||
                  verified.sourceFormat !== sourceFormat
                ) {
                  throw new Error('IndexedDB transaction içi snapshot doğrulaması başarısız.');
                }
              } catch (error) {
                transaction.abort();
                fail(error);
              }
            };
          };
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
  sourceFormat: BrowserSnapshotSourceFormat,
  allowPolicyHistoryReplacement = false
): Promise<number> {
  const revision = await writeSnapshotToDatabase(
    database,
    payload,
    expectedRevision,
    sourceFormat,
    allowPolicyHistoryReplacement
  );
  if (allowPolicyHistoryReplacement) return revision;
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
  if (raw.backupVersion === 5) {
    const canonical = canonicalizeV5BackupRecord(
      raw as Record<string, unknown>,
      stored.sourceFormat === 'current' ? {} : { allowLegacyMissingGvBase: true }
    );
    return {
      payload: serializePayrollStorage(canonical),
      // V5 already promises replayable legacy payroll and finalized retro authority.
      sourceFormat: 'current',
    };
  }
  if (stored.sourceFormat === 'current') {
    return {
      payload: serializePayrollStorage(parseCurrentBrowserSnapshot(stored.payload)),
      sourceFormat: 'current',
    };
  }
  if (raw.backupVersion === BACKUP_FORMAT_VERSION) {
    return {
      payload: serializePayrollStorage(parseCurrentBrowserSnapshot(stored.payload)),
      sourceFormat: 'current',
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
        // V5/V6 snapshots may still contain person-level legacy opening
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
    // The same current V6 schema used on load protects every normal browser
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

  /**
   * Replaces the complete validated backup snapshot. This is reserved for
   * explicit recovery/import flows; ordinary saves and domain mutations keep
   * immutable policy-history preservation enabled.
   */
  async replacePayload(
    payload: string,
    expectedRevision?: number,
    sourceFormat: BrowserSnapshotSourceFormat = 'current'
  ): Promise<number> {
    if (!hasIndexedDb()) {
      throw new Error(
        'Tarayıcı bordro verisi geri yüklenemedi: IndexedDB desteği bulunamadı. Payroll persistence devre dışı bırakıldı.'
      );
    }
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
          sourceFormat,
          true
        );
        this.knownRevision = revision;
        this.channel?.postMessage({ payload, revision, sourceFormat });
        return revision;
      } finally {
        database.close();
      }
    });
  }

  /**
   * Re-reads the authoritative snapshot, applies one synchronous domain
   * mutation, then commits with the same IndexedDB revision CAS used by full
   * snapshot writes. A competing tab causes a conflict; the updater is never
   * replayed against an unreviewed revision.
   */
  async mutateSnapshot(mutate: BrowserSnapshotMutation): Promise<BrowserPayrollSnapshot> {
    if (!hasIndexedDb()) {
      throw new Error(
        'Tarayıcı bordro verisi kaydedilemedi: IndexedDB desteği bulunamadı. Payroll persistence devre dışı bırakıldı.'
      );
    }

    return this.writeQueue.enqueue(async () => {
      const database = await openDatabase();
      try {
        const stored = await readSnapshotFromDatabase(database);
        if (!stored) {
          throw new Error('Yetkili tarayıcı bordro snapshotı bulunamadı.');
        }
        const canonical = parseStoredSnapshotPayload(stored);
        const current = parseCurrentBrowserSnapshot(canonical.payload);
        const next = mutate(current);
        const payload = serializePayrollStorage(next);
        parseCurrentBrowserSnapshot(payload);
        const revision = await writeAndVerify(
          database,
          payload,
          stored.revision,
          'current'
        );
        const snapshot: BrowserPayrollSnapshot = {
          payload,
          revision,
          sourceFormat: 'current',
        };
        this.knownRevision = revision;
        this.channel?.postMessage(snapshot);
        return snapshot;
      } finally {
        database.close();
      }
    });
  }
}

export const browserPayrollStore = new BrowserPayrollStore();
