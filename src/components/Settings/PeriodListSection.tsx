import React, { useState } from 'react';
import { Calendar, Plus } from 'lucide-react';
import { describeError } from '../../utils/errorMessage';
import type { BordroDonemi } from '../../types/payroll';

interface PeriodListSectionProps {
  donemler: BordroDonemi[];
  aktifDonemId: string;
  onDeleteDonem: (donemId: string) => Promise<void> | void;
  onSelectDonem: (donemId: string) => Promise<void> | void;
  onOpenNewPeriod: () => void;
}

export const PeriodListSection: React.FC<PeriodListSectionProps> = ({
  donemler,
  aktifDonemId,
  onSelectDonem,
  onDeleteDonem,
  onOpenNewPeriod,
}) => {
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const handleDelete = async (donem: BordroDonemi) => {
    if (isDeleting) return;
    setIsDeleting(true);
    setError(null);
    setSuccess(null);
    try {
      await onDeleteDonem(donem.id);
      setPendingDeleteId(null);
      setSuccess(`${donem.donemAdi} dönemi silindi.`);
    } catch (cause) {
      setError(`Dönem silinemedi: ${describeError(cause)}`);
    } finally {
      setIsDeleting(false);
    }
  };
  return (
    <section data-testid="period-settings-donemler" className="space-y-5">
      <header>
        <h2 className="text-xl font-bold text-slate-900">Dönemler</h2>
        <p className="mt-1 text-xs text-slate-500">
          Mevcut bordro dönemlerini görüntüleyin ve aktif dönemi seçin.
        </p>
      </header>

      {error && <p role="alert" className="text-sm text-rose-700">{error}</p>}
      {success && <p role="status" className="text-sm text-emerald-700">{success}</p>}
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
              <div key={donem.id}>
                <div className="flex items-center">
                  <button
                    type="button"
                    data-testid={`period-row-${donem.id}`}
                    disabled={isDeleting}
                    onClick={() => void onSelectDonem(donem.id)}
                    className={`min-w-0 flex-1 p-4 flex items-center justify-between text-left cursor-pointer transition-colors ${
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
                  <button
                    type="button"
                    data-testid={`delete-period-${donem.id}`}
                    aria-label={`${donem.donemAdi} dönemini sil`}
                    disabled={isDeleting}
                    onClick={() => { setPendingDeleteId(donem.id); setError(null); setSuccess(null); }}
                    className="mx-3 rounded-lg px-3 py-2 text-xs font-semibold text-rose-700 hover:bg-rose-50 disabled:opacity-50"
                  >Sil</button>
                </div>
                {pendingDeleteId === donem.id && (
                  <div className="border-t border-rose-100 bg-rose-50 p-4" role="region" aria-label="Dönem silme onayı">
                    <p className="text-sm font-semibold text-slate-900">{donem.donemAdi} dönemi silinsin mi?</p>
                    <p className="mt-1 text-xs leading-relaxed text-slate-700">
                      Bu dönemin bordroları (kesinleşmiş olanlar dahil), puantajları, parametreleri ve döneme bağlı vergi devir kayıtları kalıcı olarak silinir.
                      Etkilenen geriye dönük fark kayıtları ve ödemeleri de silinir. Etkilenen sonraki bordrolar yeniden hesaplanmalıdır. Bu işlem geri alınamaz.
                    </p>
                    <div className="mt-3 flex gap-2">
                      <button type="button" disabled={isDeleting} onClick={() => { setPendingDeleteId(null); setError(null); }} className="rounded-lg bg-white px-3 py-2 text-xs font-semibold text-slate-700 disabled:opacity-50">Vazgeç</button>
                      <button type="button" disabled={isDeleting} onClick={() => void handleDelete(donem)} className="rounded-lg bg-rose-600 px-3 py-2 text-xs font-semibold text-white hover:bg-rose-700 disabled:opacity-50">{isDeleting ? 'Siliniyor…' : 'Evet, Dönemi Sil'}</button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
};
