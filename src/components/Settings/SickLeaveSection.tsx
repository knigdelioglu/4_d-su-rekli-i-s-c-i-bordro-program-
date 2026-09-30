import React from 'react';
import { Check, Edit2, FileText, Plus, Trash2, X } from 'lucide-react';
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
  editingSickRecord?: SickLeaveRecord | null;
  onStartEditSickLeave: (record: SickLeaveRecord) => void;
  onCancelEditSickLeave: () => void;
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
  editingSickRecord,
  onStartEditSickLeave,
  onCancelEditSickLeave,
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

    <form
      onSubmit={onAddSickLeave}
      className={`p-4 rounded-2xl space-y-3 border transition-colors ${
        editingSickRecord ? 'bg-amber-50/60 border-amber-300' : 'bg-slate-50 border-slate-200'
      }`}
    >
      <div className="flex items-center justify-between">
        <h3 className="font-bold text-xs text-slate-900 flex items-center gap-1.5">
          {editingSickRecord ? (
            <>
              <Edit2 className="w-4 h-4 text-amber-600" />
              <span>Rapor Olayını Düzenle</span>
            </>
          ) : (
            <>
              <Plus className="w-4 h-4 text-indigo-600" />
              <span>Yeni Rapor Olayı (İstirahat Kaydı) Ekle</span>
            </>
          )}
        </h3>
        {editingSickRecord && (
          <button
            type="button"
            onClick={onCancelEditSickLeave}
            className="text-xs text-slate-500 hover:text-slate-800 font-semibold flex items-center gap-1 px-2.5 py-1 rounded-lg hover:bg-slate-200/60 transition-colors cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
            <span>Düzenlemeden Vazgeç</span>
          </button>
        )}
      </div>

      {editingSickRecord && (
        <div className="p-2.5 bg-amber-100/70 border border-amber-300 rounded-xl text-xs text-amber-900 flex items-center justify-between">
          <span>
            <strong>Düzenleme Modu:</strong> Seçili personelin {editingSickRecord.startDate} - {editingSickRecord.endDate} tarihli raporu güncelleniyor. Tarihleri daraltırsanız aralık dışı kalan günler varsayılan puantaja döner.
          </span>
        </div>
      )}

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

      <div className="flex justify-end gap-2 pt-1">
        {editingSickRecord && (
          <button
            type="button"
            onClick={onCancelEditSickLeave}
            className="px-3.5 py-2 text-slate-600 hover:bg-slate-200/70 rounded-xl text-xs font-semibold transition-colors cursor-pointer"
          >
            İptal
          </button>
        )}
        <button
          type="submit"
          className={`px-4 py-2 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer ${
            editingSickRecord
              ? 'bg-amber-600 hover:bg-amber-700'
              : 'bg-indigo-600 hover:bg-indigo-700'
          }`}
        >
          {editingSickRecord ? (
            <>
              <Edit2 className="w-3.5 h-3.5" />
              <span>Rapor Olayını Güncelle</span>
            </>
          ) : (
            <>
              <Plus className="w-3.5 h-3.5" />
              <span>Rapor Olayını Kaydet</span>
            </>
          )}
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
        <div className="border border-slate-200 rounded-2xl overflow-hidden overflow-x-auto shadow-2xs bg-white">
          <table className="w-full min-w-[620px] text-left text-xs">
            <thead className="bg-slate-100 text-slate-700 font-bold uppercase text-[10px] tracking-wider border-b border-slate-200">
              <tr>
                <th className="p-3">Personel</th>
                <th className="p-3">Başlangıç Tarihi</th>
                <th className="p-3">Bitiş Tarihi</th>
                <th className="p-3 text-center">Toplam Gün</th>
                <th className="p-3 text-right whitespace-nowrap min-w-[170px]">İşlemler</th>
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
                    <td className="p-3 font-mono text-slate-700 whitespace-nowrap">{record.startDate}</td>
                    <td className="p-3 font-mono text-slate-700 whitespace-nowrap">{record.endDate}</td>
                    <td className="p-3 text-center font-bold font-mono text-rose-700 whitespace-nowrap">
                      {totalDays} Gün
                    </td>
                    <td className="p-3 text-right whitespace-nowrap min-w-[170px]">
                      <div className="flex items-center justify-end gap-2 shrink-0 flex-nowrap">
                        <button
                          type="button"
                          data-testid={`edit-sick-leave-${record.id}`}
                          onClick={() => onStartEditSickLeave(record)}
                          className="inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-700 active:bg-indigo-800 rounded-lg shadow-xs transition-colors cursor-pointer shrink-0"
                          title="Rapor Olayını Düzenle"
                          aria-label={`Rapor Olayını Düzenle (${record.startDate} - ${record.endDate})`}
                        >
                          <Edit2 className="w-3.5 h-3.5 text-white shrink-0" aria-hidden="true" />
                          <span className="font-bold text-white tracking-wide">Düzenle</span>
                        </button>
                        <button
                          type="button"
                          data-testid={`delete-sick-leave-${record.id}`}
                          onClick={() => void onDeleteSickLeave(record.id)}
                          className="inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-bold text-rose-700 bg-rose-50 hover:bg-rose-100 active:bg-rose-200 border border-rose-300 rounded-lg shadow-xs transition-colors cursor-pointer shrink-0"
                          title="Rapor Olayını Sil"
                          aria-label={`Rapor Olayını Sil (${record.startDate} - ${record.endDate})`}
                        >
                          <Trash2 className="w-3.5 h-3.5 text-rose-600 shrink-0" aria-hidden="true" />
                          <span className="font-bold text-rose-700 tracking-wide">Sil</span>
                        </button>
                      </div>
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
