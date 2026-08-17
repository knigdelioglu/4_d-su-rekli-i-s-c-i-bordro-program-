import * as XLSX from 'xlsx';
import { BordroDonemi, BordroKaydi, Personel } from '../types/payroll';
import { PayrollNotice } from '../types/payrollNotice';
import {
  PayrollExportLine,
  PayrollExportModel,
  isAuthoritativePayroll,
  payrollExportFileStem,
  periodExportFileStem,
} from './payrollExportModel';

const MONEY_FORMAT = '#,##0.00';

function setColumnWidths(sheet: XLSX.WorkSheet, widths: number[]): void {
  sheet['!cols'] = widths.map((wch) => ({ wch }));
}

function setMoneyCells(sheet: XLSX.WorkSheet, refs: string[]): void {
  for (const ref of refs) {
    const cell = sheet[ref];
    if (cell && typeof cell.v === 'number') cell.z = MONEY_FORMAT;
  }
}

function moneyRefsForColumn(startRow: number, endRow: number, column: string): string[] {
  const refs: string[] = [];
  for (let row = startRow; row <= endRow; row += 1) refs.push(`${column}${row}`);
  return refs;
}

function addLineSection(
  rows: Array<Array<string | number>>,
  title: string,
  lines: PayrollExportLine[]
): void {
  rows.push([title, '']);
  for (const line of lines) rows.push([line.label, line.amount]);
  rows.push(['', '']);
}

function buildPayslipSheet(model: PayrollExportModel): XLSX.WorkSheet {
  const rows: Array<Array<string | number>> = [
    ['4/D SÜREKLİ İŞÇİ ÜCRET PUSULASI', ''],
    ['Bordro Dönemi', model.periodName],
    ['Çalışma Aralığı', `${model.periodStart} - ${model.periodEnd}`],
    ['Vergi Dönemi', `${model.taxYear}-${String(model.taxMonth).padStart(2, '0')}`],
    ['Bordro Durumu', model.status],
    ['', ''],
    ['PERSONEL BİLGİLERİ', ''],
    ['T.C. Kimlik No', model.employee.tcNo],
    ['Adı Soyadı', model.employee.fullName],
    ['SGK Sicil No', model.employee.sgkRegistryNo],
    ['İş Primi Grubu', model.employee.group],
    ['Ünvan', model.employee.title],
    ['Hizmet Yılı', model.employee.serviceYears],
    ['IBAN', model.employee.iban],
    ['', ''],
    ['PUANTAJ ÖZETİ', ''],
  ];

  for (const item of model.attendanceSummary) {
    rows.push([`${item.label} (${item.code})`, item.count]);
  }
  rows.push(['', '']);
  addLineSection(rows, 'GELİRLER', model.incomes);
  rows.push(['BRÜT GELİR TOPLAMI', model.totals.gross]);
  rows.push(['', '']);
  addLineSection(rows, 'SGK / VERGİ', model.sgkTax);
  addLineSection(rows, 'KESİNTİLER', model.deductions);
  rows.push(['KESİNTİ TOPLAMI', model.totals.deductions]);
  rows.push(['NET ÖDEME', model.totals.net]);
  rows.push(['', '']);
  addLineSection(rows, 'KURUM MALİYET BİLGİSİ', model.employer);
  rows.push(['Kaynak Bordro Güncelleme', model.sourceUpdatedAt]);

  const sheet = XLSX.utils.aoa_to_sheet(rows);
  setColumnWidths(sheet, [42, 28]);
  sheet['!merges'] = [
    XLSX.utils.decode_range('A1:B1'),
    XLSX.utils.decode_range('A7:B7'),
    XLSX.utils.decode_range('A16:B16'),
  ];

  for (let row = 1; row <= rows.length; row += 1) {
    const value = rows[row - 1]?.[1];
    if (typeof value === 'number' && row !== 13) setMoneyCells(sheet, [`B${row}`]);
  }
  return sheet;
}

