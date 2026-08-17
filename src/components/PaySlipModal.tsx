import React, { useMemo, useState } from 'react';
import {
  Building2,
  FileArchive,
  FileDown,
  FileSpreadsheet,
  Printer,
  Shield,
  X,
} from 'lucide-react';
import { BordroDonemi, BordroKaydi, IsPrimiGrupItem, Personel } from '../types/payroll';
import { formatTL, getGrupIsPrimiOraniDisplay } from '../utils/payrollUtils';
import { printElement } from '../utils/excelExport';
import { tauriBridge } from '../services/tauriBridge';
import {
  buildPayrollExportModel,
  buildPeriodPayrollExportModels,
  PayrollExportModel,
} from '../exports/payrollExportModel';
import {
  exportPeriodPayrollExcel,
  exportSinglePayrollExcel,
} from '../exports/payrollExcelExport';
import {
  exportPeriodPayrollPdf,
  exportSinglePayrollPdf,
} from '../exports/payrollPdfExport';

interface PaySlipModalProps {
  isOpen: boolean;
  onClose: () => void;
  bordro: BordroKaydi;
  personel: Personel;
  donem: BordroDonemi;
  isPrimiGruplari?: IsPrimiGrupItem[];
}

type ExportAction = 'single-pdf' | 'single-xlsx' | 'period-pdf' | 'period-xlsx' | null;

