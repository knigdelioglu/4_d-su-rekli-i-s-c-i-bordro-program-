import React from 'react';
import { Calendar, Plus } from 'lucide-react';
import type { BordroDonemi } from '../../types/payroll';

interface PeriodListSectionProps {
  donemler: BordroDonemi[];
  aktifDonemId: string;
  onSelectDonem: (donemId: string) => Promise<void> | void;
  onOpenNewPeriod: () => void;
}

export const PeriodListSection: React.FC<PeriodListSectionProps> = ({
  donemler,
  aktifDonemId,
  onSelectDonem,
  onOpenNewPeriod,
}) => (
  <section data-testid="period-settings-donemler" className="space-y-5">
    <header>
      <h2 className="text-xl font-bold text-slate-900">Dönemler</h2>
      <p className="mt-1 text-xs text-slate-500">
        Mevcut bordro dönemlerini görüntüleyin ve aktif dönemi seçin.
      </p>
    </header>

    {donemler.length === 0 ? (
      <div className="rounded-2xl border border-dashed border-slate-300 bg-slate-50 p-8 text-center">
        <Calendar className="mx-auto h-8 w-8 text-slate-400" />
        <p className="mt-3 text-sm font-semibold text-slate-700">Henüz bordro dönemi bulunmuyor.</p>
        <p className="mt-1 text-xs text-slate-500">
          İşlemlere başlamak için yeni bir dönem oluşturun.
        </p>
        <button
          type="button"
          onClick={onOpenNewPeriod}
          className="mt-4 inline-flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700"
        >
          <Plus className="h-3.5 w-3.5" />
          Yeni Dönem Aç
        </button>
      </div>
    ) : (
      <div className="divide-y divide-slate-100 border border-slate-200 rounded-2xl overflow-hidden">
        {donemler.map((donem) => {
          const isSelected = donem.id === aktifDonemId;
          return (
            <button
              key={donem.id}
              type="button"
              onClick={() => void onSelectDonem(donem.id)}
              className={`w-full p-4 flex items-center justify-between text-left cursor-pointer transition-colors ${
                isSelected ? 'bg-indigo-50/80 hover:bg-indigo-50' : 'hover:bg-slate-50'
              }`}
            >
              <span>
                <span className="flex items-center gap-2">
                  <span className="font-semibold text-sm text-slate-900">{donem.donemAdi}</span>
                  {isSelected && (
                    <span className="px-2 py-0.5 bg-indigo-600 text-white text-[10px] font-bold rounded-full uppercase">
                      Aktif
                    </span>
                  )}
                </span>
                <span className="block text-xs text-slate-500 font-mono mt-0.5">
                  Tarih Aralığı: {donem.baslangicTarihi} ile {donem.bitisTarihi} arası
                </span>
              </span>
              <span className="text-xs font-semibold text-indigo-600">Seç</span>
            </button>
          );
        })}
      </div>
    )}
  </section>
);