function buildDetailSheet(model: PayrollExportModel): XLSX.WorkSheet {
  const rows: Array<Record<string, string | number>> = [];
  const pushGroup = (group: string, lines: PayrollExportLine[]) => {
    for (const line of lines) {
      rows.push({ Grup: group, Kalem: line.label, Tutar: line.amount });
    }
  };
  pushGroup('Gelir', model.incomes);
  rows.push({ Grup: 'Toplam', Kalem: 'Brüt Gelir Toplamı', Tutar: model.totals.gross });
  pushGroup('SGK/Vergi', model.sgkTax);
  pushGroup('Kesinti', model.deductions);
  rows.push({ Grup: 'Toplam', Kalem: 'Kesinti Toplamı', Tutar: model.totals.deductions });
  rows.push({ Grup: 'Sonuç', Kalem: 'Net Ödeme', Tutar: model.totals.net });
  pushGroup('Kurum', model.employer);

  const sheet = XLSX.utils.json_to_sheet(rows, { header: ['Grup', 'Kalem', 'Tutar'] });
  setColumnWidths(sheet, [16, 42, 20]);
  setMoneyCells(sheet, moneyRefsForColumn(2, rows.length + 1, 'C'));
  sheet['!autofilter'] = { ref: `A1:C${Math.max(1, rows.length + 1)}` };
  return sheet;
}

function buildAttendanceSheet(model: PayrollExportModel): XLSX.WorkSheet {
  const rows = model.attendanceDays.length
    ? model.attendanceDays.map((item, index) => ({
        'S.No': index + 1,
        Tarih: item.date,
        'Puantaj Kodu': item.code,
      }))
    : model.attendanceSummary.map((item, index) => ({
        'S.No': index + 1,
        Tarih: `${item.label} özeti`,
        'Puantaj Kodu': `${item.code}: ${item.count}`,
      }));
  const sheet = XLSX.utils.json_to_sheet(rows);
  setColumnWidths(sheet, [8, 20, 24]);
  return sheet;
}

export function buildSinglePayrollWorkbook(model: PayrollExportModel): XLSX.WorkBook {
  const workbook = XLSX.utils.book_new();
  workbook.Props = {
    Title: `${model.employee.fullName} - ${model.periodName} Bordrosu`,
    Subject: '4/D Sürekli İşçi Ücret Pusulası',
    Author: '4/D Bordro Programı',
    CreatedDate: new Date(model.generatedAt),
  };
  XLSX.utils.book_append_sheet(workbook, buildPayslipSheet(model), 'Ücret Pusulası');
  XLSX.utils.book_append_sheet(workbook, buildDetailSheet(model), 'Hesap Detayı');
  XLSX.utils.book_append_sheet(workbook, buildAttendanceSheet(model), 'Puantaj');
  return workbook;
}

export function exportSinglePayrollExcel(model: PayrollExportModel): void {
  XLSX.writeFile(buildSinglePayrollWorkbook(model), `${payrollExportFileStem(model)}.xlsx`);
}

function modelRow(model: PayrollExportModel): Record<string, string | number> {
  return {
    'T.C. Kimlik No': model.employee.tcNo,
    'Ad Soyad': model.employee.fullName,
    Grup: model.employee.group,
    Durum: model.status,
    'Brüt Gelir': model.totals.gross,
    'Nihai PEK': model.totals.finalPek,
    'SGK İşçi': model.deductions.find((x) => x.key === 'isciSgkPrimi')?.amount ?? 0,
    'İşsizlik İşçi': model.deductions.find((x) => x.key === 'isciIssizlikPrimi')?.amount ?? 0,
    'Gelir Vergisi': model.deductions.find((x) => x.key === 'gelirVergisi')?.amount ?? 0,
    'Damga Vergisi': model.deductions.find((x) => x.key === 'damgaVergisi')?.amount ?? 0,
    'Diğer Kesintiler': Math.max(
      0,
      model.totals.deductions -
        (model.deductions.find((x) => x.key === 'isciSgkPrimi')?.amount ?? 0) -
        (model.deductions.find((x) => x.key === 'isciIssizlikPrimi')?.amount ?? 0) -
        (model.deductions.find((x) => x.key === 'gelirVergisi')?.amount ?? 0) -
        (model.deductions.find((x) => x.key === 'damgaVergisi')?.amount ?? 0)
    ),
    'Kesinti Toplamı': model.totals.deductions,
    'Net Ödeme': model.totals.net,
  };
}

