import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  BellRing,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Info,
  RefreshCw,
} from 'lucide-react';
import { PayrollDatasetSnapshot, PayrollEngine } from '../services/payrollEngine';
import { PayrollNotice, PayrollNoticeSeverity } from '../types/payrollNotice';

const severityRank: Record<PayrollNoticeSeverity, number> = {
  CRITICAL: 0,
  WARNING: 1,
  INFO: 2,
  SUCCESS: 3,
};

const severityStyles: Record<
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

function actionLabel(action?: string): string | null {
  switch (action) {
    case 'GO_TO_PUANTAJ':
      return 'Puantaj kontrolü gerekli';
    case 'RECALCULATE_PAYROLL':
      return 'Bordro yeniden hesaplanmalı';
    case 'CHECK_ANNUAL_PARAMETERS':
      return 'Yıllık vergi parametrelerini kontrol edin';
    case 'CHECK_PERIOD_PARAMETERS':
      return 'Dönem kurum parametrelerini kontrol edin';
    case 'CHECK_RAISE_PARAMETERS':
      return 'Zam parametrelerini kontrol edin';
    case 'CHECK_SICK_LEAVE':
      return 'Rapor kotasını kontrol edin';
    case 'REVIEW_PEK':
      return 'PEK devrini kontrol edin';
    case 'REVIEW_TAX_DETAIL':
      return 'Vergi hesap detayını inceleyin';
    default:
      return action ? 'Kontrol gerekli' : null;
  }
}

interface PayrollNoticeCenterProps {
  notices: PayrollNotice[];
  isRefreshing: boolean;
  loadError: string | null;
  onRefresh: () => void | Promise<void>;
}

export interface PayrollNoticeCounts {
  critical: number;
  warning: number;
  info: number;
}

export function getPayrollNoticeCounts(notices: PayrollNotice[]): PayrollNoticeCounts {
  return {
    critical: notices.filter((notice) => notice.severity === 'CRITICAL').length,
    warning: notices.filter((notice) => notice.severity === 'WARNING').length,
    info: notices.filter((notice) => notice.severity === 'INFO').length,
  };
}

export function usePayrollNotices(
  enabled: boolean,
  periodId: string | undefined,
  engine: PayrollEngine,
  dataset: PayrollDatasetSnapshot
) {
  const [notices, setNotices] = useState<PayrollNotice[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!enabled || !periodId) {
      setNotices([]);
      setLoadError(null);
      return;
    }

    setIsRefreshing(true);
    try {
      const next = await engine.getPayrollNotices(periodId, dataset);
      next.sort((a, b) => severityRank[a.severity] - severityRank[b.severity]);
      setNotices(next);
      setLoadError(null);
    } catch (err) {
      console.error('Payroll notices could not be loaded:', err);
      setLoadError('Bordro uyarıları alınamadı.');
    } finally {
      setIsRefreshing(false);
    }
  }, [dataset, enabled, engine, periodId]);

  useEffect(() => {
    void refresh();

    const onDataChanged = () => void refresh();
    const onFocus = () => void refresh();
    window.addEventListener('payroll:data-changed', onDataChanged);
    window.addEventListener('focus', onFocus);

    return () => {
      window.removeEventListener('payroll:data-changed', onDataChanged);
      window.removeEventListener('focus', onFocus);
    };
  }, [refresh]);

  const counts = useMemo(() => getPayrollNoticeCounts(notices), [notices]);

  return { notices, counts, isRefreshing, loadError, refresh };
}

export const PayrollNoticeCenter: React.FC<PayrollNoticeCenterProps> = ({
  notices,
  isRefreshing,
  loadError,
  onRefresh,
}) => {
  const [isOpen, setIsOpen] = useState(true);
  const counts = useMemo(() => getPayrollNoticeCounts(notices), [notices]);

  if (!loadError && notices.length === 0) return null;

  return (
    <section data-testid="payroll-notice-center" className="rounded-2xl border border-slate-200 bg-white shadow-sm">
      <div className="flex items-center justify-between gap-3 border-b border-slate-200 px-4 py-3">
        <button
          type="button"
          onClick={() => setIsOpen((current) => !current)}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          <BellRing className="h-4 w-4 shrink-0 text-indigo-600" />
          <div className="min-w-0">
            <div className="text-xs font-bold text-slate-900">Kontroller</div>
            <div className="mt-0.5 flex flex-wrap gap-2 text-[10px] font-semibold">
              {counts.critical > 0 && <span className="text-rose-700">{counts.critical} kritik</span>}
              {counts.warning > 0 && <span className="text-amber-700">{counts.warning} kontrol</span>}
              {counts.info > 0 && <span className="text-blue-700">{counts.info} bilgi</span>}
              {counts.critical === 0 && counts.warning === 0 && counts.info === 0 && !loadError && (
                <span className="text-emerald-700">Aktif bildirim yok</span>
              )}
            </div>
          </div>
          {isOpen ? (
            <ChevronUp className="ml-auto h-4 w-4 shrink-0 text-slate-400" />
          ) : (
            <ChevronDown className="ml-auto h-4 w-4 shrink-0 text-slate-400" />
          )}
        </button>

        <button
          type="button"
          onClick={() => void onRefresh()}
          disabled={isRefreshing}
          className="rounded-lg p-1.5 text-slate-500 transition-colors hover:bg-slate-100 hover:text-indigo-700 disabled:opacity-50"
          title="Uyarıları yenile"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${isRefreshing ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {isOpen && (
        <div className="max-h-[65vh] space-y-2 overflow-y-auto p-3">
          {loadError && (
            <div className="rounded-xl border border-amber-300 bg-amber-50 p-3 text-xs font-semibold text-amber-900">
              {loadError}
            </div>
          )}

          {notices.map((notice, index) => {
            const style = severityStyles[notice.severity];
            const Icon = style.Icon;
            const label = actionLabel(notice.action);
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
                    {label && (
                      <div className="mt-2 text-[10px] font-bold uppercase tracking-wide opacity-70">
                        {label}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
};
