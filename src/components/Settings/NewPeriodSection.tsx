import React from 'react';
import { AlertCircle, ArrowRight, CheckCircle2, Info, Loader2, Plus } from 'lucide-react';
import { AY_ISIMLERI } from '../../utils/payrollPresentation';
import type { BordroDonemi } from '../../types/payroll';

interface NewPeriodSectionProps {
  newYear: number;
  setNewYear: (year: number) => void;
  newMonth: number;
  setNewMonth: (month: number) => void;
  newTaxYear: number;
  setNewTaxYear: (year: number) => void;
  newTaxMonth: number;
  setNewTaxMonth: (month: number) => void;
  yearOptions: number[];
  previewDonem: BordroDonemi;
  previewExists: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
  isSubmitting?: boolean;
  errorMessage?: string | null;
  successMessage?: string | null;
}

export const NewPeriodSection: React.FC<NewPeriodSectionProps> = ({
  newYear,
  setNewYear,
  newMonth,
  setNewMonth,
  newTaxYear,
  setNewTaxYear,
  newTaxMonth,
  setNewTaxMonth,
  yearOptions,
  previewDonem,
  previewExists,
  onSubmit,
  isSubmitting = false,
  errorMessage,
  successMessage,
}) => (
  <section data-testid="period-settings-yeni-donem" className="space-y-5">
    <header>
      <h2 className="text-xl font-bold text-slate-900">Yeni Dönem Aç</h2>
    </header>

    <form onSubmit={onSubmit} className="space-y-5">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div>
          <label htmlFor="new-period-year" className="block text-xs font-semibold text-slate-700 mb-1.5">Yıl</label>
          <select
            id="new-period-year"
            disabled={isSubmitting}
            value={newYear}
            onChange={(e) => {
              const year = parseInt(e.target.value, 10);
              setNewYear(year);
            }}
            className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
          >
            {yearOptions.map((year) => (
              <option key={year} value={year}>
                {year}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label htmlFor="new-period-month" className="block text-xs font-semibold text-slate-700 mb-1.5">Ay</label>
          <select
            id="new-period-month"
            disabled={isSubmitting}
            value={newMonth}
            onChange={(e) => {
              const month = parseInt(e.target.value, 10);
              setNewMonth(month);
            }}
            className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
          >
            {AY_ISIMLERI.map((monthName, index) => (
              <option key={index + 1} value={index + 1}>
                {monthName} ({index + 1}. Ay)
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="bg-indigo-50/70 border border-indigo-200 rounded-xl p-4 space-y-1">
        <div className="font-bold text-sm text-indigo-950">{previewDonem.donemAdi}</div>
        <div className="text-xs text-indigo-800 font-mono">
          {previewDonem.baslangicTarihi} → {previewDonem.bitisTarihi}
        </div>
        <div className="text-xs text-indigo-800 font-mono">
          Ödeme/Tahakkuk Ayı: {AY_ISIMLERI[previewDonem.taxMonth - 1]} {previewDonem.taxYear}
        </div>
      </div>

      {previewExists && (
        <div
          data-testid="period-already-exists-banner"
          className="bg-amber-50 border border-amber-300 rounded-xl p-3.5 flex items-start gap-2.5 text-xs text-amber-900"
        >
          <Info className="w-5 h-5 text-amber-600 shrink-0 mt-0.5" />
          <div className="leading-relaxed">
            <strong>{previewDonem.id}</strong> dönemi zaten mevcut.
          </div>
        </div>
      )}

      <details className="space-y-3">
        <summary className="cursor-pointer text-xs font-semibold text-slate-700 focus-visible:outline-2 focus-visible:outline-indigo-500">
          Ödeme / Tahakkuk Ayını Değiştir
        </summary>
        <p className="text-xs text-slate-500">
          Ödeme / tahakkuk ayı otomatik olarak dönemi izleyen ay seçilir. Farklı bir ayda ödeme yapılacaksa değiştirebilirsiniz.
        </p>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label htmlFor="new-period-tax-year" className="block text-xs font-semibold text-slate-700 mb-1.5">Vergi Yılı</label>
            <select
              id="new-period-tax-year"
              disabled={isSubmitting}
              value={newTaxYear}
              onChange={(e) => setNewTaxYear(parseInt(e.target.value, 10))}
              className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
            >
              {yearOptions.map((year) => (
                <option key={year} value={year}>
                  {year}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label htmlFor="new-period-tax-month" className="block text-xs font-semibold text-slate-700 mb-1.5">Vergi Ayı</label>
            <select
              id="new-period-tax-month"
              disabled={isSubmitting}
              value={newTaxMonth}
              onChange={(e) => setNewTaxMonth(parseInt(e.target.value, 10))}
              className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
            >
              {AY_ISIMLERI.map((monthName, index) => (
                <option key={index + 1} value={index + 1}>
                  {monthName} ({index + 1}. Ay)
                </option>
              ))}
            </select>
          </div>
        </div>
      </details>

      {errorMessage && (
        <div
          role="alert"
          data-testid="period-creation-error-banner"
          className="bg-rose-50 border border-rose-300 rounded-xl p-3.5 flex items-start gap-2.5 text-xs text-rose-900"
        >
          <AlertCircle className="w-5 h-5 text-rose-600 shrink-0 mt-0.5" />
          <div className="leading-relaxed">
            <strong>İşlem gerçekleştirilemedi:</strong> {errorMessage}
          </div>
        </div>
      )}

      {successMessage && (
        <div
          role="status"
          data-testid="period-creation-success-banner"
          className="bg-emerald-50 border border-emerald-300 rounded-xl p-3.5 flex items-start gap-2.5 text-xs text-emerald-900"
        >
          <CheckCircle2 className="w-5 h-5 text-emerald-600 shrink-0 mt-0.5" />
          <div className="leading-relaxed">{successMessage}</div>
        </div>
      )}

      <div className="pt-2 flex justify-end">
        <button
          type="submit"
          data-testid="submit-period-action"
          disabled={isSubmitting}
          className="px-5 py-2.5 bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-400 disabled:cursor-not-allowed text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
        >
          {isSubmitting ? (
            <>
              <Loader2 className="w-4 h-4 animate-spin" />
              <span>İşleniyor...</span>
            </>
          ) : previewExists ? (
            <>
              <ArrowRight className="w-4 h-4" />
              <span>Mevcut Döneme Geç</span>
            </>
          ) : (
            <>
              <Plus className="w-4 h-4" />
              <span>Dönemi Oluştur ve Geç</span>
            </>
          )}
        </button>
      </div>
    </form>
  </section>
);
