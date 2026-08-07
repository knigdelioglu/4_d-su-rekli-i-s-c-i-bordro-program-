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
  X,
} from 'lucide-react';
import {
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  KesintiKalemleri,
  Personel,
  PersonelPuantaj,
} from '../types/payroll';
import {
  autoFillGelirlerFromPuantaj,
  calculateGelirToplam,
  calculateKesintiToplam,
  calculateNetOdeme,
  calculatePuantajOzeti,
  calculateStatutoryDeductions,
  calculatePreviousCumulativeGvMatrah,
  calculatePreviousCumulativeAsgariUcretGvMatrah,
  calculateIncomingDevredenPek,
  DEFAULT_KURUM_DEGERLERI,
  formatTL,
} from '../utils/payrollUtils';
import { PaySlipModal } from './PaySlipModal';

interface BordroHesaplamaProps {
  aktifDonem: BordroDonemi;
  donemler: BordroDonemi[];
  personeller: Personel[];
  kurumDegerleriMap: Record<string, DönemselKurumDegerleri>;
  puantajlar: PersonelPuantaj[];
  bordrolar: BordroKaydi[];
  onSaveBordro: (bordro: BordroKaydi) => void;
  onSavePersonel?: (personel: Personel) => void;
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
  onSaveBordro,
  onSavePersonel,
  onGoToPuantaj,
}) => {
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [activePaySlip, setActivePaySlip] = useState<{
    personel: Personel;
    bordro: BordroKaydi;
  } | null>(null);
  const [isBatchProcessing, setIsBatchProcessing] = useState<boolean>(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Manual cumulative GV override state per person for the current period
  const [manualKumulatifGvMap, setManualKumulatifGvMap] = useState<
    Record<string, number>
  >({});
  const [isKumulatifModalOpen, setIsKumulatifModalOpen] = useState<boolean>(false);

  // Active institution defaults for current period
  const activeKurumDegerleri =
    kurumDegerleriMap[aktifDonem.id] || {
      donemId: aktifDonem.id,
      ...DEFAULT_KURUM_DEGERLERI,
    };

  // Helper to calculate and save bordro for a person ONLY IF saved puantaj exists
  const calculateAndSaveForPerson = (person: Personel): BordroKaydi | null => {
    const pPuantaj = puantajlar.find(
      (p) => p.personelId === person.id && p.donemId === aktifDonem.id
    );

    if (!pPuantaj || !pPuantaj.gunler || Object.keys(pPuantaj.gunler).length === 0) {
      // Saved puantaj NOT found for this person in this period! DO NOT calculate automatically!
      return null;
    }

    const puantajOzeti = calculatePuantajOzeti(pPuantaj.gunler);
    const gelirler = autoFillGelirlerFromPuantaj(
      puantajOzeti,
      activeKurumDegerleri,
      person.hizmetYili,
      person.grup
    );

    const existingBordro = bordrolar.find(
      (b) => b.personelId === person.id && b.donemId === aktifDonem.id
    );

    // Calculate auto cumulative from previous saved bordros + person devir
    const autoGvOnceki = calculatePreviousCumulativeGvMatrah(
      person.id,
      aktifDonem,
      bordrolar,
      donemler,
      person
    );

    const kumulatifAsgariGvOnceki = calculatePreviousCumulativeAsgariUcretGvMatrah(
      person.id,
      aktifDonem,
      bordrolar,
      donemler,
      kurumDegerleriMap,
      person
    );

    const sessionManual = manualKumulatifGvMap[person.id];
    const kumulatifGvOnceki = sessionManual ?? autoGvOnceki;
    const hasManualGv = sessionManual !== undefined || (person.devirKumulatifGvMatrahi !== undefined && person.devirKumulatifGvMatrahiYili === aktifDonem.yil);

    const devredenPekGelen = calculateIncomingDevredenPek(
      person.id,
      aktifDonem,
      bordrolar,
      donemler
    );

    const statutory = calculateStatutoryDeductions(
      gelirler,
      activeKurumDegerleri,
      person,
      puantajOzeti,
      kumulatifGvOnceki,
      devredenPekGelen,
      kumulatifAsgariGvOnceki
    );

    const kesintiler: KesintiKalemleri = {
      isciSgkPrimi: statutory.isciSgkPrimi ?? null,
      isciIssizlikPrimi: statutory.isciIssizlikPrimi ?? null,
      gelirVergisi: statutory.gelirVergisi ?? null,
      damgaVergisi: statutory.damgaVergisi ?? null,
      sendikaAidati: statutory.sendikaAidati ?? null,
      bes: statutory.bes ?? null,
      icra: statutory.icra ?? null,
      kisiBorcu: statutory.kisiBorcu ?? null,
      dogumAskerlikBorclanmasi: statutory.dogumAskerlikBorclanmasi ?? null,
      hayatSaglikSigortasi: statutory.hayatSaglikSigortasi ?? null,
      digerKesinti: statutory.digerKesinti ?? null,
    };

    const gelirToplam = calculateGelirToplam(gelirler);
    const kesintiToplam = calculateKesintiToplam(kesintiler);
    const netOdeme = calculateNetOdeme(gelirToplam, kesintiToplam);

    const newBordro: BordroKaydi = {
      id: `${person.id}_${aktifDonem.id}`,
      personelId: person.id,
      donemId: aktifDonem.id,
      puantajOzeti,
      gelirler,
      gelirToplam,
      kesintiler,
      kesintiToplam,
      netOdeme,
      olusturulmaTarihi: new Date().toISOString(),
      sonGuncellemeTarihi: new Date().toISOString(),
      notlar: `${aktifDonem.donemAdi} hesaplandı.`,
      oncekiKumulatifGvMatrahi: kumulatifGvOnceki,
      oncekiKumulatifAsgariGvMatrahi: kumulatifAsgariGvOnceki,
      manuelKumulatifGvMatrahi: hasManualGv ? kumulatifGvOnceki : undefined,
      devredenPekGelen,
      sonrakiDevredenPek: statutory.pekResult?.sonrakiDevredenList || [],
      pekDetay: statutory.pekResult
        ? {
            hesaplananPek: statutory.pekResult.hesaplananPek,
            finalPek: statutory.pekResult.finalPek,
            devredenPekAşanTutar: statutory.pekResult.devredenPekAşanTutar,
            pekAltSinir: statutory.pekResult.pekAltSinir,
            pekUstSinir: statutory.pekResult.pekUstSinir,
            fiiliYemekGunu: statutory.pekResult.fiiliYemekGunu,
            yemekIstisnasiTutar: statutory.pekResult.yemekIstisnasiTutar,
          }
        : undefined,
    };

    onSaveBordro(newBordro);
    return newBordro;
  };

  // Batch calculation handler
  const handleCalculateAll = () => {
    setIsBatchProcessing(true);
    let successCount = 0;
    let failCount = 0;
    const missingPuantajPersons: string[] = [];

    personeller.forEach((person) => {
      const res = calculateAndSaveForPerson(person);
      if (res) {
        successCount++;
      } else {
        failCount++;
        missingPuantajPersons.push(`${person.ad} ${person.soyad}`);
      }
    });

    setIsBatchProcessing(false);

    if (failCount === 0) {
      setErrorMessage(null);
      setSuccessMessage(`${successCount} personelin bordrosu başarıyla güncellendi.`);
      setTimeout(() => setSuccessMessage(null), 3500);
    } else if (successCount > 0) {
      setSuccessMessage(`${successCount} personelin bordrosu hesaplandı.`);
      setErrorMessage(
        `${failCount} personelin kayıtlı puantajı bulunmadığı için bordrosu HESAPLANAMADI (${missingPuantajPersons.slice(0, 3).join(', ')}${missingPuantajPersons.length > 3 ? '...' : ''}). Lütfen önce Puantaj Cetvelinden puantaj girişi yapın.`
      );
    } else {
      setSuccessMessage(null);
      setErrorMessage(
        `Hiçbir personelin kayıtlı puantajı bulunamadığı için bordro hesaplanamadı! Lütfen önce Puantaj Cetveli ekranından personellerin puantajını girip kaydedin.`
      );
    }
  };

  // Open Pay Slip modal for a person
  const handleOpenPaySlip = (person: Personel) => {
    let bordro = bordrolar.find(
      (b) => b.personelId === person.id && b.donemId === aktifDonem.id
    );

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

      bordro = calculateAndSaveForPerson(person) || undefined;
    }

    if (bordro) {
      setActivePaySlip({ personel: person, bordro });
    }
  };

  // Single calculation handler
  const handleCalculateSingle = (person: Personel, e: React.MouseEvent) => {
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

    const res = calculateAndSaveForPerson(person);
    if (res) {
      setErrorMessage(null);
      setSuccessMessage(`${person.ad} ${person.soyad} bordrosu başarıyla hesaplandı.`);
      setTimeout(() => setSuccessMessage(null), 3000);
    }
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

  // Period statistics
  const activePeriodBordrolar = bordrolar.filter((b) => b.donemId === aktifDonem.id);
  const totalGross = activePeriodBordrolar.reduce((acc, b) => acc + (b.gelirToplam || 0), 0);
  const totalNet = activePeriodBordrolar.reduce((acc, b) => acc + (b.netOdeme || 0), 0);
  const totalDeductions = activePeriodBordrolar.reduce((acc, b) => acc + (b.kesintiToplam || 0), 0);

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
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="bg-white p-5 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-4">
          <div className="p-3 bg-blue-50 text-blue-700 rounded-xl">
            <Users className="w-6 h-6" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">Toplam Personel</span>
            <div className="text-xl font-bold text-slate-900">{personeller.length} Kişi</div>
            <span className="text-[11px] text-blue-600 font-medium">
              {activePeriodBordrolar.length} / {personeller.length} Hesaplandı
            </span>
          </div>
        </div>

        <div className="bg-white p-5 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-4">
          <div className="p-3 bg-indigo-50 text-indigo-700 rounded-xl">
            <Wallet className="w-6 h-6" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">Toplam Brüt Gelir</span>
            <div className="text-xl font-bold text-indigo-900 font-mono">
              {formatTL(totalGross)}
            </div>
            <span className="text-[11px] text-slate-400">Vergi ve SGK Öncesi</span>
          </div>
        </div>

        <div className="bg-white p-5 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-4">
          <div className="p-3 bg-rose-50 text-rose-700 rounded-xl">
            <Receipt className="w-6 h-6" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">Toplam Kesintiler</span>
            <div className="text-xl font-bold text-rose-800 font-mono">
              {formatTL(totalDeductions)}
            </div>
            <span className="text-[11px] text-slate-400">SGK + Vergi + Özel Kesinti</span>
          </div>
        </div>

        <div className="bg-white p-5 rounded-2xl border border-slate-200 shadow-xs flex items-center gap-4">
          <div className="p-3 bg-emerald-50 text-emerald-700 rounded-xl">
            <TrendingUp className="w-6 h-6" />
          </div>
          <div>
            <span className="text-xs text-slate-500 font-medium">Toplam Net Ödeme</span>
            <div className="text-xl font-bold text-emerald-700 font-mono">
              {formatTL(totalNet)}
            </div>
            <span className="text-[11px] text-emerald-600 font-semibold">Banka Ele Geçen</span>
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
            <span>İsme tıklayarak detaylı bordro zarfını açabilirsiniz</span>
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
                <th className="py-3 px-4 text-right">Kesintiler</th>
                <th className="py-3 px-4 text-right">Net Ele Geçen</th>
                <th className="py-3 px-4 text-center">Durum</th>
                <th className="py-3 px-4 text-center">İşlemler</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 text-xs text-slate-800">
              {filteredPersoneller.length === 0 ? (
                <tr>
                  <td colSpan={10} className="py-12 text-center text-slate-500">
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

                  const isCalculated = !!bordro;
                  const brut = bordro?.gelirToplam || 0;
                  const kesinti = bordro?.kesintiToplam || 0;
                  const net = bordro?.netOdeme || 0;

                  return (
                    <tr
                      key={person.id}
                      onClick={() => handleOpenPaySlip(person)}
                      className="hover:bg-indigo-50/50 transition-colors cursor-pointer group"
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
                              <ChevronRight className="w-3.5 h-3.5 text-slate-400 group-hover:text-indigo-600 opacity-0 group-hover:opacity-100 transition-opacity" />
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
                          <div className="inline-flex items-center gap-1 text-[10px] font-mono">
                            <span className="px-1.5 py-0.5 bg-emerald-100 text-emerald-800 rounded font-bold">
                              {bordro.puantajOzeti.Ç} Ç
                            </span>
                            <span className="px-1.5 py-0.5 bg-blue-100 text-blue-800 rounded font-bold">
                              {bordro.puantajOzeti.T} T
                            </span>
                            {bordro.puantajOzeti.İ > 0 && (
                              <span className="px-1.5 py-0.5 bg-amber-100 text-amber-800 rounded font-bold">
                                {bordro.puantajOzeti.İ} İ
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
                          const val = sessionManual ?? bordro?.oncekiKumulatifGvMatrahi ?? bordro?.manuelKumulatifGvMatrahi ?? calculatePreviousCumulativeGvMatrah(person.id, aktifDonem, bordrolar, donemler, person);

                          if (val > 0) {
                            return (
                              <div className="flex flex-col items-end">
                                <span className="font-bold text-slate-800">{formatTL(val)}</span>
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
                      <td className="py-3 px-4 text-right font-mono font-medium text-slate-800">
                        {isCalculated ? formatTL(brut) : '—'}
                      </td>

                      {/* Kesintiler */}
                      <td className="py-3 px-4 text-right font-mono font-medium text-rose-700">
                        {isCalculated ? formatTL(kesinti) : '—'}
                      </td>

                      {/* Net */}
                      <td className="py-3 px-4 text-right font-mono font-bold text-emerald-700 text-sm">
                        {isCalculated ? formatTL(net) : '—'}
                      </td>

                      {/* Durum */}
                      <td className="py-3 px-4 text-center">
                        {isCalculated ? (
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
                              <button
                                onClick={(e) => handleCalculateSingle(person, e)}
                                title="Bordroyu Hesapla/Yeniden Hesapla"
                                className="p-1.5 bg-slate-100 hover:bg-indigo-600 hover:text-white text-slate-700 rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
                              >
                                <RefreshCw className="w-3.5 h-3.5" />
                                <span>Hesapla</span>
                              </button>

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
          isPrimiGruplari={activeKurumDegerleri.isPrimiGruplari}
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
                      const autoGv = calculatePreviousCumulativeGvMatrah(
                        person.id,
                        aktifDonem,
                        bordrolar,
                        donemler,
                        person
                      );

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
                    onClick={() => {
                      let updatedAny = false;
                      Object.keys(manualKumulatifGvMap).forEach((pId) => {
                        const person = personeller.find((p) => p.id === pId);
                        if (person && onSavePersonel) {
                          const val = manualKumulatifGvMap[pId];
                          onSavePersonel({
                            ...person,
                            devirKumulatifGvMatrahi: val,
                            devirKumulatifGvMatrahiYili: aktifDonem.yil,
                            devirKumulatifGvMatrahiBaslangicAyi: aktifDonem.ay,
                          });
                          updatedAny = true;
                        }
                      });
                      setIsKumulatifModalOpen(false);
                      handleCalculateAll();
                      if (updatedAny) {
                        setSuccessMessage(
                          `${aktifDonem.yil} yılı kümülatif GV başlangıç matrahı güncellendi. Bu yıla ait bordrolar yeniden hesaplandı.`
                        );
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
