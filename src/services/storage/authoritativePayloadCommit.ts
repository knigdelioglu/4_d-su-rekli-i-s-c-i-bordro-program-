export interface AuthoritativePayloadCommitQueue {
  current: Promise<void>;
}

export interface AuthoritativePayloadCommitDependencies<T extends { exportedAt: string }> {
  getCurrent: () => T | null;
  getGeneration: () => number;
  markDirty: () => void;
  persist: (payload: T) => Promise<void>;
  publish: (payload: T) => void;
  missingPayloadMessage: string;
}

/** Serializes bulk commits and rebases them when another local mutation lands during persistence. */
export function queueAuthoritativePayloadCommit<T extends { exportedAt: string }>(
  queue: AuthoritativePayloadCommitQueue,
  dependencies: AuthoritativePayloadCommitDependencies<T>,
  update: (current: T) => T | Promise<T>
): Promise<void> {
  const task = queue.current.catch(() => undefined).then(async () => {
    for (;;) {
      const current = dependencies.getCurrent();
      if (!current) throw new Error(dependencies.missingPayloadMessage);
      dependencies.markDirty();
      const generation = dependencies.getGeneration();
      const next = { ...(await update(current)), exportedAt: new Date().toISOString() };
      await dependencies.persist(next);
      if (generation !== dependencies.getGeneration()) continue;
      dependencies.publish(next);
      return;
    }
  });
  queue.current = task.then(() => undefined, () => undefined);
  return task;
}
