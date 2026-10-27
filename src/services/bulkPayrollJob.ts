import type { PayrollBoundaryAccrualInput } from './payrollEngine';

export type BulkItemStatus = 'pending' | 'running' | 'success' | 'error' | 'skipped';
export interface BulkPayrollItem {
  personnelId: string;
  status: BulkItemStatus;
  message?: string;
  accrual?: PayrollBoundaryAccrualInput;
}
export interface BulkPayrollJob {
  version: 1;
  id: string;
  periodId: string;
  accrualType: 'NORMAL' | 'TEDIYE' | 'TIS_IKRAMIYE' | 'SUPPLEMENTAL';
  status: 'paused' | 'running' | 'completed';
  items: BulkPayrollItem[];
  paymentDate?: string;
  grossAmount?: string;
  description?: string;
}
export interface BulkJobSnapshot { job: BulkPayrollJob | null; busy: boolean; error: string | null }
const KEY = '4d-bordro-bulk-job-v1';
const empty: BulkJobSnapshot = { job: null, busy: false, error: null };

/** Stores only the operation intent/checkpoints. Payroll authority remains in the existing repositories. */
export class BulkPayrollJobStore {
  private snapshot: BulkJobSnapshot = empty;
  private loaded = false;
  private listeners = new Set<() => void>();
  constructor(private storage: () => Pick<Storage, 'getItem' | 'setItem'> | undefined = () =>
    typeof localStorage === 'undefined' ? undefined : localStorage) {}
  getSnapshot = (): BulkJobSnapshot => {
    if (!this.loaded) {
      this.loaded = true;
      try {
        const raw = this.storage()?.getItem(KEY);
        if (raw) {
          const job = JSON.parse(raw) as BulkPayrollJob;
          if (job.version !== 1 || !job.id || !job.periodId || !Array.isArray(job.items)
            || !['NORMAL', 'TEDIYE', 'TIS_IKRAMIYE', 'SUPPLEMENTAL'].includes(job.accrualType)
            || !job.items.every((item) => typeof item.personnelId === 'string'
              && ['pending', 'running', 'success', 'error', 'skipped'].includes(item.status))) throw new Error();
          this.snapshot = { job: { ...job, status: job.status === 'running' ? 'paused' : job.status,
            items: job.items.map((item) => item.status === 'running' ? { ...item, status: 'pending' } : item) }, busy: false, error: null };
        }
      } catch {
        this.snapshot = { ...empty, error: 'Önceki toplu işlemin ilerleme kaydı okunamadı. Kayıtlı bordroları kontrol ederek yeni işlem başlatın.' };
      }
    }
    return this.snapshot;
  };
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private publish(job: BulkPayrollJob, busy = this.snapshot.busy) {
    // If checkpointing fails, stop before the next financial write.
    this.storage()?.setItem(KEY, JSON.stringify(job));
    this.snapshot = { job, busy, error: null };
    this.listeners.forEach((listener) => listener());
  }
  checkpoint = (personnelId: string, patch: Partial<BulkPayrollItem>) => {
    const job = this.snapshot.job;
    if (!job) throw new Error('Toplu işlem kaydı bulunamadı.');
    this.publish({ ...job, items: job.items.map((item) => item.personnelId === personnelId ? { ...item, ...patch } : item) });
  };
  async runManual<T>(work: () => Promise<T>): Promise<T> {
    if (this.getSnapshot().busy) throw new Error('Başka bir bordro hesaplaması sürüyor. Tamamlanmasını bekleyin.');
    this.snapshot = { ...this.snapshot, busy: true };
    this.listeners.forEach((listener) => listener());
    try {
      if (typeof navigator !== 'undefined' && navigator.locks) {
        return await navigator.locks.request('4d-bordro-bulk-calculation', { ifAvailable: true }, async (lock) => {
          if (!lock) throw new Error('Başka bir pencerede bordro hesaplaması sürüyor.');
          return work();
        });
      }
      return await work();
    } finally {
      this.snapshot = { ...this.snapshot, busy: false };
      this.listeners.forEach((listener) => listener());
    }
  }
  close() {
    if (this.getSnapshot().busy || !this.snapshot.job) return;
    this.publish({ ...this.snapshot.job, status: 'completed' }, false);
  }
  async run(job: BulkPayrollJob, execute: (item: BulkPayrollItem, job: BulkPayrollJob) => Promise<{ status: 'success' | 'skipped'; message?: string }>): Promise<boolean> {
    if (this.getSnapshot().busy) return false;
    this.snapshot = { ...this.snapshot, busy: true };
    this.listeners.forEach((listener) => listener());
    const work = async () => {
      try {
        this.publish({ ...job, status: 'running' }, true);
        for (const item of job.items) {
          if (item.status === 'success' || item.status === 'skipped') continue;
          this.checkpoint(item.personnelId, { status: 'running', message: undefined });
          let result: { status: 'success' | 'skipped'; message?: string };
          try { result = await execute(item, this.snapshot.job!); }
          catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            this.checkpoint(item.personnelId, { status: 'error', message });
            continue;
          }
          this.checkpoint(item.personnelId, result);
        }
        const latest = this.snapshot.job!;
        this.publish({ ...latest, status: latest.items.some((item) => item.status === 'error') ? 'paused' : 'completed' }, false);
        return true;
      } catch {
        this.snapshot = { ...this.snapshot, busy: false,
          job: this.snapshot.job ? { ...this.snapshot.job, status: 'paused' } : null,
          error: 'Toplu işlem güvenle durduruldu. İlerleme kaydedilemedi; depolama alanını kontrol edip işlemi sürdürün.' };
        this.listeners.forEach((listener) => listener());
        return false;
      }
    };
    // A single writer across browser tabs as well as across mounted views.
    if (typeof navigator !== 'undefined' && navigator.locks) {
      try {
        return await navigator.locks.request('4d-bordro-bulk-calculation', { ifAvailable: true }, async (lock) => {
          if (lock) return work();
          this.snapshot = { ...this.snapshot, busy: false, error: 'Başka bir pencerede toplu hesaplama sürüyor. İşlem tamamlandıktan sonra tekrar deneyin.' };
          this.listeners.forEach((listener) => listener());
          return false;
        });
      } catch {
        this.snapshot = { ...this.snapshot, busy: false, error: 'Hesaplama kilidi alınamadı. Uygulamayı yeniden açıp tekrar deneyin.' };
        this.listeners.forEach((listener) => listener());
        return false;
      }
    }
    return work();
  }
}
export const bulkPayrollJobs = new BulkPayrollJobStore();
