import { expect, test } from 'bun:test';
import { queueAuthoritativePayloadCommit, type AuthoritativePayloadCommitQueue } from './authoritativePayloadCommit';

interface TestPayload {
  exportedAt: string;
  attendanceUpdates: string[];
}

test('rebases a bulk attendance commit over a local mutation that arrives while the save is pending', async () => {
  const initial: TestPayload = { exportedAt: 'before', attendanceUpdates: [] };
  let current: TestPayload | null = initial;
  let persisted: TestPayload | null = initial;
  let generation = 0;
  let releaseFirstWrite!: () => void;
  let markWriteStarted!: () => void;
  const firstWriteStarted = new Promise<void>((resolve) => { markWriteStarted = resolve; });
  const firstWriteGate = new Promise<void>((resolve) => { releaseFirstWrite = resolve; });
  let writes = 0;
  const queue: AuthoritativePayloadCommitQueue = { current: Promise.resolve() };
  const commit = queueAuthoritativePayloadCommit(queue, {
    getCurrent: () => current,
    getGeneration: () => generation,
    markDirty: () => { generation += 1; },
    persist: async (payload) => {
      writes += 1;
      if (writes === 1) {
        markWriteStarted();
        await firstWriteGate;
      }
      persisted = payload;
    },
    publish: (payload) => { current = payload; },
    missingPayloadMessage: 'missing test payload',
  }, (payload) => ({
    ...payload,
    attendanceUpdates: [...payload.attendanceUpdates, 'batch-A'],
  }));

  await firstWriteStarted;
  current = { ...current!, attendanceUpdates: [...current!.attendanceUpdates, 'single-B'] };
  generation += 1;
  releaseFirstWrite();
  await commit;

  expect(current?.attendanceUpdates.sort()).toEqual(['batch-A', 'single-B']);
  expect(persisted?.attendanceUpdates.sort()).toEqual(['batch-A', 'single-B']);
  expect(writes).toBe(2);
});

test('serializes concurrent commits and applies each to the preceding authoritative payload', async () => {
  let current: TestPayload | null = { exportedAt: 'before', attendanceUpdates: [] };
  let generation = 0;
  const queue: AuthoritativePayloadCommitQueue = { current: Promise.resolve() };
  const dependencies = {
    getCurrent: () => current,
    getGeneration: () => generation,
    markDirty: () => { generation += 1; },
    persist: async (_payload: TestPayload) => undefined,
    publish: (payload: TestPayload) => { current = payload; },
    missingPayloadMessage: 'missing test payload',
  };
  const commit = (name: string) => queueAuthoritativePayloadCommit(
    queue,
    dependencies,
    (payload) => ({ ...payload, attendanceUpdates: [...payload.attendanceUpdates, name] })
  );

  await Promise.all([commit('person-A'), commit('person-B')]);
  expect(current?.attendanceUpdates.sort()).toEqual(['person-A', 'person-B']);
});

test('does not let an autosave queued during bulk persistence overwrite the rebased commit', async () => {
  let current: TestPayload | null = { exportedAt: 'before', attendanceUpdates: [] };
  let persisted: TestPayload | null = current;
  let generation = 0;
  let releaseFirstWrite!: () => void;
  let markWriteStarted!: () => void;
  const firstWriteStarted = new Promise<void>((resolve) => { markWriteStarted = resolve; });
  const firstWriteGate = new Promise<void>((resolve) => { releaseFirstWrite = resolve; });
  let writes = 0;
  let chain = Promise.resolve();
  const enqueueWrite = (payload: TestPayload, autosaveGeneration?: number) => {
    const task = chain.catch(() => undefined).then(async () => {
      if (autosaveGeneration !== undefined && autosaveGeneration !== generation) return;
      if (writes === 0) {
        markWriteStarted();
        await firstWriteGate;
      }
      writes += 1;
      persisted = payload;
    });
    chain = task.then(() => undefined, () => undefined);
    return task;
  };
  const queue: AuthoritativePayloadCommitQueue = { current: Promise.resolve() };
  const commit = queueAuthoritativePayloadCommit(queue, {
    getCurrent: () => current,
    getGeneration: () => generation,
    markDirty: () => { generation += 1; },
    persist: (payload) => enqueueWrite(payload),
    publish: (payload) => { current = payload; },
    missingPayloadMessage: 'missing test payload',
  }, (payload) => ({
    ...payload,
    attendanceUpdates: [...payload.attendanceUpdates, 'bulk-A'],
  }));

  await firstWriteStarted;
  current = { ...current!, attendanceUpdates: [...current!.attendanceUpdates, 'single-B'] };
  generation += 1;
  const autosaveGeneration = generation;
  const autosave = enqueueWrite(current, autosaveGeneration);
  releaseFirstWrite();
  await Promise.all([commit, autosave]);

  expect(current?.attendanceUpdates.sort()).toEqual(['bulk-A', 'single-B']);
  expect(persisted?.attendanceUpdates.sort()).toEqual(['bulk-A', 'single-B']);
  expect(writes).toBeGreaterThan(1);
});

test('does not publish a bulk commit when durable persistence fails', async () => {
  const original: TestPayload = { exportedAt: 'before', attendanceUpdates: [] };
  let current: TestPayload | null = original;
  let published = false;
  const queue: AuthoritativePayloadCommitQueue = { current: Promise.resolve() };
  const commit = queueAuthoritativePayloadCommit(queue, {
    getCurrent: () => current,
    getGeneration: () => 0,
    markDirty: () => undefined,
    persist: async () => { throw new Error('IndexedDB write failed'); },
    publish: (payload) => { current = payload; published = true; },
    missingPayloadMessage: 'missing test payload',
  }, (payload) => ({ ...payload, attendanceUpdates: ['batch-A'] }));

  await expect(commit).rejects.toThrow('IndexedDB write failed');
  expect(current).toBe(original);
  expect(published).toBe(false);
});
