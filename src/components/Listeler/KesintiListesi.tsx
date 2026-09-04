/**
 * Sections 11, 12, 13, 14: Sendika, BES, İcra, Kişi Borcu Listeleri
 */

import React, { useState } from 'react';
import {
  Users2,
  Download,
  Printer,
  ShieldAlert,
  Scale,
  Receipt,
  Wallet,
  Search,
  Baby,
  HeartPulse,
  MoreHorizontal,
} from 'lucide-react';
import { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import type { KesintiTipi } from '../../types/navigation';
import { formatTL } from '../../utils/payrollPresentation';
import { exportToExcel, printElement } from '../../utils/excelExport';
import {
  filterAccrualRowsByPaymentDate,
  getAuthoritativeAccrualRows,
  getPaymentDateOptions,
} from './accrualListData';

interface KesintiListesiProps {
  aktifDonem: BordroDonemi;
  personeller: Personel[];
  bordrolar: BordroKaydi[];
  activeType: KesintiTipi;
}

export const KesintiListesi: React.FC<KesintiListesiProps> = ({
  aktifDonem,
  personeller,
  bordrolar,
  activeType,
}) => {
  const [search, setSearch] = useState<string>('');
  const [paymentDateFilter, setPaymentDateFilter] = useState<string>('all');

  // Resmî kesinti listelerine yalnız authoritative CALCULATED/FINALIZED
  // bordrolar girer. STALE/DRAFT snapshot'lar yeniden hesaplanmadan dışlanır.
  const entries = getAuthoritativeAccrualRows(aktifDonem, personeller, bordrolar).map((row) => ({
    ...row,
    sendikaAidati: row.bordro.kesintiler.sendikaAidati ?? 0,
    bes: row.bordro.kesintiler.bes ?? 0,
    icra: row.bordro.kesintiler.icra ?? 0,
    kisiBorcu: row.bordro.kesintiler.kisiBorcu ?? 0,
    dogumAskerlikBorclanmasi: row.bordro.kesintiler.dogumAskerlikBorclanmasi ?? 0,
    hayatSaglikSigortasi: row.bordro.kesintiler.hayatSaglikSigortasi ?? 0,
    digerKesinti: row.bordro.kesintiler.digerKesinti ?? 0,
  }));
  const paymentDateOptions = getPaymentDateOptions(entries);

  const getActiveTypeConfig = () => {
    switch (activeType) {
      case 'sendika':
        return {
          title: 'Sendika Aidatı Listesi',
          icon: Users2,
          fieldKey: 'sendikaAidati' as const,
          label: 'Sendika Aidatı',
          totalLabel: 'TOPLAM SENDİKA AİDATI',
          color: 'indigo',
        };
      case 'bes':
        return {
          title: 'BES Kesintisi Listesi',
          icon: Wallet,
          fieldKey: 'bes' as const,
          label: 'BES Kesintisi',
          totalLabel: 'TOPLAM BES KESİNTİSİ',
          color: 'emerald',
        };
      case 'icra':
        return {
          title: 'İcra Kesintisi Listesi',
          icon: Scale,
          fieldKey: 'icra' as const,
          label: 'İcra Kesintisi',
          totalLabel: 'TOPLAM İCRA KESİNTİSİ',
          color: 'rose',
        };
      case 'kisiBorcu':
        return {
          title: 'Kişi Borcu Listesi',
          icon: Receipt,
          fieldKey: 'kisiBorcu' as const,
          label: 'Kişi Borcu Kesintisi',
          totalLabel: 'TOPLAM KİŞİ BORCU',
          color: 'amber',
        };
      case 'dogumAskerlik':
        return {
          title: 'Doğum / Askerlik Borçlanması Listesi',
          icon: Baby,
          fieldKey: 'dogumAskerlikBorclanmasi' as const,
          label: 'Doğum / Askerlik Borçlanması',
          totalLabel: 'TOPLAM DOĞUM / ASKERLİK BORÇLANMASI',
          color: 'purple',
        };
      case 'hayatSaglik':
        return {
          title: 'Hayat / Sağlık Sigortası Kesintisi Listesi',
          icon: HeartPulse,
          fieldKey: 'hayatSaglikSigortasi' as const,
          label: 'Hayat / Sağlık Sigortası',
          totalLabel: 'TOPLAM HAYAT / SAĞLIK SİGORTASI',
          color: 'teal',
        };
      case 'digerKesinti':
        return {
          title: 'Diğer Özel Kesintiler Listesi',
          icon: MoreHorizontal,
          fieldKey: 'digerKesinti' as const,
          label: 'Diğer Kesinti',
          totalLabel: 'TOPLAM DİĞER KESİNTİ',
          color: 'slate',
        };
    }
  };

  const config = getActiveTypeConfig();

  // Filter entries that have amount > 0 or match search
  const filteredList = filterAccrualRowsByPaymentDate(entries, paymentDateFilter)
    .filter((e) => {
      const amount = e[config.fieldKey] || 0;
      return amount > 0;
    })
    .filter((e) => {
      const term = search.toLowerCase();
      return (
        (
          e.personel.ad.toLowerCase().includes(term) ||
          e.personel.soyad.toLowerCase().includes(term) ||
          e.personel.tcNo.includes(term)
        )
      );
    });

  const toplamTutar = filteredList.reduce(
    (acc, item) => acc + (item[config.fieldKey] || 0),
    0
  );

  const handleExportExcel = () => {
    const cols = [
      { header: 'T.C. Kimlik No', key: 'tcNo', width: 18 },
      { header: 'Ad Soyad', key: 'adSoyad', width: 26 },
      { header: 'İş Primi Grubu', key: 'unvan', width: 22 },
      { header: 'Ödeme Tarihi', key: 'paymentDate', width: 16 },
      { header: 'Tahakkuk Türü', key: 'accrualType', width: 18 },
      { header: 'Sıra', key: 'sequence', width: 8 },
      { header: `${config.label} (TL)`, key: 'tutar', width: 20 },
    ];

    const data = filteredList.map((e) => ({
      tcNo: e.personel.tcNo,
      adSoyad: `${e.personel.ad} ${e.personel.soyad}`,
      unvan: (e.personel.grup || e.personel.unvan || '1. Grup').replace(/\s*\(.*?\)/, ''),
      paymentDate: e.paymentDate,
      accrualType: e.accrualTypeLabel,
      sequence: e.sequence,
      tutar: e[config.fieldKey] || 0,
    }));

    const summaryRows = [
      {
        tcNo: 'TOPLAM',
        adSoyad: '',
        unvan: '',
        paymentDate: '',
        accrualType: '',
        sequence: '',
        tutar: toplamTutar,
      },
    ];

    exportToExcel(
      `${config.title.replace(/\s+/g, '_')}_${aktifDonem.id}_${paymentDateFilter === 'all' ? 'Tumu' : paymentDateFilter}`,
      config.label,
      cols,
      data,
      summaryRows
    );
  };

  const handlePrint = () => {
    printElement('kesinti-listesi-print-area');
  };

  return (
    <div
      className="space-y-6"
      data-testid="deduction-screen"
      data-deduction-type={activeType}
    >
      {/* Top Action Banner */}
      <div className="bg-white rounded-2xl p-5 border border-slate-200 shadow-2xs flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h2 className="text-lg font-bold text-slate-900 flex items-center gap-2">
            <span>{config.title}</span>
            <span className="text-xs px-2 py-0.5 bg-slate-100 border rounded-full text-slate-600 font-normal">
              {aktifDonem.donemAdi.match(/\(([^)]+)\)/)?.[1] || aktifDonem.donemAdi}
            </span>
          </h2>
          <p className="text-xs text-slate-500 mt-1">
            Maaş bordrosunda kesintisi olan personellerin detaylı listesi
          </p>
        </div>

        <div className="flex items-center gap-3 flex-wrap">
          <div className="relative min-w-52">
            <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Ad veya T.C. ara..."
              className="w-full pl-9 pr-3 py-2 bg-slate-50 border border-slate-300 rounded-xl text-xs text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <label className="flex items-center gap-2 text-xs font-semibold text-slate-600">
            <span className="whitespace-nowrap">Ödeme Tarihi</span>
            <select
              data-testid="deduction-payment-date-filter"
              value={paymentDateFilter}
              onChange={(e) => setPaymentDateFilter(e.target.value)}
              className="rounded-xl border border-slate-300 bg-slate-50 px-3 py-2 text-xs font-semibold text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
            >
              <option value="all">Tümü</option>
              {paymentDateOptions.map((date) => (
                <option key={date} value={date}>{date}</option>
              ))}
            </select>
          </label>

          <button
            onClick={handlePrint}
            className="px-3.5 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition-colors flex items-center gap-1.5"
          >
            <Printer className="w-4 h-4" />
            <span>Yazdır</span>
          </button>

          <button
            onClick={handleExportExcel}
            className="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-xl text-xs font-bold shadow-xs transition-colors flex items-center gap-1.5"
          >
            <Download className="w-4 h-4" />
            <span>Excel'e Aktar</span>
          </button>
        </div>
      </div>

      {/* Printable Area */}
      <div id="kesinti-listesi-print-area" className="bg-white rounded-2xl border border-slate-200 shadow-2xs overflow-hidden p-6 space-y-4">
        <div className="border-b pb-3 flex items-center justify-between">
          <div>
            <h3 className="font-bold text-base text-slate-900 uppercase">
              T.C. KAMU KURUMU 4/D SÜREKLİ İŞÇİ {config.title.toUpperCase()}
            </h3>
            <div className="text-xs text-slate-500 font-mono">
              Bordro Dönemi: {aktifDonem.donemAdi.match(/\(([^)]+)\)/)?.[1] || aktifDonem.donemAdi}
            </div>
          </div>
          <div className="text-right text-xs text-slate-500 font-mono">
            Kayıt Sayısı: {filteredList.length}
            {paymentDateFilter !== 'all' && <div>Filtre: {paymentDateFilter}</div>}
          </div>
        </div>

        {/* Table */}
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs border-collapse">
            <thead>
              <tr className="bg-slate-50 text-slate-900 border-b border-slate-200 font-bold uppercase text-[10px] tracking-wider">
                <th className="p-3">S.No</th>
                <th className="p-3">T.C. Kimlik No</th>
                <th className="p-3">Adı Soyadı</th>
                <th className="p-3">İş Primi Grubu</th>
                <th className="p-3">Ödeme Tarihi</th>
                <th className="p-3">Tahakkuk Türü</th>
                <th className="p-3 text-center">Sıra</th>
                <th className="p-3 text-right">{config.label} (TL)</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 font-mono">
              {filteredList.map((e, idx) => {
                const amount = e[config.fieldKey] || 0;
                return (
                  <tr key={`${e.personel.id}-${e.accrualId}`} className="hover:bg-slate-50/80">
                    <td className="p-3 text-slate-400 font-sans">{idx + 1}</td>
                    <td className="p-3 font-bold text-slate-900">{e.personel.tcNo}</td>
                    <td className="p-3 font-sans font-bold text-slate-900">
                      {e.personel.ad} {e.personel.soyad}
                    </td>
                    <td className="p-3 font-sans font-semibold text-slate-800">
                      {(e.personel.grup || e.personel.unvan || '1. Grup').replace(/\s*\(.*?\)/, '')}
                    </td>
                    <td className="p-3 text-slate-800">{e.paymentDate}</td>
                    <td className="p-3 font-sans font-semibold text-indigo-700">{e.accrualTypeLabel}</td>
                    <td className="p-3 text-center">{e.sequence}</td>
                    <td className="p-3 text-right font-bold text-slate-900 text-sm">
                      {formatTL(amount)}
                    </td>
                  </tr>
                );
              })}

              {filteredList.length === 0 && (
                <tr>
                  <td colSpan={8} className="p-8 text-center text-slate-500 font-sans italic">
                    Bu dönem için {config.label.toLowerCase()} kaydı bulunan personel bulunmamaktadır.
                  </td>
                </tr>
              )}
            </tbody>
            <tfoot>
              <tr className="bg-slate-50 border-t-2 border-slate-200 font-bold">
                <td colSpan={7} className="p-4 text-right font-sans text-sm text-slate-900 uppercase">
                  {config.totalLabel}:
                </td>
                <td className="p-4 text-right font-mono text-lg text-slate-900">
                  {formatTL(toplamTutar)}
                </td>
              </tr>
            </tfoot>
          </table>
        </div>
      </div>
    </div>
  );
};