function buildJsonSheet(
  rows: Array<Record<string, string | number>>,
  widths: number[],
  moneyColumns: string[] = []
): XLSX.WorkSheet {
  const sheet = XLSX.utils.json_to_sheet(rows);
  setColumnWidths(sheet, widths);
  if (rows.length > 0) {
    const range = XLSX.utils.decode_range(sheet['!ref'] ?? 'A1:A1');
    for (let col = range.s.c; col <= range.e.c; col += 1) {
      const headerRef = XLSX.utils.encode_cell({ r: 0, c: col });
      const header = String(sheet[headerRef]?.v ?? '');
      if (!moneyColumns.includes(header)) continue;
      for (let row = 1; row <= range.e.r; row += 1) {
        const ref = XLSX.utils.encode_cell({ r: row, c: col });
        if (sheet[ref] && typeof sheet[ref].v === 'number') sheet[ref].z = MONEY_FORMAT;
      }
    }
    sheet['!autofilter'] = { ref: sheet['!ref']! };
  }
  return sheet;
}

export function buildPeriodPayrollWorkbook(args: {
  period: BordroDonemi;
  models: PayrollExportModel[];
  people: Personel[];
  payrolls: BordroKaydi[];
  notices?: PayrollNotice[];
}): XLSX.WorkBook {
  const { period, models, people, payrolls, notices = [] } = args;
  const workbook = XLSX.utils.book_new();

  const summary = models.map(modelRow);
  const summarySheet = buildJsonSheet(
    summary,
    [18, 28, 18, 14, 16, 16, 16, 16, 16, 16, 18, 18, 18],
    [
      'Brüt Gelir',
      'Nihai PEK',
      'SGK İşçi',
      'İşsizlik İşçi',
      'Gelir Vergisi',
      'Damga Vergisi',
      'Diğer Kesintiler',
      'Kesinti Toplamı',
      'Net Ödeme',
    ]
  );
  XLSX.utils.book_append_sheet(workbook, summarySheet, 'Bordro İcmali');

  const incomes = models.map((model) => {
    const row: Record<string, string | number> = {
      'T.C. Kimlik No': model.employee.tcNo,
      'Ad Soyad': model.employee.fullName,
    };
    for (const line of model.incomes) row[line.label] = line.amount;
    row['Brüt Gelir Toplamı'] = model.totals.gross;
    return row;
  });
  XLSX.utils.book_append_sheet(
    workbook,
    buildJsonSheet(incomes, [18, 28, ...Array(14).fill(18)], [
      ...models[0]?.incomes.map((line) => line.label) ?? [],
      'Brüt Gelir Toplamı',
    ]),
    'Gelirler'
  );

  const deductions = models.map((model) => {
    const row: Record<string, string | number> = {
      'T.C. Kimlik No': model.employee.tcNo,
      'Ad Soyad': model.employee.fullName,
    };
    for (const line of model.deductions) row[line.label] = line.amount;
    row['Kesinti Toplamı'] = model.totals.deductions;
    row['Net Ödeme'] = model.totals.net;
    return row;
  });
  XLSX.utils.book_append_sheet(
    workbook,
    buildJsonSheet(deductions, [18, 28, ...Array(13).fill(18)], [
      ...models[0]?.deductions.map((line) => line.label) ?? [],
      'Kesinti Toplamı',
      'Net Ödeme',
    ]),
    'Kesintiler'
  );

  const sgkTax = models.map((model) => {
    const row: Record<string, string | number> = {
      'T.C. Kimlik No': model.employee.tcNo,
      'Ad Soyad': model.employee.fullName,
    };
    for (const line of model.sgkTax) row[line.label] = line.amount;
    for (const line of model.employer) row[line.label] = line.amount;
    return row;
  });
  XLSX.utils.book_append_sheet(
    workbook,
    buildJsonSheet(sgkTax, [18, 28, ...Array(18).fill(20)], [
      ...models[0]?.sgkTax.map((line) => line.label) ?? [],
      ...models[0]?.employer.map((line) => line.label) ?? [],
    ]),
    'SGK-Vergi'
  );

  const attendanceRows: Array<Record<string, string | number>> = [];
  for (const model of models) {
    if (model.attendanceDays.length === 0) {
      attendanceRows.push({
        'T.C. Kimlik No': model.employee.tcNo,
        'Ad Soyad': model.employee.fullName,
        Tarih: 'Detay kayıt yok',
        Kod: '',
      });
      continue;
    }
    for (const day of model.attendanceDays) {
      attendanceRows.push({
        'T.C. Kimlik No': model.employee.tcNo,
        'Ad Soyad': model.employee.fullName,
        Tarih: day.date,
        Kod: day.code,
      });
    }
  }
  XLSX.utils.book_append_sheet(
    workbook,
    buildJsonSheet(attendanceRows, [18, 28, 18, 12]),
    'Puantaj'
  );

  const bank = models.map((model) => ({
    'T.C. Kimlik No': model.employee.tcNo,
    'Ad Soyad': model.employee.fullName,
    IBAN: model.employee.iban,
    'Net Ödeme': model.totals.net,
  }));
  XLSX.utils.book_append_sheet(
    workbook,
    buildJsonSheet(bank, [18, 28, 34, 18], ['Net Ödeme']),
    'Banka'
  );

  const payrollByPerson = new Map(
    payrolls.filter((payroll) => payroll.donemId === period.id).map((payroll) => [payroll.personelId, payroll])
  );
  const control = people.map((person) => {
    const payroll = payrollByPerson.get(person.id);
    const personNotices = notices.filter(
      (notice) => notice.scope === 'PERIOD' || notice.personnelId === person.id
    );
    const critical = personNotices.filter((notice) => notice.severity === 'CRITICAL');
    const warnings = personNotices.filter((notice) => notice.severity === 'WARNING');
    return {
      'T.C. Kimlik No': person.tcNo,
      'Ad Soyad': `${person.ad} ${person.soyad}`,
      'Bordro Durumu': payroll?.status ?? 'YOK',
      'Resmi Çıktıya Dahil': isAuthoritativePayroll(payroll) ? 'EVET' : 'HAYIR',
      'Kritik Uyarı': critical.map((item) => item.title).join(' | ') || '—',
      'Kontrol Uyarısı': warnings.map((item) => item.title).join(' | ') || '—',
      'Zam Geçişi': personNotices.some((item) => item.code === 'RAISE_TRANSITION_PERIOD') ? 'VAR' : '—',
      'GV Dilim Geçişi': personNotices.some((item) => item.code === 'INCOME_TAX_BRACKET_TRANSITION') ? 'VAR' : '—',
      'PEK Devri': personNotices.some((item) => item.code.includes('PEK_CARRY')) ? 'VAR' : '—',
      'Rapor Kotası': personNotices.some((item) => item.code.includes('SICK_LEAVE')) ? 'KONTROL' : '—',
    };
  });
  XLSX.utils.book_append_sheet(
    workbook,
    buildJsonSheet(control, [18, 28, 16, 18, 40, 40, 15, 18, 15, 16]),
    'Kontrol'
  );

  workbook.Props = {
    Title: `${period.donemAdi} 4/D Bordro İcmali`,
    Subject: '4/D Sürekli İşçi Bordro Çalışma Kitabı',
    Author: '4/D Bordro Programı',
    CreatedDate: new Date(),
  };
  return workbook;
}

export function exportPeriodPayrollExcel(args: {
  period: BordroDonemi;
  models: PayrollExportModel[];
  people: Personel[];
  payrolls: BordroKaydi[];
  notices?: PayrollNotice[];
}): void {
  XLSX.writeFile(buildPeriodPayrollWorkbook(args), `${periodExportFileStem(args.period)}.xlsx`);
}
