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
} from './payrollExcelExport';
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
      cariGvMatrahi: 90000,
      yeniKumulatifGvMatrahi: 390000,
      brutGelirVergisi: 18000,
      asgariUcretGvMatrahi: 26000,
      asgariUcretReferansKumulatifMatrahi: 200000,
      asgariUcretGvIstisnasi: 6000,
      uygulananGvIstisnasi: 6000,
      kesilenGelirVergisi: 12000,
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
