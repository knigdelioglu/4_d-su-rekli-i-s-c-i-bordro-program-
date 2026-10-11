/**
 * Section 3: Puantaj (15-14 Period Attendance Grid)
 */

import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  Calendar,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Download,
  RotateCcw,
  Sparkles,
  Users,
} from 'lucide-react';
import {
  BordroDonemi,
  Personel,
  PersonelPuantaj,
  PUANTAJ_KODLARI,
  PuantajKodu,
  SickLeaveRecord,
} from '../types/payroll';
import {
  calculatePuantajOzeti,
  generateDefaultPuantajGunler,
  getPeriodDaysList,
} from '../utils/payrollPresentation';
import { exportToExcel } from '../utils/excelExport';
import { formatPayrollError } from './useBordroCalculationController';
import { describeError } from '../utils/errorMessage';
import { createPuantajBulkPlan } from '../utils/puantajBulkPlan';

type BulkApplyRequest =
  | { scope: 'all' | 'range'; dates: string[]; code: PuantajKodu }
  | { scope: 'personnel'; dates: string[]; code: PuantajKodu };

interface PuantajGridProps {
  aktifDonem: BordroDonemi;
  personeller: Personel[];
  puantajlar: PersonelPuantaj[];
  sickLeaveRecords: SickLeaveRecord[];
  onSavePuantaj: (puantaj: PersonelPuantaj) => Promise<void> | void;
  onSavePuantajBatch: (puantajlar: PersonelPuantaj[]) => Promise<void> | void;
  onSelectPersonelForBordro?: (personelId: string) => void;
}

