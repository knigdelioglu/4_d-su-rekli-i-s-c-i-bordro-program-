/**
 * Print Modal for 4/D Sürekli İşçi Ücret Pusulası / Bordro Zarfı
 */

import React from 'react';
import { Printer, X, Download, Shield, Building2 } from 'lucide-react';
import { BordroDonemi, BordroKaydi, IsPrimiGrupItem, Personel } from '../types/payroll';
import { formatDateTR, formatTL, getGrupIsPrimiOraniDisplay } from '../utils/payrollUtils';
import { printElement } from '../utils/excelExport';

interface PaySlipModalProps {
  isOpen: boolean;
  onClose: () => void;
  bordro: BordroKaydi;
  personel: Personel;
  donem: BordroDonemi;
  isPrimiGruplari?: IsPrimiGrupItem[];
}

export const PaySlipModal: React.FC<PaySlipModalProps> = ({
  isOpen,
  onClose,
  bordro,
  personel,
  donem,
  isPrimiGruplari,
}) => {
  if (!isOpen) return null;

  const handlePrint = () => {
    printElement('payslip-print-container');
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/70 backdrop-blur-xs p-4 overflow-y-auto"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-2xl shadow-2xl border border-slate-200 max-w-3xl w-full max-h-[90vh] flex flex-col overflow-hidden my-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header Actions */}
        <div className="bg-slate-900 text-white px-6 py-4 flex items-center justify-between no-print shrink-0">
          <div className="flex items-center gap-2">
            <Shield className="w-5 h-5 text-indigo-400" />
            <h3 className="font-semibold text-sm">Ücret Pusulası (Bordro Zarfı) Önizleme</h3>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handlePrint}
              className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer"
            >
              <Printer className="w-4 h-4" />
              <span>Yazdır / PDF İndir</span>
            </button>
            <button
              type="button"
              onClick={onClose}
              className="p-2 text-slate-400 hover:text-white rounded-lg transition-colors cursor-pointer"
              title="Kapat"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Printable Document Area */}
        <div id="payslip-print-container" className="p-8 space-y-6 text-slate-900 bg-white overflow-y-auto flex-1">
          {/* Header Title */}
          <div className="border-b-2 border-slate-900 pb-4 text-center space-y-1">
            <div className="flex items-center justify-center gap-2 text-slate-700 text-xs font-semibold uppercase tracking-widest">
              <Building2 className="w-4 h-4" />
              <span>T.C. Kamu Kurumu 4/D Sürekli İşçi Bordro Birimi</span>
            </div>
            <h1 className="text-xl font-black uppercase tracking-wide text-slate-900">
              SÜREKLİ İŞÇİ ÜCRET PUSULASI / BORDRO ZARFI
            </h1>
            <div className="text-xs font-bold text-indigo-800 font-mono">
              BORDRO DÖNEMİ: {donem.donemAdi}
            </div>
          </div>

          {/* Employee Header Metadata */}
          <div className="grid grid-cols-2 gap-4 bg-slate-50 p-4 rounded-xl border border-slate-200 text-xs">
            <div className="space-y-1">
              <div><strong className="text-slate-600">T.C. Kimlik No:</strong> <span className="font-mono">{personel.tcNo}</span></div>
              <div><strong className="text-slate-600">Adı Soyadı:</strong> <span className="font-bold">{personel.ad} {personel.soyad}</span></div>
              <div><strong className="text-slate-600">İş Primi Grubu:</strong> <span className="font-semibold text-indigo-900">{personel.grup || personel.unvan || '1. Grup'} (%{getGrupIsPrimiOraniDisplay(personel.grup, isPrimiGruplari) ?? '—'})</span></div>
              <div><strong className="text-slate-600">Kıdem Yılı:</strong> {personel.hizmetYili} Yıl</div>
            </div>
            <div className="space-y-1">
              <div><strong className="text-slate-600">SGK Sicil No:</strong> <span className="font-mono">{personel.sgkSicilNo || '—'}</span></div>
              <div><strong className="text-slate-600">Maaş IBAN:</strong> <span className="font-mono text-[11px]">{personel.iban}</span></div>
              <div><strong className="text-slate-600">Hesaplama Tarihi:</strong> {formatDateTR(bordro.sonGuncellemeTarihi.split('T')[0])}</div>
              <div><strong className="text-slate-600">Birim/Not:</strong> {personel.aciklama || '—'}</div>
            </div>
          </div>

          {/* Puantaj Breakdown Summary */}
          <div className="space-y-1">
            <div className="text-[11px] font-bold uppercase tracking-wider text-slate-600 flex items-center justify-between">
              <span>Puantaj İcmal Özeti (15-14 Dönemi)</span>
              {bordro.odenenRaporluGun !== undefined && (
                <span className="text-[10px] text-rose-700 font-mono">
                  Ödeme Yapılan Rapor: <strong>{bordro.odenenRaporluGun} Gün</strong> (Toplam R: {bordro.puantajOzeti.R} Gün)
                </span>
              )}
            </div>
            <div className="grid grid-cols-7 gap-1 text-center text-xs font-mono">
              <div className="p-1.5 bg-slate-100 rounded border"><div className="text-[10px] text-slate-500 font-sans">Çalışılan (Ç)</div><div className="font-bold">{bordro.puantajOzeti.Ç}</div></div>
              <div className="p-1.5 bg-slate-100 rounded border"><div className="text-[10px] text-slate-500 font-sans">H. Tatili (T)</div><div className="font-bold">{bordro.puantajOzeti.T}</div></div>
              <div className="p-1.5 bg-slate-100 rounded border"><div className="text-[10px] text-slate-500 font-sans">G. Tatil (G)</div><div className="font-bold">{bordro.puantajOzeti.G}</div></div>
              <div className="p-1.5 bg-slate-100 rounded border"><div className="text-[10px] text-slate-500 font-sans">İzin (İ)</div><div className="font-bold">{bordro.puantajOzeti.İ}</div></div>
              <div className="p-1.5 bg-slate-100 rounded border"><div className="text-[10px] text-slate-500 font-sans">Gece Ç. (GÇ)</div><div className="font-bold">{bordro.puantajOzeti.GÇ}</div></div>
              <div className="p-1.5 bg-slate-100 rounded border"><div className="text-[10px] text-slate-500 font-sans">Gece T. (GÇT)</div><div className="font-bold">{bordro.puantajOzeti.GÇT}</div></div>
              <div className="p-1.5 bg-slate-100 rounded border"><div className="text-[10px] text-slate-500 font-sans">Rapor (R)</div><div className="font-bold">{bordro.puantajOzeti.R}</div></div>
            </div>
          </div>

          {/* SGK PEK & Devreden PEK & Yemek İstisnası Bilgilendirme Kartı */}
          {bordro.pekDetay && (
            <div className="bg-slate-50 border border-slate-200 rounded-xl p-3 text-xs space-y-2">
              <div className="flex items-center justify-between text-[11px] font-bold text-slate-700">
                <span>SGK Prime Esas Kazanç (PEK) ve Yemek İstisnası Detayları:</span>
                <span className="text-indigo-700 font-mono">PEK Matrahı (Nihai): {formatTL(bordro.pekDetay.finalPek)}</span>
              </div>
              <div className="grid grid-cols-2 sm:grid-cols-5 gap-2 text-[11px] text-slate-600 pt-1 border-t border-slate-200">
                <div>
                  <span className="text-slate-500">Ham PEK (Gerçek): </span>
                  <span className="font-semibold font-mono">{formatTL(bordro.pekDetay.hesaplananPek)}</span>
                </div>
                <div>
                  <span className="text-slate-500">Nihai / Bildirim PEK: </span>
                  <span className="font-semibold font-mono text-indigo-900">{formatTL(bordro.pekDetay.finalPek)}</span>
                </div>
                {(bordro.pekDetay.altSinirTamamlamaFarki ?? (bordro.pekDetay.finalPek > bordro.pekDetay.hesaplananPek ? bordro.pekDetay.finalPek - bordro.pekDetay.hesaplananPek : 0)) > 0 && (
                  <div>
                    <span className="text-slate-500">Alt Sınır Farkı: </span>
                    <span className="font-semibold font-mono text-amber-700">
                      {formatTL(bordro.pekDetay.altSinirTamamlamaFarki ?? (bordro.pekDetay.finalPek - bordro.pekDetay.hesaplananPek))}
                    </span>
                  </div>
                )}
                <div>
                  <span className="text-slate-500">SGK Yemek İstisnası ({bordro.pekDetay.fiiliYemekGunu} Gün): </span>
                  <span className="font-semibold font-mono text-emerald-700">
                    {formatTL(bordro.pekDetay.yemekIstisnasiTutar)}
                  </span>
                </div>
                <div>
                  <span className="text-slate-500">Sonraki Aya Devreden PEK: </span>
                  <span className="font-semibold font-mono text-rose-700">
                    {bordro.sonrakiDevredenPek && bordro.sonrakiDevredenPek.length > 0
                      ? formatTL(bordro.sonrakiDevredenPek.reduce((a, b) => a + b.tutar, 0))
                      : '0,00 TL'}
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* İşveren Prim ve Maliyet Detayları (Kurum Payı) Kartı */}
          {bordro.pekDetay && (
            <div className="bg-indigo-50/70 border border-indigo-200 rounded-xl p-3 text-xs space-y-1.5">
              <div className="flex items-center justify-between text-[11px] font-bold text-indigo-950">
                <div className="flex items-center gap-1.5">
                  <Building2 className="w-3.5 h-3.5 text-indigo-700" />
                  <span>İşveren SGK ve İşsizlik Prim Maliyeti (Kurum Maliyet Kalemleri):</span>
                </div>
                <span className="text-indigo-900 font-mono font-bold">
                  Toplam İşveren Primi: {formatTL(bordro.pekDetay.isverenPrimToplami ?? ((bordro.pekDetay.isverenSgkPrimi ?? (bordro.pekDetay.finalPek * (bordro.pekDetay.sgkIsverenOraniYuzde ?? 21.75) / 100)) + (bordro.pekDetay.isverenIssizlikPrimi ?? (bordro.pekDetay.finalPek * (bordro.pekDetay.isverenIssizlikOraniYuzde ?? 2) / 100)) + (bordro.pekDetay.pekAltSinirTamamlamaIsverenPrimi ?? 0)))}
                </span>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-4 gap-2 text-[11px] text-slate-700 pt-1 border-t border-indigo-200 font-mono">
                <div>
                  <span className="text-slate-500 font-sans">SSK Primi — İşveren Payı (%{bordro.pekDetay.sgkIsverenOraniYuzde ?? 21.75}): </span>
                  <span className="font-bold text-indigo-900">
                    {formatTL(bordro.pekDetay.isverenSgkPrimi ?? (bordro.pekDetay.finalPek * (bordro.pekDetay.sgkIsverenOraniYuzde ?? 21.75) / 100))}
                  </span>
                </div>
                <div>
                  <span className="text-slate-500 font-sans">İşsizlik Primi — İşveren Payı (%{bordro.pekDetay.isverenIssizlikOraniYuzde ?? 2}): </span>
                  <span className="font-bold text-indigo-900">
                    {formatTL(bordro.pekDetay.isverenIssizlikPrimi ?? (bordro.pekDetay.finalPek * (bordro.pekDetay.isverenIssizlikOraniYuzde ?? 2) / 100))}
                  </span>
                </div>
                {(bordro.pekDetay.pekAltSinirTamamlamaIsverenPrimi ?? 0) > 0 && (
                  <div>
                    <span className="text-slate-500 font-sans">PEK Alt Sınır Tamamlama — İşveren: </span>
                    <span className="font-bold text-amber-900">
                      {formatTL(bordro.pekDetay.pekAltSinirTamamlamaIsverenPrimi)}
                    </span>
                  </div>
                )}
                <div>
                  <span className="text-slate-500 font-sans">Toplam İşveren Prim Maliyeti: </span>
                  <span className="font-black text-indigo-950">
                    {formatTL(bordro.pekDetay.isverenPrimToplami ?? ((bordro.pekDetay.isverenSgkPrimi ?? (bordro.pekDetay.finalPek * (bordro.pekDetay.sgkIsverenOraniYuzde ?? 21.75) / 100)) + (bordro.pekDetay.isverenIssizlikPrimi ?? (bordro.pekDetay.finalPek * (bordro.pekDetay.isverenIssizlikOraniYuzde ?? 2) / 100)) + (bordro.pekDetay.pekAltSinirTamamlamaIsverenPrimi ?? 0)))}
                  </span>
                </div>
              </div>
              <div className="text-[10px] text-slate-500 italic pt-0.5">
                * Bu primler işveren/kurum maliyeti olup personelin net ödemesinden kesilmez.
              </div>
            </div>
          )}

          {/* Gelir Vergisi Kümülatif Matrah Bilgilendirme Kartı */}
          <div className="bg-amber-50/60 border border-amber-200 rounded-xl p-3 text-xs space-y-1">
            <div className="flex items-center justify-between text-[11px] font-bold text-amber-900">
              <span>Gelir Vergisi Kümülatif Matrah Bilgileri:</span>
              {bordro.manuelKumulatifGvMatrahi !== undefined && (
                <span className="px-2 py-0.5 rounded bg-amber-200 text-amber-900 text-[10px] font-bold">
                  Manuel Giriş Kullanıldı
                </span>
              )}
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 text-[11px] text-slate-700 pt-1 border-t border-amber-200 font-mono">
              <div>
                <span className="text-slate-500 font-sans">Önceki Küm. GV Matrahı: </span>
                <span className="font-bold text-slate-900">{formatTL(bordro.oncekiKumulatifGvMatrahi || 0)}</span>
              </div>
              <div>
                <span className="text-slate-500 font-sans">Cari Dönem GV Matrahı: </span>
                <span className="font-bold text-slate-900">
                  {formatTL(Math.max(0, bordro.gelirToplam - (bordro.kesintiler.isciSgkPrimi || 0) - (bordro.kesintiler.isciIssizlikPrimi || 0)))}
                </span>
              </div>
              <div>
                <span className="text-slate-500 font-sans">Dönem Sonu Küm. Matrah: </span>
                <span className="font-bold text-indigo-900">
                  {formatTL((bordro.oncekiKumulatifGvMatrahi || 0) + Math.max(0, bordro.gelirToplam - (bordro.kesintiler.isciSgkPrimi || 0) - (bordro.kesintiler.isciIssizlikPrimi || 0)))}
                </span>
              </div>
            </div>
          </div>

          {/* Asgari Ücret Gelir Vergisi İstisnası Detay Kartı (GİB 7349 S.K.) */}
          {bordro.gvDetay && (
            <div className="bg-emerald-50/70 border border-emerald-200 rounded-xl p-3 text-xs space-y-1.5">
              <div className="flex items-center justify-between text-[11px] font-bold text-emerald-950">
                <div className="flex items-center gap-1.5">
                  <Shield className="w-3.5 h-3.5 text-emerald-700" />
                  <span>Asgari Ücret Gelir Vergisi İstisnası Detayları (7349 Sayılı Kanun):</span>
                </div>
                <span className="text-emerald-900 font-mono font-bold">
                  Kesilen Gelir Vergisi: {formatTL(bordro.gvDetay.kesilenGelirVergisi)}
                </span>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 text-[11px] text-slate-700 pt-1 border-t border-emerald-200 font-mono">
                <div>
                  <span className="text-slate-500 font-sans">Asgari Ücret Aylık GV Matrahı: </span>
                  <span className="font-semibold text-emerald-900">{formatTL(bordro.gvDetay.asgariUcretGvMatrahi)}</span>
                </div>
                <div>
                  <span className="text-slate-500 font-sans">Takvim Referans Küm. Matrah: </span>
                  <span className="font-semibold text-emerald-900">{formatTL(bordro.gvDetay.asgariUcretReferansKumulatifMatrahi)}</span>
                </div>
                <div>
                  <span className="text-slate-500 font-sans">Aylık İstisna Hakkı (Vergi): </span>
                  <span className="font-semibold text-emerald-900">{formatTL(bordro.gvDetay.asgariUcretGvIstisnasi)}</span>
                </div>
                <div>
                  <span className="text-slate-500 font-sans">Brüt Gelir Vergisi (İstisna Öncesi): </span>
                  <span className="font-semibold text-slate-900">{formatTL(bordro.gvDetay.brutGelirVergisi)}</span>
                </div>
                <div>
                  <span className="text-slate-500 font-sans">Uygulanan İstisna: </span>
                  <span className="font-semibold text-emerald-900">{formatTL(bordro.gvDetay.uygulananGvIstisnasi)}</span>
                </div>
                <div>
                  <span className="text-slate-500 font-sans">Kesilecek Gelir Vergisi: </span>
                  <span className="font-bold text-rose-900">{formatTL(bordro.gvDetay.kesilenGelirVergisi)}</span>
                </div>
              </div>
              <div className="text-[10px] text-slate-500 italic pt-0.5">
                * Cari GV matrahı: {formatTL(bordro.gvDetay.cariGvMatrahi)} · Dönem sonu kümülatif matrah: {formatTL(bordro.gvDetay.yeniKumulatifGvMatrahi)}. İstisna, çalışanın kendi bordrosuna değil takvim konumuna göre hesaplanır.
              </div>
            </div>
          )}

          {/* Income & Deduction Tables */}
          <div className="grid grid-cols-2 gap-6 text-xs">
            {/* GELİRLER */}
            <div className="space-y-2">
              <div className="bg-emerald-700 text-white font-bold p-2 text-center rounded-t-lg uppercase text-[11px] tracking-wider">
                GELİR KALEMLERİ
              </div>
              <table className="w-full text-left border-collapse border border-slate-200">
                <tbody className="divide-y divide-slate-200 font-mono">
                  <tr><td className="p-1.5 font-sans">Taban / Brüt Aylık</td><td className="p-1.5 text-right font-bold">{formatTL(bordro.gelirler.tabanBrutAylik)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Tediye</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.tediye)}</td></tr>
                  <tr><td className="p-1.5 font-sans">TİS İkramiyesi</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.tisIkramiyesi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Ek Ödeme</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.ekOdeme)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Yemek Yardım</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.yemek)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Sosyal Yardım</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.birlestirilmisSosyalYardim)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Vasıta / Yol</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.vasitaYol)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Giyim Yardımı</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.giyimYardimi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">İş Primi</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.isPrimi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Gece Çalışması Ücreti</td><td className="p-1.5 text-right font-semibold text-indigo-700">{formatTL(bordro.gelirler.geceCalismasiUcreti)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Gece Çalışması Tatili Ücreti</td><td className="p-1.5 text-right font-semibold text-teal-700">{formatTL(bordro.gelirler.geceCalismasiTatiliUcreti)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Hizmet Zammı</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.hizmetZammi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Diğer Gelir</td><td className="p-1.5 text-right">{formatTL(bordro.gelirler.digerGelir)}</td></tr>
                </tbody>
                <tfoot>
                  <tr className="bg-emerald-50 border-t-2 border-emerald-300 font-bold font-mono">
                    <td className="p-2 font-sans text-emerald-900">GELİRLER TOPLAMI</td>
                    <td className="p-2 text-right text-emerald-900">{formatTL(bordro.gelirToplam)}</td>
                  </tr>
                </tfoot>
              </table>
            </div>

            {/* KESİNTİLER */}
            <div className="space-y-2">
              <div className="bg-rose-700 text-white font-bold p-2 text-center rounded-t-lg uppercase text-[11px] tracking-wider">
                KESİNTİ KALEMLERİ
              </div>
              <table className="w-full text-left border-collapse border border-slate-200">
                <tbody className="divide-y divide-slate-200 font-mono">
                  <tr><td className="p-1.5 font-sans">İşçi SGK Primi (%14)</td><td className="p-1.5 text-right font-bold">{formatTL(bordro.kesintiler.isciSgkPrimi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">İşçi İşsizlik Primi (%1)</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.isciIssizlikPrimi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Gelir Vergisi (%15)</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.gelirVergisi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Damga Vergisi (0.00759)</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.damgaVergisi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Sendika Aidatı</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.sendikaAidati)}</td></tr>
                  <tr><td className="p-1.5 font-sans">BES Kesintisi</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.bes)}</td></tr>
                  <tr><td className="p-1.5 font-sans">İcra Kesintisi</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.icra)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Kişi Borcu Kesintisi</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.kisiBorcu)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Doğum/Askerlik Borç.</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.dogumAskerlikBorclanmasi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Hayat/Sağlık Sigortası</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.hayatSaglikSigortasi)}</td></tr>
                  <tr><td className="p-1.5 font-sans">Diğer Kesinti</td><td className="p-1.5 text-right">{formatTL(bordro.kesintiler.digerKesinti)}</td></tr>
                </tbody>
                <tfoot>
                  <tr className="bg-rose-50 border-t-2 border-rose-300 font-bold font-mono">
                    <td className="p-2 font-sans text-rose-900">KESİNTİLER TOPLAMI</td>
                    <td className="p-2 text-right text-rose-900">{formatTL(bordro.kesintiToplam)}</td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>

          {/* NET PAYMENT BANNER */}
          <div className="bg-gradient-to-r from-slate-900 to-indigo-950 text-white p-5 rounded-2xl flex items-center justify-between shadow-md">
            <div>
              <div className="text-xs uppercase font-bold text-indigo-300 tracking-wider">
                Banka Hesabına Yatırılacak Tutar
              </div>
              <div className="text-xs text-slate-300 mt-0.5">
                NET ÖDEME = Gelirler Toplamı − Kesintiler Toplamı
              </div>
            </div>

            <div className="text-right">
              <div className="text-3xl font-black text-amber-400 font-mono">
                {formatTL(bordro.netOdeme)}
              </div>
            </div>
          </div>

          {/* Signatures */}
          <div className="pt-8 grid grid-cols-2 gap-12 text-center text-xs">
            <div className="space-y-8">
              <div><strong>Bordro Düzenleyen Yetkili</strong></div>
              <div className="text-slate-400 italic">İmza / Mühür</div>
            </div>
            <div className="space-y-8">
              <div><strong>İşçi Teslim / Onay</strong></div>
              <div className="text-slate-400 italic">{personel.ad} {personel.soyad}</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
