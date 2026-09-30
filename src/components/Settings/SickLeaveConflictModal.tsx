import React from 'react';
import { AlertTriangle, Check, X } from 'lucide-react';
import { PUANTAJ_KODLARI } from '../../types/payroll';
import type { SickLeaveConflict } from '../../utils/sickLeaveSync';

export interface SickLeaveConflictModalProps {
  isOpen: boolean;
  conflicts: SickLeaveConflict[];
  onApply: () => Promise<void> | void;
  onCancel: () => void;
  isSubmitting?: boolean;
}

export const SickLeaveConflictModal: React.FC<SickLeaveConflictModalProps> = ({
  isOpen,
  conflicts,
  onApply,
  onCancel,
  isSubmitting = false,
}) => {
  if (!isOpen) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="sick-leave-conflict-title"
      data-testid="sick-leave-conflict-modal"
      className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs z-50 flex items-center justify-center p-4 animate-in fade-in duration-200"
    >
      <div className="bg-white rounded-2xl shadow-2xl max-w-lg w-full border border-slate-200 overflow-hidden flex flex-col max-h-[90vh]">
        {/* Modal Header */}
        <div className="p-5 border-b border-slate-100 flex items-start gap-3 bg-amber-50/50">
          <div className="p-2 bg-amber-100 text-amber-700 rounded-xl shrink-0 mt-0.5">
            <AlertTriangle className="w-5 h-5" />
          </div>
          <div className="flex-1">
            <h3 id="sick-leave-conflict-title" className="text-sm font-bold text-slate-900">
              Puantaj Kod Çakışması Onayı
            </h3>
            <p className="mt-1 text-xs text-slate-600 leading-relaxed">
              Kaydedilmek istenen rapor tarihlerinde ({conflicts.length} gün) mevcut puantaj kodları bulunmaktadır.
            </p>
          </div>
        </div>

        {/* Modal Body */}
        <div className="p-5 space-y-4 overflow-y-auto">
          <div className="bg-amber-50 border border-amber-200 rounded-xl p-3 text-xs text-amber-900 leading-relaxed">
            <strong>&quot;Uygula&quot;</strong> seçildiğinde aşağıdaki günlerdeki puantaj kodları{' '}
            <strong>&quot;R&quot; (Raporlu)</strong> olarak güncellenecek ve rapor kaydedilecektir.{' '}
            <strong>&quot;İptal&quot;</strong> seçildiğinde ne rapor ne de puantaj kayıtları değiştirilmeyecektir.
          </div>

          <div className="border border-slate-200 rounded-xl overflow-hidden overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead className="bg-slate-50 text-slate-700 font-bold uppercase text-[10px] tracking-wider border-b border-slate-200">
                <tr>
                  <th className="p-2.5">Tarih</th>
                  <th className="p-2.5">Dönem</th>
                  <th className="p-2.5">Mevcut Kod</th>
                  <th className="p-2.5">Yeni Kod</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {conflicts.map((conflict) => {
                  const info = PUANTAJ_KODLARI[conflict.currentCode];
                  return (
                    <tr key={`${conflict.periodId}_${conflict.date}`} className="hover:bg-slate-50">
                      <td className="p-2.5 font-mono font-semibold text-slate-900">
                        {conflict.date}
                      </td>
                      <td className="p-2.5 text-slate-600">
                        {conflict.periodName || conflict.periodId}
                      </td>
                      <td className="p-2.5">
                        <span
                          className={`inline-flex items-center px-2 py-0.5 rounded text-[11px] font-semibold border ${
                            info?.bgRenk || 'bg-slate-100 text-slate-700 border-slate-200'
                          }`}
                        >
                          {conflict.currentCode} — {info?.tanim || conflict.currentCode}
                        </span>
                      </td>
                      <td className="p-2.5">
                        <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-bold border bg-rose-50 border-rose-200 text-rose-700">
                          R — Raporlu
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>

        {/* Modal Footer */}
        <div className="p-4 bg-slate-50 border-t border-slate-100 flex items-center justify-end gap-2">
          <button
            type="button"
            data-testid="sick-leave-conflict-cancel"
            onClick={onCancel}
            disabled={isSubmitting}
            className="px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-200/70 rounded-xl transition-colors cursor-pointer flex items-center gap-1.5"
          >
            <X className="w-4 h-4" />
            <span>İptal</span>
          </button>
          <button
            type="button"
            data-testid="sick-leave-conflict-apply"
            onClick={() => void onApply()}
            disabled={isSubmitting}
            className="px-4 py-2 text-xs font-semibold text-white bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 rounded-xl shadow-xs transition-colors cursor-pointer flex items-center gap-1.5"
          >
            <Check className="w-4 h-4" />
            <span>{isSubmitting ? 'Uygulanıyor...' : 'Uygula'}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
