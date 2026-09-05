import React, { useEffect, useMemo, useState } from 'react';
import { AlertTriangle, CheckCircle2, Download, ShieldCheck } from 'lucide-react';
import type { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import { formatTL } from '../../utils/payrollPresentation';
import { exportToExcel } from '../../utils/excelExport';
import {
  compareSgkPrimTotals,
  getSgkPrimKontroluRows,
  getSgkPrimKontroluTotals,
  kurusToAmount,
  type SgkPrimKontroluRow,
} from './sgkPrimKontroluData';

interface SgkPrimKontroluProps {
  aktifDonem: BordroDonemi;
  personeller: Personel[];
  bordrolar: BordroKaydi[];
}

const periodLabel = (period: BordroDonemi): string =>
  period.donemAdi.match(/\(([^)]+)\)/)?.[1] || period.donemAdi;

function missingStatusLabel(row: SgkPrimKontroluRow): string {
  return row.status === 'stale' ? 'Yeniden hesaplanmalı' : 'Bordro hesaplanmadı';
}

function displayAmount(row: SgkPrimKontroluRow, amount: number): string {
  return row.status === 'authoritative' ? formatTL(amount) : '—';
}

export const SgkPrimKontrolu: React.FC<SgkPrimKontroluProps> = ({
  aktifDonem,
  personeller,
  bordrolar,
}) => {
  const [sgkTutarInput, setSgkTutarInput] = useState('');

  useEffect(() => {
    setSgkTutarInput('');
  }, [aktifDonem.id]);

  const rows = useMemo(
    () => getSgkPrimKontroluRows(aktifDonem, personeller, bordrolar),
    [aktifDonem, personeller, bordrolar]
  );
  const totals = useMemo(() => getSgkPrimKontroluTotals(rows), [rows]);
  const comparison = useMemo(
    () => compareSgkPrimTotals(totals.genelPrimToplami, sgkTutarInput),
    [sgkTutarInput, totals.genelPrimToplami]
  );

  const handleExportExcel = () => {
    const columns = [
      { header: 'S.No', key: 'siraNo', width: 8 },
      { header: 'T.C. Kimlik No', key: 'tcNo', width: 18 },
      { header: 'SGK Sicil No', key: 'sgkSicilNo', width: 18 },
      { header: 'Ad Soyad', key: 'adSoyad', width: 28 },
      { header: 'SGK İşveren %21,75', key: 'isverenSgkPrimi', width: 20 },
      { header: 'İşveren İşsizlik %2', key: 'isverenIssizlikPrimi', width: 20 },
      { header: 'SGK İşçi %14', key: 'isciSgkPrimi', width: 18 },
      { header: 'İşçi İşsizlik %1', key: 'isciIssizlikPrimi', width: 18 },
      { header: 'Toplam', key: 'toplam', width: 18 },
    ];

    const data = rows.map((row, index) => ({
      siraNo: index + 1,
      tcNo: row.personel.tcNo,
      sgkSicilNo: row.personel.sgkSicilNo,
      adSoyad: `${row.personel.ad} ${row.personel.soyad}`,
      isverenSgkPrimi: row.status === 'authoritative' ? row.isverenSgkPrimi : '',
      isverenIssizlikPrimi: row.status === 'authoritative' ? row.isverenIssizlikPrimi : '',
      isciSgkPrimi: row.status === 'authoritative' ? row.isciSgkPrimi : '',
      isciIssizlikPrimi: row.status === 'authoritative' ? row.isciIssizlikPrimi : '',
      toplam: row.status === 'authoritative' ? row.toplam : '',
    }));

    const summaryRows = [
      {
        siraNo: 'TOPLAM',
        tcNo: '',
        sgkSicilNo: '',
        adSoyad: 'Dört prim kolonu toplamı',
        isverenSgkPrimi: totals.isverenSgkPrimi,
        isverenIssizlikPrimi: totals.isverenIssizlikPrimi,
        isciSgkPrimi: totals.isciSgkPrimi,
        isciIssizlikPrimi: totals.isciIssizlikPrimi,
        toplam: totals.genelPrimToplami,
      },
      {
        siraNo: '',
        tcNo: '',
        sgkSicilNo: '',
        adSoyad: 'GENEL PRİM TOPLAMI',
        isverenSgkPrimi: '',
        isverenIssizlikPrimi: '',
        isciSgkPrimi: '',
        isciIssizlikPrimi: '',
        toplam: totals.genelPrimToplami,
      },
    ];

    if (comparison.sgkTutarKurus !== null) {
      summaryRows.push({
        siraNo: '',
        tcNo: '',
        sgkSicilNo: '',
        adSoyad: "SGK'dan Girilen Tutar",
        isverenSgkPrimi: '',
        isverenIssizlikPrimi: '',
        isciSgkPrimi: '',
        isciIssizlikPrimi: '',
        toplam: kurusToAmount(comparison.sgkTutarKurus),
      });
    }

    if (comparison.farkKurus !== null) {
      summaryRows.push({
        siraNo: '',
        tcNo: '',
        sgkSicilNo: '',
        adSoyad: 'FARK',
        isverenSgkPrimi: '',
        isverenIssizlikPrimi: '',
        isciSgkPrimi: '',
        isciIssizlikPrimi: '',
        toplam: kurusToAmount(comparison.farkKurus),
      });
    }

    exportToExcel(
      `SGK_Prim_Kontrolu_${aktifDonem.id}`,
      'SGK Prim Kontrolü',
      columns,
      data,
      summaryRows
    );
  };

  const comparisonCardClass =
    comparison.status === 'compatible'
      ? 'border-emerald-200 bg-emerald-50'
      : comparison.status === 'empty' || comparison.status === 'invalid'
        ? 'border-slate-200 bg-slate-50'
        : 'border-rose-200 bg-rose-50';
  const comparisonTextClass =
    comparison.status === 'compatible'
      ? 'text-emerald-800'
      : comparison.status === 'empty' || comparison.status === 'invalid'
        ? 'text-slate-600'
        : 'text-rose-800';

  return (
    <div className="space-y-6" data-testid="sgk-prim-kontrolu-screen">
      <div className="flex flex-col justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-2xs md:flex-row md:items-center">
        <div>
          <div className="flex items-center gap-2">
            <ShieldCheck className="h-5 w-5 text-indigo-600" aria-hidden="true" />
            <h2 className="text-lg font-bold text-slate-900">
              SGK Prim Kontrolü — {periodLabel(aktifDonem)}
            </h2>
          </div>
          <p className="mt-1 text-xs text-slate-500">
            Bordroda hesaplanan SGK ve işsizlik sigortası primlerinin SGK bildirimi ile mutabakat kontrolü.
          </p>
        </div>

        <button
          type="button"
          onClick={handleExportExcel}
          className="inline-flex items-center justify-center gap-1.5 rounded-xl bg-emerald-600 px-4 py-2 text-xs font-bold text-white shadow-xs transition-colors hover:bg-emerald-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500 focus-visible:ring-offset-2"
        >
          <Download className="h-4 w-4" aria-hidden="true" />
          <span>Excel'e Aktar</span>
        </button>
      </div>

      <div className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xs">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[1180px] border-collapse text-left text-xs">
            <thead>
              <tr className="border-b border-slate-200 bg-slate-50 text-[10px] font-bold uppercase tracking-wider text-slate-900">
                <th scope="col" className="p-3">S.No</th>
                <th scope="col" className="p-3">T.C. Kimlik No</th>
                <th scope="col" className="p-3">SGK Sicil No</th>
                <th scope="col" className="p-3">Ad Soyad</th>
                <th scope="col" className="p-3 text-right">SGK İşveren %21,75</th>
                <th scope="col" className="p-3 text-right">İşveren İşsizlik %2</th>
                <th scope="col" className="p-3 text-right">SGK İşçi %14</th>
                <th scope="col" className="p-3 text-right">İşçi İşsizlik %1</th>
                <th scope="col" className="p-3 text-right">Toplam</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {rows.map((row, index) => (
                <tr key={row.personel.id} className="hover:bg-slate-50/80">
                  <td className="p-3 font-sans text-slate-400">{index + 1}</td>
                  <td className="p-3 font-mono font-bold text-slate-900">{row.personel.tcNo}</td>
                  <td className="p-3 font-mono text-slate-800">{row.personel.sgkSicilNo || '—'}</td>
                  <td className="p-3 font-sans font-semibold text-slate-900">
                    <div>{row.personel.ad} {row.personel.soyad}</div>
                    {row.status !== 'authoritative' && (
                      <span className="mt-1 inline-flex items-center gap-1 rounded-md border border-amber-200 bg-amber-50 px-1.5 py-0.5 text-[10px] font-semibold text-amber-800">
                        <AlertTriangle className="h-3 w-3" aria-hidden="true" />
                        {missingStatusLabel(row)}
                      </span>
                    )}
                  </td>
                  <td className="p-3 text-right font-mono font-semibold tabular-nums text-slate-900">{displayAmount(row, row.isverenSgkPrimi)}</td>
                  <td className="p-3 text-right font-mono font-semibold tabular-nums text-slate-900">{displayAmount(row, row.isverenIssizlikPrimi)}</td>
                  <td className="p-3 text-right font-mono font-semibold tabular-nums text-slate-900">{displayAmount(row, row.isciSgkPrimi)}</td>
                  <td className="p-3 text-right font-mono font-semibold tabular-nums text-slate-900">{displayAmount(row, row.isciIssizlikPrimi)}</td>
                  <td className="p-3 text-right font-mono text-sm font-bold tabular-nums text-indigo-900">{displayAmount(row, row.toplam)}</td>
                </tr>
              ))}
              {rows.length === 0 && (
                <tr>
                  <td colSpan={9} className="p-8 text-center font-sans italic text-slate-500">
                    Bu dönem için kayıtlı personel bulunmamaktadır.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-5" aria-label="SGK prim toplamları">
        {[
          ['SGK İşveren %21,75 Toplamı', totals.isverenSgkPrimi],
          ['İşveren İşsizlik %2 Toplamı', totals.isverenIssizlikPrimi],
          ['SGK İşçi %14 Toplamı', totals.isciSgkPrimi],
          ['İşçi İşsizlik %1 Toplamı', totals.isciIssizlikPrimi],
        ].map(([label, value]) => (
          <div key={label as string} className="rounded-xl border border-slate-200 bg-white p-4 shadow-2xs">
            <div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">{label}</div>
            <div className="mt-2 font-mono text-lg font-bold tabular-nums text-slate-900">{formatTL(value as number)}</div>
          </div>
        ))}
        <div className="rounded-xl border border-indigo-200 bg-indigo-50 p-4 shadow-2xs sm:col-span-2 xl:col-span-1">
          <div className="text-[10px] font-bold uppercase tracking-wide text-indigo-700">GENEL PRİM TOPLAMI</div>
          <div className="mt-2 font-mono text-xl font-black tabular-nums text-indigo-950">{formatTL(totals.genelPrimToplami)}</div>
        </div>
      </div>

      <section className="rounded-2xl border border-slate-200 bg-white p-5 shadow-2xs" aria-labelledby="sgk-karsilastirma-basligi">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="max-w-xl">
            <h3 id="sgk-karsilastirma-basligi" className="text-sm font-bold text-slate-900">SGK'dan Alınan Toplam Prim Tutarı</h3>
            <p className="mt-1 text-xs text-slate-500">Kontrol amacıyla girilir; veritabanına kaydedilmez.</p>
          </div>
          <div className="w-full max-w-xs">
            <label htmlFor="sgk-reported-total" className="mb-1.5 block text-xs font-semibold text-slate-700">TL tutarı</label>
            <div className="relative">
              <input
                id="sgk-reported-total"
                data-testid="sgk-reported-total-input"
                type="text"
                inputMode="decimal"
                value={sgkTutarInput}
                onChange={(event) => setSgkTutarInput(event.target.value)}
                placeholder="757.876,39"
                aria-describedby="sgk-reported-total-help"
                aria-invalid={comparison.status === 'invalid'}
                className="w-full rounded-xl border border-slate-300 bg-slate-50 px-3 py-2 pr-12 text-right font-mono text-sm font-semibold text-slate-900 outline-none transition-colors focus:border-indigo-500 focus:bg-white focus:ring-2 focus:ring-indigo-500"
              />
              <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs font-semibold text-slate-500">TL</span>
            </div>
            <p id="sgk-reported-total-help" className="mt-1 text-[11px] text-slate-500">Örnek: 757.876,39</p>
            {comparison.status === 'invalid' && (
              <p role="alert" className="mt-1 text-[11px] font-semibold text-rose-700">Geçerli bir TL tutarı giriniz.</p>
            )}
          </div>
        </div>

        <div className={`mt-5 rounded-xl border p-4 ${comparisonCardClass}`} role="status" aria-live="polite">
          <div className="grid gap-4 sm:grid-cols-3">
            <div>
              <div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">Program Genel Prim Toplamı</div>
              <div className="mt-1 font-mono text-base font-bold tabular-nums text-slate-900">{formatTL(totals.genelPrimToplami)}</div>
            </div>
            <div>
              <div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">SGK Tutarı</div>
              <div className="mt-1 font-mono text-base font-bold tabular-nums text-slate-900">
                {comparison.sgkTutarKurus === null ? '—' : formatTL(kurusToAmount(comparison.sgkTutarKurus))}
              </div>
            </div>
            <div>
              <div className="text-[10px] font-bold uppercase tracking-wide text-slate-500">Fark</div>
              <div className="mt-1 font-mono text-base font-bold tabular-nums text-slate-900">
                {comparison.farkKurus === null ? '—' : formatTL(kurusToAmount(comparison.farkKurus))}
              </div>
            </div>
          </div>

          <div className={`mt-4 flex items-center gap-2 border-t pt-3 text-xs font-bold ${comparisonTextClass}`}>
            {comparison.status === 'compatible' && <CheckCircle2 className="h-4 w-4" aria-hidden="true" />}
            {comparison.status === 'programHigher' || comparison.status === 'programLower' ? <AlertTriangle className="h-4 w-4" aria-hidden="true" /> : null}
            <span>
              {comparison.status === 'empty' && 'SGK tutarını giriniz.'}
              {comparison.status === 'invalid' && 'Karşılaştırma için geçerli bir SGK tutarı giriniz.'}
              {comparison.status === 'compatible' && 'Uyumlu'}
              {comparison.status === 'programHigher' && `Program SGK tutarından ${formatTL(kurusToAmount(comparison.farkKurus ?? 0))} fazla`}
              {comparison.status === 'programLower' && `Program SGK tutarından ${formatTL(Math.abs(kurusToAmount(comparison.farkKurus ?? 0)))} düşük`}
            </span>
          </div>
        </div>
      </section>
    </div>
  );
};
