export type PayrollNoticeSeverity = 'INFO' | 'WARNING' | 'CRITICAL' | 'SUCCESS';
export type PayrollNoticeScope = 'PERIOD' | 'PERSONNEL';
export type PayrollNoticeAction = 'GO_TO_PUANTAJ' | 'RECALCULATE_PAYROLL';

export interface PayrollNotice {
  code: string;
  severity: PayrollNoticeSeverity;
  scope: PayrollNoticeScope;
  personnelId?: string | null;
  title: string;
  message: string;
  details: string[];
  action?: PayrollNoticeAction | null;
}
