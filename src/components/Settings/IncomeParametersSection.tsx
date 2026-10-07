import React from 'react';
import { Check, Info, Save } from 'lucide-react';
import type { PayrollPeriodInstitutionSettings } from '../../types/payroll';

interface IncomeParametersSectionProps {
  aktifDonemId: string;
  paramsForm: PayrollPeriodInstitutionSettings;
  setParamsForm: React.Dispatch<React.SetStateAction<PayrollPeriodInstitutionSettings>>;
  savedSuccess: boolean;
  errorMessage: string | null;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
  onOpenCompensationPolicy: () => void;
}

export const IncomeParametersSection: React.FC<IncomeParametersSectionProps> = ({
  aktifDonemId,
  paramsForm,
  setParamsForm,
  savedSuccess,
  errorMessage,
  onSubmit,
  onOpenCompensationPolicy,
}) => (
  <section data-testid="period-settings-gelir" className="space-y-5">
    <header>
      <h2 className="text-xl font-bold text-slate-900">Ücret politikası</h2>
      <p className="mt-1 text-xs text-slate-500">Normal bordro ücretleri dönem ayarlarından değil, tarihli kurum ücret politikası sürümlerinden alınır.</p>
    </header>

    <div className="rounded-2xl border border-indigo-200 bg-indigo-50 p-5">
      <div className="flex items-start gap-3">
        <Info aria-hidden="true" className="mt-0.5 h-5 w-5 shrink-0 text-indigo-700" />
        <div className="flex-1 text-sm leading-relaxed text-indigo-950">
          <p className="font-semibold">{aktifDonemId} dönemi için ücret geçmişini yönetin</p>
          <p className="mt-1">Başlangıç ücretini ve yürürlük tarihli değişiklikleri Ücret Politikası ekranında kaydedin. Başlangıç sürümü yoksa NORMAL bordro hesaplanmaz.</p>
          <button type="button" onClick={onOpenCompensationPolicy} className="mt-3 rounded-lg bg-indigo-700 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-700">
            Ücret Politikası ekranını aç
          </button>
        </div>
      </div>
    </div>

    <form onSubmit={onSubmit} className="space-y-4 rounded-2xl border border-slate-200 bg-white p-5">
      <div>
        <h3 className="text-sm font-bold text-slate-900">Yemek yardımı vergi uygulaması</h3>
        <p className="mt-1 text-xs text-slate-600">Bu kurum tercihi bordro hesaplama snapshot’ında saklanır; yasal günlük istisna sınırlarını değiştirmez.</p>
      </div>
      <label className="flex items-start gap-2 rounded-lg border border-slate-200 bg-slate-50 px-3 py-3 text-xs text-slate-800">
        <input
          type="checkbox"
          data-testid="meal-payroll-tax-policy"
          checked={paramsForm.yemekTamGvIstisnaDamgaTabi ?? true}
          onChange={(event) => {
            const checked = event.target.checked;
            setParamsForm((current) => ({ ...current, yemekTamGvIstisnaDamgaTabi: checked }));
          }}
          className="mt-0.5 shrink-0 accent-indigo-600"
        />
        <span>Yemek yardımının tamamını gelir vergisinden istisna, tamamını damga vergisine tabi tut</span>
      </label>
      {errorMessage && <p role="alert" className="text-sm font-medium text-rose-700">{errorMessage}</p>}
      {savedSuccess && <p role="status" className="flex items-center gap-2 text-sm font-semibold text-emerald-700"><Check className="h-4 w-4" />Kurum tercihi kaydedildi.</p>}
      <button type="submit" className="inline-flex items-center gap-2 rounded-lg bg-slate-800 px-4 py-2 text-sm font-semibold text-white hover:bg-slate-900">
        <Save aria-hidden="true" className="h-4 w-4" />Kaydet
      </button>
    </form>
  </section>
);
