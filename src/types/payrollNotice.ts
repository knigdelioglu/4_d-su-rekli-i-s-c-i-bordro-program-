export type PayrollNoticeSeverity = 'INFO' | 'WARNING' | 'CRITICAL' | 'SUCCESS';
export type PayrollNoticeScope = 'PERIOD' | 'PERSONNEL';
export type PayrollNoticeAction =
  | 'GO_TO_PUANTAJ'
  | 'RECALCULATE_PAYROLL'
  | 'CHECK_ANNUAL_PARAMETERS'
  | 'CHECK_PERIOD_PARAMETERS'
  | 'CHECK_RAISE_PARAMETERS'
  | 'CHECK_SICK_LEAVE'
  | 'REVIEW_PEK'
  | 'REVIEW_TAX_DETAIL';

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
