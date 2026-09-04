import React from 'react';
import { Percent, Plus, Save, Trash2, Check } from 'lucide-react';
import type { TaxBracket } from '../../types/payroll';

interface AnnualTaxSectionProps {
  annualTaxYear: number;
  setAnnualTaxYear: React.Dispatch<React.SetStateAction<number>>;
  annualTaxBrackets: TaxBracket[];
  setAnnualTaxBrackets: React.Dispatch<React.SetStateAction<TaxBracket[]>>;
  annualInsuranceGvCap: number;
  setAnnualInsuranceGvCap: React.Dispatch<React.SetStateAction<number>>;
  annualTaxSuccess: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
}

export const AnnualTaxSection: React.FC<AnnualTaxSectionProps> = ({
  annualTaxYear,
  setAnnualTaxYear,
  annualTaxBrackets,
  setAnnualTaxBrackets,
  annualInsuranceGvCap,
  setAnnualInsuranceGvCap,
  annualTaxSuccess,
  onSubmit,
}) => {
  const updateAnnualTaxBracket = (index: number, field: keyof TaxBracket, value: number) => {
    setAnnualTaxBrackets((current) =>
      current.map((bracket, bracketIndex) =>
        bracketIndex === index ? { ...bracket, [field]: value } : bracket
      )
    );
  };

  return (
    <section data-testid="period-settings-gv" className="space-y-5">
      <header>
        <h2 className="text-xl font-bold text-slate-900">Yıllık GV Tarifesi</h2>
        <p className="mt-1 text-xs text-slate-500">
          Yıllık kümülatif gelir vergisi dilimlerini ve sigorta GV tavanını yönetin.
        </p>
      </header>

      <form onSubmit={onSubmit} className="space-y-5">
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

        <div className="flex flex-wrap items-end gap-3">
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
          <div className="w-64">
            <label className="block text-xs font-semibold text-slate-700 mb-1">
              Sigorta GV Yıllık Brüt Asgari Ücret Tavanı (TL)
            </label>
            <input
              type="number"
              min={0.01}
              step="0.01"
              value={annualInsuranceGvCap || ''}
              onChange={(e) => setAnnualInsuranceGvCap(Number(e.target.value) || 0)}
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
                <div
                  key={`${annualTaxYear}-${index}`}
                  className="grid grid-cols-[1fr_1fr_auto] gap-3 items-center px-4 py-3"
                >
                  <input
                    type="number"
                    min={0}
                    step="0.01"
                    value={Number.isFinite(bracket.limit) ? bracket.limit : ''}
                    onChange={(e) =>
                      updateAnnualTaxBracket(index, 'limit', parseFloat(e.target.value) || 0)
                    }
                    className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-violet-500"
                  />
                  <input
                    type="number"
                    min={0}
                    max={100}
                    step="0.01"
                    value={(bracket.oran * 100).toString()}
                    onChange={(e) =>
                      updateAnnualTaxBracket(index, 'oran', (parseFloat(e.target.value) || 0) / 100)
                    }
                    className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-violet-500"
                  />
                  <button
                    type="button"
                    disabled={annualTaxBrackets.length <= 1}
                    onClick={() =>
                      setAnnualTaxBrackets((current) =>
                        current.filter((_, itemIndex) => itemIndex !== index)
                      )
                    }
                    className="p-2 text-rose-600 hover:bg-rose-50 disabled:opacity-30 disabled:cursor-not-allowed rounded-lg"
                    title="Dilim sil"
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
    </section>
  );
};
