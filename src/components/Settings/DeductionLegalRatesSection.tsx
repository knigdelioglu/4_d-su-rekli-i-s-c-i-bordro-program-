import React from 'react';
import { Award, Percent, Plus, Save, ShieldAlert, Trash2, Check } from 'lucide-react';
import type { BordroDonemi, DönemselKurumDegerleri } from '../../types/payroll';

type StatutoryField =
  | 'effectiveFrom'
  | 'gunlukAsgariUcret'
  | 'pekTavanKatsayisi'
  | 'gunlukYemekIstisnasiSGK'
  | 'gunlukYemekIstisnasiGV';

interface DeductionLegalRatesSectionProps {
  aktifDonemId: string;
  activePeriodForParams?: BordroDonemi;
  paramsForm: DönemselKurumDegerleri;
  setParamsForm: React.Dispatch<React.SetStateAction<DönemselKurumDegerleri>>;
  savedSuccess: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
}

export const DeductionLegalRatesSection: React.FC<DeductionLegalRatesSectionProps> = ({
  aktifDonemId,
  activePeriodForParams,
  paramsForm,
  setParamsForm,
  savedSuccess,
  onSubmit,
}) => {
  const updateNumber = (
    field: keyof DönemselKurumDegerleri,
    value: number
  ) => {
    setParamsForm((current) => ({ ...current, [field]: value }));
  };

  const addStatutorySegment = () => {
    setParamsForm((current) => ({
      ...current,
      statutoryParameterSegments: [
        ...(current.statutoryParameterSegments || []),
        { effectiveFrom: activePeriodForParams?.baslangicTarihi || '' },
      ],
    }));
  };

  const updateStatutorySegment = (
    index: number,
    field: StatutoryField,
    value: string | number | undefined
  ) => {
    setParamsForm((current) => ({
      ...current,
      statutoryParameterSegments: (current.statutoryParameterSegments || []).map(
        (segment, segmentIndex) =>
          segmentIndex === index ? { ...segment, [field]: value } : segment
      ),
    }));
  };

  const removeStatutorySegment = (index: number) => {
    setParamsForm((current) => ({
      ...current,
      statutoryParameterSegments: (current.statutoryParameterSegments || []).filter(
        (_, segmentIndex) => segmentIndex !== index
      ),
    }));
  };

  const segments = paramsForm.statutoryParameterSegments || [];

  return (
    <section data-testid="period-settings-kesinti" className="space-y-5">
      <header>
        <h2 className="text-xl font-bold text-slate-900">Kesinti &amp; Yasal Oranlar</h2>
        <p className="mt-1 text-xs text-slate-500">
          Aktif dönemin yasal kesinti oranlarını ve dönem içi mevzuat değişikliklerini yönetin.
        </p>
      </header>

      <form onSubmit={onSubmit} className="space-y-5">
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

        <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl space-y-3">
          <h3 className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
            <Percent className="w-4 h-4 text-indigo-600" />
            <span>Yasal Vergi ve Sigorta Oranları</span>
          </h3>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                SGK İşçi Primi Oranı (%)
              </label>
              <input
                type="number"
                step="0.1"
                value={paramsForm.sgkIsciOraniYuzde ?? 14}
                onChange={(e) => updateNumber('sgkIsciOraniYuzde', parseFloat(e.target.value) || 0)}
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
                onChange={(e) => updateNumber('issizlikIsciOraniYuzde', parseFloat(e.target.value) || 0)}
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
                Tarifeyi “Yıllık GV Tarifesi” bölümünden yönetin.
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
                onChange={(e) => updateNumber('damgaVergisiOraniBinde', parseFloat(e.target.value) || 0)}
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
                onChange={(e) => updateNumber('sgkIsverenOraniYuzde', parseFloat(e.target.value) || 0)}
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
                onChange={(e) => updateNumber('issizlikIsverenOraniYuzde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Yasal Standart: %2,00 (PEK Matrahı üzerinden işveren payı)
              </span>
            </div>
          </div>
        </div>

        <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl space-y-3">
          <h3 className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
            <Award className="w-4 h-4 text-amber-600" />
            <span>Sendika Aidatı ve Otomatik Katılım (OKS) Parametreleri</span>
          </h3>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                Sendika Aidatı — Günlük Çıplak Ücret Oranı (%)
              </label>
              <input
                type="number"
                step="1"
                value={paramsForm.sendikaAidatiYuzde ?? 65}
                onChange={(e) => updateNumber('sendikaAidatiYuzde', parseFloat(e.target.value) || 0)}
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
                onChange={(e) => updateNumber('sabitSendikaAidati', parseFloat(e.target.value) || 0)}
                placeholder="0.00"
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                0 kalırsa günlük çıplak ücretin %65&apos;i uygulanır
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
                onChange={(e) => updateNumber('besOraniYuzde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                PEK × %3 (Hesaplanan tutarın kuruş kısmı atılır)
              </span>
            </div>
          </div>

          <div className="pt-4 border-t border-slate-200">
            <h3 className="text-xs font-bold text-slate-900 uppercase tracking-wider mb-3">
              SGK Prime Esas Kazanç (PEK) Parametreleri (2026)
            </h3>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">
                  Günlük SGK Yemek İstisnası (TL)
                </label>
                <input
                  type="number"
                  step="0.01"
                  value={paramsForm.gunlukYemekIstisnasiSGK ?? 300.00}
                  onChange={(e) => updateNumber('gunlukYemekIstisnasiSGK', parseFloat(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  2026-08 dönemi için 300,00 TL (17.04.2026 sonrası)
                </span>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">
                  Günlük GV Yemek İstisnası (TL)
                </label>
                <input
                  type="number"
                  step="0.01"
                  value={paramsForm.gunlukYemekIstisnasiGV ?? ''}
                  onChange={(e) => updateNumber('gunlukYemekIstisnasiGV', parseFloat(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  Gelir vergisi istisnası SGK limitinden bağımsızdır.
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
                  onChange={(e) => updateNumber('pekTavanKatsayisi', parseFloat(e.target.value) || 9)}
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
                  onChange={(e) => updateNumber('gunlukAsgariUcret', parseFloat(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  PEK Alt Sınırı = 1.101,00 TL (30 gün = 33.030 TL)
                </span>
              </div>
            </div>

            <div className="mt-4 pt-4 border-t border-slate-200 space-y-3">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <h4 className="text-xs font-bold text-slate-900">Dönem İçi Yasal Parametre Değişimleri</h4>
                  <p className="text-[11px] text-slate-600 mt-1">
                    Yalnız aktif/gelecek {activePeriodForParams?.baslangicTarihi}–{activePeriodForParams?.bitisTarihi}
                    dönemi içindeki yürürlük değişimlerini girin. Bu alan geçmiş mevzuat arşivi oluşturmaz.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={addStatutorySegment}
                  className="px-3 py-1.5 rounded-lg bg-indigo-100 hover:bg-indigo-200 text-indigo-800 text-xs font-semibold flex items-center gap-1 shrink-0"
                >
                  <Plus className="w-3.5 h-3.5" /> Segment Ekle
                </button>
              </div>

              {segments.map((segment, index) => (
                <div
                  key={`${segment.effectiveFrom}-${index}`}
                  className="border border-indigo-200 bg-indigo-50/50 rounded-xl p-3 space-y-3"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-bold text-indigo-900">Segment {index + 1}</span>
                    <button
                      type="button"
                      onClick={() => removeStatutorySegment(index)}
                      className="p-1.5 rounded-lg text-rose-600 hover:bg-rose-100"
                      title="Segmenti kaldır"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
                    <div>
                      <label className="block text-[11px] font-semibold text-slate-700 mb-1">
                        Yürürlük Tarihi
                      </label>
                      <input
                        type="date"
                        min={activePeriodForParams?.baslangicTarihi}
                        max={activePeriodForParams?.bitisTarihi}
                        value={segment.effectiveFrom}
                        onChange={(e) =>
                          updateStatutorySegment(index, 'effectiveFrom', e.target.value)
                        }
                        className="w-full px-2 py-2 bg-white border border-slate-300 rounded-lg text-xs"
                      />
                    </div>
                    {[
                      ['gunlukAsgariUcret', 'Günlük Asgari'],
                      ['pekTavanKatsayisi', 'PEK Katsayı'],
                      ['gunlukYemekIstisnasiSGK', 'SGK Yemek'],
                      ['gunlukYemekIstisnasiGV', 'GV Yemek'],
                    ].map(([field, label]) => (
                      <div key={field}>
                        <label className="block text-[11px] font-semibold text-slate-700 mb-1">
                          {label}
                        </label>
                        <input
                          type="number"
                          step="0.01"
                          value={(segment as Record<string, number | string | undefined>)[field] ?? ''}
                          placeholder="Değişmiyor"
                          onChange={(e) =>
                            updateStatutorySegment(
                              index,
                              field as StatutoryField,
                              e.target.value === '' ? undefined : Number(e.target.value)
                            )
                          }
                          className="w-full px-2 py-2 bg-white border border-slate-300 rounded-lg text-xs font-mono"
                        />
                      </div>
                    ))}
                  </div>
                </div>
              ))}
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
    </section>
  );
};
