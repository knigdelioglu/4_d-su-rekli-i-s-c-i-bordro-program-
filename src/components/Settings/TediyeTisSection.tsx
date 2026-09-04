import React from 'react';
import { Award, Check, Gift, Info, Plus, Save, Trash2 } from 'lucide-react';
import type {
  DönemselKurumDegerleri,
  TediyeKalemi,
  TisIkramiyeKalemi,
} from '../../types/payroll';
import {
  AY_ISIMLERI,
  DEFAULT_KURUM_DEGERLERI,
  DEFAULT_TEDIYE_LISTESI,
  DEFAULT_TIS_IKRAMIYE_LISTESI,
} from '../../utils/payrollPresentation';

interface TediyeTisSectionProps {
  paramsForm: DönemselKurumDegerleri;
  setParamsForm: React.Dispatch<React.SetStateAction<DönemselKurumDegerleri>>;
  savedSuccess: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
}

export const TediyeTisSection: React.FC<TediyeTisSectionProps> = ({
  paramsForm,
  setParamsForm,
  savedSuccess,
  onSubmit,
}) => {
  const tediyeListesi = paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI;
  const tisListesi = paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI;

  const handleTediyeChange = (id: number, field: keyof TediyeKalemi, value: any) => {
    setParamsForm((current) => ({
      ...current,
      tediyeListesi: (current.tediyeListesi || DEFAULT_TEDIYE_LISTESI).map((item) =>
        item.id === id ? { ...item, [field]: value } : item
      ),
    }));
  };

  const handleToggleTediyeActive = (id: number) => {
    setParamsForm((current) => ({
      ...current,
      tediyeListesi: (current.tediyeListesi || DEFAULT_TEDIYE_LISTESI).map((item) => ({
        ...item,
        aktifDonemdeOdensin:
          item.id === id ? !item.aktifDonemdeOdensin : item.aktifDonemdeOdensin,
      })),
    }));
  };

  const handleAddTediye = () => {
    setParamsForm((current) => {
      const currentList = current.tediyeListesi || DEFAULT_TEDIYE_LISTESI;
      return {
        ...current,
        tediyeListesi: [
          ...currentList,
          {
            id: Date.now(),
            ad: `${currentList.length + 1}. Tediye`,
            odemeAyi: 'Ocak',
            gunSayisi: 13,
            aktifDonemdeOdensin: false,
          },
        ],
      };
    });
  };

  const handleDeleteTediye = (id: number) => {
    setParamsForm((current) => ({
      ...current,
      tediyeListesi: (current.tediyeListesi || DEFAULT_TEDIYE_LISTESI).filter(
        (item) => item.id !== id
      ),
    }));
  };

  const handleTisChange = (id: number, field: keyof TisIkramiyeKalemi, value: any) => {
    setParamsForm((current) => ({
      ...current,
      tisIkramiyeListesi: (current.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI).map(
        (item) => (item.id === id ? { ...item, [field]: value } : item)
      ),
    }));
  };

  const handleToggleTisActive = (id: number) => {
    setParamsForm((current) => ({
      ...current,
      tisIkramiyeListesi: (current.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI).map(
        (item) => ({
          ...item,
          aktifDonemdeOdensin:
            item.id === id ? !item.aktifDonemdeOdensin : item.aktifDonemdeOdensin,
        })
      ),
    }));
  };

  const handleAddTis = () => {
    setParamsForm((current) => {
      const currentList = current.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI;
      return {
        ...current,
        tisIkramiyeListesi: [
          ...currentList,
          {
            id: Date.now(),
            ad: `${currentList.length + 1}. TİS İkramiyesi`,
            odemeAyi: 'Ocak',
            gunSayisi: 13,
            aktifDonemdeOdensin: false,
          },
        ],
      };
    });
  };

  const handleDeleteTis = (id: number) => {
    setParamsForm((current) => ({
      ...current,
      tisIkramiyeListesi: (current.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI).filter(
        (item) => item.id !== id
      ),
    }));
  };

  return (
    <section data-testid="period-settings-tediye-tis" className="space-y-6">
      <header>
        <h2 className="text-xl font-bold text-slate-900">TİS / Tediye Takvimi</h2>
        <p className="mt-1 text-xs text-slate-500">
          Tediye ve TİS ikramiye referans takvimlerini aktif dönem için yönetin.
        </p>
      </header>

      <form onSubmit={onSubmit} className="space-y-6">
        <div className="bg-indigo-50 border border-indigo-200 rounded-2xl p-4 text-xs text-indigo-950 leading-relaxed">
          <strong className="block mb-1">Manuel bordro girdisi</strong>
          Bu sekmedeki Tediye/TİS listeleri yalnız referans takvim ve açıklama amacıyla korunur.
          Bordro motoru buradaki gün sayısı, aktiflik veya sabit tutardan otomatik ödeme üretmez.
          Gerçek brüt Tediye ve TİS ikramiyesi tutarını Bordro Hesaplama ekranında her personel için manuel girin.
        </div>

        <div className="bg-amber-50/90 border border-amber-200 rounded-2xl p-4 space-y-2">
          <div className="flex items-center justify-between gap-2">
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
            value={paramsForm.tediyeTisNotu ?? DEFAULT_KURUM_DEGERLERI.tediyeTisNotu}
            onChange={(e) => setParamsForm({ ...paramsForm, tediyeTisNotu: e.target.value })}
            placeholder="Açıklama / Not metnini buraya yazabilirsiniz..."
            className="w-full text-xs text-amber-950 bg-amber-100/50 border border-amber-300 rounded-xl p-2.5 leading-relaxed focus:bg-white focus:ring-2 focus:ring-amber-500 focus:outline-none transition-all resize-y"
          />
        </div>

        {savedSuccess && (
          <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
            <Check className="w-4 h-4 text-emerald-600" />
            <span>Tediye ve TİS referans ayarları başarıyla kaydedildi!</span>
          </div>
        )}

        <div className="space-y-3">
          <div className="flex items-center justify-between border-b border-slate-200 pb-2">
            <div className="flex items-center gap-2">
              <div className="p-1.5 bg-amber-100 text-amber-800 rounded-lg">
                <Gift className="w-4 h-4" />
              </div>
              <h3 className="font-bold text-sm text-slate-900">Tediye Ödemeleri</h3>
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
            {tediyeListesi.map((tediye) => (
              <div
                key={tediye.id}
                className={`p-3.5 rounded-2xl border transition-all ${
                  tediye.aktifDonemdeOdensin
                    ? 'bg-amber-50/60 border-amber-300 ring-2 ring-amber-400/30'
                    : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                }`}
              >
                <div className="flex items-center justify-between mb-2 gap-2">
                  <div className="flex items-center gap-1.5 flex-1 min-w-0">
                    <span className="w-2 h-2 rounded-full bg-amber-500 shrink-0" />
                    <input
                      type="text"
                      value={tediye.ad}
                      onChange={(e) => handleTediyeChange(tediye.id, 'ad', e.target.value)}
                      placeholder="Tediye Adı"
                      className="font-bold text-xs text-slate-900 bg-white border border-slate-200 px-2 py-1 rounded-lg focus:ring-2 focus:ring-amber-500 w-full"
                    />
                  </div>

                  <div className="flex items-center gap-1 shrink-0">
                    <button
                      type="button"
                      onClick={() => handleToggleTediyeActive(tediye.id)}
                      className={`px-2.5 py-1 rounded-full text-[11px] font-bold transition-all flex items-center gap-1 ${
                        tediye.aktifDonemdeOdensin
                          ? 'bg-amber-600 text-white shadow-2xs'
                          : 'bg-slate-200 text-slate-600 hover:bg-slate-300'
                      }`}
                    >
                      {tediye.aktifDonemdeOdensin ? (
                        <>
                          <Check className="w-3 h-3" />
                          <span>Referans aktif</span>
                        </>
                      ) : (
                        <span>Pasif</span>
                      )}
                    </button>
                    <button
                      type="button"
                      onClick={() => handleDeleteTediye(tediye.id)}
                      title="Tediyeyi Sil"
                      className="p-1 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-2 text-xs">
                  <div>
                    <label className="block text-[11px] text-slate-500 mb-0.5">Ödeme Ayı</label>
                    <select
                      value={tediye.odemeAyi}
                      onChange={(e) => handleTediyeChange(tediye.id, 'odemeAyi', e.target.value)}
                      className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-800"
                    >
                      {AY_ISIMLERI.map((month) => (
                        <option key={month} value={month}>
                          {month}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label className="block text-[11px] text-slate-500 mb-0.5">Hak Edilen Gün</label>
                    <input
                      type="number"
                      value={tediye.gunSayisi ?? 13}
                      onChange={(e) =>
                        handleTediyeChange(
                          tediye.id,
                          'gunSayisi',
                          parseFloat(e.target.value) || 0
                        )
                      }
                      className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-mono font-bold text-slate-900"
                    />
                  </div>
                </div>

                <div className="mt-2.5 pt-2 border-t border-slate-200/60 flex items-center justify-between text-xs">
                  <span className="text-slate-500">Bordro tutarı:</span>
                  <span className="font-bold text-amber-900">Personel bazında manuel girilir</span>
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="space-y-3 pt-2">
          <div className="flex items-center justify-between border-b border-slate-200 pb-2">
            <div className="flex items-center gap-2">
              <div className="p-1.5 bg-indigo-100 text-indigo-800 rounded-lg">
                <Award className="w-4 h-4" />
              </div>
              <h3 className="font-bold text-sm text-slate-900">
                Toplu İş Sözleşmesi (TİS) İkramiye Ödemeleri
              </h3>
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
            {tisListesi.map((tis) => (
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
                    <span className="w-2 h-2 rounded-full bg-indigo-500 shrink-0" />
                    <input
                      type="text"
                      value={tis.ad}
                      onChange={(e) => handleTisChange(tis.id, 'ad', e.target.value)}
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
                          <span>Referans aktif</span>
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
                    <label className="block text-[11px] text-slate-500 mb-0.5">Ödeme Ayı</label>
                    <select
                      value={tis.odemeAyi || ''}
                      onChange={(e) => handleTisChange(tis.id, 'odemeAyi', e.target.value)}
                      className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-800"
                    >
                      <option value="">Seçiniz (Manuel)</option>
                      {AY_ISIMLERI.map((month) => (
                        <option key={month} value={month}>
                          {month}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label className="block text-[11px] text-slate-500 mb-0.5">Hak Edilen Gün</label>
                    <input
                      type="number"
                      value={tis.gunSayisi || ''}
                      placeholder="0"
                      onChange={(e) =>
                        handleTisChange(tis.id, 'gunSayisi', parseFloat(e.target.value) || 0)
                      }
                      className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-mono font-bold text-slate-900"
                    />
                  </div>
                </div>

                <div className="mt-2.5 pt-2 border-t border-slate-200/60 flex items-center justify-between text-xs">
                  <span className="text-slate-500">Bordro tutarı:</span>
                  <span className="font-bold text-indigo-900">Personel bazında manuel girilir</span>
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="pt-3 border-t border-slate-200 flex justify-end">
          <button
            type="submit"
            className="px-5 py-2.5 bg-amber-600 hover:bg-amber-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
          >
            <Save className="w-4 h-4" />
            <span>Tediye &amp; TİS Referanslarını Kaydet</span>
          </button>
        </div>
      </form>
    </section>
  );
};
