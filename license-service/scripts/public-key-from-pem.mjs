import { createPublicKey } from 'node:crypto';

let pem = '';
for await (const chunk of process.stdin) pem += chunk;
const key = createPublicKey(pem);
if (key.asymmetricKeyType !== 'ed25519') {
  process.stderr.write('Ed25519 açık anahtarı gerekli.\n');
  process.exitCode = 1;
} else {
  const der = key.export({ type: 'spki', format: 'der' });
  process.stdout.write(`${der.subarray(der.length - 32).toString('base64url')}\n`);
}
pem = '';
