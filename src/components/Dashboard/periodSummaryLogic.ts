import type { ParametreSection, PayrollViewType, TabType } from '../../types/navigation';

export interface PeriodSummaryDecisionState {
  periodParametersReady: boolean;
  annualParametersReady: boolean;
  missingAttendanceCount: number;
  staleNormalCount: number;
  missingNormalCount: number;
  missingTis: number;
  missingTediye: number;
  finalizedPending: number;
}

export interface PeriodSummaryNextTask {
  label: string;
  tab: TabType;
  payrollView?: PayrollViewType;
  parametreSection?: ParametreSection;
}

export function getPeriodSummaryNextTask(
  summary: PeriodSummaryDecisionState
): PeriodSummaryNextTask | null {
  if (!summary.periodParametersReady) {
    return {
      label: 'Dönem ücret parametrelerini tamamla',
      tab: 'parametrelar',
      parametreSection: 'gelir',
    };
  }
  if (!summary.annualParametersReady) {
    return {
      label: 'Yıllık vergi tarifesini tamamla',
      tab: 'parametrelar',
      parametreSection: 'annualTax',
    };
  }
  if (summary.missingAttendanceCount > 0) {
    return { label: 'Eksik puantajları tamamla', tab: 'puantaj' };
  }
  if (summary.staleNormalCount > 0) {
    return {
      label: 'Yeniden hesaplanması gereken bordrolara git',
      tab: 'bordro',
      payrollView: 'normal',
    };
  }
  if (summary.missingNormalCount > 0) {
    return { label: 'Normal maaşları hesapla', tab: 'bordro', payrollView: 'normal' };
  }
  if (summary.missingTis > 0) {
    return {
      label: 'Bekleyen TİS ikramiyelerine git',
      tab: 'bordro',
      payrollView: 'tis',
    };
  }
  if (summary.missingTediye > 0) {
    return {
      label: 'Bekleyen tediye kayıtlarına git',
      tab: 'bordro',
      payrollView: 'tediye',
    };
  }
  if (summary.finalizedPending > 0) {
    return { label: 'Bordroları kesinleştir', tab: 'bordro', payrollView: 'normal' };
  }
  return null;
}
