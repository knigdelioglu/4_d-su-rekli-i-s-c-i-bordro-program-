import { createHash, randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BlobListItem, BlobStore } from '../netlify/lib/storage';

interface FileRecord { key: string; data: unknown; etag: string }

export class FileAtomicBlobEmulator implements BlobStore {
  private static readonly locks = new Map<string, Promise<void>>();

  private constructor(readonly directory: string) {}

  static async create(): Promise<FileAtomicBlobEmulator> {
    const directory = await mkdtemp(join(tmpdir(), '4d-license-blobs-'));
    await mkdir(join(directory, 'records'));
    return new FileAtomicBlobEmulator(directory);
  }

  static async open(directory: string): Promise<FileAtomicBlobEmulator> {
    await mkdir(join(directory, 'records'), { recursive: true });
    return new FileAtomicBlobEmulator(directory);
  }

  private recordPath(key: string): string {
    return join(this.directory, 'records', `${Buffer.from(key).toString('base64url')}.json`);
  }

  private async readRecord(key: string): Promise<FileRecord | null> {
    try { return JSON.parse(await readFile(this.recordPath(key), 'utf8')) as FileRecord; }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null;
      throw error;
    }
  }

  private async withKeyLock<T>(key: string, work: () => Promise<T>): Promise<T> {
    const lockKey = this.recordPath(key);
    const previous = FileAtomicBlobEmulator.locks.get(lockKey) ?? Promise.resolve();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const tail = previous.then(() => gate);
    FileAtomicBlobEmulator.locks.set(lockKey, tail);
    await previous;
    try { return await work(); }
    finally {
      release();
      if (FileAtomicBlobEmulator.locks.get(lockKey) === tail) FileAtomicBlobEmulator.locks.delete(lockKey);
    }
  }

  async getWithMetadata(key: string): Promise<{ data: unknown; etag: string } | null> {
    const record = await this.readRecord(key);
    return record ? { data: record.data, etag: record.etag } : null;
  }

  async setJSON(key: string, value: unknown, options: { onlyIfNew?: boolean; onlyIfMatch?: string } = {}): Promise<{ modified: boolean; etag?: string }> {
    return this.withKeyLock(key, async () => {
      const current = await this.readRecord(key);
      if (options.onlyIfNew && current) return { modified: false };
      if (options.onlyIfMatch !== undefined && current?.etag !== options.onlyIfMatch) return { modified: false };
      const serialized = JSON.stringify(value);
      const etag = createHash('sha256').update(serialized).digest('hex');
      const record: FileRecord = { key, data: value, etag };
      const target = this.recordPath(key);
      const temporary = `${target}.tmp-${randomUUID()}`;
      await writeFile(temporary, JSON.stringify(record), { mode: 0o600 });
      await rename(temporary, target);
      return { modified: true, etag };
    });
  }

  async delete(key: string): Promise<void> {
    await this.withKeyLock(key, async () => {
      await rm(this.recordPath(key), { force: true });
    });
  }

  async list({ prefix = '' }: { prefix?: string } = {}): Promise<{ blobs: BlobListItem[] }> {
    const files = await readdir(join(this.directory, 'records'));
    const records = await Promise.all(files.map(async (file) => {
      try { return JSON.parse(await readFile(join(this.directory, 'records', file), 'utf8')) as FileRecord; }
      catch { return null; }
    }));
    return { blobs: records.filter((item): item is FileRecord => Boolean(item) && item!.key.startsWith(prefix)).map(({ key, etag }) => ({ key, etag })) };
  }

  async dispose(): Promise<void> { await rm(this.directory, { recursive: true, force: true }); }
}
