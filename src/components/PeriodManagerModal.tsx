/**
 * Section 1 & 4: Bordro Dönemi & Dönemsel Kurum Değerleri Management
 */

import React, { useState, useEffect } from 'react';
import { Calendar, Save, Plus, Settings, Check, X, Info, Gift, Award, Sparkles, ShieldAlert, Percent, Trash2, Layers, Settings2, FileText, User } from 'lucide-react';
import {
  BordroDonemi,
  DönemselKurumDegerleri,
  AnnualPayrollParameters,
  IsPrimiGrupItem,
  Personel,
  SickLeaveRecord,
  TaxBracket,
  TediyeKalemi,
  TisIkramiyeKalemi,
} from '../types/payroll';
import {
  AY_ISIMLERI,
  createBordroDonemi,
  DEFAULT_IS_PRIMI_GRUPLARI,
  DEFAULT_KURUM_DEGERLERI,
  DEFAULT_TEDIYE_LISTESI,
  DEFAULT_TIS_IKRAMIYE_LISTESI,
  formatTL,
} from '../utils/payrollUtils';

interface PeriodManagerModalProps {
  isOpen: boolean;
  onClose: () => void;
  donemler: BordroDonemi[];
  aktifDonemId: string;
  onSelectDonem: (donemId: string) => Promise<void> | void;
  onCreateDonem: (
    donem: BordroDonemi,
    kurumDegerleri: DönemselKurumDegerleri
  ) => Promise<void> | void;
  kurumDegerleriMap: Record<string, DönemselKurumDegerleri>;
  onSaveKurumDegerleri: (kurumDegerleri: DönemselKurumDegerleri) => Promise<void> | void;
  personeller?: Personel[];
  annualPayrollParameters: AnnualPayrollParameters[];
  onSaveAnnualPayrollParameters: (parameters: AnnualPayrollParameters) => Promise<void> | void;
  sickLeaveRecords: SickLeaveRecord[];
  onSaveSickLeaveRecord: (record: SickLeaveRecord) => Promise<void> | void;
  onDeleteSickLeaveRecord: (id: string) => Promise<void> | void;
}

