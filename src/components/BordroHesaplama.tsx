/**
 * Bordro Hesaplama Ekranı — Temiz, Otomatik ve Hızlı Bordro Yönetimi
 */

import React, { useState } from 'react';
import {
  Calculator,
  Search,
  CheckCircle2,
  Clock,
  Printer,
  Sparkles,
  Users,
  Wallet,
  TrendingUp,
  Receipt,
  FileText,
  ChevronRight,
  RefreshCw,
  AlertTriangle,
  CalendarCheck,
  Building2,
  X,
  Plus,
} from 'lucide-react';
import {
  AccrualType,
  BordroDonemi,
  BordroKaydi,
  AnnualPayrollParameters,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  SickLeaveRecord,
} from '../types/payroll';
import type { PayrollViewType } from '../types/navigation';
import { PAYROLL_VIEW_LABELS } from '../types/navigation';
import { formatTL, getDefaultAccrualPaymentDate } from '../utils/payrollPresentation';
import { PaySlipModal } from './PaySlipModal';
import { PayrollFinalizeModal } from './PayrollFinalizeModal';
import {
  getPayrollEngine,
  PayrollBoundaryAccrualInput,
  PayrollBoundaryPayroll,
  PayrollBoundaryPersonel,
  PayrollBoundaryTaxOpening,
  PayrollCalculationRequest,
  PayrollDatasetSnapshot,
} from '../services/payrollEngine';
import {
  countAuthoritativeNormalPersonnel,
  getPayrollStatusLabel,
} from './Listeler/accrualListData';
import {
  isExactDecimalString,
  mergePayrollUiIntoBoundary,
  toPayrollUiModel,
} from '../services/payrollEngine/decimalBoundary';

function formatPayrollError(err: unknown): string {
  if (err && typeof err === 'object') {
    const tagged = err as { type?: string; message?: unknown };
    if (tagged.type === 'NegativeNetPayment' && tagged.message && typeof tagged.message === 'object') {
      const details = tagged.message as { gelir?: number; kesinti?: number; fark?: number };
      return `Kesintiler geliri aşıyor. Gelir: ${formatTL(details.gelir ?? 0)}, kesinti: ${formatTL(details.kesinti ?? 0)}, açık: ${formatTL(details.fark ?? 0)}.`;
    }
    if (typeof tagged.message === 'string') return tagged.message;
    try {
      return JSON.stringify(err);
    } catch {
      return String(err);
    }
  }
  return String(err);
}

const ACCRUAL_TYPE_LABELS: Record<AccrualType, string> = {
  NORMAL: 'Normal Maaş',
  TEDIYE: 'Tediye',
  TIS_IKRAMIYE: 'TİS İkramiyesi',
  SUPPLEMENTAL: 'Ek Ödeme',
};

type SupplementaryAccrualType = Exclude<AccrualType, 'NORMAL'>;

type PayrollRowFilter =
  | 'all'
  | 'attendanceMissing'
  | 'notCalculated'
  | 'stale'
  | 'calculated'
  | 'finalized';

interface SupplementaryAccrualDraft {
  accrualType: SupplementaryAccrualType;
  paymentDate: string;
  grossAmount: string;
  description: string;
}

interface BordroHesaplamaProps {
  aktifDonem: BordroDonemi;
  donemler: BordroDonemi[];
  personeller: Personel[];
  kurumDegerleriMap: Record<string, DönemselKurumDegerleri>;
  puantajlar: PersonelPuantaj[];
  bordrolar: BordroKaydi[];
  taxOpenings: PersonelTaxOpening[];
  sickLeaveRecords: SickLeaveRecord[];
  annualPayrollParameters: AnnualPayrollParameters[];
  zamAylari: number[];
  activePayrollView: PayrollViewType;
  authoritativeDataset: PayrollDatasetSnapshot;
  onSaveBordro: (bordro: PayrollBoundaryPayroll) => Promise<void> | void;
  onSavePersonel?: (personel: Personel | PayrollBoundaryPersonel) => Promise<void> | void;
  onSaveTaxOpening?: (
    opening: PersonelTaxOpening | PayrollBoundaryTaxOpening
  ) => Promise<void> | void;
  initialPersonelId?: string;
  onGoToPuantaj?: (personelId?: string) => void;
}

