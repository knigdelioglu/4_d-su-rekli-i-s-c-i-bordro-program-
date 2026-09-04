import React, { useMemo } from 'react';
import {
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  Circle,
  ClipboardCheck,
  FileWarning,
  Settings2,
  Wallet,
} from 'lucide-react';
import type {
  AnnualPayrollParameters,
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
} from '../../types/payroll';
import type { PayrollNotice } from '../../types/payrollNotice';
import type { ParametreSection, PayrollViewType, TabType } from '../../types/navigation';
import { AY_ISIMLERI, formatDateTR } from '../../utils/payrollPresentation';
import { PayrollNoticeCenter } from '../PayrollNoticeCenter';

interface PeriodSummaryProps {
  aktifDonem?: BordroDonemi;
  personeller: Personel[];
  puantajlar: PersonelPuantaj[];
  bordrolar: BordroKaydi[];
  activeKurumDegerleri?: DönemselKurumDegerleri;
  annualPayrollParameters: AnnualPayrollParameters[];
  payrollNotices: PayrollNotice[];
  isRefreshingNotices: boolean;
  noticeLoadError: string | null;
  onRefreshNotices: () => void | Promise<void>;
  onNavigate: (
    tab: TabType,
    payrollView?: PayrollViewType,
    parametreSection?: ParametreSection
  ) => void;
}

function hasSavedAttendance(attendance: PersonelPuantaj | undefined): boolean {
  return Boolean(attendance?.gunler && Object.keys(attendance.gunler).length > 0);
}

function isAuthoritative(payroll: BordroKaydi): boolean {
  return payroll.status === 'CALCULATED' || payroll.status === 'FINALIZED';
}

