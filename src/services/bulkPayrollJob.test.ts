import { describe, expect, test } from 'bun:test';
import { BulkPayrollJobStore, type BulkPayrollJob } from './bulkPayrollJob';
const job = (): BulkPayrollJob => ({ version: 1, id: 'job', periodId: 'period', accrualType: 'TEDIYE', status: 'paused', items: [
  { personnelId: 'a', status: 'pending' }, { personnelId: 'b', status: 'pending' },
] });
function storage() { let value: string | null = null; return { getItem: () => value, setItem: (_key: string, next: string) => { value = next; } }; }

describe('bulk payroll lifecycle', () => {
  test('locks before the first await, keeps progress without subscribers, and blocks manual duplicates', async () => {
    const disk = storage(), store = new BulkPayrollJobStore(() => disk);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const active = store.run(job(), async () => { await gate; return { status: 'success' }; });
    expect(store.getSnapshot().busy).toBe(true);
    expect(await store.run(job(), async () => ({ status: 'success' }))).toBe(false);
    await expect(store.runManual(async () => undefined)).rejects.toThrow('sürüyor');
    const reloaded = new BulkPayrollJobStore(() => disk).getSnapshot();
    expect(reloaded.job?.status).toBe('paused');
    expect(reloaded.job?.items[0].status).toBe('pending');
    release(); await active;
    expect(store.getSnapshot().job?.items.map((item) => item.status)).toEqual(['success', 'success']);
    expect(store.getSnapshot().busy).toBe(false);
  });
  test('retries only unfinished people after restart and preserves deterministic accrual intent', async () => {
    const disk = storage(), store = new BulkPayrollJobStore(() => disk);
    await store.run(job(), async (item) => {
      if (item.personnelId === 'b') {
        store.checkpoint('b', { accrual: { accrualId: 'bulk_job_b', accrualType: 'TEDIYE', paymentDate: '2027-01-31', sequence: 2, grossAmount: '123.45', description: null } });
        throw new Error('Yazma başarısız');
      }
      return { status: 'success' };
    });
    const restored = new BulkPayrollJobStore(() => disk), calls: string[] = [];
    expect(restored.getSnapshot().job?.status).toBe('paused');
    await restored.run(restored.getSnapshot().job!, async (item) => {
      calls.push(item.personnelId); expect(item.accrual?.accrualId).toBe('bulk_job_b');
      return { status: 'success' };
    });
    expect(calls).toEqual(['b']);
    expect(restored.getSnapshot().job?.status).toBe('completed');
  });
  test('does not start financial writes when checkpoint persistence fails', async () => {
    const store = new BulkPayrollJobStore(() => ({ getItem: () => null, setItem: () => { throw new Error('Disk full'); } }));
    let calls = 0;
    expect(await store.run(job(), async () => { calls++; return { status: 'success' }; })).toBe(false);
    expect(calls).toBe(0);
    expect(store.getSnapshot().busy).toBe(false);
    expect(store.getSnapshot().error).toContain('İlerleme kaydedilemedi');
  });
});

test('dataset replacement clears the completed job in memory and persistent storage', async () => {
  const disk = storage(), store = new BulkPayrollJobStore(() => disk);
  await store.run(job(), async () => ({ status: 'success' }));
  await store.runManual(async () => { store.clearForDatasetReplacement(); });
  expect(store.getSnapshot().job).toBeNull();
  expect(new BulkPayrollJobStore(() => disk).getSnapshot().job).toBeNull();
});
