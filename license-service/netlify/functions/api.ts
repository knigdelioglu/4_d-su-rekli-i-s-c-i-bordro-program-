import { getStore } from '@netlify/blobs';
import { handleLicenseRequest, type ServiceContext } from '../lib/service';
import { createStore, type StoreFactory } from '../lib/storage';

const factory = ((name, options) => getStore({ name, consistency: options.consistency })) as StoreFactory;

export default async (request: Request, context: ServiceContext) => {
  const contextName = context.deploy?.context ?? process.env.CONTEXT;
  const store = createStore(factory, contextName);
  try {
    return await handleLicenseRequest(request, {
      store,
      env: process.env,
      clientIp: context.ip,
    });
  } catch (cause) {
    // Keep secrets and raw storage errors out of responses.
    console.error('License service request failed', cause instanceof Error ? cause.name : 'unknown');
    return new Response(JSON.stringify({ error: 'service_unavailable', message: 'Lisans servisine şu anda erişilemiyor.' }), {
      status: 503,
      headers: { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' },
    });
  }
};

export const config = { path: '/api/license/*' };
