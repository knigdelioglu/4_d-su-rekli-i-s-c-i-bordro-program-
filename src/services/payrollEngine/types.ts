import {
  AnnualPayrollParameters,
  BordroDonemi,
  BordroKaydi,
  PayrollPeriodInstitutionSettings,
  ManualPayrollIncomeInput,
  PayrollAccrualInput,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  CompensationRevision,
  CompensationRevisionOverride,
  CompensationPolicyVersion,
  RetroAdjustmentBatch,
  RetroAllocation,
  SickLeaveRecord,
  PayrollCalculationMetadata,
} from '../../types/payroll';
import { PayrollNotice } from '../../types/payrollNotice';
import type { Exactify } from './decimalBoundary';

/** UI/native compatibility shape before the explicit Decimal boundary adapter. */
export interface PayrollDatasetSnapshotModel {
  personnel: Personel[];
  periods: BordroDonemi[];
  institutionSettings: Record<string, PayrollRuntimeInstitutionSettings>;
  attendances: PersonelPuantaj[];
  payrolls: BordroKaydi[];
  taxOpenings: PersonelTaxOpening[];
  sickLeaveRecords: SickLeaveRecord[];
  annualPayrollParameters: AnnualPayrollParameters[];
  compensationRevisions: CompensationRevision[];
  compensationRevisionOverrides: CompensationRevisionOverride[];
  retroBatches: RetroAdjustmentBatch[];
  retroAllocations: RetroAllocation[];
  /** May be absent only in legacy dataset JSON; Rust deserialization treats that as an empty collection. */
  compensationPolicyVersions: CompensationPolicyVersion[];
}

/** Period inputs allowed into calculation requests; NORMAL compensation comes only from the policy timeline. */
export type PayrollRuntimeInstitutionSettings = PayrollPeriodInstitutionSettings;

/** Presentation-only snapshot shape; never use it for WASM or persistence. */
export type PayrollUiModel = PayrollDatasetSnapshotModel;

/** Exact JSON shape sent to WASM and retained by browser persistence. */
export type PayrollDatasetSnapshot = Exactify<PayrollDatasetSnapshotModel>;
export type PayrollBoundaryPayroll = Exactify<BordroKaydi>;
export type PayrollBoundaryCalculationMetadata = Exactify<PayrollCalculationMetadata>;
export type PayrollBoundaryManualIncomeInput = Exactify<ManualPayrollIncomeInput>;
export type PayrollBoundaryAccrualInput = Exactify<PayrollAccrualInput>;
export type PayrollBoundaryPersonel = Exactify<Personel>;
export type PayrollBoundaryTaxOpening = Exactify<PersonelTaxOpening>;

export interface RetroPeriodPreview {
  sourcePeriodId: string;
  originalRecognizedAmount: number;
  previousAuthoritativeRetroAmount: number;
  targetAmount: number;
  deltaAmount: number;
}

export interface RetroCalculationResultModel {
  batch: RetroAdjustmentBatch;
  allocations: RetroAllocation[];
  periods: RetroPeriodPreview[];
}

export type RetroCalculationResult = Exactify<RetroCalculationResultModel>;

export interface RetroCalculationRequestModel {
  batchId: string;
  revision: CompensationRevision;
  overrides: CompensationRevisionOverride[];
  personnelId: string;
  paymentDate: string;
  calculatedAt: string;
  description?: string | null;
  dataset: PayrollDatasetSnapshotModel;
}

export type RetroCalculationRequest = Exactify<RetroCalculationRequestModel>;

export interface PayrollCalculationRequest {
  personnelId: string;
  periodId: string;
  calculatedAt: string;
  manualIncome?: PayrollBoundaryManualIncomeInput | null;
  accrual?: PayrollBoundaryAccrualInput | null;
  dataset: PayrollDatasetSnapshot;
}

export type PayrollMutation =
  | { kind: 'PERSON'; personnelId: string }
  | { kind: 'PERSON_PERIOD'; personnelId: string; periodId: string }
  | { kind: 'PERSON_TAX_YEAR'; personnelId: string; taxYear: number }
  | { kind: 'TAX_YEAR'; taxYear: number }
  | { kind: 'PERIOD'; periodId: string }
  | { kind: 'COMPENSATION_POLICY'; effectiveFrom: string; throughInclusive: string | null }
  | { kind: 'PERIOD_FROM_POSITION'; startDate: string; taxYear: number; taxMonth: number }
  | { kind: 'PERIOD_INSERT'; startDate: string; taxYear: number; taxMonth: number }
  | { kind: 'PERSON_FROM_DATE'; personnelId: string; effectiveFrom: string }
  | { kind: 'PAYROLL_CALCULATION'; personnelId: string; periodId: string }
  | { kind: 'ACCRUAL_DELETE'; personnelId: string; periodId: string; accrualId: string }
  | { kind: 'ACCRUAL_CALCULATION'; personnelId: string; periodId: string; accrualId: string }
  | {
      kind: 'ACCRUAL_INSERT';
      personnelId: string;
      periodId: string;
      accrualId: string;
      paymentDate: string;
      sequence: number;
    }
  | {
      kind: 'RETRO_BATCH_SAVE';
      personnelId: string;
      batchId: string;
      paymentDate: string;
    }
  | { kind: 'ALL' };

export interface PayrollKey {
  personnelId: string;
  periodId: string;
  accrualId?: string;
}

export interface MutationImpact {
  affectedPayrolls: PayrollKey[];
  blockedByFinalized: PayrollKey[];
  affectedRetroBatches: string[];
  blockedByFinalizedRetroBatches: string[];
  payrollCauses?: Array<{ payroll: PayrollKey; causePeriodId: string }>;
}

export interface PayrollEngine {
  readonly kind: 'tauri' | 'wasm';
  calculatePayroll(request: PayrollCalculationRequest): Promise<PayrollBoundaryPayroll>;
  /** Explicit policy API retained for compatibility; ordinary calculation uses the same policy resolver. */
  calculatePayrollWithCompensationPolicy(
    request: PayrollCalculationRequest
  ): Promise<PayrollBoundaryPayroll>;
  calculateRetroPreview(request: RetroCalculationRequest): Promise<RetroCalculationResult>;
  validatePayroll(request: PayrollCalculationRequest): Promise<void>;
  getPayrollNotices(
    periodId: string,
    dataset: PayrollDatasetSnapshot
  ): Promise<PayrollNotice[]>;
  getPayrolls(dataset: PayrollDatasetSnapshot): Promise<PayrollBoundaryPayroll[]>;
  finalizePayroll(
    personnelId: string,
    periodId: string,
    dataset: PayrollDatasetSnapshot,
    accrualId?: string
  ): Promise<PayrollBoundaryPayroll>;
  evaluateMutationPolicy(
    mutation: PayrollMutation,
    dataset: PayrollDatasetSnapshot
  ): Promise<MutationImpact>;
}
