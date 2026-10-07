import React, { useEffect, useState } from 'react';
import type { IsPrimiGrupItem } from '../../types/payroll';

interface GroupDraft {
  id: string;
  ad: string;
  oran: string;
  aktif: boolean;
}

interface Props {
  groups: IsPrimiGrupItem[];
  onSave: (groups: IsPrimiGrupItem[]) => Promise<void> | void;
}

const inputClassName = 'min-h-10 w-full rounded-md border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-100';

function toDraft(groups: IsPrimiGrupItem[]): GroupDraft[] {
  return groups.map((group) => ({
    id: group.id,
    ad: group.ad,
    oran: String(group.oran),
    aktif: group.aktif ?? true,
  }));
}

export const IsPrimiGruplariSection: React.FC<Props> = ({ groups, onSave }) => {
  const [draft, setDraft] = useState<GroupDraft[]>(() => toDraft(groups));
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    setDraft(toDraft(groups));
    setError(null);
  }, [groups]);

  const update = (index: number, patch: Partial<GroupDraft>) => {
    setDraft((current) => current.map((group, itemIndex) =>
      itemIndex === index ? { ...group, ...patch } : group
    ));
    setError(null);
    setSaved(false);
  };

  const handleSave = async () => {
    const normalized = draft.map((group) => ({
      ...group,
      id: group.id.trim(),
      ad: group.ad.trim(),
      rate: group.oran.trim().replace(',', '.'),
    }));
    if (normalized.length === 0) {
      setError('En az bir iş primi grubu ekleyin.');
      return;
    }
    if (normalized.some((group) => !group.id || !group.ad || !/^(?:\d+)(?:\.\d+)?$/.test(group.rate)
      || !Number.isFinite(Number(group.rate)) || Number(group.rate) < 0 || Number(group.rate) > 100)) {
      setError('Grup kodu, adı ve %0–%100 arasında geçerli bir oran girin.');
      return;
    }
    const ids = normalized.map((group) => group.id.toLocaleLowerCase('tr'));
    const names = normalized.map((group) => group.ad.toLocaleLowerCase('tr'));
    if (new Set(ids).size !== ids.length || new Set(names).size !== names.length) {
      setError('Grup kodları ve adları birbirinden farklı olmalıdır.');
      return;
    }

    setIsSaving(true);
    setError(null);
    try {
      await onSave(normalized.map(({ rate, ...group }) => ({
        id: group.id,
        ad: group.ad,
        oran: Number(rate),
        aktif: group.aktif,
      })));
      setSaved(true);
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : 'İş primi grupları kaydedilemedi.');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <section data-testid="period-settings-is-primi-gruplari" className="space-y-5" aria-labelledby="premium-groups-title">
      <header>
        <h2 id="premium-groups-title" className="text-xl font-bold text-slate-900">İş Primi Grupları</h2>
        <p className="mt-1 text-sm text-slate-600">Personel kartlarında seçilen grupları ve oranlarını düzenleyin. Bu ayar tüm dönemlerde kullanılır.</p>
      </header>

      {error && <p role="alert" className="rounded-md border border-rose-300 bg-rose-50 p-3 text-sm text-rose-900">{error}</p>}
      {saved && <p role="status" className="rounded-md border border-emerald-300 bg-emerald-50 p-3 text-sm text-emerald-900">İş primi grupları kaydedildi.</p>}

      <div className="overflow-x-auto rounded-xl border border-slate-200 bg-white">
        <table className="w-full min-w-[760px] border-collapse text-left text-sm">
          <caption className="sr-only">İş primi grupları, personel kodları, oranları ve etkinlik durumları</caption>
          <thead className="border-b border-slate-200 bg-slate-50 text-xs font-semibold text-slate-600">
            <tr>
              <th scope="col" className="px-3 py-3">Grup adı</th>
              <th scope="col" className="px-3 py-3">Personel kodu</th>
              <th scope="col" className="w-36 px-3 py-3">Oran (%)</th>
              <th scope="col" className="w-28 px-3 py-3">Durum</th>
              <th scope="col" className="w-24 px-3 py-3 text-right">İşlemler</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {draft.map((group, index) => (
              <tr key={`${group.id}:${index}`}>
                <td className="px-3 py-3">
                  <label className="sr-only" htmlFor={`premium-group-name-${index}`}>Grup adı</label>
                  <input id={`premium-group-name-${index}`} className={inputClassName} value={group.ad} disabled={isSaving}
                    onChange={(event) => update(index, { ad: event.target.value })} />
                </td>
                <td className="px-3 py-3">
                  <label className="sr-only" htmlFor={`premium-group-code-${index}`}>Personel kodu</label>
                  <input id={`premium-group-code-${index}`} className={inputClassName} value={group.id} disabled={isSaving}
                    onChange={(event) => update(index, { id: event.target.value })} />
                </td>
                <td className="px-3 py-3">
                  <label className="sr-only" htmlFor={`premium-group-rate-${index}`}>Oran (%)</label>
                  <input id={`premium-group-rate-${index}`} className={inputClassName} type="text" inputMode="decimal" autoComplete="off" value={group.oran} disabled={isSaving}
                    onChange={(event) => update(index, { oran: event.target.value })} />
                </td>
                <td className="px-3 py-3">
                  <label className="inline-flex min-h-10 items-center gap-2 text-sm font-medium text-slate-800">
                    <input type="checkbox" checked={group.aktif} disabled={isSaving} className="h-4 w-4 accent-indigo-600"
                      aria-label={`${group.ad || `Grup ${index + 1}`} etkin`}
                      onChange={(event) => update(index, { aktif: event.target.checked })} />
                    Etkin
                  </label>
                </td>
                <td className="px-3 py-3 text-right">
                  <button type="button" className="min-h-9 rounded px-2 text-sm font-medium text-rose-700 hover:bg-rose-50 disabled:opacity-50"
                    disabled={isSaving} aria-label={`${group.ad || `Grup ${index + 1}`} grubunu kaldır`}
                    onClick={() => {
                      setDraft((current) => current.filter((_, itemIndex) => itemIndex !== index));
                      setSaved(false);
                      setError(null);
                    }}>
                    Kaldır
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <button type="button" className="min-h-9 rounded px-2 text-sm font-semibold text-indigo-800 underline underline-offset-2 hover:bg-indigo-50 disabled:opacity-50"
          disabled={isSaving} onClick={() => {
            setDraft((current) => [...current, { id: '', ad: '', oran: '', aktif: true }]);
            setSaved(false);
            setError(null);
          }}>
          Grup ekle
        </button>
        <button type="button" className="min-h-10 rounded-md bg-indigo-700 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-800 disabled:cursor-not-allowed disabled:bg-slate-300"
          disabled={isSaving} onClick={() => void handleSave()}>
          {isSaving ? 'Kaydediliyor…' : 'Kaydet'}
        </button>
      </div>
    </section>
  );
};
