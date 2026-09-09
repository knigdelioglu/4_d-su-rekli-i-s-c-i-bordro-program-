import { useLayoutEffect, useRef, useState, type FormEvent, type MouseEvent } from 'react';
import { comparePaymentEvents, nextPaymentSequence } from '../services/payrollEngine/paymentEventOrder';
import {
  AccrualType,
  BordroDonemi,
  BordroKaydi,
  Personel,
  PersonelPuantaj,
} from '../types/payroll';
import { formatTL, getDefaultAccrualPaymentDate } from '../utils/payrollPresentation';
import {
  getPayrollEngine,
  PayrollBoundaryAccrualInput,
  PayrollBoundaryPayroll,
  PayrollCalculationRequest,
  PayrollDatasetSnapshot,
} from '../services/payrollEngine';
import {
  isExactDecimalString,
  toPayrollUiModel,
} from '../services/payrollEngine/decimalBoundary';

function formatActionableParameterError(message: string): string {
  const annualYear = message.match(/\b(20\d{2})\b/)?.[1];
  if (
    annualYear &&
    /(yıllık bordro parametreleri|sigorta gv yıllık)/i.test(message) &&
    /(eksik|bulunamadı|yok)/i.test(message)
  ) {
    return `${annualYear} yılı gelir vergisi tarifesi henüz tanımlı değil. Yıllık Parametreler bölümünü tamamlayın.`;
  }
  if (
    /(zorunlu yasal parametre|yasal parametresi|kurum ayarları|yasal parametre baseline)/i.test(message) &&
    /(eksik|yok|bulunamadı)/i.test(message)
  ) {
    return 'Dönem yasal parametreleri henüz tamamlanmamış. Dönem Parametreleri bölümünü tamamlayın.';
  }
  if (/(günlük taban ücret|iş primi grupları)/i.test(message) && /(eksik|geçerli)/i.test(message)) {
    return 'Dönem kurum ücretleri henüz tamamlanmamış. Ücretler bölümünde kurum değerlerini tamamlayın.';
  }
  return message;
}

export function formatPayrollError(err: unknown): string {
  if (err && typeof err === 'object') {
    const tagged = err as { type?: string; message?: unknown };
    if (tagged.type === 'NegativeNetPayment' && tagged.message && typeof tagged.message === 'object') {
      const details = tagged.message as { gelir?: number; kesinti?: number; fark?: number };
      return `Kesintiler geliri aşıyor. Gelir: ${formatTL(details.gelir ?? 0)}, kesinti: ${formatTL(details.kesinti ?? 0)}, açık: ${formatTL(details.fark ?? 0)}.`;
    }
    if (typeof tagged.message === 'string') return formatActionableParameterError(tagged.message);
    try {
      return JSON.stringify(err);
    } catch {
      return String(err);
    }
  }
  return formatActionableParameterError(String(err));
}

export const ACCRUAL_TYPE_LABELS: Record<AccrualType, string> = {
  NORMAL: 'Normal Maaş',
  TEDIYE: 'Tediye',
  TIS_IKRAMIYE: 'TİS İkramiyesi',
  SUPPLEMENTAL: 'Ek Ödeme',
  RETRO_ADJUSTMENT: 'Geriye Dönük Fark',
};

export type SupplementaryAccrualType = Exclude<AccrualType, 'NORMAL'>;

export type PayrollRowFilter =
  | 'all'
  | 'attendanceMissing'
  | 'notCalculated'
  | 'stale'
  | 'calculated'
  | 'finalized';

export interface SupplementaryAccrualDraft {
  accrualType: SupplementaryAccrualType;
  paymentDate: string;
  grossAmount: string;
  description: string;
}

interface UseBordroCalculationControllerOptions {
  aktifDonem: BordroDonemi;
  activeAccrualType: AccrualType;
  activeViewTitle: string;
  authoritativeDataset: PayrollDatasetSnapshot;
  bordrolar: BordroKaydi[];
  personeller: Personel[];
  puantajlar: PersonelPuantaj[];
  isSupplementaryView: boolean;
  onSaveBordro: (bordro: PayrollBoundaryPayroll) => Promise<void> | void;
}