export const PeriodManagerModal: React.FC<PeriodManagerModalProps> = ({
  isOpen,
  onClose,
  donemler,
  aktifDonemId,
  onSelectDonem,
  onCreateDonem,
  kurumDegerleriMap,
  onSaveKurumDegerleri,
  personeller = [],
  annualPayrollParameters,
  onSaveAnnualPayrollParameters,
  sickLeaveRecords,
  onSaveSickLeaveRecord,
  onDeleteSickLeaveRecord,
}) => {
  const currentYear = new Date().getFullYear();
  const [newYear, setNewYear] = useState<number>(currentYear);
  const [newMonth, setNewMonth] = useState<number>(1); // 1 = Ocak
  const [newTaxYear, setNewTaxYear] = useState<number>(currentYear);
  const [newTaxMonth, setNewTaxMonth] = useState<number>(2); // 1 = Ocak (varsayılan: bitiş ayı)

  // Yıl/Ay değişince ödeme-tahakkuk (vergi) yılı/ayı varsayılana sıfırlanır: bitiş ayı (ay+1; Aralık → Ocak, yıl+1).
  const resetTaxDefaults = (yr: number, mo: number) => {
    const tm = mo === 12 ? 1 : mo + 1;
    const ty = mo === 12 ? yr + 1 : yr;
    setNewTaxMonth(tm);
    setNewTaxYear(ty);
  };
  const [activeTab, setActiveTab] = useState<'editParams' | 'editDeductions' | 'annualTax' | 'tediyeTis' | 'sickLeave' | 'select' | 'new'>(
    'editParams'
  );

  // Active parameters state
  const activeKurumDegerleri =
    kurumDegerleriMap[aktifDonemId] || {
      donemId: aktifDonemId,
      ...DEFAULT_KURUM_DEGERLERI,
    };

  const sanitizeTediyeList = (list?: TediyeKalemi[]) =>
    (list || DEFAULT_TEDIYE_LISTESI).map((t) => ({
      ...t,
      ad: t.ad.replace(/\s*\(\d+\s*gün\)/i, ''),
    }));

  const [paramsForm, setParamsForm] = useState<DönemselKurumDegerleri>({
    ...activeKurumDegerleri,
    isPrimiGruplari: activeKurumDegerleri.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI,
    tediyeListesi: sanitizeTediyeList(activeKurumDegerleri.tediyeListesi),
    tisIkramiyeListesi: activeKurumDegerleri.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
  });
  const [savedSuccess, setSavedSuccess] = useState<boolean>(false);
  const [isGrupModalOpen, setIsGrupModalOpen] = useState<boolean>(false);
  const [annualTaxYear, setAnnualTaxYear] = useState<number>(currentYear);
  const [annualTaxBrackets, setAnnualTaxBrackets] = useState<TaxBracket[]>([]);
  const [annualTaxSuccess, setAnnualTaxSuccess] = useState<boolean>(false);

  // Sick leave records state
  const [selectedPersonForSick, setSelectedPersonForSick] = useState<string>(personeller[0]?.id || '');
  const [sickStartDate, setSickStartDate] = useState<string>('');
  const [sickEndDate, setSickEndDate] = useState<string>('');
  const [sickSuccessMsg, setSickSuccessMsg] = useState<string | null>(null);

  const sickLeaveList = sickLeaveRecords;

  useEffect(() => {
    if (personeller.length > 0 && !selectedPersonForSick) {
      setSelectedPersonForSick(personeller[0].id);
    }
  }, [personeller, selectedPersonForSick]);

  const handleAddSickLeave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedPersonForSick || !sickStartDate || !sickEndDate) return;

    const newRecord: SickLeaveRecord = {
      id: `sick_${selectedPersonForSick}_${Date.now()}`,
      personnelId: selectedPersonForSick,
      startDate: sickStartDate,
      endDate: sickEndDate,
    };

    try {
      await onSaveSickLeaveRecord(newRecord);
      setSickSuccessMsg('Rapor olayı başarıyla kaydedildi.');
      setTimeout(() => setSickSuccessMsg(null), 2500);
      setSickStartDate('');
      setSickEndDate('');
    } catch (err) {
      alert(`Rapor olayı kaydedilemedi: ${String(err)}`);
    }
  };

  const handleDeleteSickLeave = async (id: string) => {
    try {
      await onDeleteSickLeaveRecord(id);
    } catch (err) {
      alert(`Rapor olayı silinemedi: ${String(err)}`);
    }
  };

  useEffect(() => {
    const active = kurumDegerleriMap[aktifDonemId] || {
      donemId: aktifDonemId,
      ...DEFAULT_KURUM_DEGERLERI,
    };
    setParamsForm({
      ...active,
      isPrimiGruplari: active.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI,
      tediyeListesi: sanitizeTediyeList(active.tediyeListesi),
      tisIkramiyeListesi: active.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
    });
  }, [aktifDonemId, kurumDegerleriMap]);

  useEffect(() => {
    const activePeriod = donemler.find((period) => period.id === aktifDonemId);
    const year = activePeriod?.taxYear || newTaxYear;
    const savedParameters = annualPayrollParameters.find((parameters) => parameters.year === year);
    setAnnualTaxYear(year);
    setAnnualTaxBrackets(
      savedParameters?.gelirVergisiDilimleri.map((bracket) => ({ ...bracket })) || []
    );
  }, [aktifDonemId, annualPayrollParameters, donemler, newTaxYear]);

  // Preview generated dates
  const previewDonem = createBordroDonemi(newYear, newMonth, newTaxYear, newTaxMonth);
  const previewExists = donemler.some((d) => d.id === previewDonem.id);
  const existingPreview = donemler.find((d) => d.id === previewDonem.id);
  const previewTaxChanged =
    previewExists &&
    existingPreview !== undefined &&
    (existingPreview.taxYear !== previewDonem.taxYear ||
      existingPreview.taxMonth !== previewDonem.taxMonth);

  if (!isOpen) return null;

  const handleCreateNewPeriod = async (e: React.FormEvent) => {
    e.preventDefault();
    const newDonem = createBordroDonemi(newYear, newMonth, newTaxYear, newTaxMonth);
    const initialKurum: DönemselKurumDegerleri = {
      ...DEFAULT_KURUM_DEGERLERI,
      ...paramsForm,
      donemId: newDonem.id,
      gunlukTabanUcret: paramsForm.gunlukTabanUcret ?? DEFAULT_KURUM_DEGERLERI.gunlukTabanUcret,
      gunlukYemek: paramsForm.gunlukYemek ?? DEFAULT_KURUM_DEGERLERI.gunlukYemek,
      birlestirilmisSosyalYardim:
        paramsForm.birlestirilmisSosyalYardim ??
        DEFAULT_KURUM_DEGERLERI.birlestirilmisSosyalYardim,
      gunlukVasitaYol:
        paramsForm.gunlukVasitaYol ?? DEFAULT_KURUM_DEGERLERI.gunlukVasitaYol,
      giyimYardimi: paramsForm.giyimYardimi ?? DEFAULT_KURUM_DEGERLERI.giyimYardimi,
      hizmetZammiBirimi:
        paramsForm.hizmetZammiBirimi ?? DEFAULT_KURUM_DEGERLERI.hizmetZammiBirimi,
      isPrimiYuzde: paramsForm.isPrimiYuzde || 0,
      isPrimiGruplari: paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI,
      ekOdeme: paramsForm.ekOdeme || 0,
      tediyeListesi: paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI,
      tisIkramiyeListesi: paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
      tediyeTisNotu: paramsForm.tediyeTisNotu || DEFAULT_KURUM_DEGERLERI.tediyeTisNotu,
      sgkIsciOraniYuzde: paramsForm.sgkIsciOraniYuzde ?? DEFAULT_KURUM_DEGERLERI.sgkIsciOraniYuzde,
      issizlikIsciOraniYuzde: paramsForm.issizlikIsciOraniYuzde ?? DEFAULT_KURUM_DEGERLERI.issizlikIsciOraniYuzde,
      sgkIsverenOraniYuzde: paramsForm.sgkIsverenOraniYuzde ?? DEFAULT_KURUM_DEGERLERI.sgkIsverenOraniYuzde,
      issizlikIsverenOraniYuzde: paramsForm.issizlikIsverenOraniYuzde ?? DEFAULT_KURUM_DEGERLERI.issizlikIsverenOraniYuzde,
      gunlukYemekIstisnasiSGK: paramsForm.gunlukYemekIstisnasiSGK ?? DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiSGK,
      pekTavanKatsayisi: paramsForm.pekTavanKatsayisi ?? DEFAULT_KURUM_DEGERLERI.pekTavanKatsayisi,
      gunlukAsgariUcret: paramsForm.gunlukAsgariUcret ?? DEFAULT_KURUM_DEGERLERI.gunlukAsgariUcret,
    };

    try {
      await onCreateDonem(newDonem, initialKurum);
      await onSelectDonem(newDonem.id);
      setActiveTab('editParams');
    } catch (err) {
      alert(`Dönem kaydedilemedi: ${String(err)}`);
    }
  };

  const handleSaveParams = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await onSaveKurumDegerleri({
        ...paramsForm,
        donemId: aktifDonemId,
      });
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 2500);
    } catch (err) {
      alert(`Kurum ayarları kaydedilemedi: ${String(err)}`);
    }
  };

  const handleSaveAnnualTaxParameters = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!Number.isInteger(annualTaxYear) || annualTaxYear <= 0 || annualTaxBrackets.length === 0) {
      alert('Vergi yılı ve en az bir gelir vergisi dilimi girilmelidir.');
      return;
    }

    let previousLimit = 0;
    for (const bracket of annualTaxBrackets) {
      if (bracket.limit <= previousLimit || bracket.oran < 0 || bracket.oran > 1) {
        alert('Vergi dilimleri artan limitlere ve %0-%100 arasında oranlara sahip olmalıdır.');
        return;
      }
      previousLimit = bracket.limit;
    }

    try {
      await onSaveAnnualPayrollParameters({
        year: annualTaxYear,
        gelirVergisiDilimleri: annualTaxBrackets,
      });
      setAnnualTaxSuccess(true);
      setTimeout(() => setAnnualTaxSuccess(false), 2500);
    } catch (err) {
      alert(`Yıllık vergi parametreleri kaydedilemedi: ${String(err)}`);
    }
  };

  const updateAnnualTaxBracket = (index: number, field: keyof TaxBracket, value: number) => {
    setAnnualTaxBrackets((current) =>
      current.map((bracket, bracketIndex) =>
        bracketIndex === index ? { ...bracket, [field]: value } : bracket
      )
    );
  };

  // Tediye Handlers
  const handleTediyeChange = (
    id: number,
    field: keyof TediyeKalemi,
    val: any
  ) => {
    const currentList = paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI;
    const updated = currentList.map((item) => {
      if (item.id === id) {
        return { ...item, [field]: val };
      }
      return item;
    });
    setParamsForm({ ...paramsForm, tediyeListesi: updated });
  };

  const handleToggleTediyeActive = (id: number) => {
    const currentList = paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI;
    const updated = currentList.map((item) => ({
      ...item,
      aktifDonemdeOdensin: item.id === id ? !item.aktifDonemdeOdensin : item.aktifDonemdeOdensin,
    }));
    setParamsForm({ ...paramsForm, tediyeListesi: updated });
  };

  const handleAddTediye = () => {
    const currentList = paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI;
    const newId = Date.now();
    const nextNum = currentList.length + 1;
    const newItem: TediyeKalemi = {
      id: newId,
      ad: `${nextNum}. Tediye`,
      odemeAyi: 'Ocak',
      gunSayisi: 13,
      aktifDonemdeOdensin: false,
    };
    setParamsForm({ ...paramsForm, tediyeListesi: [...currentList, newItem] });
  };

  const handleDeleteTediye = (id: number) => {
    const currentList = paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI;
    const updated = currentList.filter((item) => item.id !== id);
    setParamsForm({ ...paramsForm, tediyeListesi: updated });
  };

  // TİS Handlers
  const handleTisChange = (
    id: number,
    field: keyof TisIkramiyeKalemi,
    val: any
  ) => {
    const currentList = paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI;
    const updated = currentList.map((item) => {
      if (item.id === id) {
        return { ...item, [field]: val };
      }
      return item;
    });
    setParamsForm({ ...paramsForm, tisIkramiyeListesi: updated });
  };

  const handleToggleTisActive = (id: number) => {
    const currentList = paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI;
    const updated = currentList.map((item) => ({
      ...item,
      aktifDonemdeOdensin: item.id === id ? !item.aktifDonemdeOdensin : item.aktifDonemdeOdensin,
    }));
    setParamsForm({ ...paramsForm, tisIkramiyeListesi: updated });
  };

  const handleAddTis = () => {
    const currentList = paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI;
    const newId = Date.now();
    const nextNum = currentList.length + 1;
    const newItem: TisIkramiyeKalemi = {
      id: newId,
      ad: `${nextNum}. TİS İkramiyesi`,
      odemeAyi: 'Ocak',
      gunSayisi: 13,
      aktifDonemdeOdensin: false,
    };
    setParamsForm({ ...paramsForm, tisIkramiyeListesi: [...currentList, newItem] });
  };

  const handleDeleteTis = (id: number) => {
    const currentList = paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI;
    const updated = currentList.filter((item) => item.id !== id);
    setParamsForm({ ...paramsForm, tisIkramiyeListesi: updated });
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 overflow-y-auto"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-2xl shadow-xl border border-slate-200 max-w-3xl w-full max-h-[90vh] flex flex-col overflow-hidden my-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="bg-slate-900 text-white px-6 py-4 flex items-center justify-between shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="p-2 bg-indigo-600/30 rounded-lg text-indigo-300">
              <Calendar className="w-5 h-5" />
            </div>
            <div>
              <h3 className="font-semibold text-base text-white">
                Bordro Dönemi ve Kurum Değerleri
              </h3>
              <p className="text-xs text-slate-400">
                15 - 14 Tarih aralıkları, dönemsel parametreler, Tediye ve TİS ikramiyeleri
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-slate-400 hover:text-white hover:bg-slate-800 p-1.5 rounded-lg transition-colors cursor-pointer"
            title="Kapat"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Selection */}
        <div className="flex border-b border-slate-200 bg-slate-50 px-6 pt-3 gap-1 sm:gap-2 overflow-x-auto no-scrollbar shrink-0">
          <button
            type="button"
            onClick={() => setActiveTab('editParams')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-t-xl border-t border-x transition-all flex items-center gap-1.5 shrink-0 ${
              activeTab === 'editParams'
                ? 'bg-white border-slate-200 text-indigo-700 shadow-2xs -mb-px'
                : 'border-transparent text-slate-600 hover:text-slate-900'
            }`}
          >
            <Settings className="w-4 h-4 text-indigo-600" />
            <span>Gelir Parametreleri</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('editDeductions')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-t-xl border-t border-x transition-all flex items-center gap-1.5 shrink-0 ${
              activeTab === 'editDeductions'
                ? 'bg-white border-slate-200 text-rose-700 shadow-2xs -mb-px'
                : 'border-transparent text-slate-600 hover:text-slate-900'
            }`}
          >
            <ShieldAlert className="w-4 h-4 text-rose-600" />
            <span>Kesinti Kalemleri & Yasal Oranlar</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('annualTax')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-t-xl border-t border-x transition-all flex items-center gap-1.5 shrink-0 ${
              activeTab === 'annualTax'
                ? 'bg-white border-slate-200 text-violet-700 shadow-2xs -mb-px'
                : 'border-transparent text-slate-600 hover:text-slate-900'
            }`}
          >
            <Percent className="w-4 h-4 text-violet-600" />
            <span>Yıllık GV Tarifesi</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('tediyeTis')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-t-xl border-t border-x transition-all flex items-center gap-1.5 shrink-0 ${
              activeTab === 'tediyeTis'
                ? 'bg-white border-slate-200 text-amber-700 shadow-2xs -mb-px'
                : 'border-transparent text-slate-600 hover:text-slate-900'
            }`}
          >
            <Gift className="w-4 h-4 text-amber-500" />
            <span>Tediye & TİS Gelirleri</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('sickLeave')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-t-xl border-t border-x transition-all flex items-center gap-1.5 shrink-0 ${
              activeTab === 'sickLeave'
                ? 'bg-white border-slate-200 text-rose-700 shadow-2xs -mb-px'
                : 'border-transparent text-slate-600 hover:text-slate-900'
            }`}
          >
            <FileText className="w-4 h-4 text-rose-600" />
            <span>Rapor Olayları & İstirahat ({sickLeaveList.length})</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('select')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-t-xl border-t border-x transition-all flex items-center gap-1.5 shrink-0 ${
              activeTab === 'select'
                ? 'bg-white border-slate-200 text-indigo-700 shadow-2xs -mb-px'
                : 'border-transparent text-slate-600 hover:text-slate-900'
            }`}
          >
            <Calendar className="w-4 h-4" />
            <span>Dönem Değiştir ({donemler.length})</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('new')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-t-xl border-t border-x transition-all flex items-center gap-1.5 shrink-0 ${
              activeTab === 'new'
                ? 'bg-white border-slate-200 text-indigo-700 shadow-2xs -mb-px'
                : 'border-transparent text-slate-600 hover:text-slate-900'
            }`}
          >
            <Plus className="w-4 h-4" />
            <span>Yeni Dönem Aç</span>
          </button>
        </div>

        {/* Body Content */}
        <div className="p-6 overflow-y-auto flex-1">
          {/* TAB 1: Edit Income Parameters for Active Period */}
          {activeTab === 'editParams' && (
            <form onSubmit={handleSaveParams} className="space-y-5">
              <div className="bg-indigo-50 border border-indigo-100 rounded-xl p-3.5 flex items-start gap-3">
                <Info className="w-5 h-5 text-indigo-600 shrink-0 mt-0.5" />
                <div className="text-xs text-indigo-900 leading-relaxed">
                  <strong>{aktifDonemId}</strong> dönemi için varsayılan gelir parametreleri. 
                  Bu birim değerler ve oranlar bordro hesaplanırken otomatik gelir tutarlarını üretir.
                </div>
              </div>

              {savedSuccess && (
                <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
                  <Check className="w-4 h-4 text-emerald-600" />
                  <span>Dönem gelir parametreleri başarıyla kaydedildi!</span>
                </div>
              )}

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Günlük Taban Ücret (TL)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.gunlukTabanUcret ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        gunlukTabanUcret: parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    Standart: {formatTL(DEFAULT_KURUM_DEGERLERI.gunlukTabanUcret)}
                  </span>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Günlük Yemek Tutarı (TL)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.gunlukYemek ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        gunlukYemek: parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    Standart: {formatTL(DEFAULT_KURUM_DEGERLERI.gunlukYemek)}
                  </span>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Birleştirilmiş Sosyal Yardım (TL / Ay)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.birlestirilmisSosyalYardim ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        birlestirilmisSosyalYardim:
                          parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    Standart: {formatTL(DEFAULT_KURUM_DEGERLERI.birlestirilmisSosyalYardim)}
                  </span>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Günlük Vasıta / Yol Tutarı (TL)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.gunlukVasitaYol ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        gunlukVasitaYol: parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    Standart: {formatTL(DEFAULT_KURUM_DEGERLERI.gunlukVasitaYol)}
                  </span>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Giyim Yardımı (TL / Ay)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.giyimYardimi ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        giyimYardimi: parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    Standart: {formatTL(DEFAULT_KURUM_DEGERLERI.giyimYardimi)}
                  </span>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Hizmet Zammı Birimi (TL / Yıl)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.hizmetZammiBirimi ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        hizmetZammiBirimi: parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    Standart: {formatTL(DEFAULT_KURUM_DEGERLERI.hizmetZammiBirimi)}
                  </span>
                </div>

                <div className="sm:col-span-2 pt-2 border-t border-slate-200">
                  <div
                    onClick={() => setIsGrupModalOpen(true)}
                    className="p-3.5 bg-indigo-50/60 hover:bg-indigo-50 border border-indigo-200/80 hover:border-indigo-300 rounded-xl cursor-pointer transition-all flex items-center justify-between group"
                  >
                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <div className="w-7 h-7 rounded-lg bg-indigo-100 text-indigo-700 flex items-center justify-center font-semibold shrink-0">
                          <Layers className="w-4 h-4" />
                        </div>
                        <span className="text-xs font-bold text-slate-900 group-hover:text-indigo-700 transition-colors">
                          İş Primi Grupları & Oranları
                        </span>
                        <span className="px-2 py-0.5 bg-indigo-100 text-indigo-700 rounded-full text-[10px] font-extrabold">
                          {(paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI).length} Grup Tanımlı
                        </span>
                      </div>
                      <p className="text-[11px] text-slate-600">
                        Personeller için tanımlı iş primi gruplarını adlandırabilir, oranlarını (%) değiştirebilir ve yeni grup ekleyebilirsiniz.
                      </p>
                      <div className="flex flex-wrap gap-1.5 pt-1">
                        {(paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI).map((g) => (
                          <span key={g.id} className="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-semibold bg-white border border-slate-200 text-slate-700">
                            {g.ad}: <strong className="ml-1 text-indigo-600">%{g.oran}</strong>
                          </span>
                        ))}
                      </div>
                    </div>
                    <div className="px-3 py-1.5 bg-white border border-indigo-200 text-indigo-700 rounded-lg text-xs font-semibold shadow-2xs group-hover:bg-indigo-600 group-hover:text-white group-hover:border-indigo-600 transition-all shrink-0 ml-3 flex items-center gap-1">
                      <span>Düzenle</span>
                      <Settings2 className="w-3.5 h-3.5" />
                    </div>
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Ek Ödeme Tutarı (TL)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.ekOdeme ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        ekOdeme: parseFloat(e.target.value) || 0,
                      })
                    }
                    placeholder="0"
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Diğer Gelir Varsayılan (TL)
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    value={paramsForm.digerGelirVarsayilan ?? ''}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        digerGelirVarsayilan: parseFloat(e.target.value) || 0,
                      })
                    }
                    placeholder="0"
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Gece Çalışması Primi (%) (GÇ Oranı)
                  </label>
                  <input
                    type="number"
                    step="0.1"
                    value={paramsForm.geceCalismaPrimiYuzde ?? 0}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        geceCalismaPrimiYuzde: parseFloat(e.target.value) || 0,
                      })
                    }
                    placeholder="0"
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    GÇ günleri için günlük taban ücret üzerinden hesaplanır (Örn: %8).
                  </span>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Gece Çalışması Tatili Primi (%) (GÇT Oranı)
                  </label>
                  <input
                    type="number"
                    step="0.1"
                    value={paramsForm.geceCalismaTatiliPrimiYuzde ?? 0}
                    onChange={(e) =>
                      setParamsForm({
                        ...paramsForm,
                        geceCalismaTatiliPrimiYuzde: parseFloat(e.target.value) || 0,
                      })
                    }
                    placeholder="0"
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                  <span className="text-[11px] text-slate-500 mt-0.5 block">
                    GÇT günleri için günlük taban ücret üzerinden hesaplanır (Örn: %10).
                  </span>
                </div>
              </div>

              <div className="pt-3 border-t border-slate-200 flex justify-end">
                <button
                  type="submit"
                  className="px-5 py-2.5 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
                >
                  <Save className="w-4 h-4" />
                  <span>Gelir Parametrelerini Kaydet</span>
                </button>
              </div>
            </form>
          )}

          {activeTab === 'annualTax' && (
            <form onSubmit={handleSaveAnnualTaxParameters} className="space-y-5">
              <div className="bg-violet-50 border border-violet-200 rounded-xl p-3.5 flex items-start gap-3">
                <Percent className="w-5 h-5 text-violet-600 shrink-0 mt-0.5" />
                <div className="text-xs text-violet-950 leading-relaxed">
                  <strong>{annualTaxYear}</strong> vergi yılı için kümülatif gelir vergisi dilimlerini
                  tanımlayın. Bordro motoru hesaplama sırasında dönemin vergi yılına ait kaydı kullanır;
                  kayıt yoksa bordro hesaplamayı reddeder.
                </div>
              </div>

              {annualTaxSuccess && (
                <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
                  <Check className="w-4 h-4 text-emerald-600" />
                  <span>{annualTaxYear} gelir vergisi tarifesi başarıyla kaydedildi.</span>
                </div>
              )}

              <div className="flex items-end gap-3">
                <div className="w-40">
                  <label className="block text-xs font-semibold text-slate-700 mb-1">Vergi Yılı</label>
                  <input
                    type="number"
                    min={2000}
                    value={annualTaxYear || ''}
                    onChange={(e) => setAnnualTaxYear(parseInt(e.target.value, 10) || 0)}
                    className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-violet-500"
                  />
                </div>
                <button
                  type="button"
                  onClick={() =>
                    setAnnualTaxBrackets((current) => [
                      ...current,
                      { limit: (current.at(-1)?.limit || 0) + 100000, oran: 0.4 },
                    ])
                  }
                  className="px-3 py-2 bg-violet-100 hover:bg-violet-200 text-violet-800 rounded-lg text-xs font-semibold flex items-center gap-1.5"
                >
                  <Plus className="w-4 h-4" />
                  Dilim Ekle
                </button>
              </div>

              {annualTaxBrackets.length === 0 ? (
                <div className="p-4 border border-dashed border-slate-300 rounded-xl text-xs text-slate-600">
                  Bu yıl için kayıtlı tarife yok. İlk dilimi ekleyerek başlayın.
                </div>
              ) : (
                <div className="border border-slate-200 rounded-xl overflow-hidden">
                  <div className="grid grid-cols-[1fr_1fr_auto] gap-3 bg-slate-100 px-4 py-2.5 text-xs font-bold text-slate-700">
                    <span>Kümülatif üst limit (TL)</span>
                    <span>Oran (%)</span>
                    <span className="sr-only">İşlem</span>
                  </div>
                  <div className="divide-y divide-slate-200">
                    {annualTaxBrackets.map((bracket, index) => (
                      <div key={`${annualTaxYear}-${index}`} className="grid grid-cols-[1fr_1fr_auto] gap-3 items-center px-4 py-3">
                        <input
                          type="number"
                          min={0}
                          step="0.01"
                          value={Number.isFinite(bracket.limit) ? bracket.limit : ''}
                          onChange={(e) => updateAnnualTaxBracket(index, 'limit', parseFloat(e.target.value) || 0)}
                          className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-violet-500"
                        />
                        <input
                          type="number"
                          min={0}
                          max={100}
                          step="0.01"
                          value={(bracket.oran * 100).toString()}
                          onChange={(e) => updateAnnualTaxBracket(index, 'oran', (parseFloat(e.target.value) || 0) / 100)}
                          className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-violet-500"
                        />
                        <button
                          type="button"
                          disabled={annualTaxBrackets.length <= 1}
                          onClick={() => setAnnualTaxBrackets((current) => current.filter((_, itemIndex) => itemIndex !== index))}
                          className="p-2 text-rose-600 hover:bg-rose-50 disabled:opacity-30 disabled:cursor-not-allowed rounded-lg"
                          title="Dilimi sil"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              <div className="pt-3 border-t border-slate-200 flex justify-end">
                <button
                  type="submit"
                  className="px-5 py-2.5 bg-violet-600 hover:bg-violet-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
                >
                  <Save className="w-4 h-4" />
                  <span>Yıllık GV Tarifesini Kaydet</span>
                </button>
              </div>
            </form>
          )}

          {/* TAB 2: Edit Deduction Parameters & Rates */}
          {activeTab === 'editDeductions' && (
            <form onSubmit={handleSaveParams} className="space-y-5">
              <div className="bg-rose-50 border border-rose-100 rounded-xl p-3.5 flex items-start gap-3">
                <ShieldAlert className="w-5 h-5 text-rose-600 shrink-0 mt-0.5" />
                <div className="text-xs text-rose-950 leading-relaxed">
                  <strong>{aktifDonemId}</strong> dönemi için kesinti parametreleri ve yasal vergi/sigorta oranları. 
                  SGK, İşsizlik, Gelir Vergisi, Damga Vergisi, Sendika ve BES kesintileri bu oranlardan hesaplanıp otomatik bordroya aktarılır.
                </div>
              </div>

              {savedSuccess && (
                <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
                  <Check className="w-4 h-4 text-emerald-600" />
                  <span>Kesinti ve yasal oran parametreleri başarıyla kaydedildi!</span>
                </div>
              )}

              {/* Group 1: Statutory Rates */}
              <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl space-y-3">
                <h4 className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <Percent className="w-4 h-4 text-indigo-600" />
                  <span>Yasal Vergi ve Sigorta Oranları</span>
                </h4>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      SGK İşçi Primi Oranı (%)
                    </label>
                    <input
                      type="number"
                      step="0.1"
                      value={paramsForm.sgkIsciOraniYuzde ?? 14}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          sgkIsciOraniYuzde: parseFloat(e.target.value) || 0,
                        })
                      }
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Yasal Standart: %14 (SGK Matrahı üzerinden)
                    </span>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      İşçi İşsizlik Sigortası Oranı (%)
                    </label>
                    <input
                      type="number"
                      step="0.1"
                      value={paramsForm.issizlikIsciOraniYuzde ?? 1}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          issizlikIsciOraniYuzde: parseFloat(e.target.value) || 0,
                        })
                      }
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Yasal Standart: %1 (SGK Matrahı üzerinden)
                    </span>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Gelir Vergisi Tarifesi
                    </label>
                    <div className="px-3 py-2 bg-slate-100 border border-slate-300 rounded-lg text-xs font-semibold text-slate-800">
                      Yıllık GV parametre tablosu kullanılır
                    </div>
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Tarifeyi “Yıllık GV Tarifesi” sekmesinden yönetin.
                    </span>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Damga Vergisi Oranı (‰ Binde)
                    </label>
                    <input
                      type="number"
                      step="0.001"
                      value={paramsForm.damgaVergisiOraniBinde ?? 7.59}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          damgaVergisiOraniBinde: parseFloat(e.target.value) || 0,
                        })
                      }
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Yasal Standart: 7.59 ‰ (%0.759 Brüt Toplam üzerinden)
                    </span>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      SGK İşveren Prim Oranı (%)
                    </label>
                    <input
                      type="number"
                      step="0.01"
                      value={paramsForm.sgkIsverenOraniYuzde ?? 21.75}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          sgkIsverenOraniYuzde: parseFloat(e.target.value) || 0,
                        })
                      }
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Yasal Standart: %21,75 (PEK Matrahı üzerinden işveren payı)
                    </span>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      İşveren İşsizlik Sigortası Oranı (%)
                    </label>
                    <input
                      type="number"
                      step="0.01"
                      value={paramsForm.issizlikIsverenOraniYuzde ?? 2.0}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          issizlikIsverenOraniYuzde: parseFloat(e.target.value) || 0,
                        })
                      }
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Yasal Standart: %2,00 (PEK Matrahı üzerinden işveren payı)
                    </span>
                  </div>
                </div>
              </div>

              {/* Group 2: Union & Pension (Sendika & BES) */}
              <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl space-y-3">
                <h4 className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <Award className="w-4 h-4 text-amber-600" />
                  <span>Sendika Aidatı ve Otomatik Katılım (OKS) Parametreleri</span>
                </h4>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Sendika Aidatı — Günlük Çıplak Ücret Oranı (%)
                    </label>
                    <input
                      type="number"
                      step="1"
                      value={paramsForm.sendikaAidatiYuzde ?? 65}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          sendikaAidatiYuzde: parseFloat(e.target.value) || 0,
                        })
                      }
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Günlük çıplak ücret × %65 (2.443,28 × 0,65 = 1.588,13 TL)
                    </span>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Genel Sabit Sendika Aidatı Tutarı (TL) (Opsiyonel Maktu)
                    </label>
                    <input
                      type="number"
                      step="0.01"
                      value={paramsForm.sabitSendikaAidati ?? 0}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          sabitSendikaAidati: parseFloat(e.target.value) || 0,
                        })
                      }
                      placeholder="0.00"
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      0 kalırsa günlük çıplak ücretin %65'i uygulanır
                    </span>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Otomatik Katılım (OKS) Oranı (%)
                    </label>
                    <input
                      type="number"
                      step="0.1"
                      value={paramsForm.besOraniYuzde ?? 3}
                      onChange={(e) =>
                        setParamsForm({
                          ...paramsForm,
                          besOraniYuzde: parseFloat(e.target.value) || 0,
                        })
                      }
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
                    />
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      PEK × %3 (Hesaplanan tutarın kuruş kısmı atılır)
                    </span>
                  </div>
                </div>

                {/* PEK (Prime Esas Kazanç) Parametreleri */}
                <div className="pt-4 border-t border-slate-200">
                  <h4 className="text-xs font-bold text-slate-900 uppercase tracking-wider mb-3 flex items-center gap-1.5">
                    <span>SGK Prime Esas Kazanç (PEK) Parametreleri (2026)</span>
                  </h4>
                  <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                    <div>
                      <label className="block text-xs font-semibold text-slate-700 mb-1">
                        Günlük SGK Yemek İstisnası (TL)
                      </label>
                      <input
                        type="number"
                        step="0.01"
                        value={paramsForm.gunlukYemekIstisnasiSGK ?? 300.00}
                        onChange={(e) =>
                          setParamsForm({
                            ...paramsForm,
                            gunlukYemekIstisnasiSGK: parseFloat(e.target.value) || 0,
                          })
                        }
                        className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                      />
                      <span className="text-[11px] text-slate-500 mt-0.5 block">
                        2026-08 dönemi için 300,00 TL (17.04.2026 sonrası)
                      </span>
                    </div>

                    <div>
                      <label className="block text-xs font-semibold text-slate-700 mb-1">
                        PEK Tavan Katsayısı (2026)
                      </label>
                      <input
                        type="number"
                        step="0.5"
                        value={paramsForm.pekTavanKatsayisi ?? 9}
                        onChange={(e) =>
                          setParamsForm({
                            ...paramsForm,
                            pekTavanKatsayisi: parseFloat(e.target.value) || 9,
                          })
                        }
                        className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                      />
                      <span className="text-[11px] text-slate-500 mt-0.5 block">
                        2026 için tavan katsayısı: 9
                      </span>
                    </div>

                    <div>
                      <label className="block text-xs font-semibold text-slate-700 mb-1">
                        Günlük Brüt Asgari Ücret (TL)
                      </label>
                      <input
                        type="number"
                        step="0.01"
                        value={paramsForm.gunlukAsgariUcret ?? 1101.00}
                        onChange={(e) =>
                          setParamsForm({
                            ...paramsForm,
                            gunlukAsgariUcret: parseFloat(e.target.value) || 0,
                          })
                        }
                        className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                      />
                      <span className="text-[11px] text-slate-500 mt-0.5 block">
                        PEK Alt Sınırı = 1.101,00 TL (30 gün = 33.030 TL)
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              <div className="pt-3 border-t border-slate-200 flex justify-end">
                <button
                  type="submit"
                  className="px-5 py-2.5 bg-rose-600 hover:bg-rose-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
                >
                  <Save className="w-4 h-4" />
                  <span>Kesinti ve Yasal Oranları Kaydet</span>
                </button>
              </div>
            </form>
          )}

          {/* TAB 2: Tediye & TİS İkramiyeleri Tab */}
          {activeTab === 'tediyeTis' && (
            <form onSubmit={handleSaveParams} className="space-y-6">
              {/* Note banner (Editable) */}
              <div className="bg-amber-50/90 border border-amber-200 rounded-2xl p-4 space-y-2">
                <div className="flex items-center justify-between">
                  <div className="font-bold text-xs text-amber-900 flex items-center gap-1.5">
                    <Info className="w-4 h-4 text-amber-600" />
                    <span>Not</span>
                  </div>
                  <span className="text-[11px] text-amber-700 font-medium">
                    (Mevzuat / kanun değişikliklerine göre düzenlenebilir)
                  </span>
                </div>
                <textarea
                  rows={3}
                  value={
                    paramsForm.tediyeTisNotu ?? DEFAULT_KURUM_DEGERLERI.tediyeTisNotu
                  }
                  onChange={(e) =>
                    setParamsForm({ ...paramsForm, tediyeTisNotu: e.target.value })
                  }
                  placeholder="Açıklama / Not metnini buraya yazabilirsiniz..."
                  className="w-full text-xs text-amber-950 bg-amber-100/50 border border-amber-300 rounded-xl p-2.5 leading-relaxed focus:bg-white focus:ring-2 focus:ring-amber-500 focus:outline-none transition-all resize-y"
                />
              </div>

              {savedSuccess && (
                <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
                  <Check className="w-4 h-4 text-emerald-600" />
                  <span>Tediye ve TİS ikramiye ayarları başarıyla kaydedildi!</span>
                </div>
              )}

              {/* Section 1: Tediye Ödemeleri */}
              <div className="space-y-3">
                <div className="flex items-center justify-between border-b border-slate-200 pb-2">
                  <div className="flex items-center gap-2">
                    <div className="p-1.5 bg-amber-100 text-amber-800 rounded-lg">
                      <Gift className="w-4 h-4" />
                    </div>
                    <h4 className="font-bold text-sm text-slate-900">
                      Tediye Ödemeleri
                    </h4>
                  </div>
                  <button
                    type="button"
                    onClick={handleAddTediye}
                    className="px-2.5 py-1 bg-amber-100 hover:bg-amber-200 text-amber-800 rounded-lg text-xs font-semibold transition-colors flex items-center gap-1 cursor-pointer"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    <span>Yeni Tediye Ekle</span>
                  </button>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  {(paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI).map((t) => {
                    const calculatedAmount = t.sabitTutar && t.sabitTutar > 0
                      ? t.sabitTutar
                      : (t.gunSayisi || 13) * (paramsForm.gunlukTabanUcret || 0);

                    return (
                      <div
                        key={t.id}
                        className={`p-3.5 rounded-2xl border transition-all ${
                          t.aktifDonemdeOdensin
                            ? 'bg-amber-50/60 border-amber-300 ring-2 ring-amber-400/30'
                            : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                        }`}
                      >
                        <div className="flex items-center justify-between mb-2 gap-2">
                          <div className="flex items-center gap-1.5 flex-1 min-w-0">
                            <span className="w-2 h-2 rounded-full bg-amber-500 shrink-0"></span>
                            <input
                              type="text"
                              value={t.ad}
                              onChange={(e) =>
                                handleTediyeChange(t.id, 'ad', e.target.value)
                              }
                              placeholder="Tediye Adı"
                              className="font-bold text-xs text-slate-900 bg-white border border-slate-200 px-2 py-1 rounded-lg focus:ring-2 focus:ring-amber-500 w-full"
                            />
                          </div>

                          <div className="flex items-center gap-1 shrink-0">
                            <button
                              type="button"
                              onClick={() => handleToggleTediyeActive(t.id)}
                              className={`px-2.5 py-1 rounded-full text-[11px] font-bold transition-all flex items-center gap-1 ${
                                t.aktifDonemdeOdensin
                                  ? 'bg-amber-600 text-white shadow-2xs'
                                  : 'bg-slate-200 text-slate-600 hover:bg-slate-300'
                              }`}
                            >
                              {t.aktifDonemdeOdensin ? (
                                <>
                                  <Check className="w-3 h-3" />
                                  <span>Ödeniyor</span>
                                </>
                              ) : (
                                <span>Pasif</span>
                              )}
                            </button>

                            <button
                              type="button"
                              onClick={() => handleDeleteTediye(t.id)}
                              title="Tediyeyi Sil"
                              className="p-1 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer"
                            >
                              <Trash2 className="w-4 h-4" />
                            </button>
                          </div>
                        </div>

                        <div className="grid grid-cols-2 gap-2 text-xs">
                          <div>
                            <label className="block text-[11px] text-slate-500 mb-0.5">
                              Ödeme Ayı
                            </label>
                            <select
                              value={t.odemeAyi}
                              onChange={(e) =>
                                handleTediyeChange(t.id, 'odemeAyi', e.target.value)
                              }
                              className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-800"
                            >
                              {AY_ISIMLERI.map((m) => (
                                <option key={m} value={m}>
                                  {m}
                                </option>
                              ))}
                            </select>
                          </div>

                          <div>
                            <label className="block text-[11px] text-slate-500 mb-0.5">
                              Hak Edilen Gün
                            </label>
                            <input
                              type="number"
                              value={t.gunSayisi ?? 13}
                              onChange={(e) =>
                                handleTediyeChange(
                                  t.id,
                                  'gunSayisi',
                                  parseFloat(e.target.value) || 0
                                )
                              }
                              className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-mono font-bold text-slate-900"
                            />
                          </div>
                        </div>

                        <div className="mt-2.5 pt-2 border-t border-slate-200/60 flex items-center justify-between text-xs">
                          <span className="text-slate-500">Tediye Brüt Tutar:</span>
                          <span className="font-bold font-mono text-amber-900">
                            {formatTL(calculatedAmount)}
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Section 2: TİS İkramiye Ödemeleri */}
              <div className="space-y-3 pt-2">
                <div className="flex items-center justify-between border-b border-slate-200 pb-2">
                  <div className="flex items-center gap-2">
                    <div className="p-1.5 bg-indigo-100 text-indigo-800 rounded-lg">
                      <Award className="w-4 h-4" />
                    </div>
                    <h4 className="font-bold text-sm text-slate-900">
                      Toplu İş Sözleşmesi (TİS) İkramiye Ödemeleri
                    </h4>
                  </div>
                  <button
                    type="button"
                    onClick={handleAddTis}
                    className="px-2.5 py-1 bg-indigo-100 hover:bg-indigo-200 text-indigo-800 rounded-lg text-xs font-semibold transition-colors flex items-center gap-1 cursor-pointer"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    <span>Yeni İkramiye Ekle</span>
                  </button>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  {(
                    paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI
                  ).map((tis) => {
                    const calculatedAmount = tis.sabitTutar && tis.sabitTutar > 0
                      ? tis.sabitTutar
                      : (tis.gunSayisi || 0) * (paramsForm.gunlukTabanUcret || 0);

                    return (
                      <div
                        key={tis.id}
                        className={`p-3.5 rounded-2xl border transition-all ${
                          tis.aktifDonemdeOdensin
                            ? 'bg-indigo-50/60 border-indigo-300 ring-2 ring-indigo-400/30'
                            : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                        }`}
                      >
                        <div className="flex items-center justify-between mb-2 gap-2">
                          <div className="flex items-center gap-1.5 flex-1 min-w-0">
                            <span className="w-2 h-2 rounded-full bg-indigo-500 shrink-0"></span>
                            <input
                              type="text"
                              value={tis.ad}
                              onChange={(e) =>
                                handleTisChange(tis.id, 'ad', e.target.value)
                              }
                              placeholder="İkramiye Adı"
                              className="font-bold text-xs text-slate-900 bg-white border border-slate-200 px-2 py-1 rounded-lg focus:ring-2 focus:ring-indigo-500 w-full"
                            />
                          </div>

                          <div className="flex items-center gap-1 shrink-0">
                            <button
                              type="button"
                              onClick={() => handleToggleTisActive(tis.id)}
                              className={`px-2.5 py-1 rounded-full text-[11px] font-bold transition-all flex items-center gap-1 ${
                                tis.aktifDonemdeOdensin
                                  ? 'bg-indigo-600 text-white shadow-2xs'
                                  : 'bg-slate-200 text-slate-600 hover:bg-slate-300'
                              }`}
                            >
                              {tis.aktifDonemdeOdensin ? (
                                <>
                                  <Check className="w-3 h-3" />
                                  <span>Ödeniyor</span>
                                </>
                              ) : (
                                <span>Pasif</span>
                              )}
                            </button>

                            <button
                              type="button"
                              onClick={() => handleDeleteTis(tis.id)}
                              title="İkramiyeyi Sil"
                              className="p-1 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer"
                            >
                              <Trash2 className="w-4 h-4" />
                            </button>
                          </div>
                        </div>

                        <div className="grid grid-cols-2 gap-2 text-xs">
                          <div>
                            <label className="block text-[11px] text-slate-500 mb-0.5">
                              Ödeme Ayı
                            </label>
                            <select
                              value={tis.odemeAyi || ''}
                              onChange={(e) =>
                                handleTisChange(tis.id, 'odemeAyi', e.target.value)
                              }
                              className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-800"
                            >
                              <option value="">Seçiniz (Manuel)</option>
                              {AY_ISIMLERI.map((m) => (
                                <option key={m} value={m}>
                                  {m}
                                </option>
                              ))}
                            </select>
                          </div>

                          <div>
                            <label className="block text-[11px] text-slate-500 mb-0.5">
                              Hak Edilen Gün
                            </label>
                            <input
                              type="number"
                              value={tis.gunSayisi || ''}
                              placeholder="0"
                              onChange={(e) =>
                                handleTisChange(
                                  tis.id,
                                  'gunSayisi',
                                  parseFloat(e.target.value) || 0
                                )
                              }
                              className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-mono font-bold text-slate-900"
                            />
                          </div>
                        </div>

                        <div className="mt-2.5 pt-2 border-t border-slate-200/60 flex items-center justify-between text-xs">
                          <span className="text-slate-500">TİS İkramiye Brüt Tutar:</span>
                          <span className="font-bold font-mono text-indigo-900">
                            {formatTL(calculatedAmount)}
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Submit footer */}
              <div className="pt-3 border-t border-slate-200 flex justify-end">
                <button
                  type="submit"
                  className="px-5 py-2.5 bg-amber-600 hover:bg-amber-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
                >
                  <Save className="w-4 h-4" />
                  <span>Tediye & TİS Ayarlarını Kaydet</span>
                </button>
              </div>
            </form>
          )}

          {/* TAB: Rapor Olayları & İstirahat Yönetimi */}
          {activeTab === 'sickLeave' && (
            <div className="space-y-6">
              <div className="bg-rose-50/90 border border-rose-200 rounded-2xl p-4 space-y-2">
                <div className="flex items-center justify-between">
                  <div className="font-bold text-xs text-rose-950 flex items-center gap-1.5">
                    <FileText className="w-4 h-4 text-rose-600" />
                    <span>Kurum Raporlu Gün Ödeme Kuralı (Takvim Yılı)</span>
                  </div>
                  <span className="text-[11px] text-rose-700 font-semibold">
                    (Yılda ilk 5 raporda en fazla ilk 2 gün)
                  </span>
                </div>
                <p className="text-xs text-rose-900 leading-relaxed">
                  Bir işçinin takvim yılı içinde aldığı ilk 5 ayrı sağlık raporunun ilk 2'şer günü kurum tarafından ödenir (6. ve sonraki rapor olaylarında kurum ödemesi 0 gündür). 15-14 dönem sınırından bölünen rapor olaylarında ilk 2 gün hakkı sadece 1 kez kullandırılır.
                </p>
              </div>

              {sickSuccessMsg && (
                <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
                  <Check className="w-4 h-4 text-emerald-600" />
                  <span>{sickSuccessMsg}</span>
                </div>
              )}

              {/* Add New Sick Leave Record Form */}
              <form onSubmit={handleAddSickLeave} className="p-4 bg-slate-50 border border-slate-200 rounded-2xl space-y-3">
                <h4 className="font-bold text-xs text-slate-900 flex items-center gap-1.5">
                  <Plus className="w-4 h-4 text-indigo-600" />
                  <span>Yeni Rapor Olayı (İstirahat Kaydı) Ekle</span>
                </h4>

                <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                  <div>
                    <label className="block text-[11px] font-semibold text-slate-700 mb-1">
                      Personel Seçimi
                    </label>
                    <select
                      value={selectedPersonForSick}
                      onChange={(e) => setSelectedPersonForSick(e.target.value)}
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-900 focus:ring-2 focus:ring-indigo-500"
                    >
                      {personeller.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.ad} {p.soyad} (TC: {p.tcNo})
                        </option>
                      ))}
                    </select>
                  </div>

                  <div>
                    <label className="block text-[11px] font-semibold text-slate-700 mb-1">
                      Rapor Başlangıç Tarihi
                    </label>
                    <input
                      type="date"
                      value={sickStartDate}
                      onChange={(e) => setSickStartDate(e.target.value)}
                      required
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-indigo-500"
                    />
                  </div>

                  <div>
                    <label className="block text-[11px] font-semibold text-slate-700 mb-1">
                      Rapor Bitiş Tarihi
                    </label>
                    <input
                      type="date"
                      value={sickEndDate}
                      onChange={(e) => setSickEndDate(e.target.value)}
                      required
                      className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-indigo-500"
                    />
                  </div>
                </div>

                <div className="flex justify-end pt-1">
                  <button
                    type="submit"
                    className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    <span>Rapor Olayını Kaydet</span>
                  </button>
                </div>
              </form>

              {/* Sick Leave Records Table */}
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold text-slate-800">
                    Kayıtlı Rapor Olayları ({sickLeaveList.length})
                  </span>
                </div>

                {sickLeaveList.length === 0 ? (
                  <div className="text-center py-8 text-xs text-slate-500 bg-slate-50 rounded-2xl border border-slate-200">
                    Henüz kayıtlı bir rapor olayı bulunmuyor. Yukarıdaki formdan ekleyebilirsiniz.
                  </div>
                ) : (
                  <div className="border border-slate-200 rounded-2xl overflow-hidden overflow-x-auto">
                    <table className="w-full text-left text-xs">
                      <thead className="bg-slate-100 text-slate-700 font-bold uppercase text-[10px] tracking-wider border-b border-slate-200">
                        <tr>
                          <th className="p-3">Personel</th>
                          <th className="p-3">Başlangıç Tarihi</th>
                          <th className="p-3">Bitiş Tarihi</th>
                          <th className="p-3 text-center">Toplam Gün</th>
                          <th className="p-3 text-right">İşlem</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-100">
                        {sickLeaveList.map((rec) => {
                          const person = personeller.find((p) => p.id === rec.personnelId);
                          let totalDays = 1;
                          try {
                            const d1 = new Date(rec.startDate + 'T00:00:00');
                            const d2 = new Date(rec.endDate + 'T00:00:00');
                            totalDays = Math.max(1, Math.round((d2.getTime() - d1.getTime()) / (1000 * 60 * 60 * 24)) + 1);
                          } catch {
                            totalDays = 1;
                          }

                          return (
                            <tr key={rec.id} className="hover:bg-slate-50">
                              <td className="p-3 font-semibold text-slate-900">
                                {person ? `${person.ad} ${person.soyad}` : rec.personnelId}
                              </td>
                              <td className="p-3 font-mono text-slate-700">{rec.startDate}</td>
                              <td className="p-3 font-mono text-slate-700">{rec.endDate}</td>
                              <td className="p-3 text-center font-bold font-mono text-rose-700">
                                {totalDays} Gün
                              </td>
                              <td className="p-3 text-right">
                                <button
                                  type="button"
                                  onClick={() => handleDeleteSickLeave(rec.id)}
                                  className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer"
                                  title="Rapor Olayını Sil"
                                >
                                  <Trash2 className="w-4 h-4" />
                                </button>
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* TAB 3: Select Existing Period */}
          {activeTab === 'select' && (
            <div className="space-y-3">
              <div className="text-xs font-semibold text-slate-700 mb-2">
                Mevcut Dönem Listesi
              </div>
              <div className="divide-y divide-slate-100 border border-slate-200 rounded-2xl overflow-hidden max-h-80 overflow-y-auto">
                {donemler.map((d) => {
                  const isSelected = d.id === aktifDonemId;
                  return (
                    <div
                      key={d.id}
                      onClick={() => {
                        onSelectDonem(d.id);
                        const active = kurumDegerleriMap[d.id] || {
                          donemId: d.id,
                          ...DEFAULT_KURUM_DEGERLERI,
                        };
                        setParamsForm({
                          ...active,
                          tediyeListesi: active.tediyeListesi || DEFAULT_TEDIYE_LISTESI,
                          tisIkramiyeListesi: active.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
                        });
                      }}
                      className={`p-4 flex items-center justify-between cursor-pointer transition-colors ${
                        isSelected
                          ? 'bg-indigo-50/80 hover:bg-indigo-50'
                          : 'hover:bg-slate-50'
                      }`}
                    >
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="font-semibold text-sm text-slate-900">
                            {d.donemAdi}
                          </span>
                          {isSelected && (
                            <span className="px-2 py-0.5 bg-indigo-600 text-white text-[10px] font-bold rounded-full uppercase">
                              Aktif
                            </span>
                          )}
                        </div>
                        <div className="text-xs text-slate-500 font-mono mt-0.5">
                          Tarih Aralığı: {d.baslangicTarihi} ile {d.bitisTarihi} arası
                        </div>
                      </div>

                      <div className="text-right">
                        <span className="text-xs font-semibold text-indigo-600">
                          Seç
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* TAB 4: Create New 15-14 Period */}
          {activeTab === 'new' && (
            <form onSubmit={handleCreateNewPeriod} className="space-y-5">
              <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl text-xs space-y-2">
                <div className="font-semibold text-slate-800">
                  Otomatik 15–14 Dönem Kuralı
                </div>
                <div className="text-slate-600 leading-relaxed">
                  4/D Sürekli işçi mevzuatına göre her bordro dönemi seçilen ayın <strong>15'i</strong> ile bir sonraki ayın <strong>14'ü</strong> arasını kapsar.
                </div>
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1.5">
                    Yıl
                  </label>
                  <select
                    value={newYear}
                    onChange={(e) => {
                      const y = parseInt(e.target.value, 10);
                      setNewYear(y);
                      resetTaxDefaults(y, newMonth);
                    }}
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
                  >
                    {[2024, 2025, 2026, 2027, 2028].map((y) => (
                      <option key={y} value={y}>
                        {y}
                      </option>
                    ))}
                  </select>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1.5">
                    Ay
                  </label>
                  <select
                    value={newMonth}
                    onChange={(e) => {
                      const m = parseInt(e.target.value, 10);
                      setNewMonth(m);
                      resetTaxDefaults(newYear, m);
                    }}
                    className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
                  >
                    {AY_ISIMLERI.map((ayAdi, idx) => (
                      <option key={idx + 1} value={idx + 1}>
                        {ayAdi} ({idx + 1}. Ay)
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Preview */}
              <div className="bg-indigo-50/70 border border-indigo-200 rounded-xl p-4 space-y-1">
                <div className="text-[10px] uppercase font-bold text-indigo-700 tracking-wider">
                  Oluşturulacak Dönem Önizlemesi
                </div>
                <div className="font-bold text-sm text-indigo-950">
                  {previewDonem.donemAdi}
                </div>
                <div className="text-xs text-indigo-800 font-mono">
                  {previewDonem.baslangicTarihi} → {previewDonem.bitisTarihi}
                </div>
                <div className="text-xs text-indigo-800 font-mono">
                  Ödeme/Tahakkuk Ayı: {AY_ISIMLERI[previewDonem.taxMonth - 1]} {previewDonem.taxYear}
                </div>
              </div>

              {previewExists && (
                <div className="bg-orange-50 border border-orange-300 rounded-xl p-3.5 flex items-start gap-2.5 text-xs text-orange-900">
                  <Info className="w-5 h-5 text-orange-600 shrink-0 mt-0.5" />
                  <div className="leading-relaxed">
                    <strong>{previewDonem.id}</strong> dönemi zaten mevcut; bu kayıt güncelleme olarak işlenecek.
                    {previewTaxChanged && (
                      <> Vergi Yılı/Ayı değişikliği yalnız bu dönemde hiç bordro kaydı yoksa kaydedilir; bordro kaydı varsa sistem bu iki alanı kilitleyecektir.</>
                    )}
                  </div>
                </div>
              )}

              <div className="bg-amber-50/60 border border-amber-200 rounded-xl p-4 space-y-3">
                <div className="text-[10px] uppercase font-bold text-amber-700 tracking-wider">
                  Ödeme / Tahakkuk (Vergi) Ayı — GİB 7349 S.K.
                </div>
                <div className="text-[11px] text-amber-800 leading-relaxed">
                  Asgari ücret GV istisnası ve referans kümülatifi bu yıl/ayın takvim konumuna göre hesaplanır
                  (varsayılan: dönem bitiş ayı; Aralık dönemi → Ocak, yıl +1).
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1.5">
                      Vergi Yılı
                    </label>
                    <select
                      value={newTaxYear}
                      onChange={(e) => setNewTaxYear(parseInt(e.target.value, 10))}
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
                    >
                      {[2024, 2025, 2026, 2027, 2028].map((y) => (
                        <option key={y} value={y}>
                          {y}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1.5">
                      Vergi Ayı
                    </label>
                    <select
                      value={newTaxMonth}
                      onChange={(e) => setNewTaxMonth(parseInt(e.target.value, 10))}
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
                    >
                      {AY_ISIMLERI.map((ayAdi, idx) => (
                        <option key={idx + 1} value={idx + 1}>
                          {ayAdi} ({idx + 1}. Ay)
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
              </div>

              <div className="pt-2 flex justify-end">
                <button
                  type="submit"
                  className="px-5 py-2.5 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
                >
                  <Plus className="w-4 h-4" />
                  <span>Dönemi Oluştur ve Geç</span>
                </button>
              </div>
            </form>
          )}
        </div>

        {/* Footer */}
        <div className="bg-slate-50 px-6 py-3 border-t border-slate-200 flex justify-between items-center shrink-0">
          <span className="text-xs text-slate-500">
            Aktif Dönem: <strong>{aktifDonemId}</strong>
          </span>
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 bg-slate-200 hover:bg-slate-300 text-slate-700 rounded-xl text-xs font-semibold transition-colors"
          >
            Kapat
          </button>
        </div>
      </div>

      {/* Sub-modal: İş Primi Grupları Yönetimi */}
      {isGrupModalOpen && (
        <div className="fixed inset-0 z-60 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4">
          <div className="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-200">
            <div className="px-5 py-4 border-b border-slate-200 flex items-center justify-between bg-slate-50">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-xl bg-indigo-100 text-indigo-700 flex items-center justify-center shrink-0">
                  <Layers className="w-4 h-4" />
                </div>
                <div>
                  <h3 className="font-bold text-slate-900 text-sm">İş Primi Grupları & Oranları</h3>
                  <p className="text-[11px] text-slate-500">Grup adlarını ve prim oranlarını düzenleyin</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setIsGrupModalOpen(false)}
                className="p-1.5 text-slate-400 hover:text-slate-600 hover:bg-slate-200 rounded-lg transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-5 space-y-4 max-h-[60vh] overflow-y-auto">
              <div className="flex items-center justify-between pb-1">
                <span className="text-xs font-bold text-slate-700">
                  Tanımlı Gruplar ({(paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI).length})
                </span>
                <button
                  type="button"
                  onClick={() => {
                    const current = paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI;
                    const newNum = current.length + 1;
                    const newGroup: IsPrimiGrupItem = {
                      id: `grup_${Date.now()}`,
                      ad: `${newNum}. Grup`,
                      oran: 5,
                      aktif: true,
                    };
                    setParamsForm({
                      ...paramsForm,
                      isPrimiGruplari: [...current, newGroup],
                    });
                  }}
                  className="px-2.5 py-1 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 rounded-lg text-xs font-semibold transition-colors flex items-center gap-1 cursor-pointer"
                >
                  <Plus className="w-3.5 h-3.5" />
                  <span>Yeni Grup Ekle</span>
                </button>
              </div>

              <div className="space-y-2">
                {(paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI).map((grupItem, idx) => (
                  <div key={grupItem.id || idx} className="flex items-center gap-2 bg-slate-50 p-2.5 rounded-xl border border-slate-200">
                    <div className="flex-1">
                      <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">Grup Adı</label>
                      <input
                        type="text"
                        value={grupItem.ad}
                        onChange={(e) => {
                          const current = [...(paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI)];
                          current[idx] = { ...current[idx], ad: e.target.value };
                          setParamsForm({ ...paramsForm, isPrimiGruplari: current });
                        }}
                        placeholder="Grup Adı (Örn: Temizlik İşçisi)"
                        className="w-full px-3 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-900 focus:ring-2 focus:ring-indigo-500"
                      />
                    </div>
                    <div className="w-28">
                      <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">İş Primi Oranı</label>
                      <div className="flex items-center gap-1">
                        <span className="text-xs font-bold text-slate-500">%</span>
                        <input
                          type="number"
                          step="0.1"
                          min="0"
                          max="100"
                          value={grupItem.oran ?? ''}
                          onChange={(e) => {
                            const current = [...(paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI)];
                            current[idx] = {
                              ...current[idx],
                              oran: parseFloat(e.target.value) || 0,
                            };
                            setParamsForm({ ...paramsForm, isPrimiGruplari: current });
                          }}
                          placeholder="Oran"
                          className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs text-slate-900 font-mono font-bold text-center focus:ring-2 focus:ring-indigo-500"
                        />
                      </div>
                    </div>
                    {(paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI).length > 1 && (
                      <button
                        type="button"
                        onClick={() => {
                          const current = (paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI).filter(
                            (_, i) => i !== idx
                          );
                          setParamsForm({ ...paramsForm, isPrimiGruplari: current });
                        }}
                        title="Grubu Sil"
                        className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer mt-4"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    )}
                  </div>
                ))}
              </div>
            </div>

            <div className="p-4 bg-slate-50 border-t border-slate-200 flex justify-end">
              <button
                type="button"
                onClick={() => setIsGrupModalOpen(false)}
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer"
              >
                <Check className="w-4 h-4" />
                <span>Tamam</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
