import { describe, expect, test } from 'bun:test';
import { exportToExcel, printElement } from './excelExport';

describe('printElement', () => {
  test('prints the selected element in the current window and restores print state afterward', async () => {
    const originalDoc = globalThis.document;
    const originalWindow = globalThis.window;
    const bodyClasses = new Set<string>();
    const elementClasses = new Set<string>();
    const listeners = new Map<string, EventListener>();
    let appendedStyle: any;
    let printCalls = 0;
    let classesDuringPrint: string[] = [];
    let styleDuringPrint = '';
    let appendedHost: any;
    let hostDuringPrint = '';
    const element = {
      cloneNode: () => ({}),
      classList: {
        contains: (value: string) => elementClasses.has(value),
        add: (value: string) => elementClasses.add(value),
        remove: (value: string) => elementClasses.delete(value),
      },
    };
    const fakeDocument = {
      getElementById: (id: string) => id === 'print-area' ? element : null,
      createElement: (tag: string) => tag === 'style'
        ? { dataset: {}, textContent: '', remove: () => { appendedStyle = undefined; } }
        : { className: '', appendChild() {}, remove: () => { appendedHost = undefined; } },
      head: { appendChild: (style: unknown) => { appendedStyle = style; } },
      body: {
        appendChild: (host: unknown) => { appendedHost = host; },
        classList: {
          add: (value: string) => bodyClasses.add(value),
          remove: (value: string) => bodyClasses.delete(value),
        },
      },
    };
    const fakeWindow = {
      addEventListener: (name: string, listener: EventListener) => listeners.set(name, listener),
      removeEventListener: (name: string) => listeners.delete(name),
      print: () => {
        printCalls += 1;
        classesDuringPrint = [...bodyClasses];
        styleDuringPrint = appendedStyle.textContent;
        hostDuringPrint = appendedHost.className;
      },
    };

    (globalThis as any).document = fakeDocument;
    (globalThis as any).window = fakeWindow;
    try {
      await printElement('print-area');
      expect(printCalls).toBe(1);
      expect(classesDuringPrint.includes('print-element-active')).toBe(true);
      expect(hostDuringPrint).toBe('print-element-host');
      expect(styleDuringPrint.includes('@media print')).toBe(true);

      listeners.get('afterprint')?.(new Event('afterprint'));
      expect(bodyClasses.has('print-element-active')).toBe(false);
      expect(appendedHost).toBe(undefined);
      expect(appendedStyle).toBe(undefined);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });

  test('uses Tauri native printing when running in the app', async () => {
    const originalDoc = globalThis.document;
    const originalWindow = globalThis.window;
    const bodyClasses = new Set<string>();
    const elementClasses = new Set<string>();
    let invokedCommand = '';
    let printCalls = 0;
    const element = {
      cloneNode: () => ({}),
      classList: {
        contains: (value: string) => elementClasses.has(value),
        add: (value: string) => elementClasses.add(value),
        remove: (value: string) => elementClasses.delete(value),
      },
    };
    (globalThis as any).document = {
      getElementById: () => element,
      createElement: (tag: string) => tag === 'style'
        ? { dataset: {}, textContent: '', remove() {} }
        : { className: '', appendChild() {}, remove() {} },
      head: { appendChild() {} },
      body: {
        appendChild() {},
        classList: { add: (value: string) => bodyClasses.add(value), remove: (value: string) => bodyClasses.delete(value) },
      },
    };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: { invoke: async (command: string) => { invokedCommand = command; return true; } },
      addEventListener() {},
      removeEventListener() {},
      print: () => { printCalls += 1; },
    };

    try {
      await printElement('print-area');
      expect(invokedCommand).toBe('print_current_webview');
      expect(printCalls).toBe(0);
      expect(bodyClasses.has('print-element-active')).toBe(false);
      expect(elementClasses.has('print-element-target')).toBe(false);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });
});

describe('exportToExcel', () => {
  test('asynchronously produces genuine xlsx blob and triggers download without blocking', async () => {
    let createdBlob: Blob | null = null;
    let clickedAnchor: { href: string; download: string } | null = null;
    let revokedUrls: string[] = [];
    const originalUrl = globalThis.URL;
    const originalDoc = (globalThis as any).document;

    // Mock DOM environment if running in node/bun
    const fakeAnchor = {
      href: '',
      download: '',
      click() {
        clickedAnchor = { href: this.href, download: this.download };
      },
    };

    (globalThis as any).document = {
      createElement(tag: string) {
        if (tag === 'a') return fakeAnchor;
        return {};
      },
      body: {
        appendChild(_elem: unknown) {},
        removeChild(_elem: unknown) {},
      },
    };

    (globalThis as any).URL = {
      createObjectURL(blob: Blob) {
        createdBlob = blob;
        return 'blob:mock-url';
      },
      revokeObjectURL(url: string) {
        revokedUrls.push(url);
      },
    };

    try {
      const columns = [
        { header: 'T.C. Kimlik No', key: 'tcNo', width: 16 },
        { header: 'Ad Soyad', key: 'adSoyad', width: 22 },
        { header: 'Günlük Ücret', key: 'gunlukUcret', width: 14 },
      ];
      const data = [
        { tcNo: '11111111110', adSoyad: 'Ali Veli', gunlukUcret: '1500.00' },
        { tcNo: '22222222220', adSoyad: 'Ayşe Yılmaz', gunlukUcret: '1600.00' },
      ];

      await exportToExcel('test_puantaj', 'Puantaj', columns, data);
      await new Promise((resolve) => setTimeout(resolve, 1100));

      expect(createdBlob !== null).toBe(true);
      expect((createdBlob?.size ?? 0) > 0).toBe(true);
      expect(clickedAnchor).toEqual({
        href: 'blob:mock-url',
        download: 'test_puantaj.xlsx',
      });
      expect(revokedUrls).toEqual(['blob:mock-url']);
    } finally {
      (globalThis as any).document = originalDoc;
      globalThis.URL = originalUrl;
    }
  });
});
