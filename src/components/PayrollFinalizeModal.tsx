import React, { useMemo, useState } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  Info,
  Loader2,
  LockKeyhole,
  RefreshCw,
  X,
} from 'lucide-react';
import { BordroDonemi, BordroKaydi, Personel } from '../types/payroll';
import { PayrollNotice, PayrollNoticeSeverity } from '../types/payrollNotice';
import { formatTL } from '../utils/payrollPresentation';
import {
  PayrollBoundaryPayroll,
  PayrollDatasetSnapshot,
  PayrollEngine,
} from '../services/payrollEngine';
import { toPayrollUiModel } from '../services/payrollEngine/decimalBoundary';
import {
  filterFinalizeNotices,
  hasBlockingFinalizeNotice,
} from './payrollFinalizeRules';
import { getPayrollStatusLabel } from './Listeler/accrualListData';

const accrualTypeLabels: Record<BordroKaydi['accrualType'], string> = {
  NORMAL: 'Normal Maaş',
  TEDIYE: 'Tediye',
  TIS_IKRAMIYE: 'TİS İkramiyesi',
  SUPPLEMENTAL: 'Ek Ödeme',
  RETRO_ADJUSTMENT: 'Geriye Dönük Fark',
};

interface PayrollFinalizeModalProps {
  personel: Personel;
  bordro: BordroKaydi;
  donem: BordroDonemi;
  engine: PayrollEngine;
  dataset: PayrollDatasetSnapshot;
  onFinalized: (bordro: PayrollBoundaryPayroll) => Promise<void> | void;
  onError?: (message: string) => void;
}

interface ReviewSnapshot {
  bordro: BordroKaydi;
  notices: PayrollNotice[];
}

const severityRank: Record<PayrollNoticeSeverity, number> = {
  CRITICAL: 0,
  WARNING: 1,
  INFO: 2,
  SUCCESS: 3,
};

const severityStyle: Record<
  PayrollNoticeSeverity,
  { wrapper: string; icon: string; Icon: React.ComponentType<{ className?: string }> }
> = {
  CRITICAL: {
    wrapper: 'border-rose-300 bg-rose-50 text-rose-950',
    icon: 'text-rose-600',
    Icon: AlertTriangle,
  },
  WARNING: {
    wrapper: 'border-amber-300 bg-amber-50 text-amber-950',
    icon: 'text-amber-600',
    Icon: AlertTriangle,
  },
  INFO: {
    wrapper: 'border-blue-300 bg-blue-50 text-blue-950',
    icon: 'text-blue-600',
    Icon: Info,
  },
  SUCCESS: {
    wrapper: 'border-emerald-300 bg-emerald-50 text-emerald-950',
    icon: 'text-emerald-600',
    Icon: CheckCircle2,
  },
};

