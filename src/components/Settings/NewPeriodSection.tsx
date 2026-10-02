import React from 'react';
import { AlertCircle, ArrowRight, CheckCircle2, Info, Loader2, Plus } from 'lucide-react';
import { AY_ISIMLERI } from '../../utils/payrollPresentation';
import type { BordroDonemi, IsPrimiGrupItem } from '../../types/payroll';

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
  resetTaxDefaults: (year: number, month: number) => void;
  previewDonem: BordroDonemi;
  previewExists: boolean;
  previewTaxChanged: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
  isSubmitting?: boolean;
  dailyBaseWage?: string;
  onDailyBaseWageChange?: (value: string) => void;
  workBonusGroups?: IsPrimiGrupItem[];
  onWorkBonusGroupsChange?: (groups: IsPrimiGrupItem[]) => void;
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
  resetTaxDefaults,
  previewDonem,
  previewExists,
  previewTaxChanged,
  onSubmit,
  isSubmitting = false,
  dailyBaseWage = '',
  onDailyBaseWageChange,
  workBonusGroups = [],
  onWorkBonusGroupsChange,
  errorMessage,
  successMessage,
}) => (
  <section data-testid="period-settings-yeni-donem" className="space-y-5">
    <header>
      <h2 className="text-xl font-bold text-slate-900">Yeni Dönem Aç</h2>
      <p className="mt-1 text-xs text-slate-500">
        15–14 tarih kuralına göre yeni bir bordro dönemi oluşturun.
      </p>
    </header>

    <form onSubmit={onSubmit} className="space-y-5">
      <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl text-xs space-y-2">
        <div className="font-semibold text-slate-800">Otomatik 15–14 Dönem Kuralı</div>
        <div className="text-slate-600 leading-relaxed">
          4/D Sürekli işçi mevzuatına göre her bordro dönemi seçilen ayın <strong>15&apos;i</strong> ile bir sonraki ayın <strong>14&apos;ü</strong> arasını kapsar.
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div>
          <label className="block text-xs font-semibold text-slate-700 mb-1.5">Yıl</label>
          <select
            value={newYear}
            onChange={(e) => {
              const year = parseInt(e.target.value, 10);
              setNewYear(year);
              resetTaxDefaults(year, newMonth);
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
          <label className="block text-xs font-semibold text-slate-700 mb-1.5">Ay</label>
          <select
            value={newMonth}
            onChange={(e) => {
              const month = parseInt(e.target.value, 10);
              setNewMonth(month);
              resetTaxDefaults(newYear, month);
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
        <div className="text-[10px] uppercase font-bold text-indigo-700 tracking-wider">
          Oluşturulacak Dönem Önizlemesi
        </div>
        <div className="font-bold text-sm text-indigo-950">{previewDonem.donemAdi}</div>
        <div className="text-xs text-indigo-800 font-mono">
          {previewDonem.baslangicTarihi} → {previewDonem.bitisTarihi}
        </div>
        <div className="text-xs text-indigo-800 font-mono">
          Ödeme/Tahakkuk Ayı: {AY_ISIMLERI[previewDonem.taxMonth - 1]} {previewDonem.taxYear}
        </div>
      </div>

      <div className="rounded-xl border border-amber-300 bg-amber-50 p-4">
          <label htmlFor="initial-daily-base-wage" className="block text-xs font-bold text-amber-950">
            Günlük Taban Ücret (TL) <span className="text-rose-700">*</span>
          </label>
          <p className="mt-1 text-[11px] leading-relaxed text-amber-900">
            Yeni dönem için kurumunuzun geçerli günlük taban ücretini kullanın. Mevcut dönem varsa tutar devralınır ve burada düzenlenebilir; temiz kurulumda tutarı siz girin.
          </p>
          <input
            id="initial-daily-base-wage"
            data-testid="initial-daily-base-wage"
            type="number"
            inputMode="decimal"
            min={previewExists ? undefined : '0.01'}
            step="0.01"
            required={!previewExists}
            value={dailyBaseWage}
            onChange={(event) => onDailyBaseWageChange?.(event.target.value)}
            className="mt-2 w-full rounded-lg border border-amber-300 bg-white px-3 py-2 font-mono text-sm text-slate-900 focus:ring-2 focus:ring-indigo-500"
          />
      </div>

      <div className="rounded-xl border border-amber-300 bg-amber-50 p-4 space-y-3">
        <div>
          <div className="text-xs font-bold text-amber-950">İş Primi Grupları <span className="text-rose-700">*</span></div>
          <p className="mt-1 text-[11px] leading-relaxed text-amber-900">
            Kurumunuzun kullandığı grup adlarını ve oranlarını girin. Mevcut dönem varsa değerler devralınır.
          </p>
        </div>
        {workBonusGroups.map((group, index) => (
          <div key={group.id || index} className="grid grid-cols-1 sm:grid-cols-[1fr_9rem_auto] gap-2 items-end">
            <div>
              <label className="block text-[10px] font-semibold text-slate-600 mb-1">Grup Adı</label>
              <input
                aria-label={`İş primi grup ${index + 1} adı`}
                value={group.ad}
                onChange={(event) => onWorkBonusGroupsChange?.(workBonusGroups.map((item, itemIndex) => itemIndex === index ? { ...item, ad: event.target.value } : item))}
                className="w-full rounded-lg border border-amber-300 bg-white px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="block text-[10px] font-semibold text-slate-600 mb-1">İş Primi Oranı (%)</label>
              <input
                aria-label={`İş primi grup ${index + 1} oranı`}
                type="number" inputMode="decimal" min="0" max="100" step="0.1"
                value={Number.isFinite(group.oran) ? group.oran : ''}
                onChange={(event) => onWorkBonusGroupsChange?.(workBonusGroups.map((item, itemIndex) => itemIndex === index ? { ...item, oran: event.target.value === '' ? Number.NaN : Number(event.target.value) } : item))}
                className="w-full rounded-lg border border-amber-300 bg-white px-3 py-2 font-mono text-sm"
              />
            </div>
            {workBonusGroups.length > 1 && (
              <button type="button" onClick={() => onWorkBonusGroupsChange?.(workBonusGroups.filter((_, itemIndex) => itemIndex !== index))} className="rounded-lg px-3 py-2 text-xs text-rose-700 hover:bg-rose-100">Kaldır</button>
            )}
          </div>
        ))}
        <button
          type="button"
          onClick={() => onWorkBonusGroupsChange?.([...workBonusGroups, { id: `group-${Date.now()}`, ad: '', oran: Number.NaN, aktif: true }])}
          className="text-xs font-semibold text-indigo-700 hover:text-indigo-900"
        >+ Grup ekle</button>
      </div>

      {previewExists && (
        <div
          data-testid="period-already-exists-banner"
          className="bg-amber-50 border border-amber-300 rounded-xl p-3.5 flex items-start gap-2.5 text-xs text-amber-900"
        >
          <Info className="w-5 h-5 text-amber-600 shrink-0 mt-0.5" />
          <div className="leading-relaxed">
            <strong>{previewDonem.id}</strong> dönemi zaten mevcut. Mevcut tahakkuk ve parametrelerin korunması için yeniden oluşturma yapılmaz; doğrudan bu döneme geçebilirsiniz.
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
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label className="block text-xs font-semibold text-slate-700 mb-1.5">Vergi Yılı</label>
            <select
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
            <label className="block text-xs font-semibold text-slate-700 mb-1.5">Vergi Ayı</label>
            <select
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
      </div>

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
