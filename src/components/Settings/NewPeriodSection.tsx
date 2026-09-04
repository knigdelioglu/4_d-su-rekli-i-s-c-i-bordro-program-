import React from 'react';
import { Info, Plus } from 'lucide-react';
import { AY_ISIMLERI } from '../../utils/payrollPresentation';
import type { BordroDonemi } from '../../types/payroll';

interface NewPeriodSectionProps {
  newYear: number;
  setNewYear: React.Dispatch<React.SetStateAction<number>>;
  newMonth: number;
  setNewMonth: React.Dispatch<React.SetStateAction<number>>;
  newTaxYear: number;
  setNewTaxYear: React.Dispatch<React.SetStateAction<number>>;
  newTaxMonth: number;
  setNewTaxMonth: React.Dispatch<React.SetStateAction<number>>;
  yearOptions: number[];
  resetTaxDefaults: (year: number, month: number) => void;
  previewDonem: BordroDonemi;
  previewExists: boolean;
  previewTaxChanged: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
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
  </section>
);
