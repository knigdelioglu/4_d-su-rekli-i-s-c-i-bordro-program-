/**
 * Section 2: Personel Ekleme / Düzenleme Modalı
 */

import React, { useState, useEffect } from 'react';
import { User, Save, X, CreditCard, Shield, Briefcase, Calendar, Layers } from 'lucide-react';
import { IsPrimiGrupItem, Personel } from '../types/payroll';
import { DEFAULT_IS_PRIMI_GRUPLARI, getGrupIsPrimiOraniDisplay } from '../utils/payrollPresentation';

interface PersonelFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  personelToEdit?: Personel | null;
  onSave: (personel: Personel) => Promise<void> | void;
  isPrimiGruplari?: IsPrimiGrupItem[];
}

type PersonelFormSection = 'basic' | 'deductions' | 'advanced';

export const PersonelFormModal: React.FC<PersonelFormModalProps> = ({
  isOpen,
  onClose,
  personelToEdit,
  onSave,
  isPrimiGruplari,
}) => {
  const groups = isPrimiGruplari && isPrimiGruplari.length > 0 ? isPrimiGruplari : DEFAULT_IS_PRIMI_GRUPLARI;

  const [formData, setFormData] = useState<Partial<Personel>>({
    tcNo: '',
    ad: '',
    soyad: '',
    grup: groups[0]?.ad || '1. Grup',
    unvan: `${groups[0]?.ad || '1. Grup'} (%${groups[0]?.oran || 9} İş Primi)`,
    sgkSicilNo: '',
    iban: 'TR',
    hizmetYili: 1,
    aciklama: '',
    kesintiler: {
      sendikaUyesi: false,
      besUyesi: false,
    },
  });

  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isSaving, setIsSaving] = useState(false);
  const [activeFormSection, setActiveFormSection] = useState<PersonelFormSection>('basic');

  useEffect(() => {
    if (personelToEdit) {
      setFormData(personelToEdit);
    } else {
      const defaultGrup = groups[0]?.ad || '1. Grup';
      const defaultRate = groups[0]?.oran || 9;
      setFormData({
        tcNo: '',
        ad: '',
        soyad: '',
        grup: defaultGrup,
        unvan: `${defaultGrup} (%${defaultRate} İş Primi)`,
        sgkSicilNo: '',
        iban: 'TR',
        hizmetYili: 1,
        aciklama: '',
        kesintiler: {
          sendikaUyesi: false,
          besUyesi: false,
        },
      });
    }
    setErrors({});
    setActiveFormSection('basic');
  }, [personelToEdit, isOpen, isPrimiGruplari]);

  if (!isOpen) return null;

  const validate = (): boolean => {
    const errs: Record<string, string> = {};

    if (!formData.tcNo || formData.tcNo.replace(/\D/g, '').length !== 11) {
      errs.tcNo = 'T.C. Kimlik numarası 11 haneli olmalıdır.';
    }

    if (!formData.ad || formData.ad.trim().length === 0) {
      errs.ad = 'Ad alanı zorunludur.';
    }

    if (!formData.soyad || formData.soyad.trim().length === 0) {
      errs.soyad = 'Soyad alanı zorunludur.';
    }

    if (!formData.iban || !formData.iban.startsWith('TR')) {
      errs.iban = 'Görünür IBAN "TR" ile başlamalıdır.';
    }

    if ((formData.hizmetYili ?? 0) < 0) {
      errs.hizmetYili = 'Hizmet yılı negatif olamaz.';
    }

    const oksRate = formData.kesintiler?.oksOraniYuzde;
    if (formData.kesintiler?.besUyesi === true && oksRate != null && (oksRate < 3 || oksRate > 100)) {
      errs.oksOraniYuzde = 'OKS özel oranı %3-%100 arasında olmalıdır.';
    }

    setErrors(errs);
    return Object.keys(errs).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!validate()) return;

    const selGrup = formData.grup || groups[0]?.ad || '1. Grup';
    const rate = getGrupIsPrimiOraniDisplay(selGrup, groups) ?? groups[0]?.oran ?? 9;

    const newPersonel: Personel = {
      id: formData.id || `p-${Date.now()}`,
      tcNo: formData.tcNo?.trim() || '',
      ad: formData.ad?.trim() || '',
      soyad: formData.soyad?.trim() || '',
      grup: selGrup,
      unvan: `${selGrup} (%${rate} İş Primi)`,
      sgkSicilNo: formData.sgkSicilNo?.trim() || '',
      iban: formData.iban?.trim().toUpperCase() || 'TR',
      hizmetYili: Number(formData.hizmetYili) || 0,
      aciklama: formData.aciklama?.trim() || '',
      devirKumulatifGvMatrahi: Number(formData.devirKumulatifGvMatrahi) || 0,
      devirKumulatifGvMatrahiYili: formData.devirKumulatifGvMatrahiYili || (Number(formData.devirKumulatifGvMatrahi) ? new Date().getFullYear() : undefined),
      devirKumulatifGvMatrahiBaslangicAyi: Number(formData.devirKumulatifGvMatrahi) > 0
        ? formData.devirKumulatifGvMatrahiBaslangicAyi || 1
        : undefined,
      devirKumulatifAsgariGvMatrahi: Number(formData.devirKumulatifAsgariGvMatrahi) || 0,
      devirKumulatifAsgariGvMatrahiYili: formData.devirKumulatifAsgariGvMatrahiYili || (Number(formData.devirKumulatifAsgariGvMatrahi) ? new Date().getFullYear() : undefined),
      kesintiler: formData.kesintiler ? {
        ...formData.kesintiler,
        sendikaUyesi: formData.kesintiler.sendikaUyesi === true,
        besUyesi: formData.kesintiler.besUyesi === true,
      } : {
        sendikaUyesi: false,
        besUyesi: false,
      },
    };

    setIsSaving(true);
    try {
      await onSave(newPersonel);
      onClose();
    } catch (err) {
      setErrors({ form: `Kayıt başarısız: ${String(err)}` });
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 overflow-y-auto"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-2xl shadow-xl border border-slate-200 max-w-xl w-full max-h-[90vh] flex flex-col overflow-hidden my-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="bg-slate-900 text-white px-6 py-4 flex items-center justify-between shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="p-2 bg-indigo-600/30 rounded-lg text-indigo-300">
              <User className="w-5 h-5" />
            </div>
            <div>
              <h3 className="font-semibold text-base text-white">
                {personelToEdit ? 'Personel Düzenle' : 'Yeni Personel Ekle'}
              </h3>
              <p className="text-xs text-slate-400">4/D Sürekli işçi kimlik ve banka bilgileri</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-slate-400 hover:text-white hover:bg-slate-800 p-1.5 rounded-lg transition-colors cursor-pointer"
            title="Kapat"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="flex-1 flex flex-col overflow-hidden">
          <div className="p-6 space-y-4 overflow-y-auto flex-1">
          {errors.form && (
            <div className="p-3 bg-rose-50 border border-rose-200 text-rose-800 rounded-xl text-xs font-semibold">
              {errors.form}
            </div>
          )}
          <div role="tablist" aria-label="Personel bilgi bölümleri" className="grid grid-cols-3 gap-1 rounded-xl bg-slate-100 p-1">
            {([
              ['basic', 'Temel Bilgiler'],
              ['deductions', 'Kesintiler'],
              ['advanced', 'Vergi / İleri'],
            ] as const).map(([section, label]) => (
              <button
                key={section}
                type="button"
                role="tab"
                aria-selected={activeFormSection === section}
                onClick={() => setActiveFormSection(section)}
                data-testid={`personel-form-tab-${section}`}
                className={`rounded-lg px-2 py-2 text-xs font-bold transition-colors ${
                  activeFormSection === section
                    ? 'bg-white text-indigo-700 shadow-2xs'
                    : 'text-slate-500 hover:bg-white/70 hover:text-slate-700'
                }`}
              >
                {label}
              </button>
            ))}
          </div>

          {activeFormSection === 'basic' && (
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            {/* T.C. Kimlik No */}
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                T.C. Kimlik Numarası *
              </label>
              <input
                type="text"
                maxLength={11}
                value={formData.tcNo || ''}
                onChange={(e) =>
                  setFormData({ ...formData, tcNo: e.target.value.replace(/\D/g, '') })
                }
                placeholder="11 Haneli T.C. No"
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
              {errors.tcNo && (
                <p className="text-[11px] text-rose-600 mt-0.5 font-medium">{errors.tcNo}</p>
              )}
            </div>

            {/* Hizmet Yılı */}
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1 flex items-center gap-1">
                <Calendar className="w-3.5 h-3.5 text-slate-400" />
                <span>Hizmet Yılı (Kıdem)</span>
              </label>
              <input
                type="number"
                min={0}
                max={50}
                value={formData.hizmetYili ?? 0}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    hizmetYili: parseInt(e.target.value, 10) || 0,
                  })
                }
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Hizmet zammı hesabında kullanılır
              </span>
            </div>

            {/* Ad */}
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Ad *</label>
              <input
                type="text"
                value={formData.ad || ''}
                onChange={(e) => setFormData({ ...formData, ad: e.target.value })}
                placeholder="Örn: Ahmet"
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
              {errors.ad && (
                <p className="text-[11px] text-rose-600 mt-0.5 font-medium">{errors.ad}</p>
              )}
            </div>

            {/* Soyad */}
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Soyad *</label>
              <input
                type="text"
                value={formData.soyad || ''}
                onChange={(e) => setFormData({ ...formData, soyad: e.target.value })}
                placeholder="Örn: Yılmaz"
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
              {errors.soyad && (
                <p className="text-[11px] text-rose-600 mt-0.5 font-medium">{errors.soyad}</p>
              )}
            </div>

            {/* İş Primi Grubu */}
            <div className="sm:col-span-2">
              <label className="block text-xs font-semibold text-slate-700 mb-1 flex items-center gap-1">
                <Layers className="w-3.5 h-3.5 text-indigo-600" />
                <span>İş Primi Grubu *</span>
              </label>
              <select
                value={formData.grup || groups[0]?.ad || '1. Grup'}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    grup: e.target.value,
                  })
                }
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-semibold focus:bg-white focus:ring-2 focus:ring-indigo-500 cursor-pointer"
              >
                {groups.map((g) => (
                  <option key={g.id || g.ad} value={g.ad}>
                    {g.ad} (%{g.oran} İş Primi)
                  </option>
                ))}
              </select>
              <span className="text-[11px] text-slate-500 mt-1 block">
                Seçilen grubun iş primi oranı taban brüt aylığa uygulanır.
              </span>
            </div>

            {/* SGK Sicil No */}
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1 flex items-center gap-1">
                <Shield className="w-3.5 h-3.5 text-slate-400" />
                <span>SGK Sicil Numarası</span>
              </label>
              <input
                type="text"
                value={formData.sgkSicilNo || ''}
                onChange={(e) => setFormData({ ...formData, sgkSicilNo: e.target.value })}
                placeholder="Örn: 48201938201"
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
            </div>

            {/* IBAN */}
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1 flex items-center gap-1">
                <CreditCard className="w-3.5 h-3.5 text-slate-400" />
                <span>Maaş IBAN Numarası *</span>
              </label>
              <input
                type="text"
                value={formData.iban || ''}
                onChange={(e) =>
                  setFormData({ ...formData, iban: e.target.value.toUpperCase() })
                }
                placeholder="TR00 0000 0000 0000 0000 0000 00"
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
              {errors.iban && (
                <p className="text-[11px] text-rose-600 mt-0.5 font-medium">{errors.iban}</p>
              )}
            </div>

            {/* Açıklama */}
            <div className="sm:col-span-2">
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                Açıklama / Birim Notu
              </label>
              <textarea
                rows={2}
                value={formData.aciklama || ''}
                onChange={(e) => setFormData({ ...formData, aciklama: e.target.value })}
                placeholder="Örn: Destek Hizmetleri Birimi A Blok"
                className="w-full px-3 py-2 bg-slate-50 border border-slate-300 rounded-lg text-sm text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
            </div>
          </div>
          )}

            {/* Sürekli Kesinti Tanımları Section */}
          {activeFormSection === 'deductions' && (
            <div className="sm:col-span-2 bg-slate-50 p-4 rounded-xl border border-slate-200 space-y-3 mt-2">
              <div className="flex items-center justify-between border-b border-slate-200 pb-2">
                <h4 className="text-xs font-bold text-slate-800 uppercase tracking-wider flex items-center gap-1.5">
                  <Shield className="w-4 h-4 text-indigo-600" />
                  <span>Sürekli / Sabit Kesinti Tanımları (Her Ay Otomatik Kesilir)</span>
                </h4>
                <span className="text-[11px] text-slate-500 font-medium">Özlük Kesintileri</span>
              </div>

              {/* Sendika & BES Checkboxes & Custom Overrides */}
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-xs">
                {/* Sendika Box */}
                <div className="p-3 bg-white rounded-xl border border-indigo-100 shadow-2xs space-y-2">
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={formData.kesintiler?.sendikaUyesi === true}
                      onChange={(e) =>
                        setFormData({
                          ...formData,
                          kesintiler: {
                            ...(formData.kesintiler || {}),
                            sendikaUyesi: e.target.checked,
                          },
                        })
                      }
                      className="w-4 h-4 text-indigo-600 rounded focus:ring-indigo-500"
                    />
                    <div>
                      <span className="font-bold text-slate-900">Sendika Üyesi</span>
                      <p className="text-[10px] text-indigo-600 font-medium">
                        Varsayılan: Dönem parametrelerinden çekilir (1.588,13 TL)
                      </p>
                    </div>
                  </label>

                  {formData.kesintiler?.sendikaUyesi === true && (
                    <div className="pt-1 border-t border-slate-100">
                      <label className="block text-[10px] font-semibold text-slate-600 mb-1">
                        Kişiye Özel Sendika Aidatı Tutarı (TL)
                      </label>
                      <input
                        type="number"
                        step="0.01"
                        placeholder="Boş kalırsa dönem parametresini kullanır"
                        value={formData.kesintiler?.sabitSendikaAidati ?? ''}
                        onChange={(e) =>
                          setFormData({
                            ...formData,
                            kesintiler: {
                              ...(formData.kesintiler || {}),
                              sabitSendikaAidati: parseFloat(e.target.value) || 0,
                            },
                          })
                        }
                        className="w-full px-2.5 py-1.5 bg-slate-50 border border-slate-200 rounded-lg text-xs font-mono text-slate-800 focus:bg-white focus:ring-2 focus:ring-indigo-500"
                      />
                    </div>
                  )}
                </div>

                {/* OKS (Otomatik Katılım Bireysel Emeklilik) Box */}
                <div className="p-3 bg-white rounded-xl border border-emerald-100 shadow-2xs space-y-2">
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={formData.kesintiler?.besUyesi === true}
                      onChange={(e) =>
                        setFormData({
                          ...formData,
                          kesintiler: {
                            ...(formData.kesintiler || {}),
                            besUyesi: e.target.checked,
                          },
                        })
                      }
                      className="w-4 h-4 text-emerald-600 rounded focus:ring-emerald-500"
                    />
                    <div>
                      <span className="font-bold text-slate-900">OKS (Otomatik Katılım Sistemi) Tabi</span>
                      <p className="text-[10px] text-emerald-600 font-medium">
                        Otomatik: PEK × %3 (Kuruş kısmı atılır). Çıkmışsa işareti kaldırın.
                      </p>
                    </div>
                  </label>

                  {formData.kesintiler?.besUyesi === true && (
                    <div className="pt-2 border-t border-slate-100 space-y-2">
                      <div>
                        <label className="block text-[10px] font-semibold text-slate-600 mb-0.5">
                          Yüksek OKS Oranı Bildirmişse OKS Oranı (%)
                        </label>
                        <input
                          type="number"
                          step="0.5"
                          min="3"
                          max="20"
                          placeholder="Varsayılan: %3"
                          value={formData.kesintiler?.oksOraniYuzde ?? ''}
                          onChange={(e) =>
                            setFormData({
                              ...formData,
                              kesintiler: {
                                ...(formData.kesintiler || {}),
                                oksOraniYuzde: e.target.value === '' ? undefined : Number(e.target.value),
                              },
                            })
                          }
                          className="w-full px-2.5 py-1.5 bg-slate-50 border border-slate-200 rounded-lg text-xs font-mono text-slate-800 focus:bg-white focus:ring-2 focus:ring-emerald-500"
                        />
                        {errors.oksOraniYuzde && (
                          <p className="text-[10px] text-rose-600 mt-1 font-medium">{errors.oksOraniYuzde}</p>
                        )}
                      </div>

                      <div>
                        <label className="block text-[10px] font-semibold text-slate-600 mb-0.5">
                          İstisnai Maktu Sabit OKS Tutarı (TL)
                        </label>
                        <input
                          type="number"
                          step="0.01"
                          placeholder="Boş kalırsa PEK x Oran hesaplanır"
                          value={formData.kesintiler?.sabitBesTutar ?? ''}
                          onChange={(e) =>
                            setFormData({
                              ...formData,
                              kesintiler: {
                                ...(formData.kesintiler || {}),
                                sabitBesTutar: parseFloat(e.target.value) || 0,
                              },
                            })
                          }
                          className="w-full px-2.5 py-1.5 bg-slate-50 border border-slate-200 rounded-lg text-xs font-mono text-slate-800 focus:bg-white focus:ring-2 focus:ring-emerald-500"
                        />
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {/* Sabit Maktu Kesintiler Grid */}
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5 pt-1">
                <div>
                  <label className="block text-[11px] font-semibold text-rose-700 mb-0.5">
                    İcra Kesintisi (TL)
                  </label>
                  <input
                    type="number"
                    min={0}
                    placeholder="0.00"
                    value={formData.kesintiler?.icraTutar ?? ''}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          icraTutar: parseFloat(e.target.value) || 0,
                        },
                      })
                    }
                    className="w-full px-2.5 py-1.5 bg-white border border-rose-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-rose-500"
                  />
                </div>

                <div>
                  <label className="block text-[11px] font-semibold text-amber-700 mb-0.5">
                    Kişi Borcu (TL)
                  </label>
                  <input
                    type="number"
                    min={0}
                    placeholder="0.00"
                    value={formData.kesintiler?.kisiBorcuTutar ?? ''}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          kisiBorcuTutar: parseFloat(e.target.value) || 0,
                        },
                      })
                    }
                    className="w-full px-2.5 py-1.5 bg-white border border-amber-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500"
                  />
                </div>

                <div>
                  <label className="block text-[11px] font-semibold text-purple-700 mb-0.5">
                    Doğum/Askerlik (TL)
                  </label>
                  <input
                    type="number"
                    min={0}
                    placeholder="0.00"
                    value={formData.kesintiler?.dogumAskerlikBorclanmasiTutar ?? ''}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          dogumAskerlikBorclanmasiTutar: parseFloat(e.target.value) || 0,
                        },
                      })
                    }
                    className="w-full px-2.5 py-1.5 bg-white border border-purple-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-purple-500"
                  />
                </div>

                <div>
                  <label className="block text-[11px] font-semibold text-teal-700 mb-0.5">
                    Sağlık Sigortası (TL)
                  </label>
                  <input
                    type="number"
                    min={0}
                    placeholder="0.00"
                    value={formData.kesintiler?.hayatSaglikSigortasiTutar ?? ''}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          hayatSaglikSigortasiTutar: parseFloat(e.target.value) || 0,
                        },
                      })
                    }
                    className="w-full px-2.5 py-1.5 bg-white border border-teal-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-teal-500"
                  />
                </div>

                <div>
                  <label className="block text-[11px] font-semibold text-slate-700 mb-0.5">
                    Diğer Kesinti (TL)
                  </label>
                  <input
                    type="number"
                    min={0}
                    placeholder="0.00"
                    value={formData.kesintiler?.digerKesintiTutar ?? ''}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          digerKesintiTutar: parseFloat(e.target.value) || 0,
                        },
                      })
                    }
                    className="w-full px-2.5 py-1.5 bg-white border border-slate-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-slate-500"
                  />
                </div>
              </div>

            </div>
          )}

          {activeFormSection === 'advanced' && (
            <>
              <div className="sm:col-span-2 bg-violet-50/70 p-4 rounded-xl border border-violet-200 space-y-3 mt-2">
                <div>
                  <h4 className="text-xs font-bold text-violet-950 uppercase tracking-wider">GV İndirimi Uygunluk Girdileri</h4>
                  <p className="text-[11px] text-violet-800 mt-1">
                    Bu alanlar net ücret kesintilerinden bağımsızdır. Yalnız belgeye dayalı ve cari bordroda GV matrahından indirime uygun tutarları girin.
                  </p>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                  <div>
                    <label className="block text-[11px] font-semibold text-violet-900 mb-1">Doğum/Askerlik GV İndirimi (TL)</label>
                    <input
                      type="number"
                      min={0}
                      step="0.01"
                      value={formData.kesintiler?.gvIndirimleri?.dogumAskerlikGvIndirimTutar ?? ''}
                      onChange={(e) => setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          gvIndirimleri: {
                            ...(formData.kesintiler?.gvIndirimleri || {}),
                            dogumAskerlikGvIndirimTutar: e.target.value === '' ? undefined : Number(e.target.value),
                          },
                        },
                      })}
                      className="w-full px-2.5 py-1.5 bg-white border border-violet-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-violet-500"
                    />
                  </div>
                  <div>
                    <label className="block text-[11px] font-semibold text-violet-900 mb-1">Hayat Sigortası Primi (TL)</label>
                    <input
                      type="number"
                      min={0}
                      step="0.01"
                      value={formData.kesintiler?.gvIndirimleri?.hayatSigortasiPrimiTutar ?? ''}
                      onChange={(e) => setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          gvIndirimleri: {
                            ...(formData.kesintiler?.gvIndirimleri || {}),
                            hayatSigortasiPrimiTutar: e.target.value === '' ? undefined : Number(e.target.value),
                          },
                        },
                      })}
                      className="w-full px-2.5 py-1.5 bg-white border border-violet-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-violet-500"
                    />
                    <span className="text-[10px] text-violet-700 mt-1 block">GV adayı: primin %50'si.</span>
                  </div>
                  <div>
                    <label className="block text-[11px] font-semibold text-violet-900 mb-1">Sağlık/Şahıs Sigortası Primi (TL)</label>
                    <input
                      type="number"
                      min={0}
                      step="0.01"
                      value={formData.kesintiler?.gvIndirimleri?.saglikSigortasiPrimiTutar ?? ''}
                      onChange={(e) => setFormData({
                        ...formData,
                        kesintiler: {
                          ...(formData.kesintiler || {}),
                          gvIndirimleri: {
                            ...(formData.kesintiler?.gvIndirimleri || {}),
                            saglikSigortasiPrimiTutar: e.target.value === '' ? undefined : Number(e.target.value),
                          },
                        },
                      })}
                      className="w-full px-2.5 py-1.5 bg-white border border-violet-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-violet-500"
                    />
                    <span className="text-[10px] text-violet-700 mt-1 block">GV adayı: primin %100'ü; yasal aylık/yıllık limit ayrıca uygulanır.</span>
                  </div>
                </div>
              </div>

              {/* Yıl İçi Devir / Başlangıç Kümülatif Vergi Matrahı */}
              <div className="p-3 bg-amber-50/70 border border-amber-200 rounded-xl space-y-2 mt-3">
                <div className="text-xs font-bold text-amber-900 flex items-center justify-between">
                  <span>Yıl İçi Devir / Başlangıç Kümülatif GV Matrahı</span>
                  <span className="text-[10px] font-normal text-amber-700">Yıl ortası katılan veya geçmiş aydan devreden</span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
                  <label className="block text-[10px] font-semibold text-amber-800 mb-0.5">
                    Önceki Kümülatif Gelir Vergisi Matrahı (TL)
                  </label>
                  <input
                    type="number"
                    min={0}
                    step="0.01"
                    placeholder="Örn: 120000.00"
                    value={formData.devirKumulatifGvMatrahi ?? ''}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        devirKumulatifGvMatrahi: parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-2.5 py-1.5 bg-white border border-amber-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500"
                  />
                  <div>
                    <label className="block text-[10px] font-semibold text-amber-800 mb-0.5">
                      GV Devir Yılı
                    </label>
                    <input
                      type="number"
                      min={2000}
                      value={formData.devirKumulatifGvMatrahiYili ?? ''}
                      onChange={(e) => setFormData({ ...formData, devirKumulatifGvMatrahiYili: parseInt(e.target.value, 10) || undefined })}
                      className="w-full px-2.5 py-1.5 bg-white border border-amber-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500"
                    />
                  </div>
                  <div>
                    <label className="block text-[10px] font-semibold text-amber-800 mb-0.5">
                      Başlangıç Vergi Ayı
                    </label>
                    <input
                      type="number"
                      min={1}
                      max={12}
                      value={formData.devirKumulatifGvMatrahiBaslangicAyi ?? ''}
                      onChange={(e) => setFormData({ ...formData, devirKumulatifGvMatrahiBaslangicAyi: parseInt(e.target.value, 10) || undefined })}
                      className="w-full px-2.5 py-1.5 bg-white border border-amber-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500"
                    />
                  </div>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                  <div>
                    <label className="block text-[10px] font-semibold text-amber-800 mb-0.5">
                      Önceki Kümülatif Asgari Ücret GV Matrahı (TL)
                    </label>
                    <input
                      type="number"
                      min={0}
                      step="0.01"
                      value={formData.devirKumulatifAsgariGvMatrahi ?? ''}
                      onChange={(e) => setFormData({ ...formData, devirKumulatifAsgariGvMatrahi: parseFloat(e.target.value) || 0 })}
                      className="w-full px-2.5 py-1.5 bg-white border border-amber-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500"
                    />
                  </div>
                  <div>
                    <label className="block text-[10px] font-semibold text-amber-800 mb-0.5">
                      Asgari GV Devir Yılı
                    </label>
                    <input
                      type="number"
                      min={2000}
                      value={formData.devirKumulatifAsgariGvMatrahiYili ?? ''}
                      onChange={(e) => setFormData({ ...formData, devirKumulatifAsgariGvMatrahiYili: parseInt(e.target.value, 10) || undefined })}
                      className="w-full px-2.5 py-1.5 bg-white border border-amber-300 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500"
                    />
                  </div>
                </div>
              </div>
            </>
          )}

          </div>

          <div className="p-4 px-6 bg-slate-50 border-t border-slate-200 flex justify-end gap-2 shrink-0">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 bg-white border border-slate-300 hover:bg-slate-100 text-slate-700 rounded-xl text-xs font-semibold transition-colors cursor-pointer"
            >
              İptal
            </button>
            <button
              type="submit"
              disabled={isSaving}
              className="px-5 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer"
            >
              <Save className="w-4 h-4" />
              <span>{isSaving ? 'Kaydediliyor…' : 'Kaydet'}</span>
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
