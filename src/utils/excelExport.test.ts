import { describe, expect, test } from 'bun:test';
import * as XLSX from 'xlsx';
import { exportToExcel, printElement } from './excelExport';

describe('printElement', () => {
  test('uses compact A4 rules while leaving long payslip sections free to paginate', async () => {
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
    let clonedElementDuringPrint: any;
    const element = {
      id: 'payslip-print-surface',
      cloneNode: () => ({
        id: 'payslip-print-surface-clone',
        children: [
          { className: 'no-print', text: 'Yazdır / Kapat' },
          { id: 'payslip-print-container', text: 'Puantaj · Net Ödeme · Alt bilgi' },
        ],
      }),
      classList: {
        contains: (value: string) => elementClasses.has(value),
        add: (value: string) => elementClasses.add(value),
        remove: (value: string) => elementClasses.delete(value),
      },
    };
    const fakeDocument = {
      getElementById: (id: string) => id === 'payslip-print-surface' ? element : null,
      createElement: (tag: string) => tag === 'style'
        ? { dataset: {}, textContent: '', remove: () => { appendedStyle = undefined; } }
        : {
          className: '',
          children: [] as unknown[],
          appendChild(child: unknown) { this.children.push(child); },
          remove: () => { appendedHost = undefined; },
        },
      head: { appendChild: (style: unknown) => { appendedStyle = style; } },
      body: {
        appendChild: (host: unknown) => { appendedHost = host; },
        classList: {
          contains: (value: string) => bodyClasses.has(value),
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
        clonedElementDuringPrint = appendedHost.children[0];
      },
    };

    (globalThis as any).document = fakeDocument;
    (globalThis as any).window = fakeWindow;
    try {
      await printElement('payslip-print-surface');
      expect(printCalls).toBe(1);
      expect(classesDuringPrint.includes('print-element-active')).toBe(true);
      expect(hostDuringPrint).toBe('print-element-host');
      expect(clonedElementDuringPrint).toMatchObject({
        id: 'payslip-print-surface-clone',
        children: [
          { className: 'no-print', text: 'Yazdır / Kapat' },
          { id: 'payslip-print-container', text: 'Puantaj · Net Ödeme · Alt bilgi' },
        ],
      });
      expect(styleDuringPrint.includes('@media print')).toBe(true);
      expect(styleDuringPrint).toContain('> *:not(.print-element-host) { display: none !important; }');
      expect(styleDuringPrint).toContain('position: static !important;');
      expect(styleDuringPrint).toContain('overflow: visible !important;');
      expect(styleDuringPrint).toContain('.print-element-host .no-print { display: none !important; }');
      expect(styleDuringPrint).toContain('.print-element-host #payslip-print-container');
      expect(styleDuringPrint).toContain('flex: none !important;');
      expect(styleDuringPrint).toContain('#payslip-print-container *');
      expect(styleDuringPrint).toContain('@page { size: auto; margin: 8mm; }');
      expect(styleDuringPrint).toContain('#payslip-print-container {\n        padding: 0 !important;\n        font-size: 9pt;');
      expect(styleDuringPrint).toContain('.payslip-print-audits {\n        margin-top: 2mm !important;');
      expect(styleDuringPrint).toContain('.payslip-print-sgk-tax-section {\n        break-inside: avoid-page;\n        page-break-inside: avoid;');
      expect(styleDuringPrint).toContain('.payslip-print-final-amount-sections {\n        display: block !important;');
      expect(styleDuringPrint).toContain('.payslip-print-net {\n        margin-top: 3mm !important;');
      expect(styleDuringPrint).toContain('.payslip-print-amount-sections {\n        margin-top: 2mm !important;');
      expect(styleDuringPrint).toContain('break-inside: avoid-page;');
      expect(styleDuringPrint).not.toContain('#payslip-print-container {\n        break-inside:');
      expect(styleDuringPrint).toContain('overflow: visible !important;');

      expect(bodyClasses.has('print-element-active')).toBe(false);
      expect(appendedHost).toBe(undefined);
      expect(appendedStyle).toBe(undefined);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });

  test('cleans up the native print clone when the print panel closes', async () => {
    const originalDoc = globalThis.document;
    const originalWindow = globalThis.window;
    const bodyClasses = new Set<string>();
    const elementClasses = new Set<string>();
    const listeners = new Map<string, EventListener>();
    let invokedCommand = '';
    let printCalls = 0;
    let appendedHost: any;
    let appendedStyle: any;
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
        ? { dataset: {}, textContent: '', remove() { appendedStyle = undefined; } }
        : { className: '', appendChild() {}, remove() { appendedHost = undefined; } },
      head: { appendChild(style: unknown) { appendedStyle = style; } },
      body: {
        appendChild(host: unknown) { appendedHost = host; },
        classList: {
          contains: (value: string) => bodyClasses.has(value),
          add: (value: string) => bodyClasses.add(value),
          remove: (value: string) => bodyClasses.delete(value),
        },
      },
    };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: { invoke: async (command: string) => { invokedCommand = command; return true; } },
      addEventListener: (name: string, listener: EventListener) => listeners.set(name, listener),
      removeEventListener: (name: string) => listeners.delete(name),
      print: () => { printCalls += 1; },
    };

    try {
      await printElement('print-area');
      expect(invokedCommand).toBe('print_current_webview');
      expect(printCalls).toBe(0);
      expect(bodyClasses.has('print-element-active')).toBe(true);
      expect(elementClasses.has('print-element-target')).toBe(false);
      expect(appendedHost).toBeDefined();
      expect(appendedStyle).toBeDefined();

      listeners.get('afterprint')?.(new Event('afterprint'));
      expect(bodyClasses.has('print-element-active')).toBe(false);
      expect(appendedHost).toBeUndefined();
      expect(appendedStyle).toBeUndefined();
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });

  test('cleans up after native print errors and allows a fresh print attempt', async () => {
    const originalDoc = globalThis.document;
    const originalWindow = globalThis.window;
    const bodyClasses = new Set<string>();
    const listeners = new Map<string, EventListener>();
    let hostCount = 0;
    let styleCount = 0;
    let failNextPrint = true;
    const element = { cloneNode: () => ({}) };
    (globalThis as any).document = {
      getElementById: () => element,
      createElement: (tag: string) => tag === 'style'
        ? { dataset: {}, textContent: '', remove() { styleCount -= 1; } }
        : { className: '', appendChild() {}, remove() { hostCount -= 1; } },
      head: { appendChild() { styleCount += 1; } },
      body: {
        appendChild() { hostCount += 1; },
        classList: {
          contains: (value: string) => bodyClasses.has(value),
          add: (value: string) => bodyClasses.add(value),
          remove: (value: string) => bodyClasses.delete(value),
        },
      },
    };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: {
        invoke: async () => {
          if (failNextPrint) throw new Error('native print failed');
          return true;
        },
      },
      addEventListener: (name: string, listener: EventListener) => listeners.set(name, listener),
      removeEventListener: (name: string) => listeners.delete(name),
      print() {},
    };

    try {
      await expect(printElement('print-area')).rejects.toThrow('native print failed');
      expect(bodyClasses.has('print-element-active')).toBe(false);
      expect(hostCount).toBe(0);
      expect(styleCount).toBe(0);
      expect(listeners.has('afterprint')).toBe(false);

      failNextPrint = false;
      await printElement('print-area');
      expect(hostCount).toBe(1);
      expect(styleCount).toBe(1);
      listeners.get('afterprint')?.(new Event('afterprint'));
      expect(hostCount).toBe(0);
      expect(styleCount).toBe(0);

      await printElement('print-area');
      expect(hostCount).toBe(1);
      expect(styleCount).toBe(1);
      listeners.get('afterprint')?.(new Event('afterprint'));
      expect(hostCount).toBe(0);
      expect(styleCount).toBe(0);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });
});

describe('exportToExcel', () => {
  test('keeps bank payments numeric and applies grouped two-decimal format in browser and Tauri XLSX files', async () => {
    const originalDoc = (globalThis as any).document;
    const originalWindow = (globalThis as any).window;
    const originalUrl = globalThis.URL;
    const originalCreateObjectURL = globalThis.URL.createObjectURL;
    const originalRevokeObjectURL = globalThis.URL.revokeObjectURL;
    let browserBlob: Blob | undefined;
    let tauriBytes: number[] | undefined;
    const columns = [
      { header: 'T.C. Kimlik No', key: 'tcNo', width: 18 },
      { header: 'IBAN', key: 'iban', width: 32 },
      { header: 'Net Ödeme (TL)', key: 'netOdeme', width: 18, numFmt: '#,##0.00' },
    ];
    const amounts = [
      6424.1, 12000, 25000, 30000, 35000,
      40000, 45000, 50000, 60000, 131476.91,
    ];
    const data = amounts.map((netOdeme, index) => ({
      tcNo: `1111111111${index}`,
      iban: `TR${String(index).padStart(2, '0')}000000000000000000000000`,
      netOdeme,
    }));
    const summaryRows = [{ tcNo: 'TOPLAM', iban: '', netOdeme: 434901.01 }];

    const assertWorkbook = (bytes: Uint8Array) => {
      const workbook = XLSX.read(bytes, { type: 'array', cellStyles: true });
      const sheet = workbook.Sheets['Banka Listesi'];
      expect(sheet).toBeDefined();
      expect(sheet.A2).toMatchObject({ t: 's', v: '11111111110' });
      expect(sheet.B2).toMatchObject({ t: 's', v: data[0].iban });
      expect(sheet.C2).toMatchObject({ t: 'n', v: 6424.1, z: '#,##0.00' });
      expect(sheet.C11).toMatchObject({ t: 'n', v: 131476.91, z: '#,##0.00' });
      expect(sheet.C12).toMatchObject({ t: 'n', v: 434901.01, z: '#,##0.00' });
    };

    (globalThis as any).document = {
      createElement: () => ({ href: '', download: '', click() {} }),
      body: { appendChild() {}, removeChild() {} },
    };
    globalThis.URL.createObjectURL = (blob: Blob) => {
      browserBlob = blob;
      return 'blob:bank-export';
    };
    globalThis.URL.revokeObjectURL = () => {};
    (globalThis as any).window = {};

    try {
      await exportToExcel('bank_export', 'Banka Listesi', columns, data, summaryRows);
      // Let this export's delayed object-URL cleanup run before restoring the URL mocks.
      await new Promise((resolve) => setTimeout(resolve, 1100));
      expect(browserBlob).toBeDefined();
      assertWorkbook(new Uint8Array(await browserBlob!.arrayBuffer()));

      (globalThis as any).window = {
        __TAURI_INTERNALS__: {
          invoke: async (_command: string, args: { excelBytes: number[] }) => {
            tauriBytes = args.excelBytes;
            return true;
          },
        },
      };
      await exportToExcel('bank_export', 'Banka Listesi', columns, data, summaryRows);
      expect(tauriBytes).toBeDefined();
      assertWorkbook(new Uint8Array(tauriBytes!));
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
      globalThis.URL.createObjectURL = originalCreateObjectURL;
      globalThis.URL.revokeObjectURL = originalRevokeObjectURL;
    }
  });

  test('asynchronously produces genuine xlsx blob and triggers download without blocking', async () => {
    let createdBlob: Blob | null = null;
    let clickedAnchor: { href: string; download: string } | null = null;
    let revokedUrls: string[] = [];
    const originalUrl = globalThis.URL;
    const originalDoc = (globalThis as any).document;
    const originalWindow = (globalThis as any).window;

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
    (globalThis as any).window = {};

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

      const saved = await exportToExcel('test_puantaj', 'Puantaj', columns, data);
      await new Promise((resolve) => setTimeout(resolve, 1100));

      expect(saved).toBe(true);
      expect(createdBlob !== null).toBe(true);
      expect((createdBlob?.size ?? 0) > 0).toBe(true);
      expect(clickedAnchor).toEqual({
        href: 'blob:mock-url',
        download: 'test_puantaj.xlsx',
      });
      expect(revokedUrls).toEqual(['blob:mock-url']);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
      globalThis.URL = originalUrl;
    }
  });

  test('routes Tauri exports through the native save command and returns cancellation without browser download', async () => {
    const originalDoc = (globalThis as any).document;
    const originalWindow = (globalThis as any).window;
    let invokedCommand: string | undefined;
    let invokedArgs: Record<string, unknown> | undefined;
    let capturedBytes: number[] | undefined;
    let capturedFileName: string | undefined;
    let anchorClicks = 0;

    (globalThis as any).document = {
      createElement: () => ({
        set href(_value: string) {},
        set download(_value: string) {},
        click: () => { anchorClicks += 1; },
      }),
      body: { appendChild() {}, removeChild() {} },
    };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, unknown>) => {
          invokedCommand = command;
          invokedArgs = args;
          capturedBytes = args.excelBytes as number[];
          capturedFileName = args.fileName as string;
          return false;
        },
      },
    };

    try {
      const saved = await exportToExcel(
        'bank_export',
        'Banka Listesi',
        [{ header: 'Net Ödeme', key: 'netOdeme' }],
        [{ netOdeme: '434901.01' }]
      );

      expect(saved).toBe(false);
      expect(invokedCommand).toBe('export_excel');
      expect(invokedArgs).toEqual({ excelBytes: capturedBytes, fileName: 'bank_export.xlsx' });
      expect(capturedFileName).toBe('bank_export.xlsx');
      expect(capturedBytes && capturedBytes.length > 0).toBe(true);
      expect(capturedBytes?.slice(0, 2)).toEqual([80, 75]);
      expect(anchorClicks).toBe(0);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });

  test('propagates native save failures instead of reporting a successful export', async () => {
    const originalDoc = (globalThis as any).document;
    const originalWindow = (globalThis as any).window;
    let anchorClicks = 0;

    (globalThis as any).document = {
      createElement: () => ({ click: () => { anchorClicks += 1; } }),
      body: { appendChild() {}, removeChild() {} },
    };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: {
        invoke: async () => { throw new Error('native save failed'); },
      },
    };

    try {
      await expect(exportToExcel(
        'bank_export',
        'Banka Listesi',
        [{ header: 'Net Ödeme', key: 'netOdeme' }],
        [{ netOdeme: '434901.01' }]
      )).rejects.toThrow('native save failed');
      expect(anchorClicks).toBe(0);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });
});
