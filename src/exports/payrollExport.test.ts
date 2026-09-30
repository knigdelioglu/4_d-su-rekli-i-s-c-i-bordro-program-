import { describe, expect, test } from 'bun:test';
import * as XLSX from 'xlsx';
import { BordroDonemi, BordroKaydi, Personel, PersonelPuantaj } from '../types/payroll';
import {
  buildPayrollExportModel,
  buildPeriodPayrollExportModels,
  sanitizeExportFilePart,
} from './payrollExportModel';
import {
  buildPeriodPayrollWorkbook,
  buildSinglePayrollWorkbook,
  exportPeriodPayrollExcel,
  exportSinglePayrollExcel,
} from './payrollExcelExport';
import { buildPeriodPayrollCsv, buildSinglePayrollCsv, escapeCsvCell } from './payrollCsvExport';
import { canvasesToPdfBlob } from './payrollPdfExport';

const period: BordroDonemi = {
  id: '2026-07',
  yil: 2026,
  ay: 7,
  baslangicTarihi: '2026-07-15',
  bitisTarihi: '2026-08-14',
  donemAdi: '15.07.2026 - 14.08.2026',
  taxYear: 2026,
  taxMonth: 8,
};

const person: Personel = {
  id: 'p1',
  tcNo: '11111111111',
  ad: 'Şule',
  soyad: 'Çığ',
  grup: '1. Grup',
  unvan: 'İşçi',
  sgkSicilNo: 'SGK-1',
  iban: 'TR000000000000000000000001',
  hizmetYili: 8,
};

const attendance: PersonelPuantaj = {
  id: 'p1_2026-07',
  personelId: 'p1',
  donemId: period.id,
  gunler: {
    '2026-07-15': 'Ç',
    '2026-07-16': 'Ç',
    '2026-07-17': 'R',
  },
};

function payroll(status: BordroKaydi['status'], personId = 'p1'): BordroKaydi {
  return {
    id: `${personId}_${period.id}`,
    personelId: personId,
    donemId: period.id,
    accrualId: `${personId}_${period.id}`,
    accrualType: 'NORMAL',
    paymentDate: '2026-08-14',
    sequence: 0,
    accrualDescription: null,
    status,
    puantajOzeti: { 'Ç': 20, T: 4, G: 1, 'İ': 2, 'GÇ': 0, 'GÇT': 0, R: 3 },
    gelirler: {
      tabanBrutAylik: 80000,
      tediye: 0,
      tisIkramiyesi: 10000,
      ekOdeme: 2000,
      yemek: 6000,
      birlestirilmisSosyalYardim: 5000,
      vasitaYol: 2500,
      giyimYardimi: 250,
      isPrimi: 5000,
      geceCalismasiUcreti: 0,
      geceCalismasiTatiliUcreti: 0,
      hizmetZammi: 500,
      digerGelir: 0,
    },
    gelirToplam: 111250,
    kesintiler: {
      isciSgkPrimi: 14000,
      isciIssizlikPrimi: 1000,
      gelirVergisi: 12000,
      damgaVergisi: 700,
      sendikaAidati: 1500,
      bes: 0,
      icra: 0,
      kisiBorcu: 0,
      dogumAskerlikBorclanmasi: 0,
      hayatSaglikSigortasi: 0,
      digerKesinti: 0,
    },
    kesintiToplam: 29200,
    netOdeme: 82050,
    olusturulmaTarihi: '2026-08-14T12:00:00Z',
    sonGuncellemeTarihi: '2026-08-14T12:00:00Z',
    oncekiKumulatifGvMatrahi: 300000,
    devredenPekGelen: [{ tutar: 5000, kalanAySayisi: 1 }],
    sonrakiDevredenPek: [{ tutar: 1000, kalanAySayisi: 1 }],
    pekDetay: {
      hesaplananPek: 105000,
      finalPek: 100000,
      devredenPekAşanTutar: 5000,
      pekAltSinir: 30000,
      pekUstSinir: 100000,
      fiiliYemekGunu: 20,
      yemekIstisnasiTutar: 6000,
      isverenSgkPrimi: 21750,
      isverenIssizlikPrimi: 2000,
      isverenPrimToplami: 23750,
    },
    gvDetay: {
      oncekiKumulatifGvMatrahi: 300000,
      cariGvMatrahi: 90000,
      yeniKumulatifGvMatrahi: 390000,
      brutGelirVergisi: 18000,
      asgariUcretGvMatrahi: 26000,
      asgariUcretReferansKumulatifMatrahi: 200000,
      asgariUcretGvIstisnasi: 6000,
      ayniAyOncekiKullanilanGvIstisnasi: 0,
      tahakkukOncesiKalanGvIstisnasi: 6000,
      uygulananGvIstisnasi: 6000,
      tahakkukSonrasiKalanGvIstisnasi: 0,
      kesilenGelirVergisi: 12000,
    },
    damgaDetay: {
      brutDamgaVergisi: 1000,
      aylikDamgaIstisnaHakki: 500,
      ayniAyOncekiKullanilanDamgaIstisnasi: 0,
      uygulananDamgaIstisnasi: 300,
      kalanDamgaIstisnasi: 200,
      kesilenDamgaVergisi: 700,
    },
  };
}

