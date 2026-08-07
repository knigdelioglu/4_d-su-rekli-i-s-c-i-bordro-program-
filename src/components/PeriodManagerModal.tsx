/**
 * Section 1 & 4: Bordro Dönemi & Dönemsel Kurum Değerleri Management
 */

import React, { useState, useEffect } from 'react';
import { Calendar, Save, Plus, Settings, Check, X, Info, Gift, Award, Sparkles, ShieldAlert, Percent, Trash2, Layers, Settings2 } from 'lucide-react';
import {
  BordroDonemi,
  DönemselKurumDegerleri,
  IsPrimiGrupItem,
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
  onSelectDonem: (donemId: string) => void;
  onCreateDonem: (
    donem: BordroDonemi,
    kurumDegerleri: DönemselKurumDegerleri
  ) => void;
  kurumDegerleriMap: Record<string, DönemselKurumDegerleri>;
  onSaveKurumDegerleri: (kurumDegerleri: DönemselKurumDegerleri) => void;
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
}) => {
  const currentYear = new Date().getFullYear();
  const [newYear, setNewYear] = useState<number>(currentYear);
  const [newMonth, setNewMonth] = useState<number>(1); // 1 = Ocak
  const [activeTab, setActiveTab] = useState<'editParams' | 'editDeductions' | 'tediyeTis' | 'select' | 'new'>(
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

  // Preview generated dates
  const previewDonem = createBordroDonemi(newYear, newMonth);

  if (!isOpen) return null;

  const handleCreateNewPeriod = (e: React.FormEvent) => {
    e.preventDefault();
    const newDonem = createBordroDonemi(newYear, newMonth);
    const initialKurum: DönemselKurumDegerleri = {
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
    };

    onCreateDonem(newDonem, initialKurum);
    onSelectDonem(newDonem.id);
    setActiveTab('editParams');
  };

  const handleSaveParams = (e: React.FormEvent) => {
    e.preventDefault();
    onSaveKurumDegerleri({
      ...paramsForm,
      donemId: aktifDonemId,
    });
    setSavedSuccess(true);
    setTimeout(() => setSavedSuccess(false), 2500);
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
                      2026 Kümülatif Vergi Tarifesi (%15, %20, %27, %35, %40)
                    </div>
                    <span className="text-[11px] text-slate-500 mt-0.5 block">
                      Otomatik kümülatif dilimli hesaplama
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
                    onChange={(e) => setNewYear(parseInt(e.target.value, 10))}
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
                    onChange={(e) => setNewMonth(parseInt(e.target.value, 10))}
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
