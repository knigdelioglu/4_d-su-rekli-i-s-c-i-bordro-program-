export interface BlobEntry<T = unknown> {
  data: T;
  etag: string;
}

export interface BlobListItem {
  key: string;
  etag?: string;
}

export interface BlobStore {
  getWithMetadata(
    key: string,
    options?: { consistency?: 'strong' | 'eventual'; type?: 'json' }
  ): Promise<{ data: unknown; etag: string } | null>;
  setJSON(
    key: string,
    value: unknown,
    options?: { onlyIfNew?: boolean; onlyIfMatch?: string }
  ): Promise<{ modified: boolean; etag?: string }>;
  delete(key: string): Promise<void>;
  list(options?: { prefix?: string }): Promise<{ blobs: BlobListItem[] }>;
}

export interface StoreFactory {
  (name: string, options: { consistency: 'strong' }): BlobStore;
}

export function namespaceForContext(contextName: string | undefined): string {
  if (contextName === 'production') return 'production';
  if (contextName === 'deploy-preview' || contextName === 'branch-deploy') return 'preview';
  if (contextName === 'test') return 'test';
  return 'development';
}

export function licenseStoreName(contextName: string | undefined): string {
  return `4d-license-${namespaceForContext(contextName)}`;
}

export function createStore(factory: StoreFactory, contextName: string | undefined): BlobStore {
  return factory(licenseStoreName(contextName), { consistency: 'strong' });
}

export async function readEntry<T>(store: BlobStore, key: string): Promise<BlobEntry<T> | null> {
  const entry = await store.getWithMetadata(key, { consistency: 'strong' });
  if (!entry) return null;
  const data = typeof entry.data === 'string' ? JSON.parse(entry.data) : entry.data;
  return { data: data as T, etag: entry.etag };
}

export async function readJSON<T>(store: BlobStore, key: string): Promise<T | null> {
  return (await readEntry<T>(store, key))?.data ?? null;
}
