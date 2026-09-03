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
  manualIncome?: ManualPayrollIncomeInput | null;
  dataset: PayrollDatasetSnapshot;
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
  setPayrollStatus(
    personnelId: string,
    periodId: string,
    status: BordroKaydi['status'],
    dataset: PayrollDatasetSnapshot
  ): Promise<BordroKaydi>;
}