export const BordroHesaplama: React.FC<BordroHesaplamaProps> = ({
  aktifDonem,
  donemler,
  personeller,
  kurumDegerleriMap,
  puantajlar,
  bordrolar,
  taxOpenings,
  sickLeaveRecords,
  annualPayrollParameters,
  zamAylari,
  activePayrollView,
  authoritativeDataset,
  onSaveBordro,
  onSavePersonel,
  onSaveTaxOpening,
  onGoToPuantaj,
}) => {
  const payrollEngine = getPayrollEngine();
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [rowFilter, setRowFilter] = useState<PayrollRowFilter>('all');
  const [activePaySlip, setActivePaySlip] = useState<{
    personel: Personel;
    bordro: BordroKaydi;
  } | null>(null);
  const [isBatchProcessing, setIsBatchProcessing] = useState<boolean>(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [normalPaymentDateMap, setNormalPaymentDateMap] = useState<Record<string, string>>({});
  const [newAccrualPersonId, setNewAccrualPersonId] = useState<string | null>(null);
  const [expandedTimelinePersonId, setExpandedTimelinePersonId] = useState<string | null>(null);
  const [supplementaryAccrualDraft, setSupplementaryAccrualDraft] = useState<SupplementaryAccrualDraft>({
    accrualType: 'TEDIYE',
    paymentDate: getDefaultAccrualPaymentDate(aktifDonem),
    grossAmount: '',
    description: '',
  });

  // Manual cumulative GV override state per person for the current period
  const [manualKumulatifGvMap, setManualKumulatifGvMap] = useState<
    Record<string, string>
  >({});
  const [isKumulatifModalOpen, setIsKumulatifModalOpen] = useState<boolean>(false);

  const activeKurumDegerleri = kurumDegerleriMap[aktifDonem.id];

  const activeAccrualType: AccrualType =
    activePayrollView === 'normal'
      ? 'NORMAL'
      : activePayrollView === 'tediye'
        ? 'TEDIYE'
        : activePayrollView === 'tis'
          ? 'TIS_IKRAMIYE'
          : 'SUPPLEMENTAL';
  const isSupplementaryView = activeAccrualType !== 'NORMAL';
  const activeViewTitle = PAYROLL_VIEW_LABELS[activePayrollView];

  const getAccrualId = (payroll: BordroKaydi): string => payroll.accrualId || payroll.id;

  const getPersonAccruals = (personId: string): BordroKaydi[] =>
    bordrolar
      .filter((item) => item.personelId === personId && item.donemId === aktifDonem.id)
      .sort((a, b) => {
        const dateOrder = (a.paymentDate || getDefaultAccrualPaymentDate(aktifDonem)).localeCompare(
          b.paymentDate || getDefaultAccrualPaymentDate(aktifDonem)
        );
        if (dateOrder !== 0) return dateOrder;
        if (a.sequence !== b.sequence) return a.sequence - b.sequence;
        return getAccrualId(a).localeCompare(getAccrualId(b));
      });

  const getNormalPayroll = (personId: string): BordroKaydi | undefined =>
    getPersonAccruals(personId).find((item) => item.accrualType === 'NORMAL');

  const getActiveViewAccruals = (personId: string): BordroKaydi[] =>
    getPersonAccruals(personId).filter((item) => item.accrualType === activeAccrualType);

  const getActiveViewPayroll = (personId: string): BordroKaydi | undefined =>
    getActiveViewAccruals(personId)[0];

  const getNormalAccrualInput = (personId: string): PayrollBoundaryAccrualInput => {
    const exactPayroll = authoritativeDataset.payrolls.find(
      (item) =>
        item.personelId === personId &&
        item.donemId === aktifDonem.id &&
        item.accrualType === 'NORMAL'
    );
    const existingPayroll = getNormalPayroll(personId);
    return {
      accrualId:
        exactPayroll?.accrualId ||
        exactPayroll?.id ||
        existingPayroll?.accrualId ||
        existingPayroll?.id ||
        `${personId}_${aktifDonem.id}`,
      accrualType: 'NORMAL',
      paymentDate:
        exactPayroll?.paymentDate ||
        existingPayroll?.paymentDate ||
        (exactPayroll || existingPayroll
          ? getDefaultAccrualPaymentDate(aktifDonem)
          : normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)),
      sequence: exactPayroll?.sequence ?? existingPayroll?.sequence ?? 0,
      grossAmount: null,
      description:
        exactPayroll?.accrualDescription ?? existingPayroll?.accrualDescription ?? null,
    };
  };

  const getLegacyManualIncomeInput = (
    personId: string
  ): PayrollCalculationRequest['manualIncome'] => {
    const exactPayroll = authoritativeDataset.payrolls.find(
      (item) =>
        item.personelId === personId &&
        item.donemId === aktifDonem.id &&
        item.accrualType === 'NORMAL'
    );
    if (!exactPayroll) return null;
    const tediye = exactPayroll.gelirler.tediye ?? null;
    const tisIkramiyesi = exactPayroll.gelirler.tisIkramiyesi ?? null;
    if (tediye === null && tisIkramiyesi === null) return null;
    return { tediye, tisIkramiyesi };
  };

  const getDevirGvMatrahiForActiveYear = (person: Personel): number => {
    const activeTaxYear = aktifDonem.taxYear ?? (aktifDonem.ay === 12 ? aktifDonem.yil + 1 : aktifDonem.yil);
    const openingYear = person.devirKumulatifGvMatrahiYili;
    const opening = person.devirKumulatifGvMatrahi ?? 0;
    return opening > 0 && (!openingYear || openingYear === activeTaxYear) ? opening : 0;
  };

  const buildDataset = (): PayrollDatasetSnapshot => authoritativeDataset;

  const calculateAndSaveForPerson = async (
    person: Personel
  ): Promise<BordroKaydi | null> => {
    const pPuantaj = puantajlar.find(
      (p) => p.personelId === person.id && p.donemId === aktifDonem.id
    );

    if (!pPuantaj || !pPuantaj.gunler || Object.keys(pPuantaj.gunler).length === 0) {
      // Saved puantaj NOT found for this person in this period! DO NOT calculate automatically!
      return null;
    }

    const existingBordro = getNormalPayroll(person.id);
    if (existingBordro?.status === 'FINALIZED') {
      setErrorMessage(`${person.ad} ${person.soyad} bordrosu kesinleştirildiği için yeniden hesaplanamaz.`);
      return null;
    }

    try {
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        // Legacy NORMAL records may still carry manual Tediye/TİS income.
        // New data entry is supplementary-accrual-only; this read-only bridge
        // keeps imported historical payrolls recalculable without duplicating
        // the old fields in the UI.
        manualIncome: getLegacyManualIncomeInput(person.id),
        accrual: getNormalAccrualInput(person.id),
        dataset: buildDataset(),
      });
      await onSaveBordro(calculated);
      return toPayrollUiModel(calculated) as unknown as BordroKaydi;
    } catch (err) {
      console.error('Payroll engine calculation failed:', err);
      setErrorMessage(`Hesaplama hatası: ${formatPayrollError(err)}`);
      return null;
    }
  };

  // Batch calculation handler
  const handleCalculateAll = async () => {
    setIsBatchProcessing(true);
    let successCount = 0;
    let failCount = 0;
    const missingPuantajPersons: string[] = [];

    for (const person of personeller) {
      const res = await calculateAndSaveForPerson(person);
      if (res) {
        successCount++;
      } else {
        failCount++;
        missingPuantajPersons.push(`${person.ad} ${person.soyad}`);
      }
    }

    setIsBatchProcessing(false);

    if (failCount === 0) {
      setErrorMessage(null);
      setSuccessMessage(`${successCount} personelin bordrosu başarıyla güncellendi.`);
      setTimeout(() => setSuccessMessage(null), 3500);
    } else if (successCount > 0) {
      setSuccessMessage(`${successCount} personelin bordrosu hesaplandı.`);
      setErrorMessage(
        `${failCount} personelin bordrosu hesaplanamadı (${missingPuantajPersons.slice(0, 3).join(', ')}${missingPuantajPersons.length > 3 ? '...' : ''}). Hata ayrıntısı için ilgili personelin kaydını ve dönem parametrelerini kontrol edin.`
      );
    } else {
      setSuccessMessage(null);
      setErrorMessage(
        `Hiçbir personelin bordrosu hesaplanamadı. Kayıtlı puantaj, dönem kurum ayarları ve yıllık vergi parametrelerini kontrol edin.`
      );
    }
  };

  // Open Pay Slip modal for a person
  const handleOpenPaySlip = async (person: Personel, requestedBordro?: BordroKaydi) => {
    let bordro = requestedBordro || getActiveViewPayroll(person.id);

    if (bordro?.status === 'STALE') {
      setErrorMessage(
        `${person.ad} ${person.soyad} bordrosu önceki dönem değişikliği nedeniyle güncelliğini yitirdi. Bordro zarfını açmadan/yazdırmadan önce yeniden hesaplayın.`
      );
      return;
    }
    if (bordro?.status === 'DRAFT') {
      setErrorMessage(`${person.ad} ${person.soyad} bordrosu taslak durumda. Önce bordroyu hesaplayın.`);
      return;
    }

    if (!bordro) {
      if (isSupplementaryView) {
        setErrorMessage(
          `${person.ad} ${person.soyad} için ${activeViewTitle.toLocaleLowerCase('tr-TR')} kaydı henüz yok. Önce tahakkuk ekleyin.`
        );
        return;
      }
      const hasPuantaj = puantajlar.some(
        (p) => p.personelId === person.id && p.donemId === aktifDonem.id
      );

      if (!hasPuantaj) {
        setErrorMessage(
          `HATA: ${person.ad} ${person.soyad} için bu dönemde kayıtlı puantaj bulunmadığından bordro zarfı açılamıyor.`
        );
        return;
      }

      bordro = (await calculateAndSaveForPerson(person)) || undefined;
    }

    if (bordro) {
      setActivePaySlip({ personel: person, bordro });
    }
  };

  const openSupplementaryAccrualForm = (person: Personel) => {
    const normal = getNormalPayroll(person.id);
    if (!normal || !['CALCULATED', 'FINALIZED'].includes(normal.status)) {
      setNewAccrualPersonId(null);
      setErrorMessage(
        'Ek ödeme oluşturulmadan önce aynı dönemin normal maaş bordrosu hesaplanmalıdır.'
      );
      return;
    }
    setNewAccrualPersonId(person.id);
    setExpandedTimelinePersonId(person.id);
    setSupplementaryAccrualDraft({
      accrualType:
        activeAccrualType === 'NORMAL' ? 'TEDIYE' : (activeAccrualType as SupplementaryAccrualType),
      paymentDate: getDefaultAccrualPaymentDate(aktifDonem),
      grossAmount: '',
      description: '',
    });
    setErrorMessage(null);
  };

  const handleRecalculateAccrual = async (
    person: Personel,
    accrual: BordroKaydi,
    event: React.MouseEvent
  ) => {
    event.stopPropagation();
    if (accrual.status === 'FINALIZED') {
      setErrorMessage(`${person.ad} ${person.soyad} için kesinleştirilmiş kayıt yeniden hesaplanamaz.`);
      return;
    }
    try {
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome: null,
        accrual: {
          accrualId: getAccrualId(accrual),
          accrualType: accrual.accrualType,
          paymentDate: accrual.paymentDate || getDefaultAccrualPaymentDate(aktifDonem),
          sequence: accrual.sequence,
          grossAmount: String(
            accrual.accrualType === 'TEDIYE'
              ? accrual.gelirler.tediye ?? 0
              : accrual.accrualType === 'TIS_IKRAMIYE'
                ? accrual.gelirler.tisIkramiyesi ?? 0
                : accrual.gelirler.ekOdeme ?? 0
          ),
          description: accrual.accrualDescription ?? null,
        },
        dataset: buildDataset(),
      });
      await onSaveBordro(calculated);
      setSuccessMessage(`${person.ad} ${person.soyad} için ${ACCRUAL_TYPE_LABELS[accrual.accrualType]} yeniden hesaplandı.`);
      setErrorMessage(null);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (err) {
      setErrorMessage(`Tahakkuk hesaplama hatası: ${formatPayrollError(err)}`);
    }
  };

  const handleCalculateSupplementary = async (
    person: Personel,
    event: React.FormEvent<HTMLFormElement>
  ) => {
    event.preventDefault();
    event.stopPropagation();

    const grossAmount = supplementaryAccrualDraft.grossAmount.trim();
    const paymentDate = supplementaryAccrualDraft.paymentDate.trim();
    if (!isExactDecimalString(grossAmount) || grossAmount.startsWith('-')) {
      setErrorMessage('Ek ödeme brüt tutarı geçerli ve negatif olmayan bir tutar olmalıdır.');
      return;
    }
    if (!/^\d{4}-\d{2}-\d{2}$/.test(paymentDate)) {
      setErrorMessage('Ödeme/tahakkuk tarihi YYYY-AA-GG biçiminde olmalıdır.');
      return;
    }

    const sameDateAccruals = getPersonAccruals(person.id).filter(
      (item) => (item.paymentDate || getDefaultAccrualPaymentDate(aktifDonem)) === paymentDate
    );
    const sequence = sameDateAccruals.reduce(
      (max, item) => Math.max(max, item.sequence),
      -1
    );
    const nextSequence = Math.max(1, sequence + 1);
    const accrual: PayrollBoundaryAccrualInput = {
      accrualId: `${person.id}_${aktifDonem.id}_${supplementaryAccrualDraft.accrualType.toLowerCase()}_${paymentDate}_${nextSequence}`,
      accrualType: supplementaryAccrualDraft.accrualType,
      paymentDate,
      sequence: nextSequence,
      grossAmount,
      description: supplementaryAccrualDraft.description.trim() || null,
    };

    const pPuantaj = puantajlar.find(
      (item) => item.personelId === person.id && item.donemId === aktifDonem.id
    );
    if (!pPuantaj || !Object.keys(pPuantaj.gunler || {}).length) {
      setErrorMessage(
        `${person.ad} ${person.soyad} için kayıtlı puantaj bulunmadığından ek ödeme hesaplanamaz.`
      );
      return;
    }

    try {
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome: null,
        accrual,
        dataset: buildDataset(),
      });
      await onSaveBordro(calculated);
      setNewAccrualPersonId(null);
      setSuccessMessage(
        `${person.ad} ${person.soyad} için ${ACCRUAL_TYPE_LABELS[accrual.accrualType]} tahakkuku hesaplandı.`
      );
      setErrorMessage(null);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (err) {
      console.error('Supplementary payroll calculation failed:', err);
      setErrorMessage(`Tahakkuk hesaplama hatası: ${formatPayrollError(err)}`);
    }
  };

  // Single calculation handler
  const handleCalculateSingle = async (
    person: Personel,
    e: React.MouseEvent,
    requestedAccrual?: BordroKaydi
  ) => {
    e.stopPropagation();
    if (isSupplementaryView) {
      if (requestedAccrual) {
        await handleRecalculateAccrual(person, requestedAccrual, e);
      } else {
        openSupplementaryAccrualForm(person);
      }
      return;
    }
    const hasPuantaj = puantajlar.some(
      (p) => p.personelId === person.id && p.donemId === aktifDonem.id
    );

    if (!hasPuantaj) {
      setSuccessMessage(null);
      setErrorMessage(
        `HATA: ${person.ad} ${person.soyad} için bu dönemde (${aktifDonem.donemAdi}) kayıtlı puantaj bulunamadı! Puantajsız bordro hesaplanamaz. Lütfen önce Puantaj Cetvelinden puantaj girişi yapın.`
      );
      return;
    }

    const res = await calculateAndSaveForPerson(person);
    if (res) {
      setErrorMessage(null);
      setSuccessMessage(`${person.ad} ${person.soyad} bordrosu başarıyla hesaplandı.`);
      setTimeout(() => setSuccessMessage(null), 3000);
    }
  };

  const handleFinalizeSuccess = async (person: Personel, finalizedBordro: PayrollBoundaryPayroll) => {
    await onSaveBordro(finalizedBordro);
    setErrorMessage(null);
    setSuccessMessage(`${person.ad} ${person.soyad} bordrosu kesinleştirildi.`);
    setTimeout(() => setSuccessMessage(null), 3500);
  };

  const rowFilters: Array<{ id: PayrollRowFilter; label: string }> = [
    { id: 'all', label: 'Tümü' },
    { id: 'attendanceMissing', label: 'Puantaj Eksik' },
    { id: 'notCalculated', label: 'Hesaplanmadı' },
    { id: 'stale', label: 'Yeniden Hesaplanmalı' },
    { id: 'calculated', label: 'Hesaplandı' },
    { id: 'finalized', label: 'Kesinleştirildi' },
  ];

  const matchesSupplementaryAccrualStatus = (payroll: BordroKaydi): boolean => {
    if (rowFilter === 'all' || rowFilter === 'attendanceMissing') return true;
    if (rowFilter === 'notCalculated') return payroll.status === 'DRAFT';
    if (rowFilter === 'stale') return payroll.status === 'STALE';
    if (rowFilter === 'calculated') return payroll.status === 'CALCULATED';
    return payroll.status === 'FINALIZED';
  };

  const matchesRowFilter = (person: Personel): boolean => {
    if (rowFilter === 'all') return true;
    const attendance = puantajlar.find(
      (item) => item.personelId === person.id && item.donemId === aktifDonem.id
    );
    const hasAttendance = Boolean(attendance?.gunler && Object.keys(attendance.gunler).length > 0);
    if (rowFilter === 'attendanceMissing') return !hasAttendance;

    if (isSupplementaryView) {
      const payrolls = getActiveViewAccruals(person.id);
      if (rowFilter === 'notCalculated') {
        return hasAttendance && (payrolls.length === 0 || payrolls.some(matchesSupplementaryAccrualStatus));
      }
      return payrolls.some(matchesSupplementaryAccrualStatus);
    }

    const payroll = getActiveViewPayroll(person.id);
    if (rowFilter === 'notCalculated') {
      return hasAttendance && (!payroll || payroll.status === 'DRAFT');
    }
    if (rowFilter === 'stale') return payroll?.status === 'STALE';
    if (rowFilter === 'calculated') return payroll?.status === 'CALCULATED';
    return payroll?.status === 'FINALIZED';
  };

  // Filtered personnel list
  const normalizedSearchTerm = searchTerm.toLocaleLowerCase('tr-TR');
  const filteredPersoneller = personeller.filter(
    (p) =>
      (p.ad.toLocaleLowerCase('tr-TR').includes(normalizedSearchTerm) ||
        p.soyad.toLocaleLowerCase('tr-TR').includes(normalizedSearchTerm) ||
        p.tcNo.includes(searchTerm) ||
        (p.grup && p.grup.toLocaleLowerCase('tr-TR').includes(normalizedSearchTerm)) ||
        (p.unvan && p.unvan.toLocaleLowerCase('tr-TR').includes(normalizedSearchTerm))) &&
      matchesRowFilter(p)
  );

  // Period statistics include only authoritative snapshots. STALE/DRAFT values
  // remain visible on their row for diagnosis but must not contaminate totals.
  const activePeriodBordrolar = bordrolar.filter(
    (b) =>
      b.donemId === aktifDonem.id &&
      b.accrualType === activeAccrualType &&
      (b.status === 'CALCULATED' || b.status === 'FINALIZED')
  );
  const totalGross = activePeriodBordrolar.reduce((acc, b) => acc + (b.gelirToplam || 0), 0);
  const totalNet = activePeriodBordrolar.reduce((acc, b) => acc + (b.netOdeme || 0), 0);
  const totalDeductions = activePeriodBordrolar.reduce((acc, b) => acc + (b.kesintiToplam || 0), 0);
  const totalEmployerCost = activePeriodBordrolar.reduce(
    (acc, b) => acc + (b.pekDetay?.isverenPrimToplami ?? 0),
    0
  );
  const calculatedViewPersonnelCount = isSupplementaryView
    ? new Set(activePeriodBordrolar.map((payroll) => payroll.personelId)).size
    : countAuthoritativeNormalPersonnel(bordrolar, aktifDonem.id);
  const activeReferenceExists =
    activeAccrualType === 'TEDIYE'
      ? Boolean(activeKurumDegerleri?.tediyeListesi?.some((item) => item.aktifDonemdeOdensin))
      : activeAccrualType === 'TIS_IKRAMIYE'
        ? Boolean(activeKurumDegerleri?.tisIkramiyeListesi?.some((item) => item.aktifDonemdeOdensin))
        : false;
  const activeViewAccrualCount = new Set(activePeriodBordrolar.map((payroll) => payroll.personelId)).size;
  const missingActiveViewAccrualCount = Math.max(0, personeller.length - activeViewAccrualCount);

  return (
    <div
      className="space-y-6"
      data-testid="payroll-screen"
      data-period-id={aktifDonem.id}
      data-payroll-view={activePayrollView}
      data-payroll-engine-kind={payrollEngine.kind}
    >
      {/* Top Banner / Title */}
      <div className="bg-slate-900 rounded-2xl p-6 text-white shadow-xl flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-white flex items-center gap-2">
            <Calculator className="w-6 h-6 text-indigo-400" />
            <span>{activeViewTitle}</span>
          </h2>
          <p className="text-xs text-slate-300 mt-1 max-w-2xl">
            {isSupplementaryView
              ? `${activeViewTitle} kayıtları mevcut tahakkuklardan gösterilir. Aynı türden birden fazla tahakkuk ayrı satır olarak listelenir.`
              : 'Kesintiler ve özlük hakları personelin kayıtlı kartından otomatik çekilir. Kişi adına tıklayarak detaylı bordro zarfını görüntüleyebilirsiniz.'}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {!isSupplementaryView && (
            <>
              <button
                onClick={() => setIsKumulatifModalOpen(true)}
                className="px-4 py-3 bg-slate-800 hover:bg-slate-700 text-amber-300 font-semibold text-xs rounded-xl shadow-md transition-all flex items-center justify-center gap-2 border border-slate-700"
                title="Sisteme ilk defa girildiğinde veya yıl ortasında önceki kümülatif vergi matrahlarını elle girmek için tıklayın"
              >
                <Receipt className="w-4 h-4 text-amber-400" />
                <span>Önceki Kümülatif Matrah Girişi</span>
              </button>

              <button
                onClick={handleCalculateAll}
                disabled={isBatchProcessing || personeller.length === 0}
                className="px-5 py-3 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-semibold text-xs rounded-xl shadow-lg transition-all flex items-center justify-center gap-2 whitespace-nowrap"
              >
                {isBatchProcessing ? (
                  <RefreshCw className="w-4 h-4 animate-spin" />
                ) : (
                  <Sparkles className="w-4 h-4 text-amber-300" />
                )}
                <span>Tüm Hesaplanabilir Bordroları Hesapla ({personeller.length})</span>
              </button>
            </>
          )}
        </div>
      </div>

      {!isSupplementaryView && <div className="flex flex-col gap-3 rounded-2xl border border-indigo-100 bg-indigo-50/60 p-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <div className="text-xs font-bold uppercase tracking-wide text-indigo-900">
            Normal Maaş Ödeme / Tahakkuk Tarihi
          </div>
          <p className="mt-1 text-[11px] text-indigo-800">
            Yeni NORMAL bordro için kullanılacak açık tarihtir. Kaydedilmiş tahakkukların tarihi değiştirilemez.
            Tediye ve TİS ikramiyesi ayrı tahakkuk olarak eklenir.
          </p>
        </div>
        <label className="flex shrink-0 items-center gap-2 text-xs font-semibold text-slate-700">
          <span className="sr-only">Normal Maaş Ödeme / Tahakkuk Tarihi</span>
          <input
            data-testid="normal-payment-date"
            type="date"
            required
            value={normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)}
            onChange={(event) =>
              setNormalPaymentDateMap((current) => ({
                ...current,
                [aktifDonem.id]: event.target.value,
              }))
            }
            className="rounded-xl border border-indigo-200 bg-white px-3 py-2 text-xs font-semibold text-slate-900 focus:ring-2 focus:ring-indigo-500"
          />
        </label>
      </div>}

      {isSupplementaryView && activeAccrualType !== 'SUPPLEMENTAL' && (
        <div
          data-testid="accrual-reference-banner"
          className="flex flex-col gap-3 rounded-2xl border border-indigo-100 bg-indigo-50/60 p-4 sm:flex-row sm:items-center sm:justify-between"
        >
          <div>
            <div className="text-xs font-bold uppercase tracking-wide text-indigo-900">
              {activeViewTitle} takvim bağlantısı
            </div>
            <p className="mt-1 text-[11px] text-indigo-800">
              Referans takvimde ödeme bekliyor: <strong>{activeReferenceExists ? '✓ Evet' : '— İşaretli değil'}</strong> ·
              Tahakkuk oluşturulan: <strong>{activeViewAccrualCount} / {personeller.length}</strong>
              {missingActiveViewAccrualCount > 0 && ` · Eksik: ${missingActiveViewAccrualCount}`}
            </p>
          </div>
          <p className="max-w-md text-[11px] font-medium text-slate-600">
            Bu ekran mevcut tahakkukları gösterir; aynı kişiye aynı türden yeni bir tahakkuk ayrıca eklenebilir.
          </p>
        </div>
      )}

      {/* Alert Banners */}
      {successMessage && (
        <div className="p-4 bg-emerald-50 border border-emerald-200 text-emerald-900 rounded-xl text-xs font-semibold flex items-center gap-2 animate-fade-in shadow-xs">
          <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
          <span>{successMessage}</span>
        </div>
      )}

      {errorMessage && (
        <div className="p-4 bg-rose-50 border border-rose-200 text-rose-900 rounded-xl text-xs font-semibold flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 animate-fade-in shadow-xs">
          <div className="flex items-center gap-2">
            <AlertTriangle className="w-5 h-5 text-rose-600 shrink-0" />
            <span>{errorMessage}</span>
          </div>
          {onGoToPuantaj && (
            <button
              onClick={() => onGoToPuantaj()}
              className="px-3 py-1.5 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-bold shrink-0 transition-colors flex items-center gap-1 shadow-xs"
            >
              <CalendarCheck className="w-3.5 h-3.5" />
              <span>Puantaj Cetveline Git →</span>
            </button>
          )}
        </div>
      )}

      {/* KPI Stats Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
        <div className="bg-white p-4 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-3.5">
          <div className="p-3 bg-blue-50 text-blue-700 rounded-xl shrink-0">
            <Users className="w-5 h-5" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">Toplam Personel</span>
            <div className="text-lg font-bold text-slate-900">{personeller.length} Kişi</div>
            <span className="text-[11px] text-blue-600 font-medium">
              {calculatedViewPersonnelCount} / {personeller.length} {isSupplementaryView ? 'Kayıt' : 'Hesaplandı'}
            </span>
            <span className="block text-[10px] text-slate-500">
              Toplam Tahakkuk: {activePeriodBordrolar.length}
            </span>
          </div>
        </div>

        <div className="bg-white p-4 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-3.5">
          <div className="p-3 bg-indigo-50 text-indigo-700 rounded-xl shrink-0">
            <Wallet className="w-5 h-5" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">Toplam Brüt Gelir</span>
            <div className="text-lg font-bold text-indigo-900 font-mono">
              {formatTL(totalGross)}
            </div>
            <span className="text-[11px] text-slate-400">Vergi ve SGK Öncesi</span>
          </div>
        </div>

        <div className="bg-white p-4 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-3.5">
          <div className="p-3 bg-rose-50 text-rose-700 rounded-xl shrink-0">
            <Receipt className="w-5 h-5" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">İşçi Kesintileri</span>
            <div className="text-lg font-bold text-rose-800 font-mono">
              {formatTL(totalDeductions)}
            </div>
            <span className="text-[11px] text-slate-400">SGK + Vergi + Özel Kesinti</span>
          </div>
        </div>

        <div className="bg-white p-4 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-3.5">
          <div className="p-3 bg-emerald-50 text-emerald-700 rounded-xl shrink-0">
            <TrendingUp className="w-5 h-5" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">Toplam Net Ödeme</span>
            <div className="text-lg font-bold text-emerald-700 font-mono">
              {formatTL(totalNet)}
            </div>
            <span className="text-[11px] text-emerald-600 font-semibold">Banka Ele Geçen</span>
          </div>
        </div>

        <div className="bg-white p-4 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-3.5">
          <div className="p-3 bg-amber-50 text-amber-700 rounded-xl shrink-0">
            <Building2 className="w-5 h-5" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">İşveren Prim Maliyeti</span>
            <div className="text-lg font-bold text-amber-900 font-mono">
              {formatTL(totalEmployerCost)}
            </div>
            <span className="text-[11px] text-amber-700 font-semibold">Kurum SGK + İşsizlik</span>
          </div>
        </div>
      </div>

      {/* Main Personnel Payroll List Table */}
      <div className="bg-white rounded-2xl border border-slate-200 shadow-sm overflow-hidden">
        {/* Table Header / Toolbar */}
        <div className="p-4 bg-slate-50 border-b border-slate-200 flex flex-col gap-3">
          <div className="flex flex-col sm:flex-row items-center justify-between gap-3">
          <div className="relative w-full sm:w-80">
            <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
            <input
              type="text"
              placeholder="Personel adı, T.C. No veya Unvan ile ara..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full pl-9 pr-4 py-2 bg-white border border-slate-300 rounded-xl text-xs text-slate-900 focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <div className="text-xs text-slate-500 font-medium flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-emerald-500 inline-block"></span>
            <span>{isSupplementaryView ? 'Her satır tek bir tahakkuku temsil eder' : 'Güncel bordrolarda isme tıklayarak bordro zarfını açabilirsiniz'}</span>
          </div>
          </div>
          <div className="flex flex-wrap items-center gap-1.5" role="group" aria-label="Bordro durum filtresi">
            {rowFilters.map((filter) => (
              <button
                key={filter.id}
                type="button"
                data-testid={`payroll-filter-${filter.id}`}
                aria-pressed={rowFilter === filter.id}
                onClick={() => setRowFilter(filter.id)}
                className={`rounded-lg border px-2.5 py-1.5 text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 ${
                  rowFilter === filter.id
                    ? 'border-indigo-300 bg-indigo-100 text-indigo-800'
                    : 'border-slate-200 bg-white text-slate-600 hover:border-indigo-200 hover:bg-indigo-50'
                }`}
              >
                {filter.label}
              </button>
            ))}
          </div>
        </div>

        {/* Table */}
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-100 text-slate-700 text-[11px] uppercase tracking-wider font-bold border-b border-slate-200">
                <th className="py-3 px-4">S.No</th>
                <th className="py-3 px-4">Personel Adı Soyadı</th>
                {!isSupplementaryView && <th className="py-3 px-4">İş Primi Grubu</th>}
                {!isSupplementaryView && <th className="py-3 px-4 text-center">Puantaj İcmal</th>}
                {!isSupplementaryView && <th className="py-3 px-4 text-right">Önceki Küm. GV</th>}
                <th className="py-3 px-4 text-center">{isSupplementaryView ? 'Ödeme Tarihi' : 'Normal Ödeme/Tahakkuk'}</th>
                <th className="py-3 px-4 text-right">Brüt</th>
                {isSupplementaryView ? (
                  <>
                    <th className="py-3 px-4 text-right">SGK</th>
                    <th className="py-3 px-4 text-right">GV</th>
                  </>
                ) : (
                  <th className="py-3 px-4 text-right">Kesintiler</th>
                )}
                <th className="py-3 px-4 text-right">Net Ele Geçen</th>
                <th className="py-3 px-4 text-center">Durum</th>
                <th className="py-3 px-4 text-center">İşlemler</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 text-xs text-slate-800">
              {filteredPersoneller.length === 0 ? (
                <tr>
                  <td colSpan={isSupplementaryView ? 9 : 11} className="py-12 text-center text-slate-500">
                    Arama kriterlerine uygun personel kaydı bulunamadı.
                  </td>
                </tr>
              ) : (
                filteredPersoneller.map((person, idx) => {
                  const personAccruals = getPersonAccruals(person.id);
                  const pPuantaj = puantajlar.find(
                    (p) => p.personelId === person.id && p.donemId === aktifDonem.id
                  );
                  const hasPuantaj = !!(
                    pPuantaj &&
                    pPuantaj.gunler &&
                    Object.keys(pPuantaj.gunler).length > 0
                  );
                  const allViewAccruals = isSupplementaryView
                    ? personAccruals.filter((item) => item.accrualType === activeAccrualType)
                    : personAccruals;
                  const viewAccruals = isSupplementaryView
                    ? allViewAccruals.filter(matchesSupplementaryAccrualStatus)
                    : allViewAccruals;
                  const bordro = isSupplementaryView
                    ? viewAccruals[0]
                    : viewAccruals.find((item) => item.accrualType === activeAccrualType);
                  const additionalViewAccruals = isSupplementaryView ? viewAccruals.slice(1) : [];
                  const normalBordro = personAccruals.find((item) => item.accrualType === 'NORMAL');

                  const hasPayrollSnapshot = !!bordro;
                  const isFinalized = bordro?.status === 'FINALIZED';
                  const isStale = bordro?.status === 'STALE';
                  const isDraft = bordro?.status === 'DRAFT';
                  const isCalculated = bordro?.status === 'CALCULATED' || isFinalized;
                  const canAddSupplementary =
                    hasPuantaj && (normalBordro?.status === 'CALCULATED' || normalBordro?.status === 'FINALIZED');
                  const brut = bordro?.gelirToplam || 0;
                  const kesinti = bordro?.kesintiToplam || 0;
                  const net = bordro?.netOdeme || 0;

                  return (
                    <React.Fragment key={person.id}>
                    <tr
                      data-testid={isSupplementaryView && bordro ? `accrual-row-${getAccrualId(bordro)}` : `payroll-row-${person.id}`}
                      onClick={() => void handleOpenPaySlip(person, bordro)}
                      className={`transition-colors group ${isStale || isDraft ? 'bg-amber-50/40 cursor-default' : 'hover:bg-indigo-50/50 cursor-pointer'}`}
                    >
                      <td className="py-3 px-4 font-mono text-slate-400 font-medium">
                        {idx + 1}{isSupplementaryView && viewAccruals.length > 1 ? '.1' : ''}
                      </td>

                      {/* Person Name & TC */}
                      <td className="py-3 px-4 font-medium">
                        <div className="flex items-center gap-2.5">
                          <div className="w-8 h-8 rounded-full bg-indigo-100 text-indigo-700 font-bold flex items-center justify-center text-xs shrink-0 group-hover:bg-indigo-600 group-hover:text-white transition-colors">
                            {person.ad.charAt(0)}
                            {person.soyad.charAt(0)}
                          </div>
                          <div>
                            <div className="font-bold text-slate-900 group-hover:text-indigo-700 transition-colors flex items-center gap-1">
                              <span>{person.ad} {person.soyad}</span>
                              {!isStale && !isDraft && <ChevronRight className="w-3.5 h-3.5 text-slate-400 group-hover:text-indigo-600 opacity-0 group-hover:opacity-100 transition-opacity" />}
                            </div>
                            <span className="font-mono text-[11px] text-slate-500">
                              TC: {person.tcNo}
                            </span>
                            {isSupplementaryView && bordro?.accrualDescription && (
                              <div className="max-w-64 truncate text-[10px] font-medium text-slate-500" title={bordro.accrualDescription}>
                                {bordro.accrualDescription}
                              </div>
                            )}
                            <button
                              type="button"
                              data-testid={`timeline-toggle-${person.id}`}
                              aria-expanded={expandedTimelinePersonId === person.id}
                              onClick={(event) => {
                                event.stopPropagation();
                                setExpandedTimelinePersonId((current) =>
                                  current === person.id ? null : person.id
                                );
                              }}
                              className="mt-1 text-left text-[10px] font-semibold text-indigo-600 hover:text-indigo-800 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
                            >
                              Ödeme Geçmişi {expandedTimelinePersonId === person.id ? '▴' : '▾'}
                            </button>
                          </div>
                        </div>
                      </td>

                      {/* Group */}
                      {!isSupplementaryView && <td className="py-3 px-4 text-slate-700">
                        <div className="font-bold text-slate-900">
                          {person.grup || (person.unvan ? person.unvan.replace(/\s*\(.*?\)/, '') : '1. Grup')}
                        </div>
                        <div className="text-[10px] text-indigo-700 font-mono font-semibold">
                          {person.hizmetYili} Yıl Kıdem
                        </div>
                      </td>}

                      {/* Puantaj Summary Pills */}
                      {!isSupplementaryView && <td className="py-3 px-4 text-center">
                        {bordro ? (
                          <div className="inline-flex flex-wrap items-center justify-center gap-1 text-[10px] font-mono max-w-[220px]">
                            <span className="px-1.5 py-0.5 bg-emerald-100 text-emerald-800 rounded font-bold" title="Çalışılan Gün">
                              {bordro.puantajOzeti.Ç} Ç
                            </span>
                            <span className="px-1.5 py-0.5 bg-blue-100 text-blue-800 rounded font-bold" title="Hafta Tatili">
                              {bordro.puantajOzeti.T} T
                            </span>
                            {bordro.puantajOzeti.GÇ > 0 && (
                              <span className="px-1.5 py-0.5 bg-indigo-100 text-indigo-800 rounded font-bold" title="Gece Çalışması">
                                {bordro.puantajOzeti.GÇ} GÇ
                              </span>
                            )}
                            {bordro.puantajOzeti.GÇT > 0 && (
                              <span className="px-1.5 py-0.5 bg-teal-100 text-teal-800 rounded font-bold" title="Gece Çalışması Tatili">
                                {bordro.puantajOzeti.GÇT} GÇT
                              </span>
                            )}
                            {bordro.puantajOzeti.İ > 0 && (
                              <span className="px-1.5 py-0.5 bg-amber-100 text-amber-800 rounded font-bold" title="İzin">
                                {bordro.puantajOzeti.İ} İ
                              </span>
                            )}
                            {bordro.puantajOzeti.R > 0 && (
                              <span className="px-1.5 py-0.5 bg-rose-100 text-rose-800 rounded font-bold" title="Raporlu Gün / Kurumun Ödediği Gün">
                                {bordro.puantajOzeti.R} R {bordro.odenenRaporluGun !== undefined ? `(${bordro.odenenRaporluGun} Öd.)` : ''}
                              </span>
                            )}
                          </div>
                        ) : hasPuantaj ? (
                          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-semibold bg-amber-50 text-amber-800 border border-amber-200">
                            <CalendarCheck className="w-3 h-3 text-amber-600" />
                            <span>Puantaj Girildi</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 text-rose-600 text-[11px] font-bold">
                            <AlertTriangle className="w-3 h-3 text-rose-500" />
                            <span>Puantaj Yok</span>
                          </span>
                        )}
                      </td>}

                      {/* Önceki Küm. GV Matrahı */}
                      {!isSupplementaryView && <td className="py-3 px-4 text-right font-mono text-xs">
                        {(() => {
                          const sessionManual = manualKumulatifGvMap[person.id];
                          const isManual = sessionManual !== undefined || bordro?.manuelKumulatifGvMatrahi !== undefined;
                          const val = Number(
                            sessionManual ??
                              bordro?.oncekiKumulatifGvMatrahi ??
                              bordro?.manuelKumulatifGvMatrahi ??
                              getDevirGvMatrahiForActiveYear(person)
                          ) || 0;

                          if (val > 0) {
                            return (
                              <div className="flex flex-col items-end">
                                <span className={`font-bold ${isStale ? 'text-amber-700 line-through' : 'text-slate-800'}`}>{formatTL(val)}</span>
                                {isManual && (
                                  <span className="text-[10px] text-indigo-600 font-semibold">(Manuel)</span>
                                )}
                              </div>
                            );
                          } else if (aktifDonem.ay > 1) {
                            return (
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setIsKumulatifModalOpen(true);
                                }}
                                className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-bold bg-amber-100 text-amber-800 border border-amber-300 hover:bg-amber-200 transition-colors"
                                title="İlk ayda olunmadığı için Önceki Kümülatif Vergi Matrahı girilmelidir."
                              >
                                <AlertTriangle className="w-3 h-3 text-amber-600 shrink-0" />
                                <span>Matrah Girin</span>
                              </button>
                            );
                          } else {
                            return <span className="text-slate-400 font-mono">0,00 TL</span>;
                          }
                        })()}
                      </td>}

                      {/* Ödeme/tahakkuk tarihi */}
                      <td className="py-3 px-4 text-center font-mono text-[11px]">
                        <div className="font-bold text-slate-800">
                          {bordro
                            ? bordro.paymentDate || getDefaultAccrualPaymentDate(aktifDonem)
                            : isSupplementaryView
                              ? '—'
                              : normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)}
                        </div>
                        <div className="text-[10px] text-slate-500">
                          {bordro
                            ? (isSupplementaryView ? `Sıra ${bordro.sequence}` : 'Değiştirilemez')
                            : isSupplementaryView ? `${activeViewTitle} için` : 'Yeni NORMAL için'}
                        </div>
                      </td>

                      {/* Brüt */}
                      <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-slate-800'}`}>
                        {hasPayrollSnapshot ? formatTL(brut) : '—'}
                      </td>

                      {/* Kesintiler / supplementary breakdown */}
                      {isSupplementaryView ? (
                        <>
                          <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                            {hasPayrollSnapshot ? formatTL(bordro?.kesintiler.isciSgkPrimi ?? 0) : '—'}
                          </td>
                          <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                            {hasPayrollSnapshot ? formatTL(bordro?.kesintiler.gelirVergisi ?? 0) : '—'}
                          </td>
                        </>
                      ) : (
                        <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                          {hasPayrollSnapshot ? formatTL(kesinti) : '—'}
                        </td>
                      )}

                      {/* Net */}
                      <td className={`py-3 px-4 text-right font-mono font-bold text-sm ${isStale ? 'text-amber-700 line-through' : 'text-emerald-700'}`}>
                        {hasPayrollSnapshot ? formatTL(net) : '—'}
                      </td>

                      {/* Durum */}
                      <td className="py-3 px-4 text-center">
                        {isFinalized ? (
                          <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-slate-200 text-slate-800 border border-slate-300">
                            <CheckCircle2 className="w-3 h-3 text-slate-700" />
                            <span>Kesinleştirildi</span>
                          </span>
                        ) : isStale ? (
                          <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-amber-100 text-amber-900 border border-amber-300">
                            <AlertTriangle className="w-3 h-3 text-amber-700" />
                            <span>Yeniden Hesaplanmalı</span>
                          </span>
                        ) : isDraft ? (
                          <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-slate-100 text-slate-700 border border-slate-300">
                            <Clock className="w-3 h-3 text-slate-600" />
                            <span>Taslak</span>
                          </span>
                        ) : isCalculated ? (
                          <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-emerald-100 text-emerald-800 border border-emerald-200">
                            <CheckCircle2 className="w-3 h-3 text-emerald-600" />
                            <span>Hesaplandı</span>
                          </span>
                        ) : hasPuantaj ? (
                          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-semibold bg-amber-50 text-amber-800 border border-amber-200">
                            <Clock className="w-3 h-3 text-amber-600" />
                            <span>{isSupplementaryView ? 'Tahakkuk Eklenmedi' : 'Hesaplanmadı'}</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-rose-100 text-rose-800 border border-rose-200">
                            <AlertTriangle className="w-3 h-3 text-rose-600" />
                            <span>Puantaj Eksik</span>
                          </span>
                        )}
                      </td>

                      {/* Actions */}
                      <td className="py-3 px-4 text-center">
                        <div className="flex flex-wrap items-center justify-center gap-1.5">
                          {hasPuantaj ? (
                            <>
                              {!isFinalized && (
                                <button
                                  data-testid={isSupplementaryView && bordro ? `recalculate-accrual-${getAccrualId(bordro)}` : `calculate-payroll-${person.id}`}
                                  onClick={(e) => handleCalculateSingle(person, e, bordro)}
                                  title={isSupplementaryView
                                    ? (bordro ? 'Bu tahakkuku yeniden hesapla' : `${activeViewTitle} tahakkuku ekle`)
                                    : isStale ? 'Güncelliğini yitiren bordroyu yeniden hesapla' : 'Bordroyu Hesapla/Yeniden Hesapla'}
                                  className="p-1.5 bg-slate-100 hover:bg-indigo-600 hover:text-white text-slate-700 rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
                                >
                                  <RefreshCw className="w-3.5 h-3.5" />
                                  <span>{isSupplementaryView ? (bordro ? 'Yeniden Hesapla' : `${activeViewTitle} Ekle`) : isStale ? 'Yeniden Hesapla' : 'Hesapla'}</span>
                                </button>
                              )}

                              {isSupplementaryView && bordro && canAddSupplementary && (
                                <button
                                  type="button"
                                  data-testid={`add-same-type-accrual-${person.id}`}
                                  onClick={(event) => {
                                    event.stopPropagation();
                                    openSupplementaryAccrualForm(person);
                                  }}
                                  title={`Aynı kişiye yeni ${activeViewTitle} tahakkuku ekle`}
                                  className="p-1.5 bg-white border border-indigo-200 text-indigo-700 hover:bg-indigo-600 hover:text-white rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
                                >
                                  <Plus className="w-3.5 h-3.5" />
                                  <span>Yeni</span>
                                </button>
                              )}

                              {!isStale && !isDraft && (bordro || !isSupplementaryView) && (
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    void handleOpenPaySlip(person, bordro);
                                  }}
                                  title="Bordro Zarfını Görüntüle & Yazdır"
                                  className="p-1.5 bg-indigo-50 text-indigo-700 hover:bg-indigo-600 hover:text-white rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
                                >
                                  <FileText className="w-3.5 h-3.5" />
                                  <span>Bordro Gör</span>
                                </button>
                              )}

                              {isCalculated && !isFinalized && bordro && (
                                <PayrollFinalizeModal
                                  personel={person}
                                  bordro={bordro}
                                  donem={aktifDonem}
                                  engine={payrollEngine}
                                  dataset={buildDataset()}
                                  onFinalized={(finalizedBordro) =>
                                    handleFinalizeSuccess(person, finalizedBordro)
                                  }
                                  onError={(message) => {
                                    setSuccessMessage(null);
                                    setErrorMessage(message);
                                  }}
                                />
                              )}
                            </>
                          ) : (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                if (onGoToPuantaj) onGoToPuantaj(person.id);
                              }}
                              title="Puantaj Cetveline Git ve Puantaj Gir"
                              className="px-2.5 py-1.5 bg-rose-50 hover:bg-rose-600 hover:text-white text-rose-700 border border-rose-200 rounded-lg transition-colors text-[11px] font-bold flex items-center gap-1"
                            >
                              <CalendarCheck className="w-3.5 h-3.5" />
                              <span>Puantaj Gir</span>
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>

                    {isSupplementaryView && additionalViewAccruals.map((accrual, additionalIndex) => {
                      const accrualIsFinalized = accrual.status === 'FINALIZED';
                      const accrualIsStale = accrual.status === 'STALE';
                      const accrualIsDraft = accrual.status === 'DRAFT';
                      const accrualIsCalculated = accrual.status === 'CALCULATED' || accrualIsFinalized;
                      return (
                        <tr
                          key={`${person.id}-${getAccrualId(accrual)}`}
                          data-testid={`accrual-row-${getAccrualId(accrual)}`}
                          onClick={() => void handleOpenPaySlip(person, accrual)}
                          className={`transition-colors group ${accrualIsStale || accrualIsDraft ? 'bg-amber-50/40 cursor-default' : 'hover:bg-indigo-50/50 cursor-pointer'}`}
                        >
                          <td className="py-3 px-4 font-mono text-slate-400 font-medium">
                            {idx + 1}.{additionalIndex + 2}
                          </td>
                          <td className="py-3 px-4 font-medium">
                            <div className="font-bold text-slate-900">{person.ad} {person.soyad}</div>
                            <div className="font-mono text-[11px] text-slate-500">TC: {person.tcNo}</div>
                            <div className="mt-0.5 text-[10px] font-semibold text-indigo-600">
                              {ACCRUAL_TYPE_LABELS[accrual.accrualType]} · kayıt {additionalIndex + 2}
                            </div>
                            {accrual.accrualDescription && (
                              <div className="max-w-64 truncate text-[10px] text-slate-500" title={accrual.accrualDescription}>
                                {accrual.accrualDescription}
                              </div>
                            )}
                          </td>
                          <td className="py-3 px-4 text-center font-mono text-[11px]">
                            <div className="font-bold text-slate-800">
                              {accrual.paymentDate || getDefaultAccrualPaymentDate(aktifDonem)}
                            </div>
                            <div className="text-[10px] text-slate-500">Sıra {accrual.sequence}</div>
                          </td>
                          <td className={`py-3 px-4 text-right font-mono font-medium ${accrualIsStale ? 'text-amber-700 line-through' : 'text-slate-800'}`}>
                            {formatTL(accrual.gelirToplam || 0)}
                          </td>
                          <td className={`py-3 px-4 text-right font-mono font-medium ${accrualIsStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                            {formatTL(accrual.kesintiler.isciSgkPrimi ?? 0)}
                          </td>
                          <td className={`py-3 px-4 text-right font-mono font-medium ${accrualIsStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                            {formatTL(accrual.kesintiler.gelirVergisi ?? 0)}
                          </td>
                          <td className={`py-3 px-4 text-right font-mono font-bold text-sm ${accrualIsStale ? 'text-amber-700 line-through' : 'text-emerald-700'}`}>
                            {formatTL(accrual.netOdeme || 0)}
                          </td>
                          <td className="py-3 px-4 text-center">
                            {accrualIsFinalized ? (
                              <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-slate-200 text-slate-800 border border-slate-300">
                                <CheckCircle2 className="w-3 h-3 text-slate-700" />
                                <span>Kesinleştirildi</span>
                              </span>
                            ) : accrualIsStale ? (
                              <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-amber-100 text-amber-900 border border-amber-300">
                                <AlertTriangle className="w-3 h-3 text-amber-700" />
                                <span>Yeniden Hesaplanmalı</span>
                              </span>
                            ) : accrualIsDraft ? (
                              <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-slate-100 text-slate-700 border border-slate-300">
                                <Clock className="w-3 h-3 text-slate-600" />
                                <span>Taslak</span>
                              </span>
                            ) : (
                              <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-emerald-100 text-emerald-800 border border-emerald-200">
                                <CheckCircle2 className="w-3 h-3 text-emerald-600" />
                                <span>Hesaplandı</span>
                              </span>
                            )}
                          </td>
                          <td className="py-3 px-4 text-center">
                            <div className="flex flex-wrap items-center justify-center gap-1.5">
                              {hasPuantaj ? (
                                <>
                                  {!accrualIsFinalized && (
                                    <button
                                      type="button"
                                      data-testid={`recalculate-accrual-${getAccrualId(accrual)}`}
                                      onClick={(event) => void handleRecalculateAccrual(person, accrual, event)}
                                      className="p-1.5 bg-slate-100 hover:bg-indigo-600 hover:text-white text-slate-700 rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
                                    >
                                      <RefreshCw className="w-3.5 h-3.5" />
                                      <span>Yeniden Hesapla</span>
                                    </button>
                                  )}
                                  {!accrualIsStale && !accrualIsDraft && (
                                    <button
                                      type="button"
                                      onClick={(event) => {
                                        event.stopPropagation();
                                        void handleOpenPaySlip(person, accrual);
                                      }}
                                      className="p-1.5 bg-indigo-50 text-indigo-700 hover:bg-indigo-600 hover:text-white rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
                                    >
                                      <FileText className="w-3.5 h-3.5" />
                                      <span>Bordro Gör</span>
                                    </button>
                                  )}
                                  {accrualIsCalculated && !accrualIsFinalized && (
                                    <PayrollFinalizeModal
                                      personel={person}
                                      bordro={accrual}
                                      donem={aktifDonem}
                                      engine={payrollEngine}
                                      dataset={buildDataset()}
                                      onFinalized={(finalizedBordro) =>
                                        handleFinalizeSuccess(person, finalizedBordro)
                                      }
                                      onError={(message) => {
                                        setSuccessMessage(null);
                                        setErrorMessage(message);
                                      }}
                                    />
                                  )}
                                </>
                              ) : (
                                <button
                                  type="button"
                                  onClick={(event) => {
                                    event.stopPropagation();
                                    if (onGoToPuantaj) onGoToPuantaj(person.id);
                                  }}
                                  className="px-2.5 py-1.5 bg-rose-50 hover:bg-rose-600 hover:text-white text-rose-700 border border-rose-200 rounded-lg transition-colors text-[11px] font-bold flex items-center gap-1"
                                >
                                  <CalendarCheck className="w-3.5 h-3.5" />
                                  <span>Puantaj Gir</span>
                                </button>
                              )}
                            </div>
                          </td>
                        </tr>
                      );
                    })}

                    {expandedTimelinePersonId === person.id && <tr key={`${person.id}-accrual-timeline`}>
                      <td colSpan={isSupplementaryView ? 9 : 11} className="px-4 py-3 bg-slate-50/80">
                        <div className="flex flex-col gap-2">
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <div className="flex items-center gap-2 text-[11px] font-bold uppercase tracking-wide text-slate-500">
                              <span>Tahakkuk Zaman Çizelgesi</span>
                              <span className="rounded-full bg-slate-200 px-2 py-0.5 text-[10px] text-slate-700">
                                {personAccruals.length} kayıt
                              </span>
                            </div>
                            <button
                              type="button"
                              data-testid={`add-accrual-${person.id}`}
                              disabled={!canAddSupplementary}
                              onClick={(event) => {
                                event.stopPropagation();
                                if (!hasPuantaj) {
                                  setErrorMessage(
                                    `${person.ad} ${person.soyad} için önce bu dönemin puantajını tamamlayın.`
                                  );
                                  return;
                                }
                                openSupplementaryAccrualForm(person);
                              }}
                              title={
                                !hasPuantaj
                                  ? 'Önce bu dönemin puantajını tamamlayın.'
                                  : 'Ek ödeme için aynı dönemin normal maaş bordrosu hesaplanmış veya kesinleştirilmiş olmalıdır.'
                              }
                              className="inline-flex items-center gap-1.5 rounded-lg border border-indigo-200 bg-white px-2.5 py-1.5 text-[11px] font-bold text-indigo-700 transition-colors hover:bg-indigo-600 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                            >
                              <Plus className="h-3.5 w-3.5" />
                              <span>Ek Ödeme</span>
                            </button>
                          </div>

                          <div className="flex flex-col gap-1.5">
                            {personAccruals.length === 0 ? (
                              <div className="rounded-lg border border-dashed border-slate-300 bg-white px-3 py-2 text-[11px] text-slate-500">
                                Henüz tahakkuk yok. Normal maaş için üst satırdaki Hesapla işlemini kullanın.
                              </div>
                            ) : (
                              personAccruals.map((accrual) => {
                                const accrualIsFinalized = accrual.status === 'FINALIZED';
                                const accrualIsCalculated =
                                  accrual.status === 'CALCULATED' || accrualIsFinalized;
                                return (
                                  <div
                                    key={getAccrualId(accrual)}
                                    className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border border-slate-200 bg-white px-3 py-2"
                                  >
                                    <span className="font-mono text-[11px] font-bold text-slate-700">
                                      {accrual.paymentDate || getDefaultAccrualPaymentDate(aktifDonem)}
                                    </span>
                                    <span className="text-[11px] font-bold text-indigo-700">
                                      {ACCRUAL_TYPE_LABELS[accrual.accrualType]}
                                    </span>
                                    <span
                                      className={`rounded-full px-2 py-0.5 text-[10px] font-bold ${
                                        accrualIsFinalized
                                          ? 'bg-slate-200 text-slate-800'
                                          : accrual.status === 'STALE'
                                            ? 'bg-amber-100 text-amber-900'
                                            : accrualIsCalculated
                                              ? 'bg-emerald-100 text-emerald-800'
                                              : 'bg-slate-100 text-slate-600'
                                      }`}
                                    >
                                      {getPayrollStatusLabel(accrual.status)}
                                    </span>
                                    <span className="ml-auto font-mono text-[11px] font-bold text-slate-800">
                                      Brüt {formatTL(accrual.gelirToplam || 0)} · Net {formatTL(accrual.netOdeme || 0)}
                                    </span>
                                    {accrualIsCalculated && (
                                      <button
                                        type="button"
                                        onClick={(event) => {
                                          event.stopPropagation();
                                          void handleOpenPaySlip(person, accrual);
                                        }}
                                        className="inline-flex items-center gap-1 rounded-lg bg-indigo-50 px-2 py-1 text-[10px] font-bold text-indigo-700 hover:bg-indigo-600 hover:text-white"
                                      >
                                        <FileText className="h-3 w-3" />
                                        Bordro Gör
                                      </button>
                                    )}
                                    {accrual.accrualType !== 'NORMAL' &&
                                      accrual.status === 'CALCULATED' && (
                                        <PayrollFinalizeModal
                                          personel={person}
                                          bordro={accrual}
                                          donem={aktifDonem}
                                          engine={payrollEngine}
                                          dataset={buildDataset()}
                                          onFinalized={(finalizedBordro) =>
                                            handleFinalizeSuccess(person, finalizedBordro)
                                          }
                                          onError={(message) => {
                                            setSuccessMessage(null);
                                            setErrorMessage(message);
                                          }}
                                        />
                                      )}
                                  </div>
                                );
                              })
                            )}
                          </div>

                          {newAccrualPersonId === person.id && (
                            <form
                              onSubmit={(event) => void handleCalculateSupplementary(person, event)}
                              onClick={(event) => event.stopPropagation()}
                              className="grid grid-cols-1 gap-2 rounded-xl border border-indigo-200 bg-indigo-50/60 p-3 sm:grid-cols-2 lg:grid-cols-5"
                            >
                              <label className="flex flex-col gap-1 text-[10px] font-bold uppercase tracking-wide text-slate-600">
                                Tür
                                <select
                                  value={supplementaryAccrualDraft.accrualType}
                                  onChange={(event) =>
                                    setSupplementaryAccrualDraft((current) => ({
                                      ...current,
                                      accrualType: event.target.value as SupplementaryAccrualType,
                                    }))
                                  }
                                  className="rounded-lg border border-slate-300 bg-white px-2 py-2 text-xs font-semibold normal-case tracking-normal text-slate-900"
                                >
                                  <option value="TEDIYE">Tediye</option>
                                  <option value="TIS_IKRAMIYE">TİS İkramiyesi</option>
                                  <option value="SUPPLEMENTAL">Ek Ödeme</option>
                                </select>
                              </label>
                              <label className="flex flex-col gap-1 text-[10px] font-bold uppercase tracking-wide text-slate-600">
                                Ödeme/Tahakkuk tarihi
                                <input
                                  type="date"
                                  required
                                  value={supplementaryAccrualDraft.paymentDate}
                                  onChange={(event) =>
                                    setSupplementaryAccrualDraft((current) => ({
                                      ...current,
                                      paymentDate: event.target.value,
                                    }))
                                  }
                                  className="rounded-lg border border-slate-300 bg-white px-2 py-2 text-xs font-semibold normal-case tracking-normal text-slate-900"
                                />
                              </label>
                              <label className="flex flex-col gap-1 text-[10px] font-bold uppercase tracking-wide text-slate-600">
                                Brüt tutar
                                <input
                                  type="text"
                                  inputMode="decimal"
                                  required
                                  value={supplementaryAccrualDraft.grossAmount}
                                  onChange={(event) =>
                                    setSupplementaryAccrualDraft((current) => ({
                                      ...current,
                                      grossAmount: event.target.value,
                                    }))
                                  }
                                  placeholder="0.00"
                                  className="rounded-lg border border-slate-300 bg-white px-2 py-2 text-right text-xs font-mono font-semibold normal-case tracking-normal text-slate-900"
                                />
                              </label>
                              <label className="flex flex-col gap-1 text-[10px] font-bold uppercase tracking-wide text-slate-600 lg:col-span-2">
                                Açıklama
                                <input
                                  type="text"
                                  value={supplementaryAccrualDraft.description}
                                  onChange={(event) =>
                                    setSupplementaryAccrualDraft((current) => ({
                                      ...current,
                                      description: event.target.value,
                                    }))
                                  }
                                  placeholder="İsteğe bağlı açıklama"
                                  className="rounded-lg border border-slate-300 bg-white px-2 py-2 text-xs font-semibold normal-case tracking-normal text-slate-900"
                                />
                              </label>
                              <div className="flex items-end gap-2 sm:col-span-2 lg:col-span-5">
                                <button
                                  type="submit"
                                  className="inline-flex items-center gap-1.5 rounded-lg bg-indigo-600 px-3 py-2 text-[11px] font-bold text-white hover:bg-indigo-700"
                                >
                                  <Calculator className="h-3.5 w-3.5" />
                                  Hesapla ve Kaydet
                                </button>
                                <button
                                  type="button"
                                  onClick={() => setNewAccrualPersonId(null)}
                                  className="rounded-lg border border-slate-300 bg-white px-3 py-2 text-[11px] font-bold text-slate-700 hover:bg-slate-100"
                                >
                                  Vazgeç
                                </button>
                                <span className="text-[10px] text-slate-500">
                                  Normal maaş gelirleri bu tahakkuka otomatik eklenmez.
                                </span>
                              </div>
                            </form>
                          )}
                        </div>
                      </td>
                    </tr>}
                    </React.Fragment>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* PaySlip Modal */}
      {activePaySlip && (
        <PaySlipModal
          isOpen={true}
          onClose={() => setActivePaySlip(null)}
          personel={activePaySlip.personel}
          bordro={activePaySlip.bordro}
          donem={aktifDonem}
          isPrimiGruplari={activeKurumDegerleri?.isPrimiGruplari}
          engine={payrollEngine}
          dataset={buildDataset()}
        />
      )}

      {/* Önceki Kümülatif Matrah Yönetimi Modalı */}
      {isKumulatifModalOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 overflow-y-auto"
          onClick={() => setIsKumulatifModalOpen(false)}
        >
          <div
            className="bg-white rounded-2xl shadow-2xl border border-slate-200 max-w-4xl w-full max-h-[90vh] flex flex-col overflow-hidden my-auto animate-fade-in"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="bg-slate-900 text-white px-6 py-4 flex items-center justify-between shrink-0">
              <div className="flex items-center gap-2.5">
                <div className="p-2 bg-indigo-600/30 rounded-lg text-indigo-300">
                  <Receipt className="w-5 h-5" />
                </div>
                <div>
                  <h3 className="font-semibold text-base text-white">
                    Önceki Kümülatif Vergi Matrahı Girişi
                  </h3>
                  <p className="text-xs text-slate-400">
                    {aktifDonem.donemAdi} dönemi için personel kümülatif vergi matrahı yönetimi
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setIsKumulatifModalOpen(false)}
                className="text-slate-400 hover:text-white hover:bg-slate-800 p-1.5 rounded-lg transition-colors cursor-pointer"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-4 overflow-y-auto flex-1">
              <div className="p-3.5 bg-amber-50 border border-amber-200 rounded-xl text-xs text-amber-900 leading-relaxed">
                <strong>Mevzuat ve Kullanım Bilgisi:</strong> Sisteme yıl ortasında başlandığında veya geçmiş ayların bordroları henüz kaydedilmediğinde, personellerin yıl içindeki <strong>Önceki Kümülatif Gelir Vergisi Matrahı</strong> değerini buradan bir defaya mahsus girebilirsiniz. Sonraki dönem bordrolarında aylık GV matrahı otomatik eklenerek bir sonraki aya aktarılır.
              </div>

              <div className="max-h-96 overflow-y-auto border border-slate-200 rounded-xl">
                <table className="w-full text-left text-xs border-collapse">
                  <thead className="bg-slate-100 text-slate-700 font-bold border-b border-slate-200 sticky top-0 z-10">
                    <tr>
                      <th className="py-2.5 px-3">Personel</th>
                      <th className="py-2.5 px-3 text-right">Otomatik (Eski Bordrolar + Devir)</th>
                      <th className="py-2.5 px-3">Önceki Küm. GV Matrahı (TL)</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-200 font-mono">
                    {personeller.map((person) => {
                      const bordro = bordrolar.find(
                        (b) => b.personelId === person.id && b.donemId === aktifDonem.id
                      );
                      const exactPerson = authoritativeDataset.personnel.find(
                        (item) => item.id === person.id
                      );
                      const exactBordro = authoritativeDataset.payrolls.find(
                        (item) => item.personelId === person.id && item.donemId === aktifDonem.id
                      );
                      const autoGv =
                        bordro?.oncekiKumulatifGvMatrahi ??
                        getDevirGvMatrahiForActiveYear(person);

                      const currentSession = manualKumulatifGvMap[person.id];
                      const displayGv =
                        currentSession ??
                        exactBordro?.manuelKumulatifGvMatrahi ??
                        exactBordro?.oncekiKumulatifGvMatrahi ??
                        exactPerson?.devirKumulatifGvMatrahi ??
                        String(autoGv);
                      const displayGvNumber = Number(displayGv) || 0;

                      return (
                        <tr key={person.id} className="hover:bg-slate-50">
                          <td className="py-2.5 px-3 font-sans font-bold text-slate-800">
                            {person.ad} {person.soyad}
                            <div className="text-[10px] text-slate-500 font-mono font-normal">
                              TC: {person.tcNo}
                            </div>
                          </td>
                          <td className="py-2.5 px-3 text-right text-slate-500 font-mono">
                            {formatTL(autoGv)}
                          </td>
                          <td className="py-2.5 px-3">
                            <input
                              type="text"
                              inputMode="decimal"
                              placeholder={autoGv.toFixed(2)}
                              value={displayGv}
                              onChange={(e) => {
                                setManualKumulatifGvMap((prev) => ({
                                  ...prev,
                                  [person.id]: e.target.value,
                                }));
                              }}
                              className="w-full px-2.5 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-mono font-bold text-slate-900 focus:ring-2 focus:ring-indigo-500"
                            />
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>

              <div className="flex flex-col sm:flex-row items-center justify-between gap-3 pt-3 border-t border-slate-200">
                <button
                  type="button"
                  onClick={() => {
                    setManualKumulatifGvMap({});
                  }}
                  className="px-3 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition-colors w-full sm:w-auto"
                >
                  Otomatik Hesaplanan Değerlere Sıfırla
                </button>

                <div className="flex gap-2 w-full sm:w-auto justify-end">
                  <button
                    type="button"
                    onClick={() => setIsKumulatifModalOpen(false)}
                    className="px-4 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition-colors"
                  >
                    Kapat
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        let updatedAny = false;
                        for (const pId of Object.keys(manualKumulatifGvMap)) {
                          const person = personeller.find((p) => p.id === pId);
                          if (person && onSavePersonel) {
                            const val = manualKumulatifGvMap[pId].trim() || '0';
                            if (!isExactDecimalString(val) || val.startsWith('-')) {
                              throw new Error('Kümülatif GV matrahı geçerli, negatif olmayan bir tutar olmalıdır.');
                            }
                            const exactPerson = mergePayrollUiIntoBoundary(
                              authoritativeDataset.personnel.find((item) => item.id === person.id),
                              {
                                ...person,
                                devirKumulatifGvMatrahi: val,
                              }
                            ) as PayrollBoundaryPersonel;
                            await onSavePersonel({
                              ...exactPerson,
                              devirKumulatifGvMatrahiYili: aktifDonem.taxYear ?? (aktifDonem.ay === 12 ? aktifDonem.yil + 1 : aktifDonem.yil),
                              devirKumulatifGvMatrahiBaslangicAyi: aktifDonem.ay,
                            } as PayrollBoundaryPersonel);
                            // The App owns persistence in both native and browser
                            // modes. Do not swallow an opening-table write failure.
                            if (onSaveTaxOpening) {
                              await onSaveTaxOpening({
                                id: `${person.id}_${aktifDonem.taxYear ?? (aktifDonem.ay === 12 ? aktifDonem.yil + 1 : aktifDonem.yil)}`,
                                personnelId: person.id,
                                year: aktifDonem.taxYear ?? (aktifDonem.ay === 12 ? aktifDonem.yil + 1 : aktifDonem.yil),
                                gvCumulativeOpening: val,
                                effectiveFromPeriodId: aktifDonem.id,
                              });
                            }
                            updatedAny = true;
                          }
                        }
                        await handleCalculateAll();
                        setIsKumulatifModalOpen(false);
                        if (updatedAny) {
                          setSuccessMessage(
                            `${aktifDonem.yil} yılı kümülatif GV başlangıç matrahı güncellendi. Bu yıla ait bordrolar yeniden hesaplandı.`
                          );
                        }
                      } catch (err) {
                        setErrorMessage(`Kümülatif matrah kaydedilemedi: ${String(err)}`);
                      }
                    }}
                    className="px-5 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-bold shadow-md transition-colors flex items-center gap-1.5 cursor-pointer"
                  >
                    <Sparkles className="w-4 h-4 text-amber-300" />
                    <span>Kaydet ve Bordroları Yeniden Hesapla</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