export const PuantajGrid: React.FC<PuantajGridProps> = ({
  aktifDonem,
  personeller,
  puantajlar,
  sickLeaveRecords,
  onSavePuantaj,
  onSavePuantajBatch,
  onSelectPersonelForBordro,
}) => {
  const periodDays = getPeriodDaysList(
    aktifDonem.baslangicTarihi,
    aktifDonem.bitisTarihi
  );
  const leadingWeekdaySlots = periodDays.length > 0
    ? (new Date(`${periodDays[0].dateStr}T00:00:00Z`).getUTCDay() + 6) % 7
    : 0;

  const [selectedPersonelId, setSelectedPersonelId] = useState<string>(
    personeller[0]?.id || ''
  );
  const [activeBulkCode, setActiveBulkCode] = useState<PuantajKodu>('Ç');
  const [rangeStart, setRangeStart] = useState<string>(periodDays[0]?.dateStr || '');
  const [rangeEnd, setRangeEnd] = useState<string>(periodDays.at(-1)?.dateStr || '');
  const [isExporting, setIsExporting] = useState<boolean>(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [puantajError, setPuantajError] = useState<string | null>(null);
  const [bulkApplyRequest, setBulkApplyRequest] = useState<BulkApplyRequest | null>(null);
  const [isApplyingBulkCode, setIsApplyingBulkCode] = useState(false);
  const [quickEntryDates, setQuickEntryDates] = useState<string[]>([]);
  const [isQuickCalendarOpen, setIsQuickCalendarOpen] = useState(false);
  const quickCalendarRef = useRef<HTMLElement>(null);

  useEffect(() => {
    setRangeStart(periodDays[0]?.dateStr || '');
    setRangeEnd(periodDays.at(-1)?.dateStr || '');
    setQuickEntryDates([]);
    setBulkApplyRequest(null);
  }, [aktifDonem.id, aktifDonem.baslangicTarihi, aktifDonem.bitisTarihi]);

  useEffect(() => {
    if (!isQuickCalendarOpen) return;
    const handlePointerDown = (event: PointerEvent) => {
      if (!quickCalendarRef.current?.contains(event.target as Node)) {
        setIsQuickCalendarOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setIsQuickCalendarOpen(false);
    };
    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isQuickCalendarOpen]);

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

  const bulkPlan = useMemo(() => {
    if (!bulkApplyRequest) return null;
    const selectedPersonnel = bulkApplyRequest.scope === 'personnel'
      ? personeller
      : activePersonel ? [activePersonel] : [];
    return createPuantajBulkPlan({
      personnel: selectedPersonnel,
      attendances: puantajlar,
      sickLeaveRecords,
      periodId: aktifDonem.id,
      periodStartInclusive: aktifDonem.baslangicTarihi,
      periodEndInclusive: aktifDonem.bitisTarihi,
      dates: bulkApplyRequest.dates,
      code: bulkApplyRequest.code,
    });
  }, [
    activePersonel,
    aktifDonem.baslangicTarihi,
    aktifDonem.bitisTarihi,
    aktifDonem.id,
    bulkApplyRequest,
    personeller,
    puantajlar,
    sickLeaveRecords,
  ]);

  const persistPuantaj = async (updated: PersonelPuantaj) => {
    setPuantajError(null);
    try {
      await onSavePuantaj(updated);
      return true;
    } catch (err) {
      setPuantajError(`Puantaj kaydedilemedi: ${formatPayrollError(err)}`);
      return false;
    }
  };

  const persistPuantajBatch = async (updated: PersonelPuantaj[]) => {
    setPuantajError(null);
    try {
      await onSavePuantajBatch(updated);
      return true;
    } catch (err) {
      setPuantajError(`Toplu puantaj kaydedilemedi: ${formatPayrollError(err)}`);
      return false;
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
    const resetDays = { ...defaultGunler };
    for (const day of periodDays) {
      const existingCode = activePuantaj.gunler[day.dateStr];
      const hasReport = sickLeaveRecords.some((record) =>
        record.personnelId === selectedPersonelId && record.startDate <= day.dateStr && record.endDate >= day.dateStr
      );
      if (hasReport && existingCode !== 'R') {
        setPuantajError(`${day.dateStr} tarihinde hastalık raporu var ancak puantaj kodu ${existingCode ?? 'yok'}. Varsayılan puantaj uygulanmadı; önce rapor ve gün kodunu eşleştirin.`);
        return;
      }
      if (existingCode === 'R') resetDays[day.dateStr] = 'R';
    }
    await persistPuantaj({
      ...activePuantaj,
      gunler: resetDays,
    });
  };

  const handleApplyBulkCodeToAll = () => {
    if (!savedPuantaj) return;
    setPuantajError(null);
    setBulkApplyRequest({
      scope: 'all',
      dates: periodDays.map((day) => day.dateStr),
      code: activeBulkCode,
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
    setPuantajError(null);
    setBulkApplyRequest({
      scope: 'range',
      dates: selectedDays.map((day) => day.dateStr),
      code: activeBulkCode,
    });
  };

  const handleApplyQuickEntryToPersonnel = () => {
    if (personeller.length === 0 || quickEntryDates.length === 0) return;
    setPuantajError(null);
    setBulkApplyRequest({
      scope: 'personnel',
      dates: periodDays
        .filter((day) => quickEntryDates.includes(day.dateStr))
        .map((day) => day.dateStr),
      code: activeBulkCode,
    });
    setIsQuickCalendarOpen(false);
  };

  const handleConfirmBulkApply = async () => {
    if (
      !bulkApplyRequest ||
      (bulkApplyRequest.scope !== 'personnel' && !savedPuantaj) ||
      isApplyingBulkCode
    ) return;
    if (!bulkPlan) return;
    if (bulkPlan.reportMismatches.length > 0) {
      setPuantajError(
        `Toplu kayıt durduruldu: ${bulkPlan.reportMismatches.length} hastalık raporu günü R koduyla eşleşmiyor. Önce rapor ve puantaj kaydını düzeltin.`
      );
      return;
    }
    if (bulkPlan.updates.length === 0) {
      setPuantajError('Kaydedilecek gün değişikliği yok. Raporlu günler korundu; seçilen diğer günler zaten bu kodda.');
      return;
    }

    setIsApplyingBulkCode(true);
    try {
      const saved = await persistPuantajBatch(bulkPlan.updates);
      if (!saved) return;
      if (bulkApplyRequest.scope === 'personnel') setQuickEntryDates([]);
      setBulkApplyRequest(null);
      setPuantajError(null);
    } finally {
      setIsApplyingBulkCode(false);
    }
  };

  const currentSummary = calculatePuantajOzeti(activePuantaj.gunler);

  // Export all employees Puantaj to Excel
  const handleExportExcel = async () => {
    if (isExporting) return;
    setIsExporting(true);
    setExportError(null);
    try {
      // Yield to let React re-render the loading state first
      await new Promise((resolve) => setTimeout(resolve, 50));

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

      const puantajByPersonelId = new Map<string, PersonelPuantaj>(
        puantajlar
          .filter((pj) => pj.donemId === aktifDonem.id)
          .map((pj) => [pj.personelId, pj])
      );
      const rows = personeller.map((p) => {
        const pPuantaj = puantajByPersonelId.get(p.id);

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

      await exportToExcel(
        `4D_Puantaj_Cetveli_${aktifDonem.id}`,
        'Puantaj Cetveli',
        cols,
        rows
      );
    } catch (err) {
      const msg = `Excel aktarımı başarısız oldu: ${describeError(err)}`;
      console.error(msg, err);
      setExportError(msg);
      alert(msg);
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Puantaj Codes Legend & Excel Export */}
      <div className="bg-white rounded-2xl p-4 border border-slate-200 shadow-2xs space-y-2">
        {puantajError && (
          <div
            role="alert"
            className="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-semibold text-rose-800"
          >
            {puantajError}
          </div>
        )}
        {exportError && (
          <div className="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-semibold text-rose-800 flex items-center justify-between">
            <span>{exportError}</span>
            <button
              type="button"
              onClick={() => setExportError(null)}
              className="text-rose-600 hover:text-rose-900 ml-2"
            >
              ✕
            </button>
          </div>
        )}
        <div className="flex items-center justify-between gap-2">
          <div className="text-xs font-semibold text-slate-700 uppercase tracking-wider text-[10px]">
            İşaretlenecek Kod
          </div>
          <button
            type="button"
            onClick={() => void handleExportExcel()}
            disabled={isExporting}
            className="px-3 py-1.5 bg-emerald-600 hover:bg-emerald-700 disabled:opacity-50 text-white rounded-xl text-xs font-semibold shadow-2xs transition-colors flex items-center gap-1.5 cursor-pointer shrink-0"
            title="Tüm personellerin puantaj cetvelini Excel'e aktar"
          >
            <Download className={`w-3.5 h-3.5 ${isExporting ? 'animate-bounce' : ''}`} />
            <span>{isExporting ? 'Aktarılıyor…' : "Excel'e Aktar"}</span>
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

      <section
        data-testid="quick-attendance-entry"
        ref={quickCalendarRef}
        className="relative rounded-2xl border border-indigo-200 bg-white p-4 shadow-2xs"
        aria-labelledby="quick-attendance-entry-title"
      >
        <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <h3
              id="quick-attendance-entry-title"
              className="flex items-center gap-2 text-sm font-bold text-slate-900"
            >
              <Calendar className="h-4 w-4 text-indigo-600" aria-hidden="true" />
              Tüm Personele Hızlı Puantaj Girişi
              <span className="font-medium text-slate-500">({aktifDonem.donemAdi})</span>
            </h3>
            <p className="mt-1 text-xs leading-relaxed text-slate-600">
              Bir puantaj kodu ve tarihleri seçin; kayıt öncesinde tüm personel için onayınız istenir.
            </p>
          </div>
          <div className="flex items-center gap-3 text-xs text-slate-600">
            <span className="inline-flex items-center gap-1.5">
              <Users className="h-3.5 w-3.5" aria-hidden="true" />
              {personeller.length} personel
            </span>
            <button
              type="button"
              data-testid="open-quick-attendance-calendar"
              aria-expanded={isQuickCalendarOpen}
              aria-controls="quick-attendance-calendar"
              onClick={() => setIsQuickCalendarOpen((open) => !open)}
              disabled={personeller.length === 0 || isApplyingBulkCode}
              className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-3 py-2 text-xs font-semibold text-white transition-colors hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Calendar className="h-3.5 w-3.5" aria-hidden="true" />
              Hızlı Puantaj Girişi
              {quickEntryDates.length > 0 && (
                <span className="rounded-full bg-white/20 px-1.5 py-0.5 text-[10px]">
                  {quickEntryDates.length}
                </span>
              )}
              <ChevronDown className={`h-3.5 w-3.5 transition-transform ${isQuickCalendarOpen ? 'rotate-180' : ''}`} aria-hidden="true" />
            </button>
          </div>
        </div>

        {isQuickCalendarOpen && (
          <div
            id="quick-attendance-calendar"
            data-testid="quick-attendance-calendar"
            role="dialog"
            aria-label={`${aktifDonem.donemAdi} dönemi hızlı puantaj takvimi`}
            className="absolute right-0 top-full z-40 mt-2 w-[min(42rem,calc(100vw-2rem))] rounded-2xl border border-slate-200 bg-white p-4 shadow-2xl"
          >
            <div className="flex items-start justify-between gap-3">
              <div>
                <h4 className="text-sm font-bold text-slate-900">Tarihleri seçin</h4>
                <p className="mt-1 text-xs text-slate-600">
                  Seçili kod: <strong className="font-mono">{activeBulkCode} — {PUANTAJ_KODLARI[activeBulkCode].tanim}</strong>
                </p>
              </div>
              <button
                type="button"
                onClick={() => setQuickEntryDates([])}
                disabled={quickEntryDates.length === 0}
                className="shrink-0 text-xs font-semibold text-indigo-700 hover:text-indigo-900 disabled:cursor-not-allowed disabled:opacity-40"
              >
                Seçimi Temizle
              </button>
            </div>

            <div
              role="group"
              aria-label="Tüm personele uygulanacak puantaj tarihleri"
              className="mt-3 grid grid-cols-7 gap-1.5 sm:gap-2"
            >
              {['Pzt', 'Sal', 'Çar', 'Per', 'Cum', 'Cmt', 'Paz'].map((weekday) => (
                <div key={weekday} className="pb-1 text-center text-[10px] font-bold text-slate-500">
                  {weekday}
                </div>
              ))}
              {Array.from({ length: leadingWeekdaySlots }, (_, index) => (
                <div key={`quick-empty-${index}`} aria-hidden="true" />
              ))}
              {periodDays.map((day) => {
                const isSelected = quickEntryDates.includes(day.dateStr);
                return (
                  <button
                    key={day.dateStr}
                    type="button"
                    data-testid={`quick-attendance-day-${day.dateStr}`}
                    aria-pressed={isSelected}
                    aria-label={`${day.dateStr}, ${day.dayNameShort}${isSelected ? ', seçili' : ''}`}
                    onClick={() =>
                      setQuickEntryDates((current) =>
                        isSelected
                          ? current.filter((dateStr) => dateStr !== day.dateStr)
                          : [...current, day.dateStr]
                      )
                    }
                    disabled={isApplyingBulkCode}
                    className={`relative flex min-h-12 flex-col items-center justify-center rounded-xl border px-1 py-1 text-xs transition-colors focus:outline-none focus:ring-2 focus:ring-indigo-500 disabled:cursor-wait disabled:opacity-60 ${
                      isSelected
                        ? `${PUANTAJ_KODLARI[activeBulkCode].bgRenk} ring-2 ring-indigo-500 ring-offset-1`
                        : day.isWeekend
                          ? 'border-slate-200 bg-slate-50 text-slate-600 hover:border-indigo-300 hover:bg-indigo-50'
                          : 'border-slate-200 bg-white text-slate-700 hover:border-indigo-300 hover:bg-indigo-50'
                    }`}
                  >
                    <span className="text-[10px] font-semibold opacity-75">{day.dayNameShort}</span>
                    <span className="font-bold font-mono">{day.dayNumber}</span>
                    {isSelected && (
                      <CheckCircle2 className="absolute right-1 top-1 h-3 w-3" aria-hidden="true" />
                    )}
                  </button>
                );
              })}
            </div>

            <div className="mt-3 flex flex-col gap-2 border-t border-slate-100 pt-3 sm:flex-row sm:items-center sm:justify-between">
              <p className="text-xs text-slate-600" aria-live="polite">
                {quickEntryDates.length} gün seçildi
              </p>
              <button
                type="button"
                data-testid="apply-quick-attendance-to-all"
                onClick={handleApplyQuickEntryToPersonnel}
                disabled={
                  personeller.length === 0 || quickEntryDates.length === 0 || isApplyingBulkCode
                }
                className="inline-flex items-center justify-center gap-2 rounded-xl bg-indigo-600 px-3 py-2 text-xs font-semibold text-white transition-colors hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Users className="h-3.5 w-3.5" aria-hidden="true" />
                Seçili Tarihleri Tüm Personele Uygula
              </button>
            </div>
          </div>
        )}
      </section>

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

        {/* Week-Row Day Grid */}
        <div className="p-4">
          <div className="space-y-3">
            <div className="text-xs text-slate-500 font-medium mb-1">
              {isAttendanceCreated
                ? `Güne tıklayarak seçili ${activeBulkCode} kodunu uygulayabilirsiniz:`
                : 'Bu günler yalnızca önerilen düzenin önizlemesidir; düzenlemek için puantajı oluşturun.'}
            </div>

            <div className="grid grid-cols-7 gap-1.5 pb-2">
              {Array.from({ length: leadingWeekdaySlots }, (_, index) => (
                <div key={`empty-weekday-${index}`} aria-hidden="true" />
              ))}
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

      {bulkApplyRequest && (
        <div
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="bulk-apply-confirm-title"
          aria-describedby="bulk-apply-confirm-description"
          data-testid="bulk-apply-confirmation"
          className="fixed inset-0 z-[60] flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4"
          onClick={(event) => {
            if (event.target === event.currentTarget && !isApplyingBulkCode) {
              setBulkApplyRequest(null);
            }
          }}
          onKeyDown={(event: React.KeyboardEvent<HTMLDivElement>) => {
            if (event.key === 'Escape') {
              event.preventDefault();
              if (!isApplyingBulkCode) setBulkApplyRequest(null);
              return;
            }
            if (event.key === 'Tab') {
              const buttons = event.currentTarget.querySelectorAll<HTMLButtonElement>(
                'button:not([disabled])'
              );
              const firstButton = buttons[0];
              const lastButton = buttons[buttons.length - 1];
              if (event.shiftKey && document.activeElement === firstButton) {
                event.preventDefault();
                lastButton?.focus();
              } else if (!event.shiftKey && document.activeElement === lastButton) {
                event.preventDefault();
                firstButton?.focus();
              }
            }
          }}
        >
          <div className="w-full max-w-md overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xl">
            <div className="flex items-start gap-3 border-b border-amber-100 bg-amber-50/70 p-5">
              <div className="mt-0.5 shrink-0 rounded-xl bg-amber-100 p-2 text-amber-700">
                <AlertTriangle className="h-5 w-5" aria-hidden="true" />
              </div>
              <div>
                <h3 id="bulk-apply-confirm-title" className="text-sm font-bold text-slate-900">
                  Puantaj kodları değiştirilsin mi?
                </h3>
                <p id="bulk-apply-confirm-description" className="mt-2 text-sm leading-relaxed text-slate-700">
                  {bulkApplyRequest.scope === 'personnel'
                    ? `${personeller.length} personelin seçili tarihleri incelendi.`
                    : `${activePersonel?.ad ?? ''} ${activePersonel?.soyad ?? ''} için ${bulkApplyRequest.dates.length} tarih incelendi.`}{' '}
                  {bulkPlan?.rows.filter((row) => row.status === 'APPLY').length ?? 0} gün değiştirilecek ve{' '}
                  {bulkPlan?.updates.length ?? 0} personel kaydı tek işlemde kaydedilecek. Hedef kod:{' '}
                  <strong>{bulkApplyRequest.code} — {PUANTAJ_KODLARI[bulkApplyRequest.code].tanim}</strong>.
                </p>
                {bulkPlan && bulkPlan.preservedRows.length > 0 && (
                  <p className="mt-2 rounded-lg bg-white/80 px-3 py-2 text-xs font-semibold text-slate-700">
                    {bulkPlan.preservedRows.length} mevcut R günü korundu ve toplu kayda alınmadı.
                  </p>
                )}
                {bulkPlan && bulkPlan.reportMismatches.length > 0 && (
                  <p role="alert" className="mt-2 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-semibold text-rose-800">
                    {bulkPlan.reportMismatches.length} hastalık raporu günü R koduyla eşleşmiyor. Hiçbir personel kaydedilmez; önce bu kayıtları düzeltin.
                  </p>
                )}
                {puantajError && (
                  <p role="alert" className="mt-2 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-semibold text-rose-800">
                    {puantajError}
                  </p>
                )}
              </div>
            </div>
            <div className="max-h-64 overflow-auto border-b border-slate-200">
              {bulkPlan?.rows.length ? (
                <table className="w-full text-left text-xs" aria-label="Toplu puantaj değişiklik önizlemesi">
                  <thead className="sticky top-0 bg-slate-100 text-slate-600">
                    <tr>
                      <th scope="col" className="p-2">Personel</th>
                      <th scope="col" className="p-2">Tarih</th>
                      <th scope="col" className="p-2">Eski → yeni</th>
                      <th scope="col" className="p-2">İşlem</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100">
                    {bulkPlan.rows.map((row) => (
                      <tr
                        key={`${row.personnelId}:${row.date}`}
                        data-testid={`bulk-apply-preview-row-${row.personnelId}-${row.date}`}
                        className={row.status === 'REPORT_MISMATCH' ? 'bg-rose-50' : row.status === 'PRESERVED_R' ? 'bg-amber-50/70' : ''}
                      >
                        <th scope="row" className="p-2 font-semibold text-slate-800">{row.personnelName}</th>
                        <td className="whitespace-nowrap p-2 text-slate-700">{row.date.split('-').reverse().join('.')}</td>
                        <td className="whitespace-nowrap p-2 font-mono font-bold text-slate-800">{row.previousCode} → {row.nextCode}</td>
                        <td className="p-2 text-slate-700">
                          {row.status === 'APPLY' ? 'Uygulanacak' : row.status === 'PRESERVED_R' ? 'R korundu' : 'Rapor / kod uyuşmuyor'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : (
                <p className="p-4 text-sm text-slate-600">Seçilen kodda değişecek gün yok.</p>
              )}
            </div>
            <div className="flex justify-end gap-2 bg-slate-50 p-4">
              <button
                type="button"
                autoFocus
                onClick={() => setBulkApplyRequest(null)}
                disabled={isApplyingBulkCode}
                className="rounded-xl px-4 py-2 text-xs font-semibold text-slate-700 transition-colors hover:bg-slate-200/70 disabled:opacity-50"
              >
                Vazgeç
              </button>
              <button
                type="button"
                data-testid="bulk-apply-confirm"
                onClick={() => void handleConfirmBulkApply()}
                disabled={isApplyingBulkCode || !bulkPlan || bulkPlan.reportMismatches.length > 0 || bulkPlan.updates.length === 0}
                className="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700 disabled:cursor-wait disabled:opacity-60"
              >
                {isApplyingBulkCode ? 'Kaydediliyor…' : 'Önizlenen değişiklikleri kaydet'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