export function useBordroCalculationController({
  aktifDonem,
  activeAccrualType,
  activeViewTitle,
  authoritativeDataset,
  bordrolar,
  personeller,
  puantajlar,
  isSupplementaryView,
  onSaveBordro,
}: UseBordroCalculationControllerOptions) {
  const payrollEngine = getPayrollEngine();
  const authoritativeDatasetRef = useRef(authoritativeDataset);
  useLayoutEffect(() => {
    authoritativeDatasetRef.current = authoritativeDataset;
  }, [authoritativeDataset]);
  const [searchTerm, setSearchTerm] = useState('');
  const [rowFilter, setRowFilter] = useState<PayrollRowFilter>('all');
  const [activePaySlip, setActivePaySlip] = useState<{
    personel: Personel;
    bordro: BordroKaydi;
  } | null>(null);
  const [deletingAccrualId, setDeletingAccrualId] = useState<string | null>(null);
  const [isBatchProcessing, setIsBatchProcessing] = useState(false);
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
  const [manualKumulatifGvMap, setManualKumulatifGvMap] = useState<Record<string, string>>({});
  const [manualKumulatifAsgariGvMap, setManualKumulatifAsgariGvMap] = useState<Record<string, string>>({});
  const [isKumulatifModalOpen, setIsKumulatifModalOpen] = useState(false);

  const getAccrualId = (payroll: BordroKaydi): string => payroll.accrualId || payroll.id;

  const getPersonAccruals = (personId: string): BordroKaydi[] =>
    bordrolar
      .filter((item) => item.personelId === personId && item.donemId === aktifDonem.id)
      .sort((a, b) => comparePaymentEvents(a, b, aktifDonem));

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
      sequence: exactPayroll?.sequence ?? existingPayroll?.sequence ?? nextPaymentSequence(
        authoritativeDataset,
        personId,
        aktifDonem,
        normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)
      ),
      grossAmount: null,
      description: exactPayroll?.accrualDescription ?? existingPayroll?.accrualDescription ?? null,
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

  const buildDataset = (): PayrollDatasetSnapshot => authoritativeDatasetRef.current;

  const calculateAndSaveForPerson = async (person: Personel): Promise<BordroKaydi | null> => {
    const pPuantaj = puantajlar.find(
      (p) => p.personelId === person.id && p.donemId === aktifDonem.id
    );
    if (!pPuantaj || !pPuantaj.gunler || Object.keys(pPuantaj.gunler).length === 0) return null;

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

  const handleCalculateAll = async () => {
    setIsBatchProcessing(true);
    let successCount = 0;
    let failCount = 0;
    const missingPuantajPersons: string[] = [];
    for (const person of personeller) {
      const res = await calculateAndSaveForPerson(person);
      if (res) successCount++;
      else {
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
      setErrorMessage('Hiçbir personelin bordrosu hesaplanamadı. Kayıtlı puantaj, dönem kurum ayarları ve yıllık vergi parametrelerini kontrol edin.');
    }
  };

  const handleOpenPaySlip = async (person: Personel, requestedBordro?: BordroKaydi) => {
    let bordro = requestedBordro || getActiveViewPayroll(person.id);
    if (bordro?.status === 'STALE') {
      setErrorMessage(`${person.ad} ${person.soyad} bordrosu önceki dönem değişikliği nedeniyle güncelliğini yitirdi. Bordro zarfını açmadan/yazdırmadan önce yeniden hesaplayın.`);
      return;
    }
    if (bordro?.status === 'DRAFT') {
      setErrorMessage(`${person.ad} ${person.soyad} bordrosu taslak durumda. Önce bordroyu hesaplayın.`);
      return;
    }
    if (!bordro) {
      if (isSupplementaryView) {
        setErrorMessage(`${person.ad} ${person.soyad} için ${activeViewTitle.toLocaleLowerCase('tr-TR')} kaydı henüz yok. Önce tahakkuk ekleyin.`);
        return;
      }
      const hasPuantaj = puantajlar.some(
        (p) => p.personelId === person.id && p.donemId === aktifDonem.id
      );
      if (!hasPuantaj) {
        setErrorMessage(`HATA: ${person.ad} ${person.soyad} için bu dönemde kayıtlı puantaj bulunmadığından bordro zarfı açılamıyor.`);
        return;
      }
      bordro = (await calculateAndSaveForPerson(person)) || undefined;
    }
    if (bordro) setActivePaySlip({ personel: person, bordro });
  };

  const openSupplementaryAccrualForm = (person: Personel) => {
    setNewAccrualPersonId(person.id);
    setExpandedTimelinePersonId(person.id);
    setSupplementaryAccrualDraft({
      accrualType: activeAccrualType === 'NORMAL' ? 'TEDIYE' : (activeAccrualType as SupplementaryAccrualType),
      paymentDate: getDefaultAccrualPaymentDate(aktifDonem),
      grossAmount: '',
      description: '',
    });
    setErrorMessage(null);
  };

  const handleRecalculateAccrual = async (person: Personel, accrual: BordroKaydi, event: MouseEvent) => {
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

  const handleCalculateSupplementary = async (person: Personel, event: FormEvent<HTMLFormElement>) => {
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
    const nextSequence = nextPaymentSequence(authoritativeDataset, person.id, aktifDonem, paymentDate);
    const accrual: PayrollBoundaryAccrualInput = {
      accrualId: `${person.id}_${aktifDonem.id}_${supplementaryAccrualDraft.accrualType.toLowerCase()}_${paymentDate}_${nextSequence}`,
      accrualType: supplementaryAccrualDraft.accrualType,
      paymentDate,
      sequence: nextSequence,
      grossAmount,
      description: supplementaryAccrualDraft.description.trim() || null,
    };
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
      setSuccessMessage(`${person.ad} ${person.soyad} için ${ACCRUAL_TYPE_LABELS[accrual.accrualType]} tahakkuku hesaplandı.`);
      setErrorMessage(null);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (err) {
      console.error('Supplementary payroll calculation failed:', err);
      setErrorMessage(`Tahakkuk hesaplama hatası: ${formatPayrollError(err)}`);
    }
  };

  const handleCalculateSingle = async (person: Personel, event: MouseEvent, requestedAccrual?: BordroKaydi) => {
    event.stopPropagation();
    if (isSupplementaryView) {
      if (requestedAccrual) await handleRecalculateAccrual(person, requestedAccrual, event);
      else openSupplementaryAccrualForm(person);
      return;
    }
    const hasPuantaj = puantajlar.some(
      (p) => p.personelId === person.id && p.donemId === aktifDonem.id
    );
    if (!hasPuantaj) {
      setSuccessMessage(null);
      setErrorMessage(`HATA: ${person.ad} ${person.soyad} için bu dönemde (${aktifDonem.donemAdi}) kayıtlı puantaj bulunamadı! Puantajsız bordro hesaplanamaz. Lütfen önce Puantaj Cetvelinden puantaj girişi yapın.`);
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
        return payrolls.length === 0 || payrolls.some(matchesSupplementaryAccrualStatus);
      }
      return payrolls.some(matchesSupplementaryAccrualStatus);
    }
    const payroll = getActiveViewPayroll(person.id);
    if (rowFilter === 'notCalculated') return hasAttendance && (!payroll || payroll.status === 'DRAFT');
    if (rowFilter === 'stale') return payroll?.status === 'STALE';
    if (rowFilter === 'calculated') return payroll?.status === 'CALCULATED';
    return payroll?.status === 'FINALIZED';
  };

  return {
    payrollEngine,
    searchTerm,
    setSearchTerm,
    rowFilter,
    setRowFilter,
    activePaySlip,
    setActivePaySlip,
    deletingAccrualId,
    setDeletingAccrualId,
    isBatchProcessing,
    successMessage,
    setSuccessMessage,
    errorMessage,
    setErrorMessage,
    normalPaymentDateMap,
    setNormalPaymentDateMap,
    newAccrualPersonId,
    setNewAccrualPersonId,
    expandedTimelinePersonId,
    setExpandedTimelinePersonId,
    supplementaryAccrualDraft,
    setSupplementaryAccrualDraft,
    manualKumulatifGvMap,
    setManualKumulatifGvMap,
    manualKumulatifAsgariGvMap,
    setManualKumulatifAsgariGvMap,
    isKumulatifModalOpen,
    setIsKumulatifModalOpen,
    getAccrualId,
    getPersonAccruals,
    getActiveViewAccruals,
    getActiveViewPayroll,
    getDevirGvMatrahiForActiveYear,
    buildDataset,
    handleCalculateAll,
    handleOpenPaySlip,
    openSupplementaryAccrualForm,
    handleRecalculateAccrual,
    handleCalculateSupplementary,
    handleCalculateSingle,
    handleFinalizeSuccess,
    matchesSupplementaryAccrualStatus,
    matchesRowFilter,
  };
}