export const PayrollFinalizeModal: React.FC<PayrollFinalizeModalProps> = ({
  personel,
  bordro,
  donem,
  engine,
  dataset,
  onFinalized,
  onError,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [isFinalizing, setIsFinalizing] = useState(false);
  const [review, setReview] = useState<ReviewSnapshot | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);

  const relevantNotices = review?.notices ?? [];
  const counts = useMemo(
    () => ({
      critical: relevantNotices.filter((notice) => notice.severity === 'CRITICAL').length,
      warning: relevantNotices.filter((notice) => notice.severity === 'WARNING').length,
      info: relevantNotices.filter((notice) => notice.severity === 'INFO').length,
    }),
    [relevantNotices]
  );
  const hasCritical = hasBlockingFinalizeNotice(relevantNotices);
  const authoritativeBordro = review?.bordro ?? bordro;
  const isProvisionalSupplementary =
    authoritativeBordro.accrualType !== 'NORMAL' &&
    authoritativeBordro.statutorySnapshot?.source !== 'ATTENDANCE_BACKED';
  const canFinalize = !isLoading && !isFinalizing && !reviewError && !isProvisionalSupplementary;

  const loadReview = async (showLoading: boolean): Promise<ReviewSnapshot> => {
    if (showLoading) setIsLoading(true);
    try {
      const [allNotices, payrolls] = await Promise.all([
        engine.getPayrollNotices(donem.id, dataset),
        engine.getPayrolls(dataset),
      ]);
      const requestedAccrualId = bordro.accrualId || bordro.id;
      const current = payrolls.find(
        (item) =>
          item.personelId === personel.id &&
          item.donemId === donem.id &&
          (item.accrualId === requestedAccrualId || item.id === bordro.id)
      );
      if (!current) {
        throw new Error('Kesinleştirilecek bordro kaydı bulunamadı.');
      }

      const filtered = filterFinalizeNotices(allNotices, personel.id).sort(
        (a, b) => severityRank[a.severity] - severityRank[b.severity]
      );

      const snapshot = {
        bordro: toPayrollUiModel(current) as BordroKaydi,
        notices: filtered,
      };
      setReview(snapshot);
      setReviewError(null);
      return snapshot;
    } finally {
      if (showLoading) setIsLoading(false);
    }
  };

  const openReview = async (event: React.MouseEvent) => {
    event.stopPropagation();
    setIsOpen(true);
    setReview(null);
    setReviewError(null);
    try {
      await loadReview(true);
    } catch (err) {
      const message = `Kesinleştirme kontrolleri alınamadı: ${String(err)}`;
      setReviewError(message);
      onError?.(message);
    }
  };

  const close = () => {
    if (isFinalizing) return;
    setIsOpen(false);
    setReview(null);
    setReviewError(null);
  };

  const finalize = async () => {
    setIsFinalizing(true);
    setReviewError(null);
    try {
      // Modal açık kaldığı sırada başka bir girdi değişmiş olabilir. Kilitlemeden
      // hemen önce native bordro ve notice listesi ikinci kez authoritative kaynaktan okunur.
      const latestReview = await loadReview(false);
      if (
        latestReview.bordro.accrualType !== 'NORMAL' &&
        latestReview.bordro.statutorySnapshot?.source !== 'ATTENDANCE_BACKED'
      ) {
        throw new Error(
          'Bu tahakkuk geçici 30 günlük SGK/PEK kapasitesiyle hesaplandı. Puantaj kesinleşince yeniden hesaplayın.'
        );
      }
      const finalized = await engine.finalizePayroll(
        personel.id,
        donem.id,
        dataset,
        latestReview.bordro.accrualId || latestReview.bordro.id
      );
      await onFinalized(finalized);
      setIsOpen(false);
      setReview(null);
    } catch (err) {
      const message = `Bordro kesinleştirilemedi: ${String(err)}`;
      setReviewError(message);
      onError?.(message);
    } finally {
      setIsFinalizing(false);
    }
  };

  return (
    <>
      <button
        type="button"
        onClick={openReview}
        title="Bordroyu kontrol ederek kesinleştir"
        className="p-1.5 bg-amber-50 text-amber-800 hover:bg-amber-600 hover:text-white rounded-lg transition-colors text-[11px] font-semibold flex items-center gap-1"
      >
        <CheckCircle2 className="w-3.5 h-3.5" />
        <span>Kesinleştir</span>
      </button>

      {isOpen && (
        <div
          className="fixed inset-0 z-[90] flex items-center justify-center bg-slate-950/65 p-4 backdrop-blur-sm"
          onClick={(event) => {
            event.stopPropagation();
            close();
          }}
        >
          <div
            className="flex max-h-[90vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xl"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="flex items-center justify-between gap-3 bg-slate-900 px-5 py-4 text-white">
              <div className="flex items-center gap-3">
                <div className="rounded-xl bg-amber-500/15 p-2 text-amber-300">
                  <LockKeyhole className="h-5 w-5" />
                </div>
                <div>
                  <h3 className="text-sm font-bold">Bordroyu Kesinleştir</h3>
                  <p className="mt-0.5 text-[11px] text-slate-400">
                    {personel.ad} {personel.soyad} · {donem.donemAdi} ·{' '}
                    {accrualTypeLabels[authoritativeBordro.accrualType]} · {authoritativeBordro.paymentDate}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={close}
                disabled={isFinalizing}
                className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-slate-800 hover:text-white disabled:opacity-40"
              >
                <X className="h-5 w-5" />
              </button>
            </div>

            <div className="flex-1 space-y-4 overflow-y-auto p-5">
              <div className="grid grid-cols-2 gap-3 rounded-xl border border-slate-200 bg-slate-50 p-4 sm:grid-cols-4">
                <div>
                  <div className="text-[10px] font-semibold uppercase tracking-wide text-slate-500">Durum</div>
                  <div className="mt-1 text-xs font-bold text-slate-900">{getPayrollStatusLabel(authoritativeBordro.status)}</div>
                </div>
                <div>
                  <div className="text-[10px] font-semibold uppercase tracking-wide text-slate-500">Brüt</div>
                  <div className="mt-1 text-xs font-bold text-slate-900">{formatTL(authoritativeBordro.gelirToplam)}</div>
                </div>
                <div>
                  <div className="text-[10px] font-semibold uppercase tracking-wide text-slate-500">Kesinti</div>
                  <div className="mt-1 text-xs font-bold text-rose-700">{formatTL(authoritativeBordro.kesintiToplam)}</div>
                </div>
                <div>
                  <div className="text-[10px] font-semibold uppercase tracking-wide text-slate-500">Net ödeme</div>
                  <div className="mt-1 text-xs font-bold text-emerald-700">{formatTL(authoritativeBordro.netOdeme)}</div>
                </div>
              </div>

              {isProvisionalSupplementary && (
                <div className="rounded-xl border border-amber-300 bg-amber-50 p-3 text-xs font-semibold text-amber-950">
                  SGK/PEK kapasitesi geçici 30 gün kabulüyle hesaplandı. Puantaj kesinleşince yeniden hesaplanacaktır; bu kayıt kesinleştirilemez.
                </div>
              )}

              {isLoading ? (
                <div className="flex items-center justify-center gap-2 rounded-xl border border-slate-200 bg-slate-50 p-8 text-xs font-semibold text-slate-600">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Bordro kontrolleri yeniden okunuyor…
                </div>
              ) : (
                <>
                  {reviewError && (
                    <div className="rounded-xl border border-rose-300 bg-rose-50 p-3 text-xs font-semibold text-rose-900">
                      {reviewError}
                    </div>
                  )}

                  {!reviewError && authoritativeBordro.status !== 'CALCULATED' && (
                    <div className="rounded-xl border border-rose-300 bg-rose-50 p-3 text-xs font-semibold text-rose-900">
                      Bu bordro hesaplanmış durumda değil. Kesinleştirme öncesi yeniden hesaplanması gerekir.
                    </div>
                  )}

                  {!reviewError && authoritativeBordro.status === 'CALCULATED' && !hasCritical && (
                    <div className="flex items-start gap-2 rounded-xl border border-emerald-300 bg-emerald-50 p-3 text-emerald-950">
                      <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-600" />
                      <div>
                        <div className="text-xs font-bold">Kesinleştirmeye hazır</div>
                        <p className="mt-0.5 text-[11px] font-medium text-emerald-800">
                          Kritik kontrol bulunmadı. Bilgi ve kontrol uyarıları bordroyu engellemez.
                        </p>
                      </div>
                    </div>
                  )}

                  {hasCritical && (
                    <div className="rounded-xl border border-rose-300 bg-rose-50 p-3 text-xs font-semibold text-rose-900">
                      {counts.critical} kritik kontrol çözülmeden bordro kesinleştirilemez.
                    </div>
                  )}

                  <div className="flex flex-wrap gap-2 text-[10px] font-bold">
                    {counts.critical > 0 && (
                      <span className="rounded-full bg-rose-100 px-2.5 py-1 text-rose-800">{counts.critical} kritik</span>
                    )}
                    {counts.warning > 0 && (
                      <span className="rounded-full bg-amber-100 px-2.5 py-1 text-amber-800">{counts.warning} kontrol</span>
                    )}
                    {counts.info > 0 && (
                      <span className="rounded-full bg-blue-100 px-2.5 py-1 text-blue-800">{counts.info} bilgi</span>
                    )}
                    {relevantNotices.length === 0 && (
                      <span className="rounded-full bg-emerald-100 px-2.5 py-1 text-emerald-800">Ek uyarı yok</span>
                    )}
                  </div>

                  {relevantNotices.length > 0 && (
                    <div className="space-y-2">
                      {relevantNotices.map((notice, index) => {
                        const style = severityStyle[notice.severity];
                        const Icon = style.Icon;
                        return (
                          <div
                            key={`${notice.code}:${notice.personnelId ?? 'period'}:${index}`}
                            className={`rounded-xl border p-3 ${style.wrapper}`}
                          >
                            <div className="flex items-start gap-2.5">
                              <Icon className={`mt-0.5 h-4 w-4 shrink-0 ${style.icon}`} />
                              <div className="min-w-0 flex-1">
                                <div className="text-xs font-bold">{notice.title}</div>
                                <p className="mt-1 text-[11px] font-medium leading-relaxed opacity-90">
                                  {notice.message}
                                </p>
                                {notice.details.length > 0 && (
                                  <div className="mt-2 flex flex-wrap gap-1">
                                    {notice.details.map((detail) => (
                                      <span
                                        key={detail}
                                        className="rounded-md border border-current/15 bg-white/70 px-1.5 py-0.5 font-mono text-[10px]"
                                      >
                                        {detail}
                                      </span>
                                    ))}
                                  </div>
                                )}
                              </div>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </>
              )}
            </div>

            <div className="flex flex-col gap-2 border-t border-slate-200 bg-slate-50 px-5 py-4 sm:flex-row sm:items-center sm:justify-between">
              <p className="text-[10px] font-medium text-slate-500">
                Kesinleştirilen bordro kilitlenir. Kesinleştirme sırasında güncellik kontrolleri yeniden uygulanır.
              </p>
              <div className="flex shrink-0 gap-2">
                <button
                  type="button"
                  onClick={() => void loadReview(true).catch((err) => {
                    const message = `Kesinleştirme kontrolleri alınamadı: ${String(err)}`;
                    setReviewError(message);
                    onError?.(message);
                  })}
                  disabled={isLoading || isFinalizing}
                  className="inline-flex items-center gap-1.5 rounded-xl border border-slate-300 bg-white px-3 py-2 text-xs font-semibold text-slate-700 transition-colors hover:bg-slate-100 disabled:opacity-50"
                >
                  <RefreshCw className={`h-3.5 w-3.5 ${isLoading ? 'animate-spin' : ''}`} />
                  Yenile
                </button>
                <button
                  type="button"
                  onClick={finalize}
                  disabled={!canFinalize}
                  className="inline-flex items-center gap-1.5 rounded-xl bg-amber-600 px-4 py-2 text-xs font-bold text-white shadow-sm transition-colors hover:bg-amber-700 disabled:cursor-not-allowed disabled:bg-slate-300 disabled:text-slate-500"
                >
                  {isFinalizing ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <LockKeyhole className="h-3.5 w-3.5" />
                  )}
                  Kesinleştir ve Kilitle
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
};