export const PeriodSummary: React.FC<PeriodSummaryProps> = ({
  aktifDonem,
  personeller,
  puantajlar,
  bordrolar,
  activeKurumDegerleri,
  annualPayrollParameters,
  payrollNotices,
  isRefreshingNotices,
  noticeLoadError,
  onRefreshNotices,
  onNavigate,
}) => {
  const summary = useMemo(() => {
    if (!aktifDonem) return null;

    const attendanceByPerson = new Map<string, PersonelPuantaj>(
      puantajlar
        .filter((item) => item.donemId === aktifDonem.id)
        .map((item) => [item.personelId, item])
    );
    const missingAttendance = personeller.filter(
      (person) => !hasSavedAttendance(attendanceByPerson.get(person.id))
    );
    const periodPayrolls = bordrolar.filter((payroll) => payroll.donemId === aktifDonem.id);
    const normalPayrolls = periodPayrolls.filter((payroll) => payroll.accrualType === 'NORMAL');
    const authoritativeNormal = normalPayrolls.filter(isAuthoritative);
    const calculatedNormal = normalPayrolls.filter((payroll) => payroll.status === 'CALCULATED');
    const staleNormal = normalPayrolls.filter((payroll) => payroll.status === 'STALE');
    const annualParametersReady = annualPayrollParameters.some(
      (parameters) => parameters.year === aktifDonem.taxYear
    );
    const monthName = AY_ISIMLERI[aktifDonem.ay - 1];
    const activeTediyeReference = Boolean(
      activeKurumDegerleri?.tediyeListesi?.some(
        (item) => item.aktifDonemdeOdensin || item.odemeAyi === monthName
      )
    );
    const activeTisReference = Boolean(
      activeKurumDegerleri?.tisIkramiyeListesi?.some(
        (item) => item.aktifDonemdeOdensin || item.odemeAyi === monthName
      )
    );
    const tediyePeople = new Set(
      periodPayrolls
        .filter((payroll) => payroll.accrualType === 'TEDIYE' && isAuthoritative(payroll))
        .map((payroll) => payroll.personelId)
    );
    const tisPeople = new Set(
      periodPayrolls
        .filter((payroll) => payroll.accrualType === 'TIS_IKRAMIYE' && isAuthoritative(payroll))
        .map((payroll) => payroll.personelId)
    );

    return {
      missingAttendance,
      normalCount: new Set(authoritativeNormal.map((payroll) => payroll.personelId)).size,
      calculatedNormalCount: calculatedNormal.length,
      staleNormalCount: staleNormal.length,
      annualParametersReady,
      activeTediyeReference,
      activeTisReference,
      missingTediye: activeTediyeReference ? Math.max(0, personeller.length - tediyePeople.size) : 0,
      missingTis: activeTisReference ? Math.max(0, personeller.length - tisPeople.size) : 0,
      finalizedPending: calculatedNormal.length,
    };
  }, [aktifDonem, activeKurumDegerleri, annualPayrollParameters, bordrolar, personeller, puantajlar]);

  if (!aktifDonem || !summary) {
    return (
      <section data-testid="period-summary" className="space-y-4">
        <div className="rounded-2xl border border-dashed border-slate-300 bg-white p-8 text-center shadow-sm">
          <Settings2 className="mx-auto h-8 w-8 text-indigo-500" aria-hidden="true" />
          <h2 className="mt-3 text-lg font-bold text-slate-900">Dönem Özeti</h2>
          <p className="mx-auto mt-2 max-w-md text-sm text-slate-600">
            İşleme başlamak için önce bir bordro dönemi açın.
          </p>
          <button
            type="button"
            onClick={() => onNavigate('parametrelar', undefined, 'newPeriod')}
            className="mt-5 inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-xs font-bold text-white hover:bg-indigo-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          >
            Yeni Dönem Aç <ArrowRight className="h-4 w-4" aria-hidden="true" />
          </button>
        </div>
      </section>
    );
  }

  const nextTask = !summary.annualParametersReady
    ? { label: 'Dönem ayarlarını tamamla', action: () => onNavigate('parametrelar', undefined, 'annualTax') }
    : summary.missingAttendance.length > 0
      ? { label: 'Eksik puantajları tamamla', action: () => onNavigate('puantaj') }
      : summary.normalCount < personeller.length
        ? { label: 'Normal maaşları hesapla', action: () => onNavigate('bordro', 'normal') }
        : summary.staleNormalCount > 0
          ? { label: 'Yeniden hesaplanması gereken bordrolara git', action: () => onNavigate('bordro', 'normal') }
          : summary.missingTis > 0
            ? { label: 'Bekleyen TİS ikramiyelerine git', action: () => onNavigate('bordro', 'tis') }
            : summary.missingTediye > 0
              ? { label: 'Bekleyen tediye kayıtlarına git', action: () => onNavigate('bordro', 'tediye') }
              : summary.finalizedPending > 0
                ? { label: 'Bordroları kesinleştir', action: () => onNavigate('bordro', 'normal') }
                : null;

  const StatusRow: React.FC<{
    icon: React.ReactNode;
    label: string;
    detail?: string;
    tone: 'success' | 'warning' | 'neutral' | 'critical';
  }> = ({ icon, label, detail, tone }) => {
    const toneClass = {
      success: 'border-emerald-200 bg-emerald-50 text-emerald-900',
      warning: 'border-amber-200 bg-amber-50 text-amber-950',
      neutral: 'border-slate-200 bg-slate-50 text-slate-800',
      critical: 'border-rose-200 bg-rose-50 text-rose-950',
    }[tone];
    return (
      <li className={`flex items-start gap-3 rounded-xl border px-3 py-2.5 text-sm ${toneClass}`}>
        <span className="mt-0.5 shrink-0" aria-hidden="true">{icon}</span>
        <span className="min-w-0">
          <span className="font-semibold">{label}</span>
          {detail && <span className="ml-1 text-xs opacity-80">{detail}</span>}
        </span>
      </li>
    );
  };

  return (
    <section data-testid="period-summary" className="space-y-5">
      <header className="flex flex-col gap-3 border-b border-slate-200 pb-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="text-[10px] font-bold uppercase tracking-[0.16em] text-indigo-600">Dönem</p>
          <h2 className="mt-1 text-2xl font-bold tracking-tight text-slate-900">
            {AY_ISIMLERI[aktifDonem.ay - 1]} {aktifDonem.yil}
          </h2>
          <p className="mt-1 text-sm text-slate-600">
            {formatDateTR(aktifDonem.baslangicTarihi)} – {formatDateTR(aktifDonem.bitisTarihi)}
          </p>
        </div>
        {nextTask ? (
          <button
            type="button"
            data-testid="period-summary-next-action"
            onClick={nextTask.action}
            className="inline-flex items-center justify-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-xs font-bold text-white shadow-sm hover:bg-indigo-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          >
            {nextTask.label} <ArrowRight className="h-4 w-4" aria-hidden="true" />
          </button>
        ) : (
          <span className="inline-flex items-center gap-2 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-2.5 text-xs font-bold text-emerald-800">
            <CheckCircle2 className="h-4 w-4" aria-hidden="true" /> Dönem hazır
          </span>
        )}
      </header>

      {payrollNotices.length > 0 || noticeLoadError ? (
        <PayrollNoticeCenter
          notices={payrollNotices}
          isRefreshing={isRefreshingNotices}
          loadError={noticeLoadError}
          onRefresh={onRefreshNotices}
        />
      ) : null}

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(280px,0.7fr)]">
        <div className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
          <div className="flex items-center justify-between gap-3 border-b border-slate-200 pb-3">
            <div>
              <h3 className="text-sm font-bold text-slate-900">Kontroller</h3>
              <p className="mt-1 text-xs text-slate-500">Kayıt ve tahakkuk durumları mevcut veriden türetilir.</p>
            </div>
            <ClipboardCheck className="h-5 w-5 text-indigo-500" aria-hidden="true" />
          </div>
          <ul className="mt-3 space-y-2" aria-label="Dönem durum özeti">
            <StatusRow
              tone={summary.annualParametersReady ? 'success' : 'critical'}
              icon={summary.annualParametersReady ? <CheckCircle2 className="h-4 w-4" /> : <AlertTriangle className="h-4 w-4" />}
              label={summary.annualParametersReady ? 'Dönem parametreleri hazır' : 'Yıllık vergi parametresi eksik'}
              detail={summary.annualParametersReady ? `${aktifDonem.taxYear} vergi yılı` : `${aktifDonem.taxYear} yılı için kayıt gerekli`}
            />
            <StatusRow
              tone={summary.missingAttendance.length === 0 ? 'success' : 'warning'}
              icon={summary.missingAttendance.length === 0 ? <CheckCircle2 className="h-4 w-4" /> : <AlertTriangle className="h-4 w-4" />}
              label={summary.missingAttendance.length === 0 ? 'Puantajlar tamam' : `${summary.missingAttendance.length} personelin puantajı eksik`}
              detail={summary.missingAttendance.length > 0 ? summary.missingAttendance.slice(0, 2).map((person) => `${person.ad} ${person.soyad}`).join(', ') : undefined}
            />
            <StatusRow
              tone={summary.normalCount === personeller.length ? 'success' : 'neutral'}
              icon={summary.normalCount === personeller.length ? <CheckCircle2 className="h-4 w-4" /> : <Circle className="h-4 w-4" />}
              label={`${summary.normalCount} / ${personeller.length} normal maaş hesaplandı`}
              detail={summary.staleNormalCount > 0 ? `${summary.staleNormalCount} yeniden hesaplanmalı` : undefined}
            />
            <StatusRow
              tone={summary.finalizedPending === 0 ? 'success' : 'neutral'}
              icon={summary.finalizedPending === 0 ? <CheckCircle2 className="h-4 w-4" /> : <Circle className="h-4 w-4" />}
              label={summary.finalizedPending === 0 ? 'Kesinleştirilecek bordro yok' : `${summary.finalizedPending} bordro kesinleştirilecek`}
            />
            {(summary.activeTisReference || summary.activeTediyeReference) && (
              <StatusRow
                tone={summary.missingTis > 0 || summary.missingTediye > 0 ? 'warning' : 'success'}
                icon={summary.missingTis > 0 || summary.missingTediye > 0 ? <FileWarning className="h-4 w-4" /> : <CheckCircle2 className="h-4 w-4" />}
                label={summary.missingTis > 0 ? `${summary.missingTis} personelin TİS tahakkuku eksik` : summary.missingTediye > 0 ? `${summary.missingTediye} personelin tediye tahakkuku eksik` : 'TİS / tediye tahakkukları tamam'}
              />
            )}
          </ul>
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
          <h3 className="text-sm font-bold text-slate-900">Hızlı erişim</h3>
          <div className="mt-3 space-y-2">
            <button type="button" onClick={() => onNavigate('puantaj')} className="flex w-full items-center justify-between rounded-xl border border-slate-200 px-3 py-3 text-left text-xs font-semibold text-slate-700 hover:border-indigo-300 hover:bg-indigo-50">
              Puantaja Git <ArrowRight className="h-4 w-4 text-indigo-500" aria-hidden="true" />
            </button>
            <button type="button" onClick={() => onNavigate('bordro', 'normal')} className="flex w-full items-center justify-between rounded-xl border border-slate-200 px-3 py-3 text-left text-xs font-semibold text-slate-700 hover:border-indigo-300 hover:bg-indigo-50">
              Bordroları Hesapla <Wallet className="h-4 w-4 text-indigo-500" aria-hidden="true" />
            </button>
            {(summary.activeTisReference || summary.activeTediyeReference) && (
              <button type="button" onClick={() => onNavigate('bordro', summary.missingTis > 0 ? 'tis' : 'tediye')} className="flex w-full items-center justify-between rounded-xl border border-slate-200 px-3 py-3 text-left text-xs font-semibold text-slate-700 hover:border-indigo-300 hover:bg-indigo-50">
                TİS / Tediye sayfasına git <ArrowRight className="h-4 w-4 text-indigo-500" aria-hidden="true" />
              </button>
            )}
          </div>
        </div>
      </div>

      {summary.missingAttendance.length > 0 && (
        <div className="rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-xs text-amber-950">
          <strong>Eksikleri Tamamla:</strong> önce puantajı oluşturun; puantaj kaydı olmayan personel için bordro hesaplama çalıştırılmaz.
        </div>
      )}
    </section>
  );
};
