import { describe, expect, test } from 'bun:test';
import type { BordroDonemi } from '../types/payroll';
import type { PayrollExportModel } from './payrollExportModel';
import { payrollExportFileStem, periodExportFileStem } from './payrollExportModel';
import {
  canvasesToPdfBlob,
  exportPeriodPayrollPdf,
  exportSinglePayrollPdf,
  renderPayrollPdfCanvas,
} from './payrollPdfExport';

const period: BordroDonemi = {
  id: '2027-02',
  yil: 2027,
  ay: 2,
  baslangicTarihi: '2027-02-15',
  bitisTarihi: '2027-03-14',
  donemAdi: '15.02.2027 - 14.03.2027',
  taxYear: 2027,
  taxMonth: 3,
};

const model = {
  accrualType: 'NORMAL',
  paymentDate: '2027-03-14',
  sequence: 0,
  accrualDescription: 'Normal maaş',
  status: 'CALCULATED',
  periodName: period.donemAdi,
  periodStart: period.baslangicTarihi,
  periodEnd: period.bitisTarihi,
  taxYear: 2027,
  taxMonth: 3,
  employee: {
    tcNo: '98765432109',
    fullName: 'Ayşe Kaya',
    group: '1. Grup',
    title: 'İşçi',
    sgkRegistryNo: 'SGK-1',
    iban: 'TR000000000000000000000001',
    serviceYears: 8,
  },
  attendanceSummary: [],
  incomes: [],
  deductions: [],
  totals: { gross: 86348.1, deductions: 26313.79, net: 60034.31 },
  sgkTax: [],
  employer: [],
  notices: [],
  sourceUpdatedAt: '2027-03-14T00:00:00.000Z',
} as unknown as PayrollExportModel;

const jpeg = new Uint8Array([0xff, 0xd8, 0xff, 0xd9]);

function canvas() {
  return {
    width: 1240,
    height: 1754,
    toBlob(callback: BlobCallback) {
      callback(new Blob([jpeg], { type: 'image/jpeg' }));
    },
  } as HTMLCanvasElement;
}

function installDocument(onAnchorClick: () => void) {
  (globalThis as any).document = {
    createElement(tag: string) {
      if (tag === 'canvas') {
        const context = new Proxy({ measureText: (value: string) => ({ width: value.length }) }, {
          get(target, property) {
            return (target as any)[property] ?? (() => undefined);
          },
          set() { return true; },
        });
        return { getContext: () => context, ...canvas() };
      }
      return {
        style: {},
        click: onAnchorClick,
        remove() {},
      };
    },
    body: { appendChild() {} },
  };
}

