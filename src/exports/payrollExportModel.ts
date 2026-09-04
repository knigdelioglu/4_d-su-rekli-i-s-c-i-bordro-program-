import {
  AccrualType,
  BordroDonemi,
  BordroKaydi,
  Personel,
  PersonelPuantaj,
} from '../types/payroll';
import { PayrollNotice } from '../types/payrollNotice';
import { getDefaultAccrualPaymentDate } from '../utils/payrollPresentation';

export interface PayrollExportLine {
  key: string;
  label: string;
  amount: number;
}

export interface PayrollAttendanceDay {
  date: string;
  code: string;
}

export interface PayrollExportModel {
  personId: string;
  payrollId: string;
  accrualId: string;
  accrualType: AccrualType;
  paymentDate: string;
  sequence: number;
  accrualDescription: string;
  status: 'CALCULATED' | 'FINALIZED';
  periodId: string;
  periodName: string;
  periodStart: string;
  periodEnd: string;
  taxYear: number;
  taxMonth: number;
  generatedAt: string;
  employee: {
    tcNo: string;
    fullName: string;
    group: string;
    title: string;
    sgkRegistryNo: string;
    iban: string;
    serviceYears: number;
    note: string;
  };
  attendanceSummary: Array<{ code: string; label: string; count: number }>;
  attendanceDays: PayrollAttendanceDay[];
  incomes: PayrollExportLine[];
  deductions: PayrollExportLine[];
  sgkTax: PayrollExportLine[];
  employer: PayrollExportLine[];
  gvAudit: {
    previousCumulativeGv: number;
    currentGvBase: number;
    newCumulativeGv: number;
    grossIncomeTax: number;
    monthlyExemptionEntitlement: number;
    sameMonthPriorUsed: number;
    beforeRemainingExemption: number;
    appliedExemption: number;
    afterRemainingExemption: number;
    withheldIncomeTax: number;
  };
  stampAudit: {
    grossStampTax: number;
    monthlyExemptionEntitlement: number;
    sameMonthPriorUsed: number;
    appliedExemption: number;
    remainingExemption: number;
    withheldStampTax: number;
  };
  totals: {
    gross: number;
    deductions: number;
    net: number;
    calculatedPek: number;
    finalPek: number;
    incomingPek: number;
    outgoingPek: number;
    previousCumulativeGv: number;
    newCumulativeGv: number;
  };
  notices: PayrollNotice[];
  sourceUpdatedAt: string;
}

const ATTENDANCE_LABELS: Record<string, string> = {
  'Ç': 'Çalışılan',
  T: 'Hafta Tatili',
  G: 'Genel Tatil',
  'İ': 'Ücretli İzin',
  'GÇ': 'Gece Çalışması',
  'GÇT': 'Gece Çalışması Tatili',
  R: 'Rapor',
};

const INCOME_LABELS: Array<[keyof BordroKaydi['gelirler'], string]> = [
  ['tabanBrutAylik', 'Taban Brüt Ücret'],
  ['tediye', 'İlave Tediye'],
  ['tisIkramiyesi', 'TİS İkramiyesi'],
  ['ekOdeme', 'Ek Ödeme'],
  ['yemek', 'Yemek Yardımı'],
  ['birlestirilmisSosyalYardim', 'Birleştirilmiş Sosyal Yardım'],
  ['vasitaYol', 'Vasıta / Yol Yardımı'],
  ['giyimYardimi', 'Giyim Yardımı'],
  ['isPrimi', 'İş Primi'],
  ['geceCalismasiUcreti', 'Gece Çalışması Ücreti'],
  ['geceCalismasiTatiliUcreti', 'Gece Çalışması Tatili Ücreti'],
  ['hizmetZammi', 'Hizmet Zammı'],
  ['digerGelir', 'Diğer Gelir'],
];

const DEDUCTION_LABELS: Array<[keyof BordroKaydi['kesintiler'], string]> = [
  ['isciSgkPrimi', 'SGK Primi - İşçi Payı'],
  ['isciIssizlikPrimi', 'İşsizlik Primi - İşçi Payı'],
  ['gelirVergisi', 'Gelir Vergisi'],
  ['damgaVergisi', 'Damga Vergisi'],
  ['sendikaAidati', 'Sendika Aidatı'],
  ['bes', 'BES / OKS'],
  ['icra', 'İcra Kesintisi'],
  ['kisiBorcu', 'Kişi Borcu'],
  ['dogumAskerlikBorclanmasi', 'Doğum / Askerlik Borçlanması'],
  ['hayatSaglikSigortasi', 'Hayat / Sağlık Sigortası'],
  ['digerKesinti', 'Diğer Kesinti'],
];

