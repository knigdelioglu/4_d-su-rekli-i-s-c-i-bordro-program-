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
} from 'lucide-react';
import {
  BordroDonemi,
  BordroKaydi,
  BordroStatus,
  AnnualPayrollParameters,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  SickLeaveRecord,
  ManualPayrollIncomeInput,
} from '../types/payroll';
import {
  formatTL,
} from '../utils/payrollUtils';
import { PaySlipModal } from './PaySlipModal';
import { PayrollFinalizeModal } from './PayrollFinalizeModal';
import { getPayrollEngine, PayrollDatasetSnapshot } from '../services/payrollEngine';

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
  onSaveBordro: (bordro: BordroKaydi) => Promise<void> | void;
  onSavePersonel?: (personel: Personel) => Promise<void> | void;
  onSaveTaxOpening?: (opening: PersonelTaxOpening) => Promise<void> | void;
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
  onSaveBordro,
  onSavePersonel,
  onSaveTaxOpening,
  onGoToPuantaj,
}) => {
  const payrollEngine = getPayrollEngine();
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [activePaySlip, setActivePaySlip] = useState<{
    personel: Personel;
    bordro: BordroKaydi;
  } | null>(null);
  const [isBatchProcessing, setIsBatchProcessing] = useState<boolean>(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [manualIncomeMap, setManualIncomeMap] = useState<
    Record<string, { tediye?: string; tisIkramiyesi?: string }>
  >({});

  // Manual cumulative GV override state per person for the current period
  const [manualKumulatifGvMap, setManualKumulatifGvMap] = useState<
    Record<string, number>
  >({});
  const [isKumulatifModalOpen, setIsKumulatifModalOpen] = useState<boolean>(false);

  const activeKurumDegerleri = kurumDegerleriMap[aktifDonem.id];

  const getManualIncomeStateKey = (personId: string): string =>
    `${aktifDonem.id}:${personId}`;

  const getManualIncomeInput = (personId: string): ManualPayrollIncomeInput => {
    const existingPayroll = bordrolar.find(
      (item) => item.personelId === personId && item.donemId === aktifDonem.id
    );
    const draft = manualIncomeMap[getManualIncomeStateKey(personId)];
    const resolveAmount = (
      field: 'tediye' | 'tisIkramiyesi'
    ): number | null => {
      const raw = draft?.[field];
      if (raw !== undefined) {
        if (raw.trim() === '') return null;
        const parsed = Number(raw);
        return Number.isFinite(parsed) ? parsed : null;
      }
      return existingPayroll?.gelirler[field] ?? null;
    };

    return {
      tediye: resolveAmount('tediye'),
      tisIkramiyesi: resolveAmount('tisIkramiyesi'),
    };
  };

  const updateManualIncomeDraft = (
    personId: string,
    field: 'tediye' | 'tisIkramiyesi',
    value: string
  ) => {
    const stateKey = getManualIncomeStateKey(personId);
    setManualIncomeMap((current) => ({
      ...current,
      [stateKey]: {
        ...current[stateKey],
        [field]: value,
      },
    }));
  };

  const getDevirGvMatrahiForActiveYear = (person: Personel): number => {
    const activeTaxYear = aktifDonem.taxYear ?? (aktifDonem.ay === 12 ? aktifDonem.yil + 1 : aktifDonem.yil);
    const openingYear = person.devirKumulatifGvMatrahiYili;
    const opening = person.devirKumulatifGvMatrahi ?? 0;
    return opening > 0 && (!openingYear || openingYear === activeTaxYear) ? opening : 0;
  };

  const buildDataset = (): PayrollDatasetSnapshot => ({
    personnel: personeller,
    periods: donemler,
    institutionSettings: kurumDegerleriMap,
    attendances: puantajlar,
    payrolls: bordrolar,
    taxOpenings,
    sickLeaveRecords,
    annualPayrollParameters,
    zamAylari,
  });

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

    const existingBordro = bordrolar.find(
      (b) => b.personelId === person.id && b.donemId === aktifDonem.id
    );
    if (existingBordro?.status === 'FINALIZED') {
      setErrorMessage(`${person.ad} ${person.soyad} bordrosu kesinleştirildiği için yeniden hesaplanamaz.`);
      return null;
    }

    try {
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        manualIncome: getManualIncomeInput(person.id),
        dataset: buildDataset(),
      });
      await onSaveBordro(calculated);
      return calculated;
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
  const handleOpenPaySlip = async (person: Personel) => {
    let bordro = bordrolar.find(
      (b) => b.personelId === person.id && b.donemId === aktifDonem.id
    );

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

  // Single calculation handler
  const handleCalculateSingle = async (person: Personel, e: React.MouseEvent) => {
    e.stopPropagation();
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

  const handleFinalizeSuccess = async (person: Personel, finalizedBordro: BordroKaydi) => {
    await onSaveBordro({ ...finalizedBordro, status: 'FINALIZED' as BordroStatus });
    setErrorMessage(null);
    setSuccessMessage(`${person.ad} ${person.soyad} bordrosu kesinleştirildi.`);
    setTimeout(() => setSuccessMessage(null), 3500);
  };

  // Filtered personnel list
  const filteredPersoneller = personeller.filter(
    (p) =>
      p.ad.toLowerCase().includes(searchTerm.toLowerCase()) ||
      p.soyad.toLowerCase().includes(searchTerm.toLowerCase()) ||
      p.tcNo.includes(searchTerm) ||
      (p.grup && p.grup.toLowerCase().includes(searchTerm.toLowerCase())) ||
      (p.unvan && p.unvan.toLowerCase().includes(searchTerm.toLowerCase()))
  );

  // Period statistics include only authoritative snapshots. STALE/DRAFT values
  // remain visible on their row for diagnosis but must not contaminate totals.
  const activePeriodBordrolar = bordrolar.filter(
    (b) =>
      b.donemId === aktifDonem.id &&
      (b.status === 'CALCULATED' || b.status === 'FINALIZED')
  );
  const totalGross = activePeriodBordrolar.reduce((acc, b) => acc + (b.gelirToplam || 0), 0);
  const totalNet = activePeriodBordrolar.reduce((acc, b) => acc + (b.netOdeme || 0), 0);
  const totalDeductions = activePeriodBordrolar.reduce((acc, b) => acc + (b.kesintiToplam || 0), 0);
  const totalEmployerCost = activePeriodBordrolar.reduce(
    (acc, b) => acc + (b.pekDetay?.isverenPrimToplami ?? 0),
    0
  );

  return (
    <div className="space-y-6">
      {/* Top Banner / Title */}
      <div className="bg-slate-900 rounded-2xl p-6 text-white shadow-xl flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-white flex items-center gap-2">
            <Calculator className="w-6 h-6 text-indigo-400" />
            <span>Bordro Hesaplama Ekranı</span>
          </h2>
          <p className="text-xs text-slate-300 mt-1 max-w-2xl">
            Kesintiler ve özlük hakları personelin kayıtlı kartından otomatik çekilir. Kişi adına tıklayarak detaylı bordro zarfını (ücret pusulasını) görüntüleyebilirsiniz.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
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
            <span>Tüm Personelleri Otomatik Hesapla ({personeller.length})</span>
          </button>
        </div>
      </div>

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
              {activePeriodBordrolar.length} / {personeller.length} Hesaplandı
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
        <div className="p-4 bg-slate-50 border-b border-slate-200 flex flex-col sm:flex-row items-center justify-between gap-3">
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
            <span>Güncel bordrolarda isme tıklayarak bordro zarfını açabilirsiniz</span>
          </div>
        </div>

        {/* Table */}
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-100 text-slate-700 text-[11px] uppercase tracking-wider font-bold border-b border-slate-200">
                <th className="py-3 px-4">S.No</th>
                <th className="py-3 px-4">Personel Adı Soyadı</th>
                <th className="py-3 px-4">İş Primi Grubu</th>
                <th className="py-3 px-4 text-center">Puantaj İcmal</th>
                <th className="py-3 px-4 text-right">Önceki Küm. GV</th>
                <th className="py-3 px-4 text-right">Brüt Gelir</th>
                <th className="py-3 px-4 text-right">Tediye (Manuel)</th>
                <th className="py-3 px-4 text-right">TİS İkramiye (Manuel)</th>
                <th className="py-3 px-4 text-right">Kesintiler</th>
                <th className="py-3 px-4 text-right">Net Ele Geçen</th>
                <th className="py-3 px-4 text-center">Durum</th>
                <th className="py-3 px-4 text-center">İşlemler</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 text-xs text-slate-800">
              {filteredPersoneller.length === 0 ? (
                <tr>
                  <td colSpan={12} className="py-12 text-center text-slate-500">
                    Arama kriterlerine uygun personel kaydı bulunamadı.
                  </td>
                </tr>
              ) : (
                filteredPersoneller.map((person, idx) => {
                  const bordro = bordrolar.find(
                    (b) => b.personelId === person.id && b.donemId === aktifDonem.id
                  );
                  const pPuantaj = puantajlar.find(
                    (p) => p.personelId === person.id && p.donemId === aktifDonem.id
                  );
                  const hasPuantaj = !!(
                    pPuantaj &&
                    pPuantaj.gunler &&
                    Object.keys(pPuantaj.gunler).length > 0
                  );

                  const hasPayrollSnapshot = !!bordro;
                  const isFinalized = bordro?.status === 'FINALIZED';
                  const isStale = bordro?.status === 'STALE';
                  const isDraft = bordro?.status === 'DRAFT';
                  const isCalculated = bordro?.status === 'CALCULATED' || isFinalized;
                  const brut = bordro?.gelirToplam || 0;
                  const kesinti = bordro?.kesintiToplam || 0;
                  const net = bordro?.netOdeme || 0;
                  const manualIncomeStateKey = getManualIncomeStateKey(person.id);
                  const tediyeInputValue =
                    manualIncomeMap[manualIncomeStateKey]?.tediye ??
                    (bordro?.gelirler.tediye != null ? String(bordro.gelirler.tediye) : '');
                  const tisInputValue =
                    manualIncomeMap[manualIncomeStateKey]?.tisIkramiyesi ??
                    (bordro?.gelirler.tisIkramiyesi != null
                      ? String(bordro.gelirler.tisIkramiyesi)
                      : '');

                  return (
                    <tr
                      key={person.id}
                      onClick={() => handleOpenPaySlip(person)}
                      className={`transition-colors group ${isStale || isDraft ? 'bg-amber-50/40 cursor-default' : 'hover:bg-indigo-50/50 cursor-pointer'}`}
                    >
                      <td className="py-3 px-4 font-mono text-slate-400 font-medium">
                        {idx + 1}
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
                          </div>
                        </div>
                      </td>

                      {/* Group */}
                      <td className="py-3 px-4 text-slate-700">
                        <div className="font-bold text-slate-900">
                          {person.grup || (person.unvan ? person.unvan.replace(/\s*\(.*?\)/, '') : '1. Grup')}
                        </div>
                        <div className="text-[10px] text-indigo-700 font-mono font-semibold">
                          {person.hizmetYili} Yıl Kıdem
                        </div>
                      </td>

                      {/* Puantaj Summary Pills */}
                      <td className="py-3 px-4 text-center">
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
                      </td>

                      {/* Önceki Küm. GV Matrahı */}
                      <td className="py-3 px-4 text-right font-mono text-xs">
                        {(() => {
                          const sessionManual = manualKumulatifGvMap[person.id];
                          const isManual = sessionManual !== undefined || bordro?.manuelKumulatifGvMatrahi !== undefined;
                          const val =
                            sessionManual ??
                            bordro?.oncekiKumulatifGvMatrahi ??
                            bordro?.manuelKumulatifGvMatrahi ??
                            getDevirGvMatrahiForActiveYear(person);

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
                      </td>

                      {/* Brüt */}
                      <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-slate-800'}`}>
                        {hasPayrollSnapshot ? formatTL(brut) : '—'}
                      </td>

                      {/* Manuel Tediye */}
                      <td className="py-3 px-4 text-right" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="number"
                          min="0"
                          step="0.01"
                          inputMode="decimal"
                          disabled={isFinalized}
                          value={tediyeInputValue}
                          onChange={(e) => updateManualIncomeDraft(person.id, 'tediye', e.target.value)}
                          placeholder="Boş"
                          title="Brüt Tediye tutarını manuel girin. Boş bırakılırsa Tediye hesaplanmaz."
                          className="w-28 px-2 py-1.5 text-right bg-white border border-amber-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500 disabled:bg-slate-100 disabled:text-slate-500"
                        />
                      </td>

                      {/* Manuel TİS İkramiyesi */}
                      <td className="py-3 px-4 text-right" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="number"
                          min="0"
                          step="0.01"
                          inputMode="decimal"
                          disabled={isFinalized}
                          value={tisInputValue}
                          onChange={(e) => updateManualIncomeDraft(person.id, 'tisIkramiyesi', e.target.value)}
                          placeholder="Boş"
                          title="Brüt TİS ikramiyesi tutarını manuel girin. Boş bırakılırsa TİS ikramiyesi hesaplanmaz."
                          className="w-28 px-2 py-1.5 text-right bg-white border border-indigo-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-indigo-500 disabled:bg-slate-100 disabled:text-slate-500"
                        />
                      </td>

                      {/* Kesintiler */}
                      <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-rose-700'}`}>
                        {hasPayrollSnapshot ? formatTL(kesinti) : '—'}
                      </td>

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
                          <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-amber-100 text-amber-800 border border-amber-200">
                            <Clock className="w-3 h-3 text-amber-600" />
                            <span>Hesaplanmadı</span>
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
                        <div className="flex items-center justify-center gap-1.5">
                          {hasPuantaj ? (
                            <>
                              {!isFinalized && (
                                <button
                                  onClick={(e) => handleCalculateSingle(person, e)}
                                  title={isStale ? 'Güncelliğini yitiren bordroyu yeniden hesapla' : 'Bordroyu Hesapla/Yeniden Hesapla'}
                                  className="p-1.5 bg-slate-100 hover:bg-indigo-600 hover:text-white text-slate-700 rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
                                >
                                  <RefreshCw className="w-3.5 h-3.5" />
                                  <span>{isStale ? 'Yeniden Hesapla' : 'Hesapla'}</span>
                                </button>
                              )}

                              {!isStale && !isDraft && (
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleOpenPaySlip(person);
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
                      const autoGv =
                        bordro?.oncekiKumulatifGvMatrahi ??
                        getDevirGvMatrahiForActiveYear(person);

                      const currentSession = manualKumulatifGvMap[person.id];
                      const displayGv = currentSession ?? bordro?.manuelKumulatifGvMatrahi ?? autoGv;

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
                              type="number"
                              step="0.01"
                              min="0"
                              placeholder={autoGv.toFixed(2)}
                              value={displayGv || ''}
                              onChange={(e) => {
                                const val = parseFloat(e.target.value);
                                setManualKumulatifGvMap((prev) => ({
                                  ...prev,
                                  [person.id]: isNaN(val) ? 0 : val,
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
                            const val = manualKumulatifGvMap[pId];
                            await onSavePersonel({
                              ...person,
                              devirKumulatifGvMatrahi: val,
                              devirKumulatifGvMatrahiYili: aktifDonem.taxYear ?? (aktifDonem.ay === 12 ? aktifDonem.yil + 1 : aktifDonem.yil),
                              devirKumulatifGvMatrahiBaslangicAyi: aktifDonem.ay,
                            });
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
