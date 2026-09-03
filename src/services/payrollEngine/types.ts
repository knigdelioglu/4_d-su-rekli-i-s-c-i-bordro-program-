import {
  AnnualPayrollParameters,
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  ManualPayrollIncomeInput,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  SickLeaveRecord,
} from '../../types/payroll';
import { PayrollNotice } from '../../types/payrollNotice';

/** JSON shape shared by the native snapshot adapter and the WASM boundary. */
export interface PayrollDatasetSnapshot {
  personnel: Personel[];
  periods: BordroDonemi[];
  institutionSettings: Record<string, DönemselKurumDegerleri>;
  attendances: PersonelPuantaj[];
  payrolls: BordroKaydi[];
  taxOpenings: PersonelTaxOpening[];
  sickLeaveRecords: SickLeaveRecord[];
  annualPayrollParameters: AnnualPayrollParameters[];
  zamAylari: number[];
}

export interface PayrollCalculationRequest {
  personnelId: string;
  periodId: string;
  calculatedAt: string;
  manualIncome?: ManualPayrollIncomeInput | null;
  dataset: PayrollDatasetSnapshot;
}

export type PayrollMutation =
  | { kind: 'PERSON'; personnelId: string }
  | { kind: 'PERSON_PERIOD'; personnelId: string; periodId: string }
  | { kind: 'PERSON_TAX_YEAR'; personnelId: string; taxYear: number }
  | { kind: 'TAX_YEAR'; taxYear: number }
  | { kind: 'PERIOD'; periodId: string }
  | { kind: 'PERIOD_FROM_POSITION'; startDate: string; taxYear: number; taxMonth: number }
  | { kind: 'PERSON_FROM_DATE'; personnelId: string; effectiveFrom: string }
  | { kind: 'PAYROLL_CALCULATION'; personnelId: string; periodId: string }
  | { kind: 'ALL' };

export interface PayrollKey {
  personnelId: string;
  periodId: string;
}

export interface MutationImpact {
  affectedPayrolls: PayrollKey[];
  blockedByFinalized: PayrollKey[];
}

export interface PayrollEngine {
  readonly kind: 'tauri' | 'wasm';
  calculatePayroll(request: PayrollCalculationRequest): Promise<BordroKaydi>;
  validatePayroll(request: PayrollCalculationRequest): Promise<void>;
  getPayrollNotices(
    periodId: string,
    dataset: PayrollDatasetSnapshot
  ): Promise<PayrollNotice[]>;
  getPayrolls(dataset: PayrollDatasetSnapshot): Promise<BordroKaydi[]>;
  finalizePayroll(
    personnelId: string,
    periodId: string,
    dataset: PayrollDatasetSnapshot
  ): Promise<BordroKaydi>;
  evaluateMutationPolicy(
    mutation: PayrollMutation,
    dataset: PayrollDatasetSnapshot
  ): Promise<MutationImpact>;
}