function numberOrZero(value: number | null | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

function mapMoneyLines<T extends object>(
  source: T,
  labels: Array<[keyof T, string]>
): PayrollExportLine[] {
  return labels.map(([key, label]) => ({
    key: String(key),
    label,
    amount: numberOrZero(source[key] as number | null | undefined),
  }));
}

export function isAuthoritativePayroll(
  payroll: BordroKaydi | null | undefined
): payroll is BordroKaydi & { status: 'CALCULATED' | 'FINALIZED' } {
  return payroll?.status === 'CALCULATED' || payroll?.status === 'FINALIZED';
}

export function assertPayrollExportable(
  payroll: BordroKaydi
): asserts payroll is BordroKaydi & { status: 'CALCULATED' | 'FINALIZED' } {
  if (!isAuthoritativePayroll(payroll)) {
    throw new Error(
      `Bordro resmi çıktıya uygun değil. Durum: ${payroll.status}. Yalnız CALCULATED veya FINALIZED bordrolar dışa aktarılabilir.`
    );
  }
}

export function sanitizeExportFilePart(value: string): string {
  return value
    .replace(/ı/g, 'i')
    .replace(/İ/g, 'I')
    .replace(/ş/g, 's')
    .replace(/Ş/g, 'S')
    .replace(/ğ/g, 'g')
    .replace(/Ğ/g, 'G')
    .replace(/ü/g, 'u')
    .replace(/Ü/g, 'U')
    .replace(/ö/g, 'o')
    .replace(/Ö/g, 'O')
    .replace(/ç/g, 'c')
    .replace(/Ç/g, 'C')
    .replace(/[^a-zA-Z0-9._-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .replace(/_+/g, '_');
}

export function payrollExportFileStem(model: PayrollExportModel): string {
  return sanitizeExportFilePart(
    `Bordro_${model.taxYear}-${String(model.taxMonth).padStart(2, '0')}_${model.employee.fullName}_${model.accrualType}_${model.paymentDate}_${model.sequence}`
  );
}

export function periodExportFileStem(period: BordroDonemi): string {
  return sanitizeExportFilePart(
    `4D_Bordro_${period.taxYear}-${String(period.taxMonth).padStart(2, '0')}`
  );
}

export function buildPayrollExportModel(args: {
  person: Personel;
  payroll: BordroKaydi;
  period: BordroDonemi;
  attendance?: PersonelPuantaj | null;
  notices?: PayrollNotice[];
}): PayrollExportModel {
  const { person, payroll, period, attendance, notices = [] } = args;
  assertPayrollExportable(payroll);

  const incomingPek = (payroll.devredenPekGelen ?? []).reduce((sum, item) => sum + numberOrZero(item.tutar), 0);
  const outgoingPek = (payroll.sonrakiDevredenPek ?? []).reduce((sum, item) => sum + numberOrZero(item.tutar), 0);
  const calculatedPek = numberOrZero(payroll.pekDetay?.hesaplananPek);
  const finalPek = numberOrZero(payroll.pekDetay?.finalPek);
  const previousCumulativeGv = numberOrZero(
    payroll.oncekiKumulatifGvMatrahi ??
      (payroll.gvDetay ? payroll.gvDetay.yeniKumulatifGvMatrahi - payroll.gvDetay.cariGvMatrahi : 0)
  );
  const newCumulativeGv = numberOrZero(payroll.gvDetay?.yeniKumulatifGvMatrahi);
  const gvAudit = {
    previousCumulativeGv: numberOrZero(
      payroll.gvDetay?.oncekiKumulatifGvMatrahi ?? previousCumulativeGv
    ),
    currentGvBase: numberOrZero(payroll.gvDetay?.cariGvMatrahi),
    newCumulativeGv,
    grossIncomeTax: numberOrZero(payroll.gvDetay?.brutGelirVergisi),
    monthlyExemptionEntitlement: numberOrZero(payroll.gvDetay?.asgariUcretGvIstisnasi),
    sameMonthPriorUsed: numberOrZero(payroll.gvDetay?.ayniAyOncekiKullanilanGvIstisnasi),
    beforeRemainingExemption: numberOrZero(payroll.gvDetay?.tahakkukOncesiKalanGvIstisnasi),
    appliedExemption: numberOrZero(payroll.gvDetay?.uygulananGvIstisnasi),
    afterRemainingExemption: numberOrZero(payroll.gvDetay?.tahakkukSonrasiKalanGvIstisnasi),
    withheldIncomeTax: numberOrZero(
      payroll.gvDetay?.kesilenGelirVergisi ?? payroll.kesintiler.gelirVergisi
    ),
  };
  const stampAudit = {
    grossStampTax: numberOrZero(payroll.damgaDetay?.brutDamgaVergisi),
    monthlyExemptionEntitlement: numberOrZero(payroll.damgaDetay?.aylikDamgaIstisnaHakki),
    sameMonthPriorUsed: numberOrZero(payroll.damgaDetay?.ayniAyOncekiKullanilanDamgaIstisnasi),
    appliedExemption: numberOrZero(payroll.damgaDetay?.uygulananDamgaIstisnasi),
    remainingExemption: numberOrZero(payroll.damgaDetay?.kalanDamgaIstisnasi),
    withheldStampTax: numberOrZero(
      payroll.damgaDetay?.kesilenDamgaVergisi ?? payroll.kesintiler.damgaVergisi
    ),
  };

  const sgkTax: PayrollExportLine[] = [
    { key: 'calculatedPek', label: 'Ham PEK', amount: calculatedPek },
    { key: 'finalPek', label: 'Nihai / Bildirim PEK', amount: finalPek },
    { key: 'incomingPek', label: 'Önceki Dönemden Gelen PEK', amount: incomingPek },
    { key: 'outgoingPek', label: 'Sonraki Döneme Devreden PEK', amount: outgoingPek },
    { key: 'isciSgkPrimi', label: 'SGK Primi - İşçi Payı', amount: numberOrZero(payroll.kesintiler.isciSgkPrimi) },
    { key: 'isciIssizlikPrimi', label: 'İşsizlik Primi - İşçi Payı', amount: numberOrZero(payroll.kesintiler.isciIssizlikPrimi) },
    { key: 'cariGvMatrahi', label: 'Cari Gelir Vergisi Matrahı', amount: numberOrZero(payroll.gvDetay?.cariGvMatrahi) },
    { key: 'previousCumulativeGv', label: 'Önceki Kümülatif GV Matrahı', amount: previousCumulativeGv },
    { key: 'newCumulativeGv', label: 'Yeni Kümülatif GV Matrahı', amount: newCumulativeGv },
    { key: 'brutGelirVergisi', label: 'Hesaplanan Gelir Vergisi', amount: numberOrZero(payroll.gvDetay?.brutGelirVergisi) },
    { key: 'gvIstisnasi', label: 'Gelir Vergisi İstisnası', amount: numberOrZero(payroll.gvDetay?.uygulananGvIstisnasi) },
    { key: 'gvMonthlyEntitlement', label: 'Aylık GV İstisna Hakkı', amount: gvAudit.monthlyExemptionEntitlement },
    { key: 'gvSameMonthPriorUsed', label: 'Aynı Ay Önceki Kullanılan GV İstisnası', amount: gvAudit.sameMonthPriorUsed },
    { key: 'gvBeforeRemaining', label: 'Tahakkuk Öncesi Kalan GV İstisnası', amount: gvAudit.beforeRemainingExemption },
    { key: 'gvAfterRemaining', label: 'Tahakkuk Sonrası Kalan GV İstisnası', amount: gvAudit.afterRemainingExemption },
    {
      key: 'kesilenGelirVergisi',
      label: 'Kesilen Gelir Vergisi',
      amount: numberOrZero(payroll.gvDetay?.kesilenGelirVergisi ?? payroll.kesintiler.gelirVergisi),
    },
    { key: 'damgaVergisi', label: 'Damga Vergisi', amount: numberOrZero(payroll.kesintiler.damgaVergisi) },
    { key: 'brutDamgaVergisi', label: 'Brüt Damga Vergisi', amount: stampAudit.grossStampTax },
    { key: 'damgaMonthlyEntitlement', label: 'Aylık Damga İstisna Hakkı', amount: stampAudit.monthlyExemptionEntitlement },
    { key: 'damgaSameMonthPriorUsed', label: 'Aynı Ay Önceki Kullanılan Damga İstisnası', amount: stampAudit.sameMonthPriorUsed },
    { key: 'damgaAppliedExemption', label: 'Uygulanan Damga İstisnası', amount: stampAudit.appliedExemption },
    { key: 'damgaRemainingExemption', label: 'Kalan Damga İstisnası', amount: stampAudit.remainingExemption },
    { key: 'damgaWithheld', label: 'Kesilen Damga Vergisi', amount: stampAudit.withheldStampTax },
  ];

  const employer: PayrollExportLine[] = [
    { key: 'isverenSgkPrimi', label: 'SGK Primi - İşveren Payı', amount: numberOrZero(payroll.pekDetay?.isverenSgkPrimi) },
    { key: 'isverenIssizlikPrimi', label: 'İşsizlik Primi - İşveren Payı', amount: numberOrZero(payroll.pekDetay?.isverenIssizlikPrimi) },
    {
      key: 'altSinirTamamlama',
      label: 'PEK Alt Sınır Tamamlama - İşveren',
      amount: numberOrZero(payroll.pekDetay?.pekAltSinirTamamlamaIsverenPrimi),
    },
    { key: 'isverenPrimToplami', label: 'Toplam İşveren Prim Maliyeti', amount: numberOrZero(payroll.pekDetay?.isverenPrimToplami) },
  ];

  const attendanceDays = Object.entries(attendance?.gunler ?? {})
    .map(([date, code]) => ({ date, code }))
    .sort((a, b) => a.date.localeCompare(b.date));

  return {
    personId: person.id,
    payrollId: payroll.id,
    accrualId: payroll.accrualId || payroll.id,
    accrualType: payroll.accrualType,
    paymentDate: payroll.paymentDate || getDefaultAccrualPaymentDate(period),
    sequence: payroll.sequence,
    accrualDescription: payroll.accrualDescription || payroll.notlar || '',
    status: payroll.status,
    periodId: period.id,
    periodName: period.donemAdi,
    periodStart: period.baslangicTarihi,
    periodEnd: period.bitisTarihi,
    taxYear: period.taxYear,
    taxMonth: period.taxMonth,
    generatedAt: new Date().toISOString(),
    employee: {
      tcNo: person.tcNo,
      fullName: `${person.ad} ${person.soyad}`.trim(),
      group: person.grup || '—',
      title: person.unvan || '—',
      sgkRegistryNo: person.sgkSicilNo || '—',
      iban: person.iban || '—',
      serviceYears: numberOrZero(person.hizmetYili),
      note: person.aciklama || '',
    },
    attendanceSummary: Object.entries(payroll.puantajOzeti).map(([code, count]) => ({
      code,
      label: ATTENDANCE_LABELS[code] ?? code,
      count: numberOrZero(count),
    })),
    attendanceDays,
    incomes: mapMoneyLines(payroll.gelirler, INCOME_LABELS),
    deductions: mapMoneyLines(payroll.kesintiler, DEDUCTION_LABELS),
    sgkTax,
    employer,
    gvAudit,
    stampAudit,
    totals: {
      gross: numberOrZero(payroll.gelirToplam),
      deductions: numberOrZero(payroll.kesintiToplam),
      net: numberOrZero(payroll.netOdeme),
      calculatedPek,
      finalPek,
      incomingPek,
      outgoingPek,
      previousCumulativeGv,
      newCumulativeGv,
    },
    notices: notices.filter((notice) => notice.scope === 'PERIOD' || notice.personnelId === person.id),
    sourceUpdatedAt: payroll.sonGuncellemeTarihi,
  };
}

export function buildPeriodPayrollExportModels(args: {
  period: BordroDonemi;
  people: Personel[];
  payrolls: BordroKaydi[];
  attendances?: PersonelPuantaj[];
  notices?: PayrollNotice[];
}): PayrollExportModel[] {
  const { period, people, payrolls, attendances = [], notices = [] } = args;
  const peopleById = new Map(people.map((person) => [person.id, person]));
  const attendanceByPerson = new Map(
    attendances.filter((item) => item.donemId === period.id).map((item) => [item.personelId, item])
  );

  return payrolls
    .filter((payroll) => payroll.donemId === period.id && isAuthoritativePayroll(payroll))
    .flatMap((payroll) => {
      const person = peopleById.get(payroll.personelId);
      if (!person) return [];
      return [
        buildPayrollExportModel({
          person,
          payroll,
          period,
          attendance: attendanceByPerson.get(person.id),
          notices,
        }),
      ];
    })
    .sort((a, b) => {
      const personOrder = a.employee.fullName.localeCompare(b.employee.fullName, 'tr');
      if (personOrder !== 0) return personOrder;
      const dateOrder = a.paymentDate.localeCompare(b.paymentDate);
      if (dateOrder !== 0) return dateOrder;
      if (a.sequence !== b.sequence) return a.sequence - b.sequence;
      return a.accrualId.localeCompare(b.accrualId);
    });
}
