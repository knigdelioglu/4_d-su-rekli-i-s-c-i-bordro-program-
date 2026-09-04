import React from 'react';
import { Check, Info, Layers, Percent, Plus, Save, Settings2, Trash2, X } from 'lucide-react';
import type { DönemselKurumDegerleri, IsPrimiGrupItem } from '../../types/payroll';
import {
  AY_ISIMLERI,
  DEFAULT_IS_PRIMI_GRUPLARI,
  DEFAULT_KURUM_DEGERLERI,
  formatTL,
} from '../../utils/payrollPresentation';

interface IncomeParametersSectionProps {
  aktifDonemId: string;
  paramsForm: DönemselKurumDegerleri;
  setParamsForm: React.Dispatch<React.SetStateAction<DönemselKurumDegerleri>>;
  zamAylariForm: number[];
  setZamAylariForm: React.Dispatch<React.SetStateAction<number[]>>;
  savedSuccess: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
}

export const IncomeParametersSection: React.FC<IncomeParametersSectionProps> = ({
  aktifDonemId,
  paramsForm,
  setParamsForm,
  zamAylariForm,
  setZamAylariForm,
  savedSuccess,
  onSubmit,
}) => {
  const [isGrupModalOpen, setIsGrupModalOpen] = React.useState(false);
  const groups = paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI;

  return (
    <section data-testid="period-settings-gelir" className="space-y-5">
      <header>
        <h2 className="text-xl font-bold text-slate-900">Gelir Parametreleri</h2>
        <p className="mt-1 text-xs text-slate-500">
          Aktif dönemin gelir, zam takvimi ve iş primi değerlerini yönetin.
        </p>
      </header>

      <form onSubmit={onSubmit} className="space-y-5">
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
            <span>Dönem gelir ve zam ayarları başarıyla kaydedildi!</span>
          </div>
        )}

        <div className="bg-amber-50 border border-amber-200 rounded-xl p-4 space-y-3">
          <div className="flex items-start gap-3">
            <Percent className="w-5 h-5 text-amber-600 shrink-0 mt-0.5" />
            <div className="text-xs text-amber-950 leading-relaxed">
              <strong className="block mb-1">Kurum Genelinde Zam Takvimi</strong>
              Zam seçilen ayın 1&apos;inde yürürlüğe girer ve kurumun tüm işçilerine uygulanır.
              15–14 bordro dönemi bu tarihi içeriyorsa, tarih öncesi günler önceki dönem
              parametreleriyle; tarih ve sonrası günler aktif dönem parametreleriyle hesaplanır.
              Yeni tutarları zam tarihini içeren dönemin gelir parametrelerine girin.
            </div>
          </div>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-2">
            {AY_ISIMLERI.map((ay, index) => {
              const month = index + 1;
              const selected = zamAylariForm.includes(month);
              return (
                <label
                  key={month}
                  className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-semibold cursor-pointer transition-colors ${
                    selected
                      ? 'bg-amber-100 border-amber-400 text-amber-900'
                      : 'bg-white border-amber-200 text-slate-700 hover:bg-amber-100/60'
                  }`}
                >
                  <input
                    type="checkbox"
                    checked={selected}
                    onChange={() =>
                      setZamAylariForm((current) =>
                        selected
                          ? current.filter((value) => value !== month)
                          : [...current, month].sort((a, b) => a - b)
                      )
                    }
                    className="accent-amber-600"
                  />
                  {ay}
                </label>
              );
            })}
          </div>
          <div className="text-[11px] text-amber-800">
            Seçili aylar:{' '}
            {zamAylariForm.length > 0
              ? zamAylariForm.map((month) => AY_ISIMLERI[month - 1]).join(', ')
              : 'Henüz seçilmedi (dönem tek ücretle hesaplanır).'}
          </div>
        </div>

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
                  birlestirilmisSosyalYardim: parseFloat(e.target.value) || 0,
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
            <button
              type="button"
              onClick={() => setIsGrupModalOpen(true)}
              className="w-full p-3.5 bg-indigo-50/60 hover:bg-indigo-50 border border-indigo-200/80 hover:border-indigo-300 rounded-xl cursor-pointer transition-all flex items-center justify-between group text-left"
            >
              <span className="space-y-1">
                <span className="flex items-center gap-2">
                  <span className="w-7 h-7 rounded-lg bg-indigo-100 text-indigo-700 flex items-center justify-center font-semibold shrink-0">
                    <Layers className="w-4 h-4" />
                  </span>
                  <span className="text-xs font-bold text-slate-900 group-hover:text-indigo-700 transition-colors">
                    İş Primi Grupları &amp; Oranları
                  </span>
                  <span className="px-2 py-0.5 bg-indigo-100 text-indigo-700 rounded-full text-[10px] font-extrabold">
                    {groups.length} Grup Tanımlı
                  </span>
                </span>
                <span className="block text-[11px] text-slate-600">
                  Personeller için tanımlı iş primi gruplarını adlandırabilir, oranlarını (%) değiştirebilir ve yeni grup ekleyebilirsiniz.
                </span>
                <span className="flex flex-wrap gap-1.5 pt-1">
                  {groups.map((group) => (
                    <span
                      key={group.id}
                      className="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-semibold bg-white border border-slate-200 text-slate-700"
                    >
                      {group.ad}: <strong className="ml-1 text-indigo-600">%{group.oran}</strong>
                    </span>
                  ))}
                </span>
              </span>
              <span className="px-3 py-1.5 bg-white border border-indigo-200 text-indigo-700 rounded-lg text-xs font-semibold shadow-2xs group-hover:bg-indigo-600 group-hover:text-white group-hover:border-indigo-600 transition-all shrink-0 ml-3 flex items-center gap-1">
                <span>Düzenle</span>
                <Settings2 className="w-3.5 h-3.5" />
              </span>
            </button>
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
                setParamsForm({ ...paramsForm, ekOdeme: parseFloat(e.target.value) || 0 })
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

      {isGrupModalOpen && (
        <div className="fixed inset-0 z-60 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4">
          <div className="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-200">
            <div className="px-5 py-4 border-b border-slate-200 flex items-center justify-between bg-slate-50">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-xl bg-indigo-100 text-indigo-700 flex items-center justify-center shrink-0">
                  <Layers className="w-4 h-4" />
                </div>
                <div>
                  <h3 className="font-bold text-slate-900 text-sm">İş Primi Grupları &amp; Oranları</h3>
                  <p className="text-[11px] text-slate-500">Grup adlarını ve prim oranlarını düzenleyin</p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setIsGrupModalOpen(false)}
                aria-label="İş primi grupları penceresini kapat"
                className="p-1.5 text-slate-400 hover:text-slate-600 hover:bg-slate-200 rounded-lg transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-5 space-y-4 max-h-[60vh] overflow-y-auto">
              <div className="flex items-center justify-between pb-1">
                <span className="text-xs font-bold text-slate-700">
                  Tanımlı Gruplar ({groups.length})
                </span>
                <button
                  type="button"
                  onClick={() => {
                    const newNum = groups.length + 1;
                    const newGroup: IsPrimiGrupItem = {
                      id: `grup_${Date.now()}`,
                      ad: `${newNum}. Grup`,
                      oran: 5,
                      aktif: true,
                    };
                    setParamsForm((current) => ({
                      ...current,
                      isPrimiGruplari: [
                        ...(current.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI),
                        newGroup,
                      ],
                    }));
                  }}
                  className="px-2.5 py-1 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 rounded-lg text-xs font-semibold transition-colors flex items-center gap-1 cursor-pointer"
                >
                  <Plus className="w-3.5 h-3.5" />
                  <span>Yeni Grup Ekle</span>
                </button>
              </div>

              <div className="space-y-2">
                {groups.map((groupItem, index) => (
                  <div
                    key={groupItem.id || index}
                    className="flex items-center gap-2 bg-slate-50 p-2.5 rounded-xl border border-slate-200"
                  >
                    <div className="flex-1">
                      <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">
                        Grup Adı
                      </label>
                      <input
                        type="text"
                        value={groupItem.ad}
                        onChange={(e) =>
                          setParamsForm((current) => {
                            const next = [
                              ...(current.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI),
                            ];
                            next[index] = { ...next[index], ad: e.target.value };
                            return { ...current, isPrimiGruplari: next };
                          })
                        }
                        placeholder="Grup Adı (Örn: Temizlik İşçisi)"
                        className="w-full px-3 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-900 focus:ring-2 focus:ring-indigo-500"
                      />
                    </div>
                    <div className="w-28">
                      <label className="block text-[10px] font-semibold text-slate-500 mb-0.5">
                        İş Primi Oranı
                      </label>
                      <div className="flex items-center gap-1">
                        <span className="text-xs font-bold text-slate-500">%</span>
                        <input
                          type="number"
                          step="0.1"
                          min="0"
                          max="100"
                          value={groupItem.oran ?? ''}
                          onChange={(e) =>
                            setParamsForm((current) => {
                              const next = [
                                ...(current.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI),
                              ];
                              next[index] = {
                                ...next[index],
                                oran: parseFloat(e.target.value) || 0,
                              };
                              return { ...current, isPrimiGruplari: next };
                            })
                          }
                          placeholder="Oran"
                          className="w-full px-2 py-1.5 bg-white border border-slate-300 rounded-lg text-xs text-slate-900 font-mono font-bold text-center focus:ring-2 focus:ring-indigo-500"
                        />
                      </div>
                    </div>
                    {groups.length > 1 && (
                      <button
                        type="button"
                        onClick={() =>
                          setParamsForm((current) => ({
                            ...current,
                            isPrimiGruplari: (
                              current.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI
                            ).filter((_, itemIndex) => itemIndex !== index),
                          }))
                        }
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
    </section>
  );
};
