import React from 'react';
import { Check, FileText, Plus, Trash2 } from 'lucide-react';
import type { Personel, SickLeaveRecord } from '../../types/payroll';

interface SickLeaveSectionProps {
  personeller: Personel[];
  sickLeaveRecords: SickLeaveRecord[];
  selectedPersonForSick: string;
  setSelectedPersonForSick: React.Dispatch<React.SetStateAction<string>>;
  sickStartDate: string;
  setSickStartDate: React.Dispatch<React.SetStateAction<string>>;
  sickEndDate: string;
  setSickEndDate: React.Dispatch<React.SetStateAction<string>>;
  sickSuccessMsg: string | null;
  onAddSickLeave: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
  onDeleteSickLeave: (id: string) => Promise<void> | void;
}

export const SickLeaveSection: React.FC<SickLeaveSectionProps> = ({
  personeller,
  sickLeaveRecords,
  selectedPersonForSick,
  setSelectedPersonForSick,
  sickStartDate,
  setSickStartDate,
  sickEndDate,
  setSickEndDate,
  sickSuccessMsg,
  onAddSickLeave,
  onDeleteSickLeave,
}) => (
  <section data-testid="period-settings-rapor" className="space-y-6">
    <header>
      <h2 className="text-xl font-bold text-slate-900">Raporlar</h2>
      <p className="mt-1 text-xs text-slate-500">
        Personel rapor olaylarını ve kurum ödeme kuralı kapsamındaki kayıtları yönetin.
      </p>
    </header>

    <div className="bg-rose-50/90 border border-rose-200 rounded-2xl p-4 space-y-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="font-bold text-xs text-rose-950 flex items-center gap-1.5">
          <FileText className="w-4 h-4 text-rose-600" />
          <span>Kurum Raporlu Gün Ödeme Kuralı (Takvim Yılı)</span>
        </div>
        <span className="text-[11px] text-rose-700 font-semibold">
          (Yılda ilk 5 raporda en fazla ilk 2 gün)
        </span>
      </div>
      <p className="text-xs text-rose-900 leading-relaxed">
        Bir işçinin takvim yılı içinde aldığı ilk 5 ayrı sağlık raporunun ilk 2&apos;şer günü kurum tarafından ödenir (6. ve sonraki rapor olaylarında kurum ödemesi 0 gündür). 15-14 dönem sınırından bölünen rapor olaylarında ilk 2 gün hakkı sadece 1 kez kullandırılır.
      </p>
    </div>

    {sickSuccessMsg && (
      <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
        <Check className="w-4 h-4 text-emerald-600" />
        <span>{sickSuccessMsg}</span>
      </div>
    )}

    <form onSubmit={onAddSickLeave} className="p-4 bg-slate-50 border border-slate-200 rounded-2xl space-y-3">
      <h3 className="font-bold text-xs text-slate-900 flex items-center gap-1.5">
        <Plus className="w-4 h-4 text-indigo-600" />
        <span>Yeni Rapor Olayı (İstirahat Kaydı) Ekle</span>
      </h3>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <div>
          <label className="block text-[11px] font-semibold text-slate-700 mb-1">
            Personel Seçimi
          </label>
          <select
            value={selectedPersonForSick}
            onChange={(e) => setSelectedPersonForSick(e.target.value)}
            className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-xs font-semibold text-slate-900 focus:ring-2 focus:ring-indigo-500"
          >
            {personeller.map((personel) => (
              <option key={personel.id} value={personel.id}>
                {personel.ad} {personel.soyad} (TC: {personel.tcNo})
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="block text-[11px] font-semibold text-slate-700 mb-1">
            Rapor Başlangıç Tarihi
          </label>
          <input
            type="date"
            value={sickStartDate}
            onChange={(e) => setSickStartDate(e.target.value)}
            required
            className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-indigo-500"
          />
        </div>

        <div>
          <label className="block text-[11px] font-semibold text-slate-700 mb-1">
            Rapor Bitiş Tarihi
          </label>
          <input
            type="date"
            value={sickEndDate}
            onChange={(e) => setSickEndDate(e.target.value)}
            required
            className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-indigo-500"
          />
        </div>
      </div>

      <div className="flex justify-end pt-1">
        <button
          type="submit"
          className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer"
        >
          <Plus className="w-3.5 h-3.5" />
          <span>Rapor Olayını Kaydet</span>
        </button>
      </div>
    </form>

    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-bold text-slate-800">
          Kayıtlı Rapor Olayları ({sickLeaveRecords.length})
        </span>
      </div>

      {sickLeaveRecords.length === 0 ? (
        <div className="text-center py-8 text-xs text-slate-500 bg-slate-50 rounded-2xl border border-slate-200">
          Henüz kayıtlı bir rapor olayı bulunmuyor. Yukarıdaki formdan ekleyebilirsiniz.
        </div>
      ) : (
        <div className="border border-slate-200 rounded-2xl overflow-hidden overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-slate-100 text-slate-700 font-bold uppercase text-[10px] tracking-wider border-b border-slate-200">
              <tr>
                <th className="p-3">Personel</th>
                <th className="p-3">Başlangıç Tarihi</th>
                <th className="p-3">Bitiş Tarihi</th>
                <th className="p-3 text-center">Toplam Gün</th>
                <th className="p-3 text-right">İşlem</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {sickLeaveRecords.map((record) => {
                const person = personeller.find((item) => item.id === record.personnelId);
                let totalDays = 1;
                try {
                  const start = new Date(record.startDate + 'T00:00:00');
                  const end = new Date(record.endDate + 'T00:00:00');
                  totalDays = Math.max(
                    1,
                    Math.round((end.getTime() - start.getTime()) / (1000 * 60 * 60 * 24)) + 1
                  );
                } catch {
                  totalDays = 1;
                }

                return (
                  <tr key={record.id} className="hover:bg-slate-50">
                    <td className="p-3 font-semibold text-slate-900">
                      {person ? `${person.ad} ${person.soyad}` : record.personnelId}
                    </td>
                    <td className="p-3 font-mono text-slate-700">{record.startDate}</td>
                    <td className="p-3 font-mono text-slate-700">{record.endDate}</td>
                    <td className="p-3 text-center font-bold font-mono text-rose-700">
                      {totalDays} Gün
                    </td>
                    <td className="p-3 text-right">
                      <button
                        type="button"
                        onClick={() => void onDeleteSickLeave(record.id)}
                        className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer"
                        title="Rapor Olayını Sil"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  </section>
);