export const PaySlipModal: React.FC<PaySlipModalProps> = ({
  isOpen,
  onClose,
  bordro,
  personel,
  donem,
  isPrimiGruplari,
}) => {
  const [busyAction, setBusyAction] = useState<ExportAction>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  const previewModel = useMemo(
    () =>
      buildPayrollExportModel({
        person: personel,
        payroll: bordro,
        period: donem,
      }),
    [personel, bordro, donem]
  );

  if (!isOpen) return null;

  const withBusy = async (action: Exclude<ExportAction, null>, fn: () => Promise<void> | void) => {
    setBusyAction(action);
    setExportError(null);
    try {
      await fn();
    } catch (error) {
      console.error('Payroll export failed:', error);
      setExportError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusyAction(null);
    }
  };

  const loadSingleModel = async (): Promise<PayrollExportModel> => {
    if (!tauriBridge.isTauriAvailable()) return previewModel;
    const [attendances, notices] = await Promise.all([
      tauriBridge.getAttendanceList(),
      tauriBridge.getPayrollNotices(donem.id),
    ]);
    return buildPayrollExportModel({
      person: personel,
      payroll: bordro,
      period: donem,
      attendance: attendances.find(
        (item) => item.personelId === personel.id && item.donemId === donem.id
      ),
      notices,
    });
  };

  const loadPeriodContext = async () => {
    if (!tauriBridge.isTauriAvailable()) {
      throw new Error('Dönem toplu çıktıları yalnızca Tauri masaüstü uygulamasında hazırlanabilir.');
    }
    const [people, payrolls, attendances, notices] = await Promise.all([
      tauriBridge.getPersonnelList(),
      tauriBridge.getPayrollList(),
      tauriBridge.getAttendanceList(),
      tauriBridge.getPayrollNotices(donem.id),
    ]);
    const models = buildPeriodPayrollExportModels({
      period: donem,
      people,
      payrolls,
      attendances,
      notices,
    });
    if (models.length === 0) {
      throw new Error('Bu dönem için resmi çıktıya uygun CALCULATED veya FINALIZED bordro bulunamadı.');
    }
    return { people, payrolls, notices, models };
  };

  const handlePrint = () => printElement('payslip-print-container');

  const visibleIncomeLines = previewModel.incomes.filter((line) => Math.abs(line.amount) > 0.0001);
  const visibleDeductionLines = previewModel.deductions.filter((line) => Math.abs(line.amount) > 0.0001);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/70 backdrop-blur-xs p-4 overflow-y-auto"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-2xl shadow-2xl border border-slate-200 max-w-5xl w-full max-h-[94vh] flex flex-col overflow-hidden my-auto"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="bg-slate-900 text-white px-5 py-4 flex flex-col xl:flex-row xl:items-center justify-between gap-3 no-print shrink-0">
          <div className="flex items-center gap-2.5">
            <Shield className="w-5 h-5 text-indigo-400" />
            <div>
              <h3 className="font-semibold text-sm">Ücret Pusulası / Bordro Zarfı</h3>
              <p className="text-[10px] text-slate-400 mt-0.5">
                PDF ve Excel çıktıları aynı authoritative bordro snapshot'ından üretilir.
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2 flex-wrap">
            <button
              type="button"
              disabled={busyAction !== null}
              onClick={() =>
                void withBusy('single-pdf', async () => exportSinglePayrollPdf(await loadSingleModel()))
              }
              className="px-3 py-2 bg-rose-600 hover:bg-rose-500 disabled:opacity-50 text-white rounded-xl text-xs font-bold flex items-center gap-1.5"
            >
              <FileDown className="w-4 h-4" />
              PDF İndir
            </button>
            <button
              type="button"
              disabled={busyAction !== null}
              onClick={() =>
                void withBusy('single-xlsx', async () => exportSinglePayrollExcel(await loadSingleModel()))
              }
              className="px-3 py-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-xl text-xs font-bold flex items-center gap-1.5"
            >
              <FileSpreadsheet className="w-4 h-4" />
              Excel İndir
            </button>
            <button
              type="button"
              onClick={handlePrint}
              disabled={busyAction !== null}
              className="px-3 py-2 bg-slate-700 hover:bg-slate-600 disabled:opacity-50 text-white rounded-xl text-xs font-semibold flex items-center gap-1.5"
            >
              <Printer className="w-4 h-4" />
              Yazdır
            </button>
            <div className="w-px h-7 bg-slate-700 hidden xl:block" />
            <button
              type="button"
              disabled={busyAction !== null || !tauriBridge.isTauriAvailable()}
              onClick={() =>
                void withBusy('period-xlsx', async () => {
                  const context = await loadPeriodContext();
                  exportPeriodPayrollExcel({ period: donem, ...context });
                })
              }
              className="px-3 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 text-white rounded-xl text-xs font-semibold flex items-center gap-1.5"
              title="Dönemin Bordro İcmali, Gelirler, Kesintiler, SGK-Vergi, Puantaj, Banka ve Kontrol sayfalarını tek çalışma kitabında oluşturur"
            >
              <FileArchive className="w-4 h-4" />
              Dönem Excel
            </button>
            <button
              type="button"
              disabled={busyAction !== null || !tauriBridge.isTauriAvailable()}
              onClick={() =>
                void withBusy('period-pdf', async () => {
                  const context = await loadPeriodContext();
                  await exportPeriodPayrollPdf(donem, context.models);
                })
              }
              className="px-3 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 text-white rounded-xl text-xs font-semibold flex items-center gap-1.5"
              title="CALCULATED/FINALIZED bordroları tek çok sayfalı PDF içinde oluşturur"
            >
              <FileDown className="w-4 h-4" />
              Tüm Bordrolar PDF
            </button>
            <button
              type="button"
              onClick={onClose}
              className="p-2 text-slate-400 hover:text-white rounded-lg transition-colors"
              title="Kapat"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {exportError && (
          <div className="no-print px-5 py-3 bg-rose-50 border-b border-rose-200 text-xs font-semibold text-rose-800">
            Çıktı oluşturulamadı: {exportError}
          </div>
        )}
        {busyAction && (
          <div className="no-print px-5 py-2 bg-indigo-50 border-b border-indigo-100 text-[11px] font-semibold text-indigo-800">
            Çıktı hazırlanıyor…
          </div>
        )}

        <div
          id="payslip-print-container"
          className="p-7 bg-white overflow-y-auto flex-1 text-slate-900 print:p-0"
        >
          <div className="border-b-2 border-slate-900 pb-4 text-center">
            <div className="flex items-center justify-center gap-2 text-slate-600 text-xs font-semibold uppercase tracking-wider">
              <Building2 className="w-4 h-4" />
              <span>4/D Sürekli İşçi Bordro Birimi</span>
            </div>
            <h1 className="text-xl font-black uppercase mt-1">SÜREKLİ İŞÇİ ÜCRET PUSULASI</h1>
            <div className="text-xs font-bold text-indigo-800 font-mono mt-1">
              {donem.donemAdi} · {donem.baslangicTarihi} - {donem.bitisTarihi} · Vergi {donem.taxYear}-{String(donem.taxMonth).padStart(2, '0')}
            </div>
          </div>

          <div className="grid md:grid-cols-2 gap-3 mt-5 text-xs">
            <div className="rounded-xl border border-slate-200 bg-slate-50 p-4 space-y-1.5">
              <div><strong>T.C. Kimlik No:</strong> <span className="font-mono">{personel.tcNo}</span></div>
              <div><strong>Adı Soyadı:</strong> {personel.ad} {personel.soyad}</div>
              <div><strong>SGK Sicil No:</strong> <span className="font-mono">{personel.sgkSicilNo || '—'}</span></div>
              <div>
                <strong>İş Primi Grubu:</strong> {personel.grup || personel.unvan || '1. Grup'}
                {getGrupIsPrimiOraniDisplay(personel.grup, isPrimiGruplari) !== undefined
                  ? ` (%${getGrupIsPrimiOraniDisplay(personel.grup, isPrimiGruplari)})`
                  : ''}
              </div>
            </div>
            <div className="rounded-xl border border-slate-200 bg-slate-50 p-4 space-y-1.5">
              <div><strong>Ünvan:</strong> {personel.unvan || '—'}</div>
              <div><strong>Hizmet Yılı:</strong> {personel.hizmetYili}</div>
              <div><strong>IBAN:</strong> <span className="font-mono text-[11px]">{personel.iban || '—'}</span></div>
              <div><strong>Durum:</strong> {bordro.status}</div>
            </div>
          </div>

          <section className="mt-5">
            <h2 className="text-[11px] font-black uppercase tracking-wider text-slate-600 mb-2">Puantaj Özeti</h2>
            <div className="grid grid-cols-7 gap-1.5">
              {previewModel.attendanceSummary.map((item) => (
                <div key={item.code} className="rounded-lg border border-slate-200 bg-slate-50 p-2 text-center">
                  <div className="text-[9px] text-slate-500 font-semibold">{item.label}</div>
                  <div className="text-sm font-black font-mono">{item.count}</div>
                  <div className="text-[9px] font-mono text-slate-400">{item.code}</div>
                </div>
              ))}
            </div>
          </section>

          <div className="grid md:grid-cols-2 gap-5 mt-5">
            <section>
              <h2 className="text-[11px] font-black uppercase tracking-wider text-slate-600 mb-2">Gelirler</h2>
              <div className="border border-slate-200 rounded-xl overflow-hidden">
                {visibleIncomeLines.map((line) => (
                  <div key={line.key} className="flex justify-between gap-3 px-3 py-1.5 border-b border-slate-100 text-xs last:border-b-0">
                    <span>{line.label}</span><span className="font-mono font-bold">{formatTL(line.amount)}</span>
                  </div>
                ))}
                <div className="flex justify-between px-3 py-2 bg-indigo-50 text-indigo-950 font-black text-xs">
                  <span>BRÜT GELİR TOPLAMI</span><span className="font-mono">{formatTL(previewModel.totals.gross)}</span>
                </div>
              </div>
            </section>

            <section>
              <h2 className="text-[11px] font-black uppercase tracking-wider text-slate-600 mb-2">Kesintiler</h2>
              <div className="border border-slate-200 rounded-xl overflow-hidden">
                {visibleDeductionLines.map((line) => (
                  <div key={line.key} className="flex justify-between gap-3 px-3 py-1.5 border-b border-slate-100 text-xs last:border-b-0">
                    <span>{line.label}</span><span className="font-mono font-bold">{formatTL(line.amount)}</span>
                  </div>
                ))}
                <div className="flex justify-between px-3 py-2 bg-amber-50 text-amber-950 font-black text-xs">
                  <span>KESİNTİ TOPLAMI</span><span className="font-mono">{formatTL(previewModel.totals.deductions)}</span>
                </div>
              </div>
            </section>
          </div>

          <div className="grid md:grid-cols-2 gap-5 mt-5">
            <section>
              <h2 className="text-[11px] font-black uppercase tracking-wider text-slate-600 mb-2">SGK / Vergi Denetimi</h2>
              <div className="rounded-xl border border-slate-200 bg-slate-50 p-3 space-y-1">
                {previewModel.sgkTax.filter((line) => Math.abs(line.amount) > 0.0001).map((line) => (
                  <div key={line.key} className="flex justify-between gap-3 text-[11px]">
                    <span className="text-slate-600">{line.label}</span>
                    <span className="font-mono font-bold">{formatTL(line.amount)}</span>
                  </div>
                ))}
              </div>
            </section>
            <section>
              <h2 className="text-[11px] font-black uppercase tracking-wider text-slate-600 mb-2">Kurum Maliyet Bilgisi</h2>
              <div className="rounded-xl border border-indigo-200 bg-indigo-50/60 p-3 space-y-1">
                {previewModel.employer.filter((line) => Math.abs(line.amount) > 0.0001).map((line) => (
                  <div key={line.key} className="flex justify-between gap-3 text-[11px]">
                    <span className="text-slate-600">{line.label}</span>
                    <span className="font-mono font-bold">{formatTL(line.amount)}</span>
                  </div>
                ))}
                {previewModel.employer.every((line) => Math.abs(line.amount) <= 0.0001) && (
                  <div className="text-[11px] text-slate-500">Kurum maliyet detayı bulunmuyor.</div>
                )}
              </div>
            </section>
          </div>

          <div className="mt-6 rounded-2xl bg-slate-900 text-white px-5 py-4 flex items-center justify-between">
            <div>
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-400 font-bold">Net Ödeme</div>
              <div className="text-xs text-slate-300 mt-1">Gelir toplamı - kesinti toplamı</div>
            </div>
            <div className="text-3xl font-black font-mono">{formatTL(previewModel.totals.net)}</div>
          </div>

          <div className="mt-4 text-[9px] text-slate-400 border-t border-slate-200 pt-2">
            Kaynak bordro güncelleme: {bordro.sonGuncellemeTarihi}. Bu belge {bordro.status} bordro snapshot'ından üretilmiştir.
          </div>
        </div>
      </div>
    </div>
  );
};
