/**
 * Bordro Hesaplama Ekranı — Temiz, Otomatik ve Hızlı Bordro Yönetimi
 */

import React from 'react';
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
  MoreHorizontal,
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
import {
  formatCompactPuantaj,
  formatTL,
  getDefaultAccrualPaymentDate,
} from '../utils/payrollPresentation';
import { PaySlipModal } from './PaySlipModal';
import { PayrollFinalizeModal } from './PayrollFinalizeModal';
import {
  PayrollBoundaryPayroll,
  PayrollBoundaryPersonel,
  PayrollBoundaryTaxOpening,
  PayrollDatasetSnapshot,
} from '../services/payrollEngine';
import {
  countAuthoritativeNormalPersonnel,
  getPayrollStatusLabel,
  getOtherPeriodAccruals,
} from './Listeler/accrualListData';
import {
  isExactDecimalString,
  mergePayrollUiIntoBoundary,
} from '../services/payrollEngine/decimalBoundary';
import {
  ACCRUAL_TYPE_LABELS,
  isPayrollTaxOpeningConfigurationError,
  type PayrollRowFilter,
  type SupplementaryAccrualType,
  useBordroCalculationController,
} from './useBordroCalculationController';
import { describeError } from '../utils/errorMessage';
import { matchesPersonSearch } from '../utils/personSearch';

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
  onDeleteBordro: (bordro: BordroKaydi) => Promise<void>;
  onSaveBordro: (
    bordro: PayrollBoundaryPayroll,
    calculationSnapshot?: PayrollDatasetSnapshot
  ) => Promise<void> | void;
  onSavePersonelAndTaxOpening: (
    personel: Personel | PayrollBoundaryPersonel,
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
  onDeleteBordro,
  onSavePersonelAndTaxOpening,
  onGoToPuantaj,
}) => {
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
  const {
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
    isSupplementaryPaymentScopePending,
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
  } = useBordroCalculationController({
    aktifDonem,
    activeAccrualType,
    activeViewTitle,
    authoritativeDataset,
    bordrolar,
    personeller,
    puantajlar,
    isSupplementaryView,
    onSaveBordro,
  });

  const rowFilters: Array<{ id: PayrollRowFilter; label: string }> = [
    { id: 'all', label: 'Tümü' },
    { id: 'attendanceMissing', label: 'Puantaj Eksik' },
    { id: 'notCalculated', label: 'Hesaplanmadı' },
    { id: 'stale', label: 'Yeniden Hesaplanmalı' },
    { id: 'calculated', label: 'Hesaplandı' },
    { id: 'finalized', label: 'Kesinleştirildi' },
  ];
  const taxOpeningConfigurationError = Boolean(
    errorMessage && isPayrollTaxOpeningConfigurationError(errorMessage)
  );

  const [openMenuPersonId, setOpenMenuPersonId] = React.useState<string | null>(null);
  const [activeFinalize, setActiveFinalize] = React.useState<{ person: Personel; bordro: BordroKaydi } | null>(null);
  const [confirmDeleteAccrualId, setConfirmDeleteAccrualId] = React.useState<string | null>(null);
  const actionMenuRef = React.useRef<HTMLDivElement | null>(null);

  React.useEffect(() => {
    if (!openMenuPersonId) return;
    const handlePointerDown = (event: MouseEvent) => {
      if (actionMenuRef.current && !actionMenuRef.current.contains(event.target as Node)) {
        setOpenMenuPersonId(null);
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpenMenuPersonId(null);
    };
    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKey);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKey);
    };
  }, [openMenuPersonId]);

  // Filtered personnel list
  const filteredPersoneller = personeller.filter(
    (p) => matchesPersonSearch(p, searchTerm) && matchesRowFilter(p)
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
      className="space-y-3"
      data-testid="payroll-screen"
      data-period-id={aktifDonem.id}
      data-payroll-view={activePayrollView}
      data-payroll-engine-kind={payrollEngine.kind}
    >
      {/* Top Banner / Title */}
      <div className="bg-slate-900 rounded-xl px-5 py-3 text-white shadow-md flex flex-col md:flex-row md:items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-bold tracking-tight text-white flex items-center gap-2">
            <Calculator className="w-5 h-5 text-indigo-400" />
            <span>{activeViewTitle}</span>
          </h2>
          <p className="text-[11px] text-slate-300 mt-0.5 max-w-2xl leading-normal">
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
                className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-amber-300 font-semibold text-xs rounded-lg shadow-sm transition-all flex items-center justify-center gap-1.5 border border-slate-700"
                title="Sisteme ilk defa girildiğinde veya yıl ortasında önceki kümülatif vergi matrahlarını elle girmek için tıklayın"
              >
                <Receipt className="w-3.5 h-3.5 text-amber-400" />
                <span>Önceki Kümülatif Matrah Girişi</span>
              </button>

              <button
                onClick={handleCalculateAll}
                disabled={isBatchProcessing || personeller.length === 0}
                className="px-4 py-1.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-semibold text-xs rounded-lg shadow-sm transition-all flex items-center justify-center gap-1.5 whitespace-nowrap"
              >
                {isBatchProcessing ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Sparkles className="w-3.5 h-3.5 text-amber-300" />
                )}
                <span>Tüm Hesaplanabilir Bordroları Hesapla ({personeller.length})</span>
              </button>
            </>
          )}
        </div>
      </div>

      {!isSupplementaryView && <div className="flex flex-col gap-2 rounded-xl border border-indigo-100 bg-indigo-50/60 px-3.5 py-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <div className="text-xs font-bold uppercase tracking-wide text-indigo-900">
            Normal Maaş Ödeme / Tahakkuk Tarihi
          </div>
          <p className="mt-0.5 text-[11px] text-indigo-800">
            Yeni normal maaş bordrosu için kullanılacak tarihtir. Kaydedilmiş tahakkukların tarihi değiştirilemez.
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
            className="rounded-lg border border-indigo-200 bg-white px-2.5 py-1 text-xs font-semibold text-slate-900 focus:ring-2 focus:ring-indigo-500"
          />
        </label>
      </div>}

      {isSupplementaryView && activeAccrualType !== 'SUPPLEMENTAL' && (
        <div
          data-testid="accrual-reference-banner"
          className="flex flex-col gap-2 rounded-xl border border-indigo-100 bg-indigo-50/60 px-3.5 py-2 sm:flex-row sm:items-center sm:justify-between"
        >
          <div>
            <div className="text-xs font-bold uppercase tracking-wide text-indigo-900">
              {activeViewTitle} takvim bağlantısı
            </div>
            <p className="mt-0.5 text-[11px] text-indigo-800">
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
        <div className="px-3.5 py-2 bg-emerald-50 border border-emerald-200 text-emerald-900 rounded-xl text-xs font-semibold flex items-center gap-2 animate-fade-in shadow-xs">
          <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
          <span>{successMessage}</span>
        </div>
      )}

      {errorMessage && (
        <div className="px-3.5 py-2 bg-rose-50 border border-rose-200 text-rose-900 rounded-xl text-xs font-semibold flex flex-col sm:flex-row items-start sm:items-center justify-between gap-2.5 animate-fade-in shadow-xs">
          <div className="flex items-center gap-2">
            <AlertTriangle className="w-4 h-4 text-rose-600 shrink-0" />
            <span>{errorMessage}</span>
          </div>
          {taxOpeningConfigurationError ? (
            <button
              onClick={() => {
                setErrorMessage(null);
                setIsKumulatifModalOpen(true);
              }}
              className="px-2.5 py-1 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-bold shrink-0 transition-colors flex items-center gap-1 shadow-xs"
            >
              <Receipt className="w-3.5 h-3.5" />
              <span>Önceki Kümülatif Matrah Girişine Git →</span>
            </button>
          ) : onGoToPuantaj && (
            <button
              onClick={() => onGoToPuantaj()}
              className="px-2.5 py-1 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-bold shrink-0 transition-colors flex items-center gap-1 shadow-xs"
            >
              <CalendarCheck className="w-3.5 h-3.5" />
              <span>Puantaj Cetveline Git →</span>
            </button>
          )}
        </div>
      )}

      {/* KPI Stats Grid */}
      <div
        data-testid="payroll-summary-cards"
        className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-2.5"
      >
        <div
          data-testid="kpi-card-personnel"
          className="bg-white px-3 py-2 rounded-xl border border-slate-200 shadow-2xs flex items-center gap-2.5 min-w-0"
        >
          <div className="w-7 h-7 rounded-lg bg-blue-50 text-blue-700 flex items-center justify-center shrink-0">
            <Users className="w-3.5 h-3.5" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center justify-between gap-1">
              <span className="text-[11px] text-slate-500 font-medium truncate" title="Toplam Personel">Toplam Personel</span>
              <span className="text-[10px] text-blue-600 font-semibold shrink-0">
                {calculatedViewPersonnelCount}/{personeller.length}
              </span>
            </div>
            <div className="text-sm 2xl:text-base font-bold text-slate-900 leading-tight whitespace-nowrap">
              {personeller.length} Kişi
            </div>
            <div
              className="text-[10px] text-slate-500 leading-tight mt-0.5 truncate"
              title={isSupplementaryView
                ? `${activePeriodBordrolar.length} Tahakkuk Kaydı`
                : `${calculatedViewPersonnelCount} / ${personeller.length} ${isSupplementaryView ? 'Kayıt' : 'Hesaplandı'}`}
            >
              {isSupplementaryView
                ? `${activePeriodBordrolar.length} Tahakkuk Kaydı`
                : `${calculatedViewPersonnelCount} / ${personeller.length} ${isSupplementaryView ? 'Kayıt' : 'Hesaplandı'}`}
            </div>
          </div>
        </div>

        <div
          data-testid="kpi-card-gross"
          className="bg-white px-3 py-2 rounded-xl border border-slate-200 shadow-2xs flex items-center gap-2.5 min-w-0"
        >
          <div className="w-7 h-7 rounded-lg bg-indigo-50 text-indigo-700 flex items-center justify-center shrink-0">
            <Wallet className="w-3.5 h-3.5" />
          </div>
          <div className="min-w-0 flex-1">
            <span className="text-[11px] text-slate-500 font-medium block truncate" title="Toplam Brüt Gelir">Toplam Brüt Gelir</span>
            <div className="text-sm 2xl:text-base font-bold text-indigo-900 font-mono tabular-nums leading-tight whitespace-nowrap">
              {formatTL(totalGross)}
            </div>
            <div className="text-[10px] text-slate-500 leading-tight mt-0.5" title="Vergi ve SGK Öncesi">
              Vergi ve SGK Öncesi
            </div>
          </div>
        </div>

        <div
          data-testid="kpi-card-deductions"
          className="bg-white px-3 py-2 rounded-xl border border-slate-200 shadow-2xs flex items-center gap-2.5 min-w-0"
        >
          <div className="w-7 h-7 rounded-lg bg-rose-50 text-rose-700 flex items-center justify-center shrink-0">
            <Receipt className="w-3.5 h-3.5" />
          </div>
          <div className="min-w-0 flex-1">
            <span className="text-[11px] text-slate-500 font-medium block truncate" title="İşçi Kesintileri">İşçi Kesintileri</span>
            <div className="text-sm 2xl:text-base font-bold text-rose-800 font-mono tabular-nums leading-tight whitespace-nowrap">
              {formatTL(totalDeductions)}
            </div>
            <div className="text-[10px] text-slate-500 leading-tight mt-0.5" title="SGK + Vergi + Kesinti">
              SGK + Vergi + Kesinti
            </div>
          </div>
        </div>

        <div
          data-testid="kpi-card-net"
          className="bg-white px-3 py-2 rounded-xl border border-slate-200 shadow-2xs flex items-center gap-2.5 min-w-0"
        >
          <div className="w-7 h-7 rounded-lg bg-emerald-50 text-emerald-700 flex items-center justify-center shrink-0">
            <TrendingUp className="w-3.5 h-3.5" />
          </div>
          <div className="min-w-0 flex-1">
            <span className="text-[11px] text-slate-500 font-medium block truncate" title="Toplam Net Ödeme">Toplam Net Ödeme</span>
            <div className="text-sm 2xl:text-base font-bold text-emerald-700 font-mono tabular-nums leading-tight whitespace-nowrap">
              {formatTL(totalNet)}
            </div>
            <div className="text-[10px] text-emerald-600 font-medium leading-tight mt-0.5" title="Banka Ele Geçen">
              Banka Ele Geçen
            </div>
          </div>
        </div>

        <div
          data-testid="kpi-card-employer-cost"
          className="bg-white px-3 py-2 rounded-xl border border-slate-200 shadow-2xs flex items-center gap-2.5 min-w-0"
        >
          <div className="w-7 h-7 rounded-lg bg-amber-50 text-amber-700 flex items-center justify-center shrink-0">
            <Building2 className="w-3.5 h-3.5" />
          </div>
          <div className="min-w-0 flex-1">
            <span className="text-[11px] text-slate-500 font-medium block truncate" title="İşveren Prim Maliyeti">İşveren Prim Maliyeti</span>
            <div className="text-sm 2xl:text-base font-bold text-amber-900 font-mono tabular-nums leading-tight whitespace-nowrap">
              {formatTL(totalEmployerCost)}
            </div>
            <div className="text-[10px] text-amber-700 font-medium leading-tight mt-0.5" title="Kurum SGK + İşsizlik">
              Kurum SGK + İşsizlik
            </div>
          </div>
        </div>
      </div>

      {/* Main Personnel Payroll List Table */}
      <div className="bg-white rounded-2xl border border-slate-200 shadow-sm overflow-hidden">
        {/* Table Header / Toolbar */}
        <div className="px-4 py-2.5 bg-slate-50 border-b border-slate-200 flex flex-col gap-2">
          <div className="flex flex-col sm:flex-row items-center justify-between gap-2.5">
          <div className="relative w-full sm:w-80">
            <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-2" />
            <input
              type="text"
              placeholder="Personel adı, T.C. No veya Unvan ile ara..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 bg-white border border-slate-300 rounded-lg text-xs text-slate-900 focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
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
                className={`rounded-md border px-2 py-1 text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 ${
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
                {isSupplementaryView && <th className="py-2 px-2">S.No</th>}
                <th className="py-2 px-2">{isSupplementaryView ? 'Personel Adı Soyadı' : 'Personel'}</th>
                {!isSupplementaryView && <th className="py-2 px-1 text-center">Puantaj</th>}
                {isSupplementaryView && <th className="py-2 px-1 text-center">Ödeme Tarihi</th>}
                <th className="py-2 px-1 text-right">Brüt</th>
                {isSupplementaryView ? (
                  <>
                    <th className="py-2 px-1 text-right">SGK</th>
                    <th className="py-2 px-1 text-right">GV</th>
                  </>
                ) : (
                  <th className="py-2 px-1 text-right">Kesinti</th>
                )}
                <th className="py-2 px-1 text-right">{isSupplementaryView ? 'Net Ele Geçen' : 'Net'}</th>
                <th className="py-2 px-1 text-center">Durum</th>
                <th className="py-2 px-1 text-center w-28">İşlemler</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 text-xs text-slate-800">
              {filteredPersoneller.length === 0 ? (
                <tr>
                  <td colSpan={isSupplementaryView ? 9 : 7} className="py-12 text-center text-slate-500">
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
                  const otherPeriodAccruals = isSupplementaryView && allViewAccruals.length === 0
                    ? getOtherPeriodAccruals(bordrolar, donemler, person.id, activeAccrualType, aktifDonem.id)
                    : [];

                  const hasPayrollSnapshot = !!bordro;
                  const isFinalized = bordro?.status === 'FINALIZED';
                  const isStale = bordro?.status === 'STALE';
                  const isDraft = bordro?.status === 'DRAFT';
                  const isCalculated = bordro?.status === 'CALCULATED' || isFinalized;
                  const canAddSupplementary = isSupplementaryView;
                  const brut = bordro?.gelirToplam || 0;
                  const kesinti = bordro?.kesintiToplam || 0;
                  const net = bordro?.netOdeme || 0;

                  return (
                    <React.Fragment key={person.id}>
                    <tr
                      data-testid={isSupplementaryView && bordro ? `accrual-row-${getAccrualId(bordro)}` : `payroll-row-${person.id}`}
                      onClick={(event) => {
                        const target = event.target as HTMLElement | null;
                        if (
                          target?.closest('button') ||
                          target?.closest('[role="menu"]') ||
                          target?.closest('td[data-cell="actions"]')
                        ) {
                          return;
                        }
                        void handleOpenPaySlip(person, bordro);
                      }}
                      className={`transition-colors group ${isStale || isDraft ? 'bg-amber-50/40 cursor-default' : 'hover:bg-indigo-50/50 cursor-pointer'}`}
                    >
                      {isSupplementaryView && (
                        <td className="py-1.5 px-2 font-mono text-slate-400 font-medium whitespace-nowrap">
                          {idx + 1}{viewAccruals.length > 1 ? '.1' : ''}
                        </td>
                      )}

                      {/* Person Name & TC */}
                      <td className="py-1.5 px-2 font-medium">
                        <div className="flex items-center gap-1.5 min-w-0">
                          <div className="w-6 h-6 rounded-full bg-indigo-100 text-indigo-700 font-bold flex items-center justify-center text-[10.5px] shrink-0 group-hover:bg-indigo-600 group-hover:text-white transition-colors">
                            {person.ad.charAt(0)}
                            {person.soyad.charAt(0)}
                          </div>
                          <div className="min-w-0 flex-1">
                            <div className="font-bold text-slate-900 group-hover:text-indigo-700 transition-colors flex items-center gap-1 leading-tight">
                              <span className="truncate max-w-[150px]" title={`${person.ad} ${person.soyad}`}>{person.ad} {person.soyad}</span>
                              {!isStale && !isDraft && <ChevronRight className="w-3 h-3 text-slate-400 group-hover:text-indigo-600 opacity-0 group-hover:opacity-100 transition-opacity shrink-0" />}
                            </div>
                            <div className="flex flex-wrap items-center gap-x-1.5 gap-y-0.5 font-mono text-[10.5px] text-slate-500 leading-tight mt-0.5">
                              <span>TC: {person.tcNo}</span>
                              <span>·</span>
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
                                title="Ödeme Geçmişi"
                                aria-label="Ödeme Geçmişi"
                                className="text-left text-[10px] font-semibold text-indigo-600 hover:text-indigo-800 hover:underline focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500 whitespace-nowrap cursor-pointer"
                              >
                                Geçmiş {expandedTimelinePersonId === person.id ? '▴' : '▾'}
                              </button>
                            </div>
                            {isSupplementaryView && bordro?.accrualDescription && (
                              <div className="max-w-48 truncate text-[10px] font-medium text-slate-500 mt-0.5" title={bordro.accrualDescription}>
                                {bordro.accrualDescription}
                              </div>
                            )}
                          </div>
                        </div>
                      </td>

                      {/* Compact attendance summary */}
                      {!isSupplementaryView && <td className="py-1.5 px-1 text-center">
                        {bordro ? (
                          <span
                            className="inline-flex max-w-[120px] items-center gap-1 text-[10px] font-mono font-semibold"
                            title={`Puantaj: ${formatCompactPuantaj(bordro.puantajOzeti)}`}
                            aria-label={`Puantaj: ${formatCompactPuantaj(bordro.puantajOzeti)}`}
                          >
                            <CheckCircle2 className="h-3 w-3 shrink-0 text-emerald-600" aria-hidden="true" />
                            <span className="truncate">{formatCompactPuantaj(bordro.puantajOzeti)}</span>
                          </span>
                        ) : hasPuantaj ? (
                          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-semibold bg-amber-50 text-amber-800 border border-amber-200 whitespace-nowrap">
                            <CalendarCheck className="w-3 h-3 text-amber-600" />
                            <span>Puantaj Girildi</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 text-rose-600 text-[10.5px] font-bold whitespace-nowrap">
                            <AlertTriangle className="w-3 h-3 text-rose-500" />
                            <span>Puantaj Yok</span>
                          </span>
                        )}
                      </td>}

                      {/* Ödeme/tahakkuk tarihi — supplementary views only */}
                      {isSupplementaryView && <td className="py-1.5 px-1 text-center font-mono text-[11px] whitespace-nowrap">
                        <div className="font-bold text-slate-800">
                          {bordro ? bordro.paymentDate || getDefaultAccrualPaymentDate(aktifDonem) : '—'}
                        </div>
                        {!bordro && <div className="text-[10px] text-slate-500">{activeViewTitle} için</div>}
                      </td>}

                      {/* Brüt */}
                      <td className={`py-1.5 px-1 text-right font-mono font-medium text-xs whitespace-nowrap ${isStale ? 'text-amber-700 line-through' : 'text-slate-800'}`}>
                        {hasPayrollSnapshot ? formatTL(brut) : '—'}
                      </td>

                      {/* Kesintiler / supplementary breakdown */}
                      {isSupplementaryView ? (
                        <>
                          <td className={`py-1.5 px-1 text-right font-mono font-medium text-xs whitespace-nowrap ${isStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                            {hasPayrollSnapshot ? formatTL(bordro?.kesintiler.isciSgkPrimi ?? 0) : '—'}
                          </td>
                          <td className={`py-1.5 px-1 text-right font-mono font-medium text-xs whitespace-nowrap ${isStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                            {hasPayrollSnapshot ? formatTL(bordro?.kesintiler.gelirVergisi ?? 0) : '—'}
                          </td>
                        </>
                      ) : (
                        <td className={`py-1.5 px-1 text-right font-mono font-medium text-xs whitespace-nowrap ${isStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                          {hasPayrollSnapshot ? formatTL(kesinti) : '—'}
                        </td>
                      )}

                      {/* Net */}
                      <td className={`py-1.5 px-1 text-right font-mono font-bold text-xs whitespace-nowrap ${isStale ? 'text-amber-700 line-through' : 'text-emerald-700'}`}>
                        {hasPayrollSnapshot ? formatTL(net) : '—'}
                      </td>

                      {/* Durum */}
                      <td className="py-1.5 px-1 text-center whitespace-nowrap">
                        {isFinalized ? (
                          <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-slate-200 text-slate-800 border border-slate-300">
                            <CheckCircle2 className="w-2.5 h-2.5 text-slate-700" />
                            <span>Kesinleştirildi</span>
                          </span>
                        ) : isStale ? (
                          <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-amber-100 text-amber-900 border border-amber-300">
                            <AlertTriangle className="w-2.5 h-2.5 text-amber-700" />
                            <span>Yeniden Hesaplanmalı</span>
                          </span>
                        ) : isDraft ? (
                          <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-slate-100 text-slate-700 border border-slate-300">
                            <Clock className="w-2.5 h-2.5 text-slate-600" />
                            <span>Taslak</span>
                          </span>
                        ) : isCalculated ? (
                          <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-100 text-emerald-800 border border-emerald-200">
                            <CheckCircle2 className="w-2.5 h-2.5 text-emerald-600" />
                            <span>Hesaplandı</span>
                          </span>
                        ) : isSupplementaryView || hasPuantaj ? (
                          <div>
                            <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[10px] font-semibold bg-amber-50 text-amber-800 border border-amber-200">
                              <Clock className="w-2.5 h-2.5 text-amber-600" />
                              <span>{isSupplementaryView ? 'Tahakkuk Eklenmedi' : 'Hesaplanmadı'}</span>
                            </span>
                            {isSupplementaryView && allViewAccruals.length === 0 && (
                              <div data-testid={`other-period-accruals-${person.id}`} className="mt-1 max-w-60 whitespace-normal text-left text-[10px] text-slate-600">
                                {otherPeriodAccruals.length === 0 ? 'Diğer dönemlerde de bu tür tahakkuk yok.' : (
                                  <details>
                                    <summary className="cursor-pointer">Diğer dönemlerde {otherPeriodAccruals.length} kayıt</summary>
                                    {otherPeriodAccruals.map(({ payroll, periodLabel }) => (
                                      <div key={getAccrualId(payroll)} className="mt-1">
                                        {periodLabel} · Ödeme tarihi: {payroll.paymentDate || (donemler.find((period) => period.id === payroll.donemId)?.bitisTarihi || 'Tarih kayıtlı değil')} · Brüt {formatTL(payroll.gelirToplam)} · {getPayrollStatusLabel(payroll.status)}
                                      </div>
                                    ))}
                                    <div className="mt-1">İşlem için ilgili çalışma dönemini seçin.</div>
                                  </details>
                                )}
                              </div>
                            )}
                          </div>
                        ) : (
                          <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-rose-100 text-rose-800 border border-rose-200">
                            <AlertTriangle className="w-2.5 h-2.5 text-rose-600" />
                            <span>Puantaj Eksik</span>
                          </span>
                        )}
                      </td>

                      {/* Actions */}
                      <td
                        data-cell="actions"
                        className="py-1.5 px-1 text-center whitespace-nowrap"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <div ref={openMenuPersonId === person.id ? actionMenuRef : undefined} className="relative inline-flex items-center justify-center gap-1">
                          {isSupplementaryView || hasPuantaj ? (
                            <>
                              {!isFinalized ? (
                                <>
                                  <button
                                    type="button"
                                    data-testid={isSupplementaryView && bordro ? `recalculate-accrual-${getAccrualId(bordro)}` : `calculate-payroll-${person.id}`}
                                    onClick={(e) => handleCalculateSingle(person, e, bordro)}
                                    title={isSupplementaryView
                                      ? (bordro ? 'Bu tahakkuku yeniden hesapla' : `${activeViewTitle} tahakkuku ekle`)
                                      : isStale ? 'Güncelliğini yitiren bordroyu yeniden hesapla' : isCalculated ? 'Bordroyu yeniden hesapla' : 'Bordroyu Hesapla'}
                                    aria-label={isSupplementaryView
                                      ? (bordro ? 'Yeniden Hesapla' : `${activeViewTitle} Ekle`)
                                      : isStale || isCalculated ? 'Yeniden Hesapla' : 'Hesapla'}
                                    className="px-1.5 py-1 bg-slate-100 hover:bg-indigo-600 hover:text-white text-slate-700 rounded-md transition-colors text-[10.5px] font-semibold flex items-center gap-0.5 whitespace-nowrap cursor-pointer"
                                  >
                                    <RefreshCw className="w-3 h-3" />
                                    <span>{isSupplementaryView ? (bordro ? 'Yeniden' : 'Ekle') : isStale || isCalculated ? 'Yeniden' : 'Hesapla'}</span>
                                  </button>

                                  {bordro && (
                                    <>
                                      <button
                                        type="button"
                                        data-testid={`row-more-actions-${person.id}`}
                                        aria-label="Diğer işlemler"
                                        aria-haspopup="menu"
                                        aria-expanded={openMenuPersonId === person.id}
                                        title="Diğer işlemler"
                                        onClick={(e) => {
                                          e.preventDefault();
                                          e.stopPropagation();
                                          setOpenMenuPersonId((prev) => (prev === person.id ? null : person.id));
                                        }}
                                        className="p-1 text-slate-500 hover:text-slate-900 hover:bg-slate-100 rounded-md transition-colors border border-slate-200 cursor-pointer"
                                      >
                                        <MoreHorizontal className="w-3.5 h-3.5" />
                                      </button>

                                      {openMenuPersonId === person.id && (
                                        <div
                                          role="menu"
                                          aria-label="Diğer işlemler menüsü"
                                          className="absolute right-0 top-full z-30 mt-1 min-w-[130px] rounded-lg border border-slate-200 bg-white py-1 shadow-lg text-left"
                                          onClick={(e) => e.stopPropagation()}
                                        >
                                          <button
                                            type="button"
                                            role="menuitem"
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              setOpenMenuPersonId(null);
                                              void handleOpenPaySlip(person, bordro);
                                            }}
                                            title="Bordro Zarfını Görüntüle & Yazdır"
                                            aria-label="Bordro Gör"
                                            className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-[11px] font-medium text-slate-700 hover:bg-indigo-50 hover:text-indigo-700 transition-colors cursor-pointer"
                                          >
                                            <FileText className="w-3.5 h-3.5 text-indigo-600" />
                                            <span>Bordro Gör</span>
                                          </button>

                                          <button
                                            type="button"
                                            role="menuitem"
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              setOpenMenuPersonId(null);
                                              setActiveFinalize({ person, bordro });
                                            }}
                                            title="Bordroyu kontrol ederek kesinleştir"
                                            aria-label="Kesinleştir"
                                            className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-[11px] font-medium text-amber-800 hover:bg-amber-50 transition-colors cursor-pointer"
                                          >
                                            <CheckCircle2 className="w-3.5 h-3.5 text-amber-600" />
                                            <span>Kesinleştir</span>
                                          </button>

                                          {isSupplementaryView && canAddSupplementary && (
                                            <button
                                              type="button"
                                              role="menuitem"
                                              onClick={(e) => {
                                                e.stopPropagation();
                                                setOpenMenuPersonId(null);
                                                openSupplementaryAccrualForm(person);
                                              }}
                                              title={`Aynı kişiye yeni ${activeViewTitle} tahakkuku ekle`}
                                              aria-label={`Yeni ${activeViewTitle} Ekle`}
                                              className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-[11px] font-medium text-indigo-700 hover:bg-indigo-50 transition-colors cursor-pointer"
                                            >
                                              <Plus className="w-3.5 h-3.5 text-indigo-600" />
                                              <span>Yeni Ekle</span>
                                            </button>
                                          )}
                                        </div>
                                      )}
                                    </>
                                  )}
                                </>
                              ) : (
                                <button
                                  type="button"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    void handleOpenPaySlip(person, bordro);
                                  }}
                                  title="Bordro Zarfını Görüntüle & Yazdır"
                                  aria-label="Bordro Zarfını Görüntüle"
                                  className="px-2 py-1 bg-indigo-50 text-indigo-700 hover:bg-indigo-600 hover:text-white rounded-md transition-colors text-[10.5px] font-semibold flex items-center gap-1 whitespace-nowrap cursor-pointer"
                                >
                                  <FileText className="w-3 h-3" />
                                  <span>Bordro Gör</span>
                                </button>
                              )}
                            </>
                          ) : (
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                if (onGoToPuantaj) onGoToPuantaj(person.id);
                              }}
                              title="Puantaj Cetveline Git ve Puantaj Gir"
                              aria-label="Puantaj Cetveline Git"
                              className="px-2 py-1 bg-rose-50 hover:bg-rose-600 hover:text-white text-rose-700 border border-rose-200 rounded-md transition-colors text-[10.5px] font-bold flex items-center gap-1 whitespace-nowrap cursor-pointer"
                            >
                              <CalendarCheck className="w-3 h-3" />
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
                              {ACCRUAL_TYPE_LABELS[accrual.accrualType]}
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
                              {isSupplementaryView || hasPuantaj ? (
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
                      <td data-testid={`accrual-timeline-${person.id}`} colSpan={isSupplementaryView ? 9 : 7} className="px-4 py-3 bg-slate-50/80">
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
                                openSupplementaryAccrualForm(person);
                              }}
                              title="Bağımsız ödeme tahakkuku oluştur."
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
                                    {accrual.status !== 'FINALIZED' && (
                                      confirmDeleteAccrualId === getAccrualId(accrual) ? (
                                        <div className="inline-flex items-center gap-1">
                                          <button
                                            type="button"
                                            disabled={deletingAccrualId !== null || isBatchProcessing}
                                            className="rounded-lg bg-rose-600 px-2 py-1 text-[10px] font-bold text-white hover:bg-rose-700 disabled:opacity-50 cursor-pointer"
                                            onClick={async (event) => {
                                              event.stopPropagation();
                                              setConfirmDeleteAccrualId(null);
                                              setDeletingAccrualId(getAccrualId(accrual));
                                              try {
                                                await onDeleteBordro(accrual);
                                                setSuccessMessage('Tahakkuk başarıyla silindi.');
                                                setErrorMessage(null);
                                                setTimeout(() => setSuccessMessage(null), 3000);
                                              } catch (error) {
                                                setErrorMessage(describeError(error));
                                              } finally {
                                                setDeletingAccrualId(null);
                                              }
                                            }}
                                          >
                                            {deletingAccrualId === getAccrualId(accrual) ? 'Siliniyor…' : 'Silmeyi Onayla'}
                                          </button>
                                          <button
                                            type="button"
                                            disabled={deletingAccrualId !== null}
                                            className="rounded-lg bg-slate-100 px-1.5 py-1 text-[10px] font-semibold text-slate-700 hover:bg-slate-200 cursor-pointer"
                                            onClick={(event) => {
                                              event.stopPropagation();
                                              setConfirmDeleteAccrualId(null);
                                            }}
                                          >
                                            Vazgeç
                                          </button>
                                        </div>
                                      ) : (
                                        <button
                                          type="button"
                                          disabled={deletingAccrualId !== null || isBatchProcessing}
                                          className="rounded-lg px-2 py-1 text-[10px] font-bold text-rose-700 hover:bg-rose-50 disabled:opacity-50 cursor-pointer"
                                          onClick={(event) => {
                                            event.stopPropagation();
                                            setConfirmDeleteAccrualId(getAccrualId(accrual));
                                          }}
                                        >
                                          Tahakkuku Sil
                                        </button>
                                      )
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
                                  disabled={isSupplementaryPaymentScopePending(
                                    person.id,
                                    supplementaryAccrualDraft.paymentDate
                                  )}
                                  className="inline-flex items-center gap-1.5 rounded-lg bg-indigo-600 px-3 py-2 text-[11px] font-bold text-white hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50"
                                >
                                  <Calculator className="h-3.5 w-3.5" />
                                  {isSupplementaryPaymentScopePending(
                                    person.id,
                                    supplementaryAccrualDraft.paymentDate
                                  ) ? 'Hesaplanıyor…' : 'Hesapla ve Kaydet'}
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

      {/* Finalize Modal */}
      {activeFinalize && (
        <PayrollFinalizeModal
          personel={activeFinalize.person}
          bordro={activeFinalize.bordro}
          donem={aktifDonem}
          engine={payrollEngine}
          dataset={buildDataset()}
          initialOpen={true}
          onClose={() => setActiveFinalize(null)}
          onFinalized={(finalizedBordro) => {
            const person = activeFinalize.person;
            setActiveFinalize(null);
            handleFinalizeSuccess(person, finalizedBordro);
          }}
          onError={(message) => {
            setSuccessMessage(null);
            setErrorMessage(message);
          }}
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
                    {aktifDonem.donemAdi} dönemi için normal GV ve asgari GV referans opening yönetimi
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
                <strong>Mevzuat ve Kullanım Bilgisi:</strong> Buradaki iki tutar birbirinden bağımsızdır: normal çalışan GV opening'i ve asgari ücret GV referans opening'i. Normal opening, aktif bordro/vergi dönemi olan <strong>{aktifDonem.id}</strong> ile saklanır; asgari opening yalnızca bu alana açıkça girildiğinde veya mevcut bir asgari opening korunurken aynı period ID ile saklanır. Eski personel kaydında tutar varsa alanı değiştirmeden Kaydet düğmesine basmak da bu period ID bilgisini tamamlar. Çalışma ayı veya vergi ayı ayrıca girilmez.
              </div>

              <div className="max-h-96 overflow-y-auto border border-slate-200 rounded-xl">
                <table className="w-full text-left text-xs border-collapse">
                  <thead className="bg-slate-100 text-slate-700 font-bold border-b border-slate-200 sticky top-0 z-10">
                    <tr>
                      <th className="py-2.5 px-3">Personel</th>
                      <th className="py-2.5 px-3 text-right">Otomatik (Eski Bordrolar + Devir)</th>
                      <th className="py-2.5 px-3">Önceki Küm. GV Matrahı (TL)</th>
                      <th className="py-2.5 px-3">Önceki Küm. Asgari GV (TL)</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-200 font-mono">
                    {personeller.map((person) => {
                      const exactPerson = authoritativeDataset.personnel.find(
                        (item) => item.id === person.id
                      );
                      const exactBordro = authoritativeDataset.payrolls.find(
                        (item) => item.personelId === person.id && item.donemId === aktifDonem.id
                      );
                      const explicitOpening = authoritativeDataset.taxOpenings.find(
                        (opening) =>
                          opening.personnelId === person.id &&
                          opening.year === aktifDonem.taxYear
                      );
                      const legacyAsgariGv =
                        exactPerson?.devirKumulatifAsgariGvMatrahi != null &&
                        (!exactPerson.devirKumulatifAsgariGvMatrahiYili ||
                          exactPerson.devirKumulatifAsgariGvMatrahiYili === aktifDonem.taxYear)
                          ? exactPerson.devirKumulatifAsgariGvMatrahi
                          : undefined;
                      const autoGv = Number(
                        explicitOpening?.gvCumulativeOpening ??
                        exactBordro?.oncekiKumulatifGvMatrahi ??
                        getDevirGvMatrahiForActiveYear(person)
                      ) || 0;
                      const autoAsgariGv = Number(
                        explicitOpening?.asgariGvCumulativeOpening ??
                          exactBordro?.oncekiKumulatifAsgariGvMatrahi ??
                          legacyAsgariGv ??
                          0
                      ) || 0;

                      const currentSession = manualKumulatifGvMap[person.id];
                      const currentAsgariSession = manualKumulatifAsgariGvMap[person.id];
                      const displayGv =
                        currentSession ??
                        String(autoGv);
                      const displayAsgariGv =
                        currentAsgariSession ??
                        String(autoAsgariGv);

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
                          <td className="py-2.5 px-3">
                            <input
                              type="text"
                              inputMode="decimal"
                              placeholder={autoAsgariGv.toFixed(2)}
                              value={displayAsgariGv}
                              onChange={(e) => {
                                setManualKumulatifAsgariGvMap((prev) => ({
                                  ...prev,
                                  [person.id]: e.target.value,
                                }));
                              }}
                              className="w-full px-2.5 py-1.5 bg-white border border-amber-300 rounded-lg text-xs font-mono font-bold text-slate-900 focus:ring-2 focus:ring-amber-500"
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
                    setManualKumulatifAsgariGvMap({});
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
                        const legacyOpeningPersonIds = personeller
                          .filter((candidate) => {
                            const source = authoritativeDataset.personnel.find(
                              (item) => item.id === candidate.id
                            );
                            const opening = authoritativeDataset.taxOpenings.find(
                              (item) =>
                                item.personnelId === candidate.id &&
                                item.year === aktifDonem.taxYear
                            );
                            const hasLegacyNormalOpening =
                              getDevirGvMatrahiForActiveYear(candidate) > 0 &&
                              opening?.gvCumulativeOpening == null;
                            const hasLegacyAsgariOpening =
                              Number(
                                source?.devirKumulatifAsgariGvMatrahi ??
                                  candidate.devirKumulatifAsgariGvMatrahi ??
                                  0
                              ) > 0 &&
                              (!source?.devirKumulatifAsgariGvMatrahiYili ||
                                source.devirKumulatifAsgariGvMatrahiYili === aktifDonem.taxYear) &&
                              opening?.asgariGvCumulativeOpening == null;
                            return hasLegacyNormalOpening || hasLegacyAsgariOpening;
                          })
                          .map((candidate) => candidate.id);
                        const editedPersonIds = new Set([
                          ...Object.keys(manualKumulatifGvMap),
                          ...Object.keys(manualKumulatifAsgariGvMap),
                          ...legacyOpeningPersonIds,
                        ]);
                        for (const pId of editedPersonIds) {
                          const person = personeller.find((p) => p.id === pId);
                          if (person) {
                            const exactPersonSource = authoritativeDataset.personnel.find(
                              (item) => item.id === person.id
                            );
                            const exactBordro = authoritativeDataset.payrolls.find(
                              (item) => item.personelId === person.id && item.donemId === aktifDonem.id
                            );
                            const legacyAsgariGv =
                              exactPersonSource?.devirKumulatifAsgariGvMatrahi != null &&
                              (!exactPersonSource.devirKumulatifAsgariGvMatrahiYili ||
                                exactPersonSource.devirKumulatifAsgariGvMatrahiYili === aktifDonem.taxYear)
                                ? exactPersonSource.devirKumulatifAsgariGvMatrahi
                                : undefined;
                            const existingOpening = authoritativeDataset.taxOpenings.find(
                              (opening) =>
                                opening.personnelId === person.id &&
                                opening.year === aktifDonem.taxYear
                            );
                            const automaticGv =
                              existingOpening?.gvCumulativeOpening ??
                              exactBordro?.oncekiKumulatifGvMatrahi ??
                              getDevirGvMatrahiForActiveYear(person);
                            const automaticAsgariGv =
                              existingOpening?.asgariGvCumulativeOpening ??
                              exactBordro?.oncekiKumulatifAsgariGvMatrahi ??
                              legacyAsgariGv ??
                              0;
                            const normalOpeningWasEdited = Object.prototype.hasOwnProperty.call(
                              manualKumulatifGvMap,
                              pId
                            );
                            const asgariOpeningWasEdited = Object.prototype.hasOwnProperty.call(
                              manualKumulatifAsgariGvMap,
                              pId
                            );
                            const hasExistingNormalOpening =
                              existingOpening?.gvCumulativeOpening != null;
                            const hasExistingNormalPeriod =
                              existingOpening?.effectiveFromPeriodId != null;
                            const hasExistingAsgariOpening =
                              existingOpening?.asgariGvCumulativeOpening != null;
                            const hasExistingAsgariPeriod =
                              existingOpening?.asgariGvEffectiveFromPeriodId != null;
                            const shouldPersistNormalOpening =
                              normalOpeningWasEdited ||
                              (!hasExistingNormalOpening && getDevirGvMatrahiForActiveYear(person) > 0);
                            const shouldPersistAsgariOpening =
                              asgariOpeningWasEdited ||
                              (!hasExistingAsgariOpening && Number(legacyAsgariGv ?? 0) > 0);
                            const val = (manualKumulatifGvMap[pId] ?? String(automaticGv)).trim() || '0';
                            const asgariVal = (
                              manualKumulatifAsgariGvMap[pId] ?? String(automaticAsgariGv)
                            ).trim() || '0';
                            if (shouldPersistNormalOpening && (!isExactDecimalString(val) || val.startsWith('-'))) {
                              throw new Error('Kümülatif GV matrahı geçerli, negatif olmayan bir tutar olmalıdır.');
                            }
                            if (shouldPersistAsgariOpening && (!isExactDecimalString(asgariVal) || asgariVal.startsWith('-'))) {
                              throw new Error('Kümülatif asgari GV matrahı geçerli, negatif olmayan bir tutar olmalıdır.');
                            }
                            if (hasExistingNormalOpening !== hasExistingNormalPeriod) {
                              throw new Error('Mevcut normal GV opening effective dönemi eksik; kayıt güvenli biçimde güncellenemiyor.');
                            }
                            if (hasExistingAsgariOpening !== hasExistingAsgariPeriod) {
                              throw new Error('Mevcut asgari GV opening effective dönemi eksik; kayıt güvenli biçimde güncellenemiyor.');
                            }
                            const exactPerson = mergePayrollUiIntoBoundary(
                              exactPersonSource,
                              {
                                ...person,
                                ...(shouldPersistNormalOpening
                                  ? { devirKumulatifGvMatrahi: val }
                                  : { devirKumulatifGvMatrahi: exactPersonSource?.devirKumulatifGvMatrahi }),
                                ...(shouldPersistAsgariOpening
                                  ? { devirKumulatifAsgariGvMatrahi: asgariVal }
                                  : { devirKumulatifAsgariGvMatrahi: exactPersonSource?.devirKumulatifAsgariGvMatrahi }),
                              }
                            ) as PayrollBoundaryPersonel;
                            const personelUpdate = {
                              ...exactPerson,
                              ...(shouldPersistNormalOpening
                                ? {
                                    devirKumulatifGvMatrahiYili: aktifDonem.taxYear,
                                    devirKumulatifGvMatrahiBaslangicAyi: aktifDonem.ay,
                                  }
                                : {}),
                              ...(shouldPersistAsgariOpening
                                ? { devirKumulatifAsgariGvMatrahiYili: aktifDonem.taxYear }
                                : {}),
                            } as PayrollBoundaryPersonel;
                            const openingUpdate: PayrollBoundaryTaxOpening = {
                              id: `${person.id}_${aktifDonem.taxYear}`,
                              personnelId: person.id,
                              year: aktifDonem.taxYear,
                              ...(shouldPersistNormalOpening
                                ? {
                                    gvCumulativeOpening: val,
                                    effectiveFromPeriodId: aktifDonem.id,
                                  }
                                : hasExistingNormalOpening
                                  ? {
                                      gvCumulativeOpening: existingOpening!.gvCumulativeOpening,
                                      effectiveFromPeriodId: existingOpening!.effectiveFromPeriodId,
                                    }
                                  : {}),
                              ...(shouldPersistAsgariOpening
                                ? {
                                    asgariGvCumulativeOpening: asgariVal,
                                    asgariGvEffectiveFromPeriodId: aktifDonem.id,
                                  }
                                : hasExistingAsgariOpening
                                  ? {
                                      asgariGvCumulativeOpening: existingOpening!.asgariGvCumulativeOpening,
                                      asgariGvEffectiveFromPeriodId: existingOpening!.asgariGvEffectiveFromPeriodId,
                                    }
                                  : {}),
                            };
                            await onSavePersonelAndTaxOpening(personelUpdate, openingUpdate);
                            updatedAny = true;
                          }
                        }
                        // Browser state is committed by the parent after the
                        // atomic save. Let that render update the calculation
                        // controller's dataset ref before the immediate recalc.
                        await new Promise<void>((resolve) => setTimeout(resolve, 0));
                        await handleCalculateAll();
                        setIsKumulatifModalOpen(false);
                        if (updatedAny) {
                          setSuccessMessage(
                            `${aktifDonem.taxYear} vergi yılı normal ve asgari GV opening değerleri güncellendi. Bu yıla ait bordrolar yeniden hesaplandı.`
                          );
                        }
                      } catch (err) {
                        setErrorMessage(`Kümülatif matrah kaydedilemedi: ${describeError(err)}`);
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
