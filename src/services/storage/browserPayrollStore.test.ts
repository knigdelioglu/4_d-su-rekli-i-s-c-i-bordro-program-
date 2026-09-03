import { describe, expect, test } from 'bun:test';
import {
  browserPayrollStore,
  canonicalizeLegacyBackupPayload,
  isMigratableBackupPayload,
  SerializedWriteQueue,
} from './browserPayrollStore';
import {
  isSupportedLegacyBackupPayload,
  parseCurrentBrowserSnapshot,
  parseImportedBackup,
  parseLegacyBackup,
} from './payrollPayload';

function makeV2Snapshot(netOdeme: unknown): string {
  return JSON.stringify({
    backupVersion: 2,
    donemler: [],
    personeller: [],
    puantajlar: [],
    bordrolar: [{ netOdeme }],
    taxOpenings: [],
    sickLeaveRecords: [],
    annualPayrollParameters: [],
  });
}

describe('BrowserPayrollStore', () => {
  test('does not fall back to localStorage when IndexedDB is unavailable', async () => {
    const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
    const originalLocalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    let localStorageReads = 0;
    let localStorageWrites = 0;

    Object.defineProperty(globalThis, 'window', {
      configurable: true,
      value: { indexedDB: undefined },
    });
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: {
        getItem: () => {
          localStorageReads += 1;
          return '{"backupVersion":2}';
        },
        setItem: () => {
          localStorageWrites += 1;
        },
      },
    });

    try {
      const loadError = await browserPayrollStore.loadPayload().catch((error) => error);
      const saveError = await browserPayrollStore.savePayload('{"backupVersion":2}').catch(
        (error) => error
      );
      expect(String(loadError).includes('IndexedDB')).toBe(true);
      expect(String(saveError).includes('IndexedDB')).toBe(true);
      expect(localStorageReads).toBe(0);
      expect(localStorageWrites).toBe(0);
    } finally {
      if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow);
      else delete (globalThis as { window?: unknown }).window;
      if (originalLocalStorage) {
        Object.defineProperty(globalThis, 'localStorage', originalLocalStorage);
      } else {
        delete (globalThis as { localStorage?: unknown }).localStorage;
      }
    }
  });

  test('rejects malformed or unsupported legacy payloads before migration', () => {
    expect(isMigratableBackupPayload('{not-json')).toBe(false);
    expect(isMigratableBackupPayload(JSON.stringify({ backupVersion: 3 }))).toBe(false);
    expect(isMigratableBackupPayload(JSON.stringify({ backupVersion: '2' }))).toBe(false);
    expect(isMigratableBackupPayload(JSON.stringify({ backupVersion: 1.5 }))).toBe(false);
    expect(
      isMigratableBackupPayload(JSON.stringify({ backupVersion: 2, donemler: [], personeller: [] }))
    ).toBe(false);
    expect(
      isMigratableBackupPayload(
        JSON.stringify({ backupVersion: 2, donemler: [], personeller: [], puantajlar: [], bordrolar: [], taxOpenings: [], sickLeaveRecords: [], annualPayrollParameters: [] })
      )
    ).toBe(true);
  });

  test('canonicalizes valid legacy JSON and rejects malformed Decimal before any write', () => {
    const canonical = canonicalizeLegacyBackupPayload(
      JSON.stringify({
        backupVersion: 1,
        donemler: [],
        personeller: [],
        bordrolar: [{ netOdeme: 0.15 }],
      })
    );
    expect(JSON.parse(canonical).bordrolar[0].netOdeme).toBe('0.15');

    expect(() =>
      canonicalizeLegacyBackupPayload(
        JSON.stringify({
          backupVersion: 1,
          donemler: [],
          personeller: [],
          bordrolar: [{ netOdeme: 'not-a-decimal' }],
        })
      )
    ).toThrow('Geçersiz Decimal metni');
  });

  test('accepts exact Decimal strings in current IndexedDB snapshots', () => {
    const parsed = parseCurrentBrowserSnapshot(makeV2Snapshot('64179.78'));

    expect(parsed.backupVersion).toBe(2);
    expect(parsed.bordrolar[0].netOdeme).toBe('64179.78');
  });

  test('rejects numeric and malformed Decimal values in current IndexedDB snapshots', () => {
    const invalidValues = [
      ['numeric Decimal', 64179.78, 'JS number kabul edilmez'],
      ['locale-formatted Decimal', '64.179,78', 'exact plain formatta'],
      ['non-Decimal text', 'not-a-decimal', 'exact plain formatta'],
    ] as const;

    for (const [_label, value, message] of invalidValues) {
      expect(() => parseCurrentBrowserSnapshot(makeV2Snapshot(value))).toThrow(message);
    }
  });

  test('keeps legacy migration and import compatibility on explicit paths', () => {
    const legacyV1 = JSON.stringify({
      backupVersion: 1,
      donemler: [],
      personeller: [],
      bordrolar: [{ netOdeme: 64179.78 }],
    });

    expect(isSupportedLegacyBackupPayload(legacyV1)).toBe(true);
    expect(parseLegacyBackup(legacyV1).bordrolar[0].netOdeme).toBe('64179.78');
    expect(parseImportedBackup(legacyV1).bordrolar[0].netOdeme).toBe('64179.78');

    const legacyV2 = makeV2Snapshot(64179.78);
    expect(parseImportedBackup(legacyV2).bordrolar[0].netOdeme).toBe('64179.78');
    expect(isSupportedLegacyBackupPayload(makeV2Snapshot(64179.78))).toBe(true);

    const malformedCurrentShape = JSON.stringify({
      backupVersion: 2,
      donemler: [],
      personeller: [],
      bordrolar: [{ netOdeme: 64179.78 }],
    });
    expect(isSupportedLegacyBackupPayload(malformedCurrentShape)).toBe(false);
    expect(() => parseImportedBackup(malformedCurrentShape)).toThrow('JS number kabul edilmez');
  });

  test('serializes concurrent saves and lets the latest completed operation win', async () => {
    const queue = new SerializedWriteQueue();
    let persisted = '';
    let releaseFirst!: () => void;
    let markFirstStarted!: () => void;
    const firstStarted = new Promise<void>((resolve) => {
      markFirstStarted = resolve;
    });

    const first = queue.enqueue(
      () =>
        new Promise<void>((resolve) => {
          markFirstStarted();
          releaseFirst = () => {
            persisted = 'A';
            resolve();
          };
        })
    );
    const second = queue.enqueue(async () => {
      persisted = 'B';
    });

    expect(persisted).toBe('');
    await firstStarted;
    releaseFirst();
    await Promise.all([first, second]);
    expect(persisted).toBe('B');
  });

  test('continues serving later saves after one write fails', async () => {
    const queue = new SerializedWriteQueue();
    const error = await queue
      .enqueue(async () => Promise.reject(new Error('write failed')))
      .catch((caught) => caught);
    expect(String(error).includes('write failed')).toBe(true);

    let persisted = false;
    await queue.enqueue(async () => {
      persisted = true;
    });
    expect(persisted).toBe(true);
  });
});
