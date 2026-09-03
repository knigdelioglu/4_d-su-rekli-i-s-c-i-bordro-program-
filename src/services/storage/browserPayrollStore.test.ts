import { describe, expect, test } from 'bun:test';
import { browserPayrollStore } from './browserPayrollStore';

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
});