describe('payroll export contracts', () => {
  test('STALE payroll cannot become an official export model', () => {
    expect(() => buildPayrollExportModel({ person, payroll: payroll('STALE'), period })).toThrow(
      /CALCULATED veya FINALIZED/
    );
  });

  test('period model collection excludes DRAFT and STALE payrolls', () => {
    const secondPerson: Personel = { ...person, id: 'p2', tcNo: '22222222222', ad: 'Ali' };
    const models = buildPeriodPayrollExportModels({
      period,
      people: [person, secondPerson],
      payrolls: [payroll('FINALIZED'), payroll('STALE', 'p2')],
      attendances: [attendance],
    });
    expect(models.length).toBe(1);
    expect(models[0].status).toBe('FINALIZED');
    expect(models[0].employee.fullName).toBe('Şule Çığ');
  });

  test('single payroll workbook has payslip, detail and attendance sheets', () => {
    const model = buildPayrollExportModel({
      person,
      payroll: payroll('CALCULATED'),
      period,
      attendance,
    });
    const workbook = buildSinglePayrollWorkbook(model);
    expect(workbook.SheetNames).toEqual(['Ücret Pusulası', 'Hesap Detayı', 'Puantaj']);
    const puantajRows = XLSX.utils.sheet_to_json<Record<string, string | number>>(
      workbook.Sheets.Puantaj
    );
    expect(puantajRows.length).toBe(3);
  });

  test('payslip keeps employee identity values visible and merges only section headings', () => {
    const model = buildPayrollExportModel({
      person: { ...person, tcNo: '01234567890' },
      payroll: payroll('CALCULATED'),
      period,
      attendance,
    });
    const workbook = XLSX.read(
      XLSX.write(buildSinglePayrollWorkbook(model), { bookType: 'xlsx', type: 'array' }),
      { type: 'array' }
    );
    const sheet = workbook.Sheets['Ücret Pusulası'];
    const rows = XLSX.utils.sheet_to_json<Array<string | number>>(sheet, { header: 1, raw: true });
    const valueFor = (label: string) => rows.find((row) => row[0] === label)?.[1];

    expect(sheet['!merges']?.map((merge) => XLSX.utils.encode_range(merge))).toEqual([
      'A1:B1',
      'A11:B11',
      'A21:B21',
    ]);
    expect(sheet.A12.v).toBe('T.C. Kimlik No');
    expect(sheet.B12.v).toBe('01234567890');
    expect(sheet.B12.t).toBe('s');
    expect(sheet.B13.v).toBe('Şule Çığ');
    expect(sheet.B14.v).toBe('SGK-1');
    expect(sheet.B18.v).toBe(person.iban);
    expect(valueFor('BRÜT GELİR TOPLAMI')).toBe(model.totals.gross);
    expect(valueFor('KESİNTİ TOPLAMI')).toBe(model.totals.deductions);
    expect(valueFor('NET ÖDEME')).toBe(model.totals.net);
  });

  test('payslip keeps money at two decimals and numeric year and attendance counts as integers', () => {
    const sourcePayroll = payroll('CALCULATED');
    const model = buildPayrollExportModel({
      person: { ...person, hizmetYili: 3 },
      payroll: { ...sourcePayroll, puantajOzeti: { ...sourcePayroll.puantajOzeti, 'Ç': 23 } },
      period,
      attendance,
    });
    const workbook = XLSX.read(
      XLSX.write(buildSinglePayrollWorkbook(model), { bookType: 'xlsx', type: 'array' }),
      { type: 'array', cellNF: true }
    );
    const sheet = workbook.Sheets['Ücret Pusulası'];
    const rows = XLSX.utils.sheet_to_json<Array<string | number>>(sheet, { header: 1, raw: true });
    const cellFor = (label: string) => `B${rows.findIndex((row) => row[0] === label) + 1}`;
    const serviceYears = sheet[cellFor('Hizmet Yılı')];
    const workedDays = sheet[cellFor('Çalışılan (Ç)')];
    const gross = sheet[cellFor('BRÜT GELİR TOPLAMI')];

    expect(serviceYears).toMatchObject({ t: 'n', v: 3, z: '0' });
    expect(workedDays).toMatchObject({ t: 'n', v: 23, z: '0' });
    expect(gross).toMatchObject({ t: 'n', v: model.totals.gross, z: '#,##0.00' });
  });

  test('period workbook has seven audit sheets and marks stale payroll as excluded', () => {
    const secondPerson: Personel = { ...person, id: 'p2', tcNo: '22222222222', ad: 'Ali' };
    const good = payroll('FINALIZED');
    const stale = payroll('STALE', 'p2');
    const models = buildPeriodPayrollExportModels({
      period,
      people: [person, secondPerson],
      payrolls: [good, stale],
      attendances: [attendance],
    });
    const workbook = buildPeriodPayrollWorkbook({
      period,
      models,
      people: [person, secondPerson],
      payrolls: [good, stale],
      notices: [],
    });
    expect(workbook.SheetNames).toEqual([
      'Bordro İcmali',
      'Gelirler',
      'Kesintiler',
      'SGK-Vergi',
      'Puantaj',
      'Banka',
      'Kontrol',
    ]);
    const control = XLSX.utils.sheet_to_json<Record<string, string>>(workbook.Sheets.Kontrol);
    expect(control.length).toBe(2);
    expect(control.find((row) => row['T.C. Kimlik No'] === '22222222222')?.['Resmi Çıktıya Dahil']).toBe('HAYIR');
  });

  test('single and period Excel exports use native save with workbook values intact and cancellation returned', async () => {
    const originalDoc = (globalThis as any).document;
    const originalWindow = (globalThis as any).window;
    const nativeCalls: Array<{ command: string; fileName: string; excelBytes: number[] }> = [];
    (globalThis as any).document = {
      createElement: () => ({ click() { throw new Error('native export must not trigger browser download'); } }),
      body: { appendChild() {}, removeChild() {} },
    };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: { fileName: string; excelBytes: number[] }) => {
          nativeCalls.push({ command, ...args });
          return nativeCalls.length === 1;
        },
      },
    };

    try {
      const exportPayroll = {
        ...payroll('CALCULATED'),
        gelirToplam: 86348.1,
        kesintiToplam: 26313.79,
        netOdeme: 60034.31,
        pekDetay: { ...payroll('CALCULATED').pekDetay!, finalPek: 80348.1 },
      };
      const model = buildPayrollExportModel({
        person,
        payroll: exportPayroll,
        period,
        attendance,
      });
      const context = {
        period,
        models: [model],
        people: [person],
        payrolls: [exportPayroll],
        notices: [],
      };

      expect(await exportSinglePayrollExcel(model)).toBe(true);
      expect(await exportPeriodPayrollExcel(context)).toBe(false);
      expect(nativeCalls).toHaveLength(2);
      expect(nativeCalls.map(({ command }) => command)).toEqual(['export_excel', 'export_excel']);
      expect(nativeCalls[0].fileName).toBe('Bordro_2026-07_Sule_Cig_NORMAL_2026-08-14_0.xlsx');
      expect(nativeCalls[1].fileName).toMatch(/\.xlsx$/);

      const single = XLSX.read(new Uint8Array(nativeCalls[0].excelBytes), { type: 'array' });
      expect(single.SheetNames).toEqual(['Ücret Pusulası', 'Hesap Detayı', 'Puantaj']);
      const slipRows = XLSX.utils.sheet_to_json<(string | number)[]>(single.Sheets['Ücret Pusulası'], {
        header: 1,
        raw: true,
      });
      const valueFor = (label: string) => slipRows.find((row) => row[0] === label)?.[1];
      expect(valueFor('BRÜT GELİR TOPLAMI')).toBe(86348.1);
      expect(valueFor('KESİNTİ TOPLAMI')).toBe(26313.79);
      expect(valueFor('NET ÖDEME')).toBe(60034.31);
      expect(valueFor('Nihai / Bildirim PEK')).toBe(80348.1);
      expect(typeof valueFor('NET ÖDEME')).toBe('number');

      const periodWorkbook = XLSX.read(new Uint8Array(nativeCalls[1].excelBytes), { type: 'array' });
      expect(periodWorkbook.SheetNames).toEqual([
        'Bordro İcmali', 'Gelirler', 'Kesintiler', 'SGK-Vergi', 'Puantaj', 'Banka', 'Kontrol',
      ]);
      const summaryRows = XLSX.utils.sheet_to_json<Record<string, string | number>>(
        periodWorkbook.Sheets['Bordro İcmali']
      );
      expect(summaryRows[0]['Brüt Gelir']).toBe(86348.1);
      expect(summaryRows[0]['Nihai PEK']).toBe(80348.1);
      expect(summaryRows[0]['Kesinti Toplamı']).toBe(26313.79);
      expect(summaryRows[0]['Net Ödeme']).toBe(60034.31);
      expect(typeof summaryRows[0]['Net Ödeme']).toBe('number');
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });

  test('browser Excel exports retain anchor download behavior for single and period workbooks', async () => {
    const originalDoc = (globalThis as any).document;
    const originalWindow = (globalThis as any).window;
    const originalUrl = globalThis.URL;
    const downloads: Array<{ fileName: string; blob: Blob }> = [];
    let nextUrl = 0;
    (globalThis as any).window = {};
    (globalThis as any).URL = {
      createObjectURL(blob: Blob) {
        downloads.push({ fileName: '', blob });
        return `blob:payroll-${++nextUrl}`;
      },
      revokeObjectURL() {},
    };
    (globalThis as any).document = {
      createElement: () => ({
        href: '',
        set download(fileName: string) {
          downloads[downloads.length - 1].fileName = fileName;
        },
        click() {},
      }),
      body: { appendChild() {}, removeChild() {} },
    };

    try {
      const model = buildPayrollExportModel({
        person,
        payroll: payroll('CALCULATED'),
        period,
        attendance,
      });
      const savedSingle = await exportSinglePayrollExcel(model);
      const savedPeriod = await exportPeriodPayrollExcel({
        period,
        models: [model],
        people: [person],
        payrolls: [payroll('CALCULATED')],
      });

      expect(savedSingle).toBe(true);
      expect(savedPeriod).toBe(true);
      expect(downloads).toHaveLength(2);
      expect(downloads.every(({ fileName }) => fileName.endsWith('.xlsx'))).toBe(true);
      const browserWorkbook = XLSX.read(new Uint8Array(await downloads[0].blob.arrayBuffer()), {
        type: 'array',
      });
      expect(browserWorkbook.SheetNames).toEqual(['Ücret Pusulası', 'Hesap Detayı', 'Puantaj']);
      await new Promise((resolve) => setTimeout(resolve, 1100));
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
      globalThis.URL = originalUrl;
    }
  });

  test('native payroll Excel save errors propagate to UI error handling', async () => {
    const originalDoc = (globalThis as any).document;
    const originalWindow = (globalThis as any).window;
    let browserClicks = 0;
    (globalThis as any).document = {
      createElement: () => ({ click() { browserClicks += 1; } }),
      body: { appendChild() {}, removeChild() {} },
    };
    (globalThis as any).window = {
      __TAURI_INTERNALS__: { invoke: async () => { throw new Error('native payroll save failed'); } },
    };

    try {
      const model = buildPayrollExportModel({
        person,
        payroll: payroll('CALCULATED'),
        period,
        attendance,
      });
      await expect(exportSinglePayrollExcel(model)).rejects.toThrow('native payroll save failed');
      expect(browserClicks).toBe(0);
    } finally {
      (globalThis as any).document = originalDoc;
      (globalThis as any).window = originalWindow;
    }
  });

  test('CSV exports preserve UTF-8, Excel separators and authoritative payroll rows', () => {
    const model = buildPayrollExportModel({
      person,
      payroll: payroll('CALCULATED'),
      period,
      attendance,
    });
    const singleCsv = buildSinglePayrollCsv(model);
    expect(singleCsv.startsWith('\uFEFF')).toBeTruthy();
    expect(singleCsv.includes('Şule Çığ')).toBeTruthy();
    expect(singleCsv.includes('SGK Primi - İşçi Payı;14000')).toBeTruthy();
    expect(escapeCsvCell('a;b')).toBe('"a;b"');

    const periodCsv = buildPeriodPayrollCsv({ period, models: [model] });
    expect(periodCsv.includes('T.C. Kimlik No;Ad Soyad;Grup;Tahakkuk Türü;Tahakkuk Tarihi;Tahakkuk Sıra No')).toBeTruthy();
    expect(periodCsv.includes('11111111111;Şule Çığ;1. Grup;NORMAL;2026-08-14;0;')).toBeTruthy();
  });

  test('PDF writer emits a real PDF binary and filenames are filesystem safe', async () => {
    const fakeCanvas = {
      width: 100,
      height: 140,
      toBlob(callback: (blob: Blob | null) => void) {
        callback(new Blob([new Uint8Array([0xff, 0xd8, 0xff, 0xd9])], { type: 'image/jpeg' }));
      },
    } as unknown as HTMLCanvasElement;
    const blob = await canvasesToPdfBlob([fakeCanvas]);
    const header = new TextDecoder().decode((await blob.arrayBuffer()).slice(0, 8));
    expect(header.startsWith('%PDF-1.4')).toBeTruthy();
    expect(sanitizeExportFilePart('Şule Çığ / Ağustos 2026')).toBe('Sule_Cig_Agustos_2026');
  });
});
