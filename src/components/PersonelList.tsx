/**
 * Section 2: Personel Bilgileri Listesi
 */

import React, { useState } from 'react';
import {
  UserPlus,
  Search,
  Edit2,
  Trash2,
  Users,
  Briefcase,
  Calendar,
  LayoutGrid,
  List,
} from 'lucide-react';
import { IsPrimiGrupItem, Personel } from '../types/payroll';
import { getGrupIsPrimiOrani, getGrupIsPrimiOraniDisplay } from '../utils/payrollPresentation';
import { PersonelFormModal } from './PersonelFormModal';

interface PersonelListProps {
  personeller: Personel[];
  onSavePersonel: (personel: Personel) => Promise<void> | void;
  onDeletePersonel: (personelId: string) => Promise<void> | void;
  onSelectPersonelForBordro?: (personelId: string) => void;
  isPrimiGruplari?: IsPrimiGrupItem[];
}

export const PersonelList: React.FC<PersonelListProps> = ({
  personeller,
  onSavePersonel,
  onDeletePersonel,
  onSelectPersonelForBordro,
  isPrimiGruplari,
}) => {
  const [search, setSearch] = useState<string>('');
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
  const [isModalOpen, setIsModalOpen] = useState<boolean>(false);
  const [personelToEdit, setPersonelToEdit] = useState<Personel | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const filteredPersoneller = personeller.filter((p) => {
    const term = search.toLowerCase();
    return (
      p.ad.toLowerCase().includes(term) ||
      p.soyad.toLowerCase().includes(term) ||
      p.tcNo.includes(term) ||
      (p.grup && p.grup.toLowerCase().includes(term)) ||
      (p.unvan && p.unvan.toLowerCase().includes(term)) ||
      (p.iban && p.iban.toLowerCase().includes(term))
    );
  });

  const handleEdit = (p: Personel) => {
    setPersonelToEdit(p);
    setIsModalOpen(true);
  };

  const handleAddNew = () => {
    setPersonelToEdit(null);
    setIsModalOpen(true);
  };

  const handleDelete = async (id: string) => {
    try {
      await onDeletePersonel(id);
      setConfirmDeleteId(null);
    } catch (err) {
      alert(`Personel silinemedi: ${String(err)}`);
    }
  };

  return (
    <div className="space-y-6">
      {/* Top Banner & Actions */}
      <div className="bg-white rounded-2xl p-5 border border-slate-200 shadow-2xs flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <Users className="w-5 h-5 text-indigo-600" />
            <h2 className="text-lg font-bold text-slate-900">
              4/D Personel Kayıtları ({personeller.length})
            </h2>
          </div>
          <p className="text-xs text-slate-500 mt-1">
            Kurum bünyesinde çalışan 4/D sürekli işçilerin özlük bilgileri
          </p>
        </div>

        <div className="flex items-center gap-3 flex-wrap">
          {/* View Mode Toggle Switcher */}
          <div className="flex items-center bg-slate-100 p-1 rounded-xl border border-slate-200 shrink-0">
            <button
              onClick={() => setViewMode('grid')}
              title="Kart Görünümü"
              className={`p-1.5 rounded-lg text-xs font-medium transition-all flex items-center gap-1 cursor-pointer ${
                viewMode === 'grid'
                  ? 'bg-white text-indigo-600 shadow-2xs font-bold'
                  : 'text-slate-500 hover:text-slate-800'
              }`}
            >
              <LayoutGrid className="w-4 h-4" />
              <span className="hidden sm:inline">Kart</span>
            </button>
            <button
              onClick={() => setViewMode('list')}
              title="Liste Görünümü"
              className={`p-1.5 rounded-lg text-xs font-medium transition-all flex items-center gap-1 cursor-pointer ${
                viewMode === 'list'
                  ? 'bg-white text-indigo-600 shadow-2xs font-bold'
                  : 'text-slate-500 hover:text-slate-800'
              }`}
            >
              <List className="w-4 h-4" />
              <span className="hidden sm:inline">Liste</span>
            </button>
          </div>

          <div className="relative min-w-56">
            <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Ad, soyad, T.C. veya grup ara..."
              className="w-full pl-9 pr-3 py-2 bg-slate-50 border border-slate-300 rounded-xl text-xs text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 transition-all"
            />
          </div>

          <button
            onClick={handleAddNew}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-all flex items-center gap-1.5 shrink-0 cursor-pointer"
          >
            <UserPlus className="w-4 h-4" />
            <span>Yeni Personel Ekle</span>
          </button>
        </div>
      </div>

      {/* Grid View */}
      {viewMode === 'grid' ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredPersoneller.map((p) => (
            <div
              key={p.id}
              onClick={() => handleEdit(p)}
              className="bg-white border border-slate-200 hover:border-indigo-400 rounded-2xl p-5 shadow-2xs hover:shadow-md transition-all flex flex-col justify-between group cursor-pointer"
            >
              <div>
                {/* Header */}
                <div className="flex items-start justify-between pb-3 border-b border-slate-100">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl bg-indigo-50 border border-indigo-100 text-indigo-700 font-bold flex items-center justify-center text-sm shadow-2xs shrink-0">
                      {p.ad[0]}
                      {p.soyad[0]}
                    </div>
                    <div>
                      <h3 className="font-bold text-slate-900 text-sm group-hover:text-indigo-600 transition-colors">
                        {p.ad} {p.soyad}
                      </h3>
                      <div className="text-[11px] text-slate-500 font-mono">
                        T.C.: {p.tcNo}
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                    <button
                      onClick={() => handleEdit(p)}
                      title="Düzenle"
                      className="p-1.5 text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 rounded-lg transition-colors cursor-pointer"
                    >
                      <Edit2 className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => setConfirmDeleteId(p.id)}
                      title="Sil"
                      className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                {/* Details */}
                <div className="py-3 space-y-2 text-xs text-slate-600">
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2">
                      <Briefcase className="w-3.5 h-3.5 text-indigo-600 shrink-0" />
                      <span className="font-bold text-slate-900">
                        {(p.grup || p.unvan || '1. Grup').replace(/\s*\(.*?\)/, '')}
                      </span>
                    </div>
                    <span className="px-2 py-0.5 bg-indigo-50 border border-indigo-200 text-indigo-700 font-bold text-[11px] rounded-md font-mono">
                      %{getGrupIsPrimiOraniDisplay(p.grup, isPrimiGruplari) ?? '—'} İş Primi
                    </span>
                  </div>

                  <div className="flex items-center gap-2 text-slate-500 font-mono">
                    <Calendar className="w-3.5 h-3.5 text-slate-400 shrink-0" />
                    <span>{p.hizmetYili} Yıl Kıdem</span>
                  </div>

                  {p.aciklama && (
                    <p className="text-[11px] text-slate-500 italic pt-1 truncate">
                      "{p.aciklama}"
                    </p>
                  )}
                </div>
              </div>

              {/* Bottom Actions */}
              <div className="pt-3 border-t border-slate-100 flex items-center justify-between" onClick={(e) => e.stopPropagation()}>
                {onSelectPersonelForBordro && (
                  <button
                    onClick={() => onSelectPersonelForBordro(p.id)}
                    className="w-full py-1.5 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 rounded-xl text-xs font-semibold transition-colors text-center cursor-pointer"
                  >
                    Bordro Hesapla / Düzenle →
                  </button>
                )}
              </div>
            </div>
          ))}

          {filteredPersoneller.length === 0 && (
            <div className="col-span-full bg-white border border-dashed border-slate-300 rounded-2xl p-12 text-center text-slate-500 space-y-3">
              <Users className="w-10 h-10 text-slate-300 mx-auto" />
              <div className="font-medium text-sm text-slate-700">Arama kriterlerine uygun personel bulunamadı.</div>
              <p className="text-xs text-slate-400">Yeni personel ekleyebilir veya arama terimini değiştirebilirsiniz.</p>
            </div>
          )}
        </div>
      ) : (
        /* List / Table View */
        <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-2xs">
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse text-xs">
              <thead>
                <tr className="bg-slate-50 border-b border-slate-200 text-slate-600 font-semibold uppercase text-[11px] tracking-wider">
                  <th className="p-3 pl-4">Personel Ad Soyad</th>
                  <th className="p-3">T.C. Kimlik No</th>
                  <th className="p-3">İş Primi Grubu</th>
                  <th className="p-3">Kıdem</th>
                  <th className="p-3 text-right pr-4">İşlemler</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {filteredPersoneller.map((p) => (
                  <tr
                    key={p.id}
                    onClick={() => handleEdit(p)}
                    className="hover:bg-slate-50 transition-colors cursor-pointer group"
                  >
                    <td className="p-3 pl-4">
                      <div className="flex items-center gap-3">
                        <div className="w-8 h-8 rounded-lg bg-indigo-50 border border-indigo-100 text-indigo-700 font-bold flex items-center justify-center text-xs shrink-0">
                          {p.ad[0]}
                          {p.soyad[0]}
                        </div>
                        <div>
                          <div className="font-bold text-slate-900 group-hover:text-indigo-600 transition-colors">
                            {p.ad} {p.soyad}
                          </div>
                          {p.aciklama && (
                            <div className="text-[10px] text-slate-400 italic truncate max-w-xs">
                              {p.aciklama}
                            </div>
                          )}
                        </div>
                      </div>
                    </td>
                    <td className="p-3 font-mono text-slate-700">{p.tcNo}</td>
                    <td className="p-3">
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-slate-900">
                          {(p.grup || p.unvan || '1. Grup').replace(/\s*\(.*?\)/, '')}
                        </span>
                        <span className="px-1.5 py-0.5 bg-indigo-50 border border-indigo-200 text-indigo-700 font-bold text-[10px] rounded font-mono">
                          %{getGrupIsPrimiOraniDisplay(p.grup, isPrimiGruplari) ?? '—'}
                        </span>
                      </div>
                    </td>
                    <td className="p-3 font-mono text-slate-600">{p.hizmetYili} Yıl Kıdem</td>
                    <td className="p-3 pr-4 text-right" onClick={(e) => e.stopPropagation()}>
                      <div className="flex items-center justify-end gap-1">
                        {onSelectPersonelForBordro && (
                          <button
                            onClick={() => onSelectPersonelForBordro(p.id)}
                            title="Bordro Hesapla"
                            className="px-2.5 py-1 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 rounded-lg text-xs font-semibold transition-colors mr-1 cursor-pointer"
                          >
                            Bordro
                          </button>
                        )}
                        <button
                          onClick={() => handleEdit(p)}
                          title="Düzenle"
                          className="p-1.5 text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 rounded-lg transition-colors cursor-pointer"
                        >
                          <Edit2 className="w-4 h-4" />
                        </button>
                        <button
                          onClick={() => setConfirmDeleteId(p.id)}
                          title="Sil"
                          className="p-1.5 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition-colors cursor-pointer"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}

                {filteredPersoneller.length === 0 && (
                  <tr>
                    <td colSpan={5} className="p-8 text-center text-slate-500">
                      Arama kriterlerine uygun personel bulunamadı.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Delete Confirmation Dialog */}
      {confirmDeleteId && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 overflow-y-auto"
          onClick={() => setConfirmDeleteId(null)}
        >
          <div
            className="bg-white rounded-2xl p-6 max-w-sm w-full border border-slate-200 shadow-xl text-center space-y-4 animate-in fade-in zoom-in-95 my-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="w-12 h-12 bg-rose-100 text-rose-600 rounded-full flex items-center justify-center mx-auto">
              <Trash2 className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-bold text-base text-slate-900">Personel Silinsin mi?</h3>
              <p className="text-xs text-slate-500 mt-1">
                Bu personeli silmek istediğinize emin misiniz? İlgili döneme ait puantaj verileri de etkilenecektir.
              </p>
            </div>
            <div className="flex gap-2 justify-center pt-2">
              <button
                onClick={() => setConfirmDeleteId(null)}
                className="px-4 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition-colors cursor-pointer"
              >
                Vazgeç
              </button>
              <button
                onClick={() => handleDelete(confirmDeleteId)}
                className="px-4 py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors cursor-pointer"
              >
                Evet, Sil
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Add / Edit Form Modal */}
      <PersonelFormModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        personelToEdit={personelToEdit}
        onSave={onSavePersonel}
        isPrimiGruplari={isPrimiGruplari}
      />
    </div>
  );
};
