import { BordroDonemi } from '../types/payroll';
import {
  PayrollExportLine,
  PayrollExportModel,
  payrollExportFileStem,
  periodExportFileStem,
} from './payrollExportModel';

const CSV_SEPARATOR = ';';
const CSV_BOM = '\uFEFF';

export function escapeCsvCell(value: string | number | null | undefined): string {
  const raw = value == null ? '' : String(value);
  return /[";\n\r]/.test(raw) ? `"${raw.replace(/"/g, '""')}"` : raw;
}

function rowsToCsv(rows: Array<Array<string | number | null | undefined>>): string {
  return `${CSV_BOM}${rows
    .map((row) => row.map(escapeCsvCell).join(CSV_SEPARATOR))
    .join('\r\n')}\r\n`;
}

function appendLines(
  rows: Array<Array<string | number | null | undefined>>,
  title: string,
  lines: PayrollExportLine[]
): void {
  rows.push([title, '']);
  for (const line of lines) rows.push([line.label, line.amount]);
  rows.push(['', '']);
}

export function buildSinglePayrollCsv(model: PayrollExportModel): string {
  const rows: Array<Array<string | number | null | undefined>> = [
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
  appendLines(rows, 'GELİRLER', model.incomes);
  rows.push(['Brüt Gelir Toplamı', model.totals.gross]);
  rows.push(['', '']);
  appendLines(rows, 'KESİNTİLER', model.deductions);
  rows.push(['Kesinti Toplamı', model.totals.deductions]);
  rows.push(['Net Ödeme', model.totals.net]);
  rows.push(['', '']);
  appendLines(rows, 'SGK / VERGİ', model.sgkTax);
  appendLines(rows, 'KURUM MALİYET BİLGİSİ', model.employer);

  if (model.attendanceDays.length > 0) {
    rows.push(['PUANTAJ GÜNLERİ', 'Kod']);
    for (const day of model.attendanceDays) rows.push([day.date, day.code]);
  }

  if (model.notices.length > 0) {
    rows.push(['KONTROL NOTLARI', 'Mesaj']);
    for (const notice of model.notices) {
      rows.push([`${notice.severity}: ${notice.title}`, [notice.message, ...notice.details].join(' ')]);
    }
  }

  rows.push(['Kaynak Bordro Güncelleme', model.sourceUpdatedAt]);
  return rowsToCsv(rows);
}

export function buildPeriodPayrollCsv(args: {
  period: BordroDonemi;
  models: PayrollExportModel[];
}): string {
  if (args.models.length === 0) {
    throw new Error('CSV için en az bir CALCULATED veya FINALIZED bordro gerekli.');
  }

  const headers = [
    'T.C. Kimlik No',
    'Ad Soyad',
    'Grup',
    'Durum',
    'Brüt Gelir',
    'Nihai PEK',
    'SGK İşçi',
    'İşsizlik İşçi',
    'Gelir Vergisi',
    'Damga Vergisi',
    'Kesinti Toplamı',
    'Net Ödeme',
  ];
  const rows: Array<Array<string | number | null | undefined>> = [
    [`4/D BORDRO DÖNEM ÖZETİ: ${args.period.donemAdi}`, ''],
    [`Vergi Dönemi: ${args.period.taxYear}-${String(args.period.taxMonth).padStart(2, '0')}`, ''],
    headers,
  ];

  for (const model of args.models) {
    const deduction = (key: string): number =>
      model.deductions.find((line) => line.key === key)?.amount ?? 0;
    rows.push([
      model.employee.tcNo,
      model.employee.fullName,
      model.employee.group,
      model.status,
      model.totals.gross,
      model.totals.finalPek,
      deduction('isciSgkPrimi'),
      deduction('isciIssizlikPrimi'),
      deduction('gelirVergisi'),
      deduction('damgaVergisi'),
      model.totals.deductions,
      model.totals.net,
    ]);
  }

  return rowsToCsv(rows);
}

function downloadCsv(content: string, fileName: string): void {
  if (typeof document === 'undefined') {
    throw new Error('CSV indirmek için tarayıcı/Tauri belge bağlamı gerekli.');
  }
  const blob = new Blob([content], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function exportSinglePayrollCsv(model: PayrollExportModel): void {
  downloadCsv(buildSinglePayrollCsv(model), `${payrollExportFileStem(model)}.csv`);
}

export function exportPeriodPayrollCsv(args: {
  period: BordroDonemi;
  models: PayrollExportModel[];
}): void {
  downloadCsv(buildPeriodPayrollCsv(args), `${periodExportFileStem(args.period)}.csv`);
}
