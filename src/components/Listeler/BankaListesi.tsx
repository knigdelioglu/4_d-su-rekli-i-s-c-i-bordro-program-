/**
 * Section 10: Banka Listesi
 */

import React, { useState } from 'react';
import { Building2, Download, Printer, Copy, Check, Search } from 'lucide-react';
import { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import { formatTL } from '../../utils/payrollUtils';
import { exportToExcel, printElement } from '../../utils/excelExport';

interface BankaListesiProps {
  aktifDonem: BordroDonemi;
  personeller: Personel[];
  bordrolar: BordroKaydi[];
}

export const BankaListesi: React.FC<BankaListesiProps> = ({
  aktifDonem,
  personeller,
  bordrolar,
}) => {
  const [search, setSearch] = useState<string>('');
  const [copied, setCopied] = useState<boolean>(false);

  // Map each employee to their active period payroll record
  const bankEntries = personeller.map((p) => {
    const b = bordrolar.find(
      (record) => record.personelId === p.id && record.donemId === aktifDonem.id
    );
    return {
      personel: p,
      bordro: b,
      netOdeme: b ? b.netOdeme : 0,
      hasBordro: !!b,
    };
  });

  const filteredEntries = bankEntries.filter((e) => {
    const term = search.toLowerCase();
    return (
      e.personel.ad.toLowerCase().includes(term) ||
      e.personel.soyad.toLowerCase().includes(term) ||
      e.personel.tcNo.includes(term) ||
      e.personel.iban.toLowerCase().includes(term)
    );
  });

  const toplamBankaOdemesi = filteredEntries.reduce(
    (acc, item) => acc + item.netOdeme,
    0
  );

  const handleExportExcel = () => {
    const cols = [
      { header: 'T.C. Kimlik No', key: 'tcNo', width: 18 },
      { header: 'Ad Soyad', key: 'adSoyad', width: 26 },
      { header: 'İş Primi Grubu', key: 'unvan', width: 22 },
      { header: 'IBAN', key: 'iban', width: 32 },
      { header: 'Net Ödeme (TL)', key: 'netOdeme', width: 18 },
    ];

    const data = filteredEntries.map((e) => ({
      tcNo: e.personel.tcNo,
      adSoyad: `${e.personel.ad} ${e.personel.soyad}`,
      unvan: (e.personel.grup || e.personel.unvan || '1. Grup').replace(/\s*\(.*?\)/, ''),
      iban: e.personel.iban,
      netOdeme: e.netOdeme,
    }));

    const summaryRows = [
      {
        tcNo: 'TOPLAM',
        adSoyad: '',
        unvan: '',
        iban: '',
        netOdeme: toplamBankaOdemesi,
      },
    ];

    exportToExcel(
      `Banka_Odeme_Listesi_${aktifDonem.id}`,
      'Banka Listesi',
      cols,
      data,
      summaryRows
    );
  };

  const handleCopyText = () => {
    const lines = filteredEntries.map(
      (e) =>
        `${e.personel.tcNo}\t${e.personel.ad} ${e.personel.soyad}\t${e.personel.iban}\t${e.netOdeme.toFixed(
          2
        )}`
    );
    const text = `T.C. Kimlik\tAd Soyad\tIBAN\tNet Ödeme\n` + lines.join('\n');
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handlePrint = () => {
    printElement('banka-listesi-print-area');
  };

  return (
    <div className="space-y-6">
      {/* Top Banner */}
      <div className="bg-white rounded-2xl p-5 border border-slate-200 shadow-2xs flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <Building2 className="w-5 h-5 text-indigo-600" />
            <h2 className="text-lg font-bold text-slate-900">
              Banka Ödeme Listesi — {aktifDonem.donemAdi.match(/\(([^)]+)\)/)?.[1] || aktifDonem.donemAdi}
            </h2>
          </div>
          <p className="text-xs text-slate-500 mt-1">
            Bordrosu tamamlanan personellerin banka maaş aktarım listesi ve toplamları
          </p>
        </div>

        <div className="flex items-center gap-3 flex-wrap">
          <div className="relative min-w-56">
            <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Ad, T.C. veya IBAN ara..."
              className="w-full pl-9 pr-3 py-2 bg-slate-50 border border-slate-300 rounded-xl text-xs text-slate-900 focus:bg-white focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <button
            onClick={handleCopyText}
            className="px-3.5 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition-colors flex items-center gap-1.5"
          >
            {copied ? (
              <>
                <Check className="w-4 h-4 text-emerald-600" />
                <span>Kopyalandı</span>
              </>
            ) : (
              <>
                <Copy className="w-4 h-4 text-slate-600" />
                <span>Metin Olarak Kopyala</span>
              </>
            )}
          </button>

          <button
            onClick={handlePrint}
            className="px-3.5 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition-colors flex items-center gap-1.5"
          >
            <Printer className="w-4 h-4 text-slate-600" />
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

      {/* Printable Bank Table Container */}
      <div id="banka-listesi-print-area" className="bg-white rounded-2xl border border-slate-200 shadow-2xs overflow-hidden p-6 space-y-4">
        {/* Printable Title Header */}
        <div className="border-b pb-3 flex items-center justify-between">
          <div>
            <h3 className="font-bold text-base text-slate-900 uppercase">
              T.C. Kamu Kurumu 4/D Sürekli İşçi Maaş Ödeme Talimat Listesi
            </h3>
            <div className="text-xs text-slate-500 font-mono">
              Bordro Dönemi: {aktifDonem.donemAdi.match(/\(([^)]+)\)/)?.[1] || aktifDonem.donemAdi}
            </div>
          </div>
          <div className="text-right text-xs text-slate-500 font-mono">
            Personel Sayısı: {filteredEntries.length}
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
                <th className="p-3">Maaş IBAN Numarası</th>
                <th className="p-3 text-right">Net Ödeme Tutar (TL)</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 font-mono">
              {filteredEntries.map((e, index) => (
                <tr key={e.personel.id} className="hover:bg-slate-50/80">
                  <td className="p-3 text-slate-400 font-sans">{index + 1}</td>
                  <td className="p-3 font-bold text-slate-900">{e.personel.tcNo}</td>
                  <td className="p-3 font-sans font-bold text-slate-900">
                    {e.personel.ad} {e.personel.soyad}
                  </td>
                  <td className="p-3 font-sans font-semibold text-slate-800">
                    {(e.personel.grup || e.personel.unvan || '1. Grup').replace(/\s*\(.*?\)/, '')}
                  </td>
                  <td className="p-3 text-slate-800 font-bold">{e.personel.iban}</td>
                  <td className="p-3 text-right font-bold text-indigo-900 text-sm">
                    {formatTL(e.netOdeme)}
                  </td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr className="bg-indigo-50 border-t-2 border-indigo-200 font-bold">
                <td colSpan={5} className="p-4 text-right font-sans text-sm text-indigo-950 uppercase">
                  TOPLAM BANKA ÖDEMESİ:
                </td>
                <td className="p-4 text-right font-mono text-xl text-indigo-950">
                  {formatTL(toplamBankaOdemesi)}
                </td>
              </tr>
            </tfoot>
          </table>
        </div>
      </div>
    </div>
  );
};