describe('payroll PDF native and browser save paths', () => {
  test('PDF layout keeps personnel rows clear and wraps long descriptions, attendance labels, and notices', () => {
    const oldDocument = (globalThis as any).document;
    const draws: Array<{ value: string; x: number; y: number }> = [];
    const rectangles: Array<{ x: number; y: number; width: number; height: number }> = [];
    const layoutModel = {
      ...model,
      accrualDescription: 'Şubat 2027 Dönemi (15 Şubat - 14 Mart) dönem ücret pusulası açıklamasının tamamı okunabilir olmalıdır',
      attendanceSummary: Array.from({ length: 10 }, (_, index) => ({
        code: 'GÇT',
        label: 'Gece Çalışması Tatili',
        count: index + 1,
      })),
      sgkTax: [{ key: 'finalPek', label: 'Nihai / Bildirim PEK', amount: 80348.1 }],
      notices: [{
        code: 'LONG_NOTE',
        severity: 'WARNING',
        scope: 'PERSONNEL',
        title: 'Bordro kontrol notu',
        message: 'Kontrol notunun bu uzun açıklama metni PDF içinde satırlara sarılmalı ve sonuna kadar okunabilir kalmalıdır.',
        details: [],
      }],
    } as unknown as PayrollExportModel;

    (globalThis as any).document = {
      createElement(tag: string) {
        if (tag !== 'canvas') return {};
        const context = new Proxy({
          measureText: (value: string) => ({ width: value.length * 8 }),
          fillText: (value: string, x: number, y: number) => draws.push({ value, x, y }),
          fillRect: (x: number, y: number, width: number, height: number) => rectangles.push({ x, y, width, height }),
        }, {
          get(target, property) { return (target as any)[property] ?? (() => undefined); },
          set(target, property, value) { (target as any)[property] = value; return true; },
        });
        return { width: 1240, height: 1754, getContext: () => context };
      },
    };

    try {
      renderPayrollPdfCanvas(layoutModel);
      const renderedText = draws.map(({ value }) => value).join(' ');
      const titleBand = rectangles.find((rect) => rect.y === 136 && rect.height === 34)!;
      const firstIdentityRow = draws.find(({ value }) => value === model.employee.tcNo)!;
      expect(firstIdentityRow.y).toBeGreaterThan(titleBand.y + titleBand.height);
      expect(renderedText).toContain('Şubat 2027 Dönemi (15 Şubat - 14 Mart) dönem ücret pusulası açıklamasının tamamı okunabilir olmalıdır');
      expect(renderedText).toContain('Gece Çalışması Tatili (GÇT)');
      expect(renderedText).toContain('Kontrol notunun bu uzun açıklama metni PDF içinde satırlara sarılmalı ve sonuna kadar okunabilir kalmalıdır.');
      expect(renderedText).toContain('86.348,10 TL');
      expect(renderedText).toContain('26.313,79 TL');
      expect(renderedText).toContain('60.034,31 TL');
      expect(renderedText).toContain('80.348,10 TL');
      expect(renderedText).not.toContain('…');
      expect(draws.every(({ y }) => y <= 1716)).toBe(true);
    } finally {
      (globalThis as any).document = oldDocument;
    }
  });

  test('browser PDF exports keep anchor downloads for single and period files', async () => {
    const oldDocument = (globalThis as any).document;
    const oldWindow = (globalThis as any).window;
    const oldUrl = globalThis.URL;
    const downloadedNames: string[] = [];
    const urls: string[] = [];
    let id = 0;
    (globalThis as any).window = {};
    (globalThis as any).URL = {
      createObjectURL: () => `blob:payroll-pdf-${++id}`,
      revokeObjectURL() {},
    };
    installDocument(() => downloadedNames.push((globalThis as any).document.lastDownload));
    (globalThis as any).document.createElement = (tag: string) => {
      if (tag === 'canvas') {
        const context = new Proxy({ measureText: (value: string) => ({ width: value.length }) }, {
          get(target, property) { return (target as any)[property] ?? (() => undefined); },
          set() { return true; },
        });
        return { getContext: () => context, ...canvas() };
      }
      return {
        style: {},
        set download(value: string) { (globalThis as any).document.lastDownload = value; },
        click() { urls.push('clicked'); downloadedNames.push((globalThis as any).document.lastDownload); },
        remove() {},
      };
    };

    try {
      await exportSinglePayrollPdf(model);
      await exportPeriodPayrollPdf(period, [model]);
      expect(urls).toEqual(['clicked', 'clicked']);
      expect(downloadedNames).toEqual([
        `${payrollExportFileStem(model)}.pdf`,
        `${periodExportFileStem(period)}_Ucret_Pusulalari.pdf`,
      ]);
    } finally {
      (globalThis as any).document = oldDocument;
      (globalThis as any).window = oldWindow;
      globalThis.URL = oldUrl;
    }
  });

  test('native single and period PDF saves transport generated bytes and exact PDF filenames', async () => {
    const oldDocument = (globalThis as any).document;
    const oldWindow = (globalThis as any).window;
    const oldUrl = globalThis.URL;
    const calls: Array<{ command: string; args: { pdfBytes: number[]; fileName: string } }> = [];
    const expectedSingle = new Uint8Array(await (await canvasesToPdfBlob([canvas()])).arrayBuffer());
    const expectedPeriod = new Uint8Array(await (await canvasesToPdfBlob([canvas()])).arrayBuffer());
    (globalThis as any).URL = { createObjectURL() { throw new Error('native path must not create a browser URL'); } };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: { pdfBytes: number[]; fileName: string }) => {
          calls.push({ command, args });
          return calls.length === 1;
        },
      },
    };
    installDocument(() => { throw new Error('native path must not click a browser download'); });

    try {
      await exportSinglePayrollPdf(model);
      await exportPeriodPayrollPdf(period, [model]);
      expect(calls.map(({ command }) => command)).toEqual(['export_pdf', 'export_pdf']);
      expect(calls.map(({ args }) => args.fileName)).toEqual([
        `${payrollExportFileStem(model)}.pdf`,
        `${periodExportFileStem(period)}_Ucret_Pusulalari.pdf`,
      ]);
      expect(calls.every(({ args }) => args.fileName.endsWith('.pdf'))).toBe(true);
      expect(calls[0].args.pdfBytes).toEqual(Array.from(expectedSingle));
      expect(calls[1].args.pdfBytes).toEqual(Array.from(expectedPeriod));
    } finally {
      (globalThis as any).document = oldDocument;
      (globalThis as any).window = oldWindow;
      globalThis.URL = oldUrl;
    }
  });

  test('native cancellation resolves without reporting an export error and save errors reject for modal handling', async () => {
    const oldDocument = (globalThis as any).document;
    const oldWindow = (globalThis as any).window;
    const oldUrl = globalThis.URL;
    let nativeCalls = 0;
    (globalThis as any).URL = { createObjectURL() { throw new Error('native path must not create a browser URL'); } };
    installDocument(() => { throw new Error('native path must not click a browser download'); });
    (globalThis as any).window = {
      __TAURI_INTERNALS__: { invoke: async () => { nativeCalls += 1; return false; } },
    };

    try {
      await expect(exportSinglePayrollPdf(model)).resolves.toBeUndefined();
      expect(nativeCalls).toBe(1);

      (globalThis as any).window.__TAURI_INTERNALS__.invoke = async () => {
        throw new Error('PDF dosyası yazılamadı: disk dolu');
      };
      await expect(exportSinglePayrollPdf(model)).rejects.toThrow('PDF dosyası yazılamadı: disk dolu');
    } finally {
      (globalThis as any).document = oldDocument;
      (globalThis as any).window = oldWindow;
      globalThis.URL = oldUrl;
    }
  });
});
