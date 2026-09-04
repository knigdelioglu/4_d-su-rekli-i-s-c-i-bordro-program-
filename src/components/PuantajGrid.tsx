/**
 * Section 3: Puantaj (15-14 Period Attendance Grid)
 */

import React, { useEffect, useState } from 'react';
import {
  Calendar,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Download,
  RotateCcw,
  Sparkles,
  User,
} from 'lucide-react';
import {
  BordroDonemi,
  Personel,
  PersonelPuantaj,
  PUANTAJ_KODLARI,
  PuantajKodu,
} from '../types/payroll';
import {
  calculatePuantajOzeti,
  generateDefaultPuantajGunler,
  getPeriodDaysList,
} from '../utils/payrollPresentation';
import { exportToExcel } from '../utils/excelExport';

interface PuantajGridProps {
  aktifDonem: BordroDonemi;
  personeller: Personel[];
  puantajlar: PersonelPuantaj[];
  onSavePuantaj: (puantaj: PersonelPuantaj) => Promise<void> | void;
  onSelectPersonelForBordro?: (personelId: string) => void;
}

export const PuantajGrid: React.FC<PuantajGridProps> = ({
  aktifDonem,
  personeller,
  puantajlar,
  onSavePuantaj,
  onSelectPersonelForBordro,
}) => {
  const periodDays = getPeriodDaysList(
    aktifDonem.baslangicTarihi,
    aktifDonem.bitisTarihi
  );

  const [selectedPersonelId, setSelectedPersonelId] = useState<string>(
    personeller[0]?.id || ''
  );
  const [activeBulkCode, setActiveBulkCode] = useState<PuantajKodu>('Ç');
  const [rangeStart, setRangeStart] = useState<string>(periodDays[0]?.dateStr || '');
  const [rangeEnd, setRangeEnd] = useState<string>(periodDays.at(-1)?.dateStr || '');

  useEffect(() => {
    setRangeStart(periodDays[0]?.dateStr || '');
    setRangeEnd(periodDays.at(-1)?.dateStr || '');
  }, [aktifDonem.id, aktifDonem.baslangicTarihi, aktifDonem.bitisTarihi]);

  const activePersonel = personeller.find((p) => p.id === selectedPersonelId);

  // Find or create initial puantaj for selected employee
  const savedPuantaj = puantajlar.find(
    (p) => p.personelId === selectedPersonelId && p.donemId === aktifDonem.id
  );
  const defaultGunler = generateDefaultPuantajGunler(
    aktifDonem.baslangicTarihi,
    aktifDonem.bitisTarihi
  );
  const activePuantaj = savedPuantaj || {
    id: `${selectedPersonelId}_${aktifDonem.id}`,
    personelId: selectedPersonelId,
    donemId: aktifDonem.id,
    gunler: defaultGunler,
  };
  const isAttendanceCreated = Boolean(savedPuantaj);

  const persistPuantaj = async (updated: PersonelPuantaj) => {
    try {
      await onSavePuantaj(updated);
    } catch (err) {
      alert(`Puantaj kaydedilemedi: ${String(err)}`);
    }
  };

  const handleCellClick = async (dateStr: string) => {
    if (!savedPuantaj) return;
    const updatedGunler = {
      ...activePuantaj.gunler,
      [dateStr]: activeBulkCode,
    };

    await persistPuantaj({
      ...activePuantaj,
      gunler: updatedGunler,
    });
  };

  const handleResetDefault = async () => {
    if (savedPuantaj) {
      const confirmed = window.confirm(
        'Bu personelin mevcut puantajı kamu kurumu varsayılan düzeniyle değiştirilecek. Devam etmek istiyor musunuz?'
      );
      if (!confirmed) return;
    }
    await persistPuantaj({
      ...activePuantaj,
      gunler: defaultGunler,
    });
  };

  const handleApplyBulkCodeToAll = async () => {
    if (!savedPuantaj) return;
    const confirmed = window.confirm(
      `${periodDays.length} günün tamamı "${activeBulkCode} — ${PUANTAJ_KODLARI[activeBulkCode].tanim}" olarak değiştirilecek. Devam etmek istiyor musunuz?`
    );
    if (!confirmed) return;
    const newGunler: Record<string, PuantajKodu> = {};
    periodDays.forEach((d) => {
      newGunler[d.dateStr] = activeBulkCode;
    });
    await persistPuantaj({
      ...activePuantaj,
      gunler: newGunler,
    });
  };

  const handleApplyRange = async () => {
    if (!savedPuantaj || !rangeStart || !rangeEnd || rangeStart > rangeEnd) {
      alert('Geçerli bir başlangıç ve bitiş tarihi seçin.');
      return;
    }
    const selectedDays = periodDays.filter(
      (day) => day.dateStr >= rangeStart && day.dateStr <= rangeEnd
    );
    if (selectedDays.length === 0) {
      alert('Seçilen aralık bu dönemin içinde değil.');
      return;
    }
    const confirmed = window.confirm(
      `${selectedDays.length} gün "${activeBulkCode}" olarak değiştirilecek. Devam etmek istiyor musunuz?`
    );
    if (!confirmed) return;
    const newGunler = { ...activePuantaj.gunler };
    selectedDays.forEach((day) => {
      newGunler[day.dateStr] = activeBulkCode;
    });
    await persistPuantaj({ ...activePuantaj, gunler: newGunler });
  };

  const currentSummary = calculatePuantajOzeti(activePuantaj.gunler);

  // Export all employees Puantaj to Excel
  const handleExportExcel = () => {
    const cols = [
      { header: 'T.C. Kimlik No', key: 'tcNo', width: 16 },
      { header: 'Ad Soyad', key: 'adSoyad', width: 22 },
      { header: 'Unvan', key: 'unvan', width: 22 },
      ...periodDays.map((d) => ({
        header: `${d.dayNumber} ${d.dayNameShort}`,
        key: `d_${d.dateStr}`,
        width: 6,
      })),
      { header: 'Ç', key: 's_Ç', width: 6 },
      { header: 'T', key: 's_T', width: 6 },
      { header: 'G', key: 's_G', width: 6 },
      { header: 'İ', key: 's_İ', width: 6 },
      { header: 'GÇ', key: 's_GÇ', width: 6 },
      { header: 'GÇT', key: 's_GÇT', width: 6 },
      { header: 'R', key: 's_R', width: 6 },
    ];

    const rows = personeller.map((p) => {
      const pPuantaj = puantajlar.find(
        (pj) => pj.personelId === p.id && pj.donemId === aktifDonem.id
      );

      const sum = pPuantaj ? calculatePuantajOzeti(pPuantaj.gunler) : null;
      const rowObj: Record<string, any> = {
        tcNo: p.tcNo,
        adSoyad: `${p.ad} ${p.soyad}`,
        unvan: p.unvan,
        s_Ç: sum?.Ç ?? '',
        s_T: sum?.T ?? '',
        s_G: sum?.G ?? '',
        s_İ: sum?.İ ?? '',
        s_GÇ: sum?.GÇ ?? '',
        s_GÇT: sum?.GÇT ?? '',
        s_R: sum?.R ?? '',
      };

      periodDays.forEach((d) => {
        rowObj[`d_${d.dateStr}`] = pPuantaj?.gunler[d.dateStr] || '';
      });

      return rowObj;
    });

    exportToExcel(
      `4D_Puantaj_Cetveli_${aktifDonem.id}`,
      'Puantaj Cetveli',
      cols,
      rows
    );
  };

  return (
    <div className="space-y-4">
      {/* Puantaj Codes Legend & Excel Export */}
      <div className="bg-white rounded-2xl p-4 border border-slate-200 shadow-2xs space-y-2">
        <div className="flex items-center justify-between gap-2">
          <div className="text-xs font-semibold text-slate-700 uppercase tracking-wider text-[10px]">
            İşaretlenecek Kod
          </div>
          <button
            type="button"
            onClick={handleExportExcel}
            className="px-3 py-1.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-xl text-xs font-semibold shadow-2xs transition-colors flex items-center gap-1.5 cursor-pointer shrink-0"
            title="Tüm personellerin puantaj cetvelini Excel'e aktar"
          >
            <Download className="w-3.5 h-3.5" />
            <span>Excel'e Aktar</span>
          </button>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-7 gap-1.5 sm:gap-2 text-xs">
          {(Object.keys(PUANTAJ_KODLARI) as PuantajKodu[]).map((kod) => {
            const info = PUANTAJ_KODLARI[kod];
            return (
              <button
                type="button"
                key={kod}
                data-testid={`attendance-code-${kod}`}
                aria-pressed={activeBulkCode === kod}
                onClick={() => setActiveBulkCode(kod)}
                title={`${kod} — ${info.tanim}`}
                className={`px-2 py-1.5 rounded-xl border flex items-center gap-1.5 min-w-0 text-left transition-all cursor-pointer focus:outline-none focus:ring-2 focus:ring-indigo-400 ${info.bgRenk} ${activeBulkCode === kod ? 'ring-2 ring-indigo-500 ring-offset-1' : 'hover:brightness-95'}`}
              >
                <span className="font-bold text-xs font-mono px-1.5 py-0.5 rounded-md bg-white/90 text-slate-900 flex items-center justify-center shrink-0 shadow-2xs">
                  {kod}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="font-semibold text-[11px] sm:text-xs leading-snug truncate" title={info.tanim}>
                    {info.tanim}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </div>

      {/* Main Interactive Matrix Area */}
      <div className="bg-white rounded-2xl border border-slate-200 shadow-2xs overflow-hidden">
        {/* Personel Switcher Header */}
        <div className="bg-slate-50/90 p-3.5 border-b border-slate-200 flex flex-col lg:flex-row lg:items-center justify-between gap-3">
          <div className="flex items-center gap-2 flex-wrap">
            <div className="flex items-center gap-1.5 bg-white border border-slate-300 rounded-xl p-1 shadow-2xs">
              {/* Previous Personel Button */}
              <button
                type="button"
                onClick={() => {
                  const currentIndex = personeller.findIndex((p) => p.id === selectedPersonelId);
                  if (currentIndex > 0) {
                    setSelectedPersonelId(personeller[currentIndex - 1].id);
                  }
                }}
                disabled={personeller.findIndex((p) => p.id === selectedPersonelId) <= 0}
                title="Önceki Personel"
                className="p-1.5 text-slate-500 hover:text-indigo-600 hover:bg-slate-100 rounded-lg disabled:opacity-30 disabled:hover:bg-transparent transition-all cursor-pointer"
              >
                <ChevronLeft className="w-4 h-4" />
              </button>

              {/* Avatar Initial Badge */}
              <div className="w-7 h-7 rounded-lg bg-indigo-600 text-white font-black text-xs flex items-center justify-center shrink-0 shadow-2xs uppercase">
                {activePersonel ? activePersonel.ad.charAt(0) : 'P'}
              </div>

              {/* Select Box */}
              <div className="relative flex items-center">
                <select
                  value={selectedPersonelId}
                  onChange={(e) => setSelectedPersonelId(e.target.value)}
                  className="bg-transparent text-xs sm:text-sm font-bold text-slate-900 focus:outline-none focus:ring-0 pr-7 py-1 pl-1 cursor-pointer appearance-none max-w-[220px] sm:max-w-[320px] truncate"
                >
                  {personeller.map((p) => {
                    const grupAd = (p.grup || p.unvan || '1. Grup').replace(/\s*\(.*?\)/, '').trim();
                    return (
                      <option key={p.id} value={p.id}>
                        {p.ad} {p.soyad} ({grupAd})
                      </option>
                    );
                  })}
                </select>
                <ChevronDown className="w-4 h-4 text-slate-400 pointer-events-none absolute right-1 top-1/2 -translate-y-1/2" />
              </div>

              {/* Group Pill */}
              {activePersonel && (
                <span className="hidden sm:inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-bold bg-indigo-50 text-indigo-700 border border-indigo-100/80 mr-1">
                  {(activePersonel.grup || activePersonel.unvan || '1. Grup').replace(/\s*\(.*?\)/, '').trim()}
                </span>
              )}

              {/* Next Personel Button */}
              <button
                type="button"
                onClick={() => {
                  const currentIndex = personeller.findIndex((p) => p.id === selectedPersonelId);
                  if (currentIndex < personeller.length - 1) {
                    setSelectedPersonelId(personeller[currentIndex + 1].id);
                  }
                }}
                disabled={personeller.findIndex((p) => p.id === selectedPersonelId) >= personeller.length - 1}
                title="Sonraki Personel"
                className="p-1.5 text-slate-500 hover:text-indigo-600 hover:bg-slate-100 rounded-lg disabled:opacity-30 disabled:hover:bg-transparent transition-all cursor-pointer"
              >
                <ChevronRight className="w-4 h-4" />
              </button>
            </div>

            <span className="text-[11px] text-slate-500 font-semibold px-1">
              ({personeller.findIndex((p) => p.id === selectedPersonelId) + 1} / {personeller.length})
            </span>
          </div>

          {/* Quick Tools */}
          <div className="flex flex-col items-stretch gap-2 text-xs">
            <div className="flex items-center gap-2 flex-wrap">
            <button
              type="button"
              onClick={handleResetDefault}
              className="px-3 py-1.5 bg-white border border-slate-200 hover:bg-slate-100 text-slate-700 rounded-xl font-medium transition-colors flex items-center gap-1.5 cursor-pointer shadow-2xs"
            >
              <RotateCcw className="w-3.5 h-3.5 text-slate-400" />
              <span>{isAttendanceCreated ? 'Varsayılan Düzeni Uygula' : 'Varsayılan Puantajı Oluştur'}</span>
            </button>

            <div className="flex items-center bg-white border border-slate-200 rounded-xl p-1 gap-1 shadow-2xs">
              <span className="px-2 text-xs text-slate-500">
                Seçili: <strong className="font-mono text-slate-800">{activeBulkCode}</strong>
              </span>
              <button
                type="button"
                onClick={handleApplyBulkCodeToAll}
                disabled={!isAttendanceCreated}
                className="px-2.5 py-1 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 rounded-lg text-xs font-semibold transition-colors flex items-center gap-1 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <Sparkles className="w-3 h-3" />
                <span>Tüm Günlere Uygula</span>
              </button>
            </div>
            </div>

            <div className="flex flex-wrap items-end gap-2 rounded-xl border border-dashed border-slate-300 bg-white/70 p-2">
              <label className="flex flex-col gap-1 text-[11px] font-semibold text-slate-600">
                Başlangıç
                <input
                  type="date"
                  value={rangeStart}
                  onChange={(e) => setRangeStart(e.target.value)}
                  min={periodDays[0]?.dateStr}
                  max={periodDays.at(-1)?.dateStr}
                  disabled={!isAttendanceCreated}
                  className="rounded-lg border border-slate-200 px-2 py-1 text-xs font-normal text-slate-800 disabled:bg-slate-100 disabled:text-slate-400"
                />
              </label>
              <label className="flex flex-col gap-1 text-[11px] font-semibold text-slate-600">
                Bitiş
                <input
                  type="date"
                  value={rangeEnd}
                  onChange={(e) => setRangeEnd(e.target.value)}
                  min={periodDays[0]?.dateStr}
                  max={periodDays.at(-1)?.dateStr}
                  disabled={!isAttendanceCreated}
                  className="rounded-lg border border-slate-200 px-2 py-1 text-xs font-normal text-slate-800 disabled:bg-slate-100 disabled:text-slate-400"
                />
              </label>
              <span className="pb-1 text-[11px] text-slate-500">
                Kod: <strong className="font-mono text-slate-800">{activeBulkCode}</strong>
              </span>
              <button
                type="button"
                onClick={handleApplyRange}
                disabled={!isAttendanceCreated}
                className="mb-0.5 px-2.5 py-1.5 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 font-semibold disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Tarih Aralığına Uygula
              </button>
            </div>
          </div>
        </div>

        {!isAttendanceCreated && (
          <div
            data-testid={`attendance-empty-${selectedPersonelId}`}
            role="note"
            className="mx-4 mt-4 rounded-xl border border-amber-200 bg-amber-50 p-3 text-sm text-amber-950"
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="font-bold">Bu dönem için puantaj henüz oluşturulmadı.</p>
                <p className="mt-1 text-xs text-amber-900">
                  Önerilen kamu kurumu çalışma düzeni: Pzt–Cum <strong>Ç</strong>, Cmt–Paz <strong>T</strong>.
                </p>
              </div>
              <button
                type="button"
                data-testid={`create-default-attendance-${selectedPersonelId}`}
                onClick={handleResetDefault}
                className="rounded-lg bg-amber-600 px-3 py-2 text-xs font-bold text-white hover:bg-amber-700"
              >
                Varsayılan Puantajı Oluştur
              </button>
            </div>
          </div>
        )}

        {/* Puantaj Summary Widget for selected employee */}
        <div className="p-4 bg-gradient-to-r from-indigo-50 via-slate-50 to-blue-50 border-b border-slate-200">
          <div className="flex items-center justify-between mb-2">
            <div className="text-xs font-bold text-slate-800 uppercase tracking-wider">
              {activePersonel?.ad} {activePersonel?.soyad} — Puantaj Özeti ({aktifDonem.donemAdi})
              {!isAttendanceCreated && ' — Önerilen düzen önizlemesi'}
            </div>
            {onSelectPersonelForBordro && activePersonel && (
              <button
                onClick={() => onSelectPersonelForBordro(activePersonel.id)}
                className="text-xs font-semibold text-indigo-700 hover:underline flex items-center gap-1"
              >
                Bu Puantajla Bordro Hesapla →
              </button>
            )}
          </div>

          <div className="grid grid-cols-4 sm:grid-cols-7 gap-2">
            {(Object.keys(PUANTAJ_KODLARI) as PuantajKodu[]).map((kod) => {
              const count = currentSummary[kod] || 0;
              const info = PUANTAJ_KODLARI[kod];
              return (
                <div
                  key={kod}
                  className="bg-white p-2.5 rounded-xl border border-slate-200 shadow-2xs text-center space-y-0.5"
                >
                  <div className="text-[11px] font-semibold text-slate-500 font-mono">
                    {kod} ({info.tanim})
                  </div>
                  <div className="text-xl font-bold text-slate-900 font-mono">
                    {count} <span className="text-xs font-normal text-slate-400">Gün</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Scrollable Day Grid */}
        <div className="p-4 overflow-x-auto">
          <div className="min-w-max space-y-3">
            <div className="text-xs text-slate-500 font-medium mb-1">
              {isAttendanceCreated
                ? `Güne tıklayarak seçili ${activeBulkCode} kodunu uygulayabilirsiniz:`
                : 'Bu günler yalnızca önerilen düzenin önizlemesidir; düzenlemek için puantajı oluşturun.'}
            </div>

            <div className="grid grid-flow-col auto-cols-[68px] gap-1.5 pb-2">
              {periodDays.map((day) => {
                const code = activePuantaj.gunler[day.dateStr] || defaultGunler[day.dateStr] || 'Ç';
                const info = PUANTAJ_KODLARI[code];

                return (
                  <div
                    key={day.dateStr}
                    data-testid={`attendance-day-${day.dateStr}`}
                    onClick={isAttendanceCreated ? () => handleCellClick(day.dateStr) : undefined}
                    aria-disabled={!isAttendanceCreated}
                    tabIndex={isAttendanceCreated ? 0 : -1}
                    className={`${isAttendanceCreated ? 'cursor-pointer hover:scale-105 hover:shadow-md' : 'cursor-default opacity-75'} border rounded-2xl p-2 text-center transition-all select-none ${
                      day.isSunday
                        ? 'ring-2 ring-blue-300/60'
                        : ''
                    } ${info.bgRenk}`}
                  >
                    <div className="text-[10px] font-bold opacity-75">
                      {day.dayNameShort}
                    </div>
                    <div className="text-sm font-bold font-mono">
                      {day.dayNumber}
                    </div>
                    <div className="mt-1 font-extrabold text-sm font-mono bg-white/90 rounded-lg py-1 shadow-2xs border border-black/5">
                      {code}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      {/* Overview Table for ALL employees */}
      <div className="bg-white rounded-2xl border border-slate-200 shadow-2xs p-5 space-y-3">
        <h3 className="font-bold text-sm text-slate-900">
          Tüm Personellerin Dönem Puantaj Özet Tablosu
        </h3>

        <div className="overflow-x-auto border border-slate-200 rounded-xl">
          <table className="w-full text-left text-xs text-slate-700">
            <thead className="bg-slate-50 text-slate-900 border-b border-slate-200 font-semibold uppercase text-[10px] tracking-wider">
              <tr>
                <th className="p-3">Personel</th>
                <th className="p-3 text-center">Ç (Çalışılan)</th>
                <th className="p-3 text-center">T (Hafta Tatili)</th>
                <th className="p-3 text-center">G (Genel Tatil)</th>
                <th className="p-3 text-center">İ (Ücretli İzin)</th>
                <th className="p-3 text-center">GÇ (Gece Ç.)</th>
                <th className="p-3 text-center">GÇT (Gece T.)</th>
                <th className="p-3 text-center">R (Rapor)</th>
                <th className="p-3 text-right">İşlem</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {personeller.map((p) => {
                const pPuantaj = puantajlar.find(
                  (pj) => pj.personelId === p.id && pj.donemId === aktifDonem.id
                );
                const sum = pPuantaj ? calculatePuantajOzeti(pPuantaj.gunler) : null;
                const isSelected = p.id === selectedPersonelId;

                return (
                  <tr
                    key={p.id}
                    onClick={() => setSelectedPersonelId(p.id)}
                    className={`cursor-pointer transition-colors ${
                      isSelected ? 'bg-indigo-50/70' : 'hover:bg-slate-50'
                    }`}
                  >
                    <td className="p-3 font-semibold text-slate-900">
                      <div>{p.ad} {p.soyad}</div>
                      <div className="text-[10px] text-slate-500 font-normal">{p.unvan}</div>
                      {!pPuantaj && (
                        <div className="mt-1 text-[10px] font-semibold text-amber-700">Puantaj oluşturulmadı</div>
                      )}
                    </td>
                    <td className="p-3 text-center font-bold text-emerald-700 font-mono">{sum?.Ç ?? '—'}</td>
                    <td className="p-3 text-center font-bold text-blue-700 font-mono">{sum?.T ?? '—'}</td>
                    <td className="p-3 text-center font-bold text-purple-700 font-mono">{sum?.G ?? '—'}</td>
                    <td className="p-3 text-center font-bold text-amber-700 font-mono">{sum?.İ ?? '—'}</td>
                    <td className="p-3 text-center font-bold text-indigo-700 font-mono">{sum?.GÇ ?? '—'}</td>
                    <td className="p-3 text-center font-bold text-teal-700 font-mono">{sum?.GÇT ?? '—'}</td>
                    <td className="p-3 text-center font-bold text-rose-700 font-mono">{sum?.R ?? '—'}</td>
                    <td className="p-3 text-right">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setSelectedPersonelId(p.id);
                        }}
                        className="px-2.5 py-1 bg-white border border-slate-200 hover:bg-indigo-50 text-indigo-700 rounded-lg text-[11px] font-semibold transition-colors"
                      >
                        Puantajı Düzenle
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};