import type { BlobEntry, BlobListItem, BlobStore } from '../netlify/lib/storage';

export class AtomicBlobEmulator implements BlobStore {
  private readonly entries = new Map<string, { data: unknown; etag: number }>();
  private nextEtag = 1;

  async getWithMetadata(key: string): Promise<{ data: unknown; etag: string } | null> {
    const item = this.entries.get(key);
    return item ? { data: structuredClone(item.data), etag: String(item.etag) } : null;
  }

  async setJSON(key: string, value: unknown, options: { onlyIfNew?: boolean; onlyIfMatch?: string } = {}): Promise<{ modified: boolean; etag?: string }> {
    const existing = this.entries.get(key);
    if (options.onlyIfNew && existing) return { modified: false };
    if (options.onlyIfMatch !== undefined && (!existing || String(existing.etag) !== options.onlyIfMatch)) {
      return { modified: false };
    }
    const etag = this.nextEtag++;
    this.entries.set(key, { data: structuredClone(value), etag });
    return { modified: true, etag: String(etag) };
  }

  async delete(key: string): Promise<void> { this.entries.delete(key); }

  async list({ prefix = '' }: { prefix?: string } = {}): Promise<{ blobs: BlobListItem[] }> {
    return { blobs: [...this.entries.entries()]
      .filter(([key]) => key.startsWith(prefix))
      .map(([key, value]) => ({ key, etag: String(value.etag) })) };
  }

  snapshot(): Map<string, BlobEntry> {
    return new Map([...this.entries].map(([key, item]) => [key, { data: structuredClone(item.data), etag: String(item.etag) }]));
  }
}
