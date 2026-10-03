import { randomBytes, scryptSync } from 'node:crypto';

let password = '';
for await (const chunk of process.stdin) password += chunk;
password = password.replace(/[\r\n]+$/, '');
if (!password) {
  process.stderr.write('Parola boş olamaz.\n');
  process.exitCode = 1;
} else {
  const salt = randomBytes(16);
  const hash = scryptSync(password, salt, 32);
  process.stdout.write(`scrypt$${salt.toString('base64url')}$${hash.toString('base64url')}\n`);
}
password = '';
