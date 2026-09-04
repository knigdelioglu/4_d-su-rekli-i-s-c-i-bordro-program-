import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, extname, join, relative, resolve } from 'node:path';

const root = process.cwd();
const distDir = resolve(root, 'dist');
const sourceWasm = resolve(root, 'src/wasm/pkg/payroll_wasm_bg.wasm');
const sourceGlue = resolve(root, 'src/wasm/pkg/payroll_wasm.js');

function fail(message) {
  console.error(`web:verify: FAIL — ${message}`);
  process.exitCode = 1;
}

function walk(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

function localReferencePath(reference, fromFile) {
  const cleanReference = reference.split(/[?#]/, 1)[0];
  if (!cleanReference || /^(?:data:|https?:|blob:)/i.test(cleanReference)) return null;
  if (!cleanReference.startsWith('/')) return resolve(dirname(fromFile), cleanReference);

  // Vite's root Netlify build emits /assets/... while the existing Pages
  // workflow may emit /<repository>/assets/... . Resolve either deployment
  // prefix to the same local dist asset, while rejecting external URLs above.
  const directPath = resolve(distDir, `.${cleanReference}`);
  if (existsSync(directPath)) return directPath;
  const assetsStart = cleanReference.indexOf('/assets/');
  return assetsStart >= 0
    ? resolve(distDir, `.${cleanReference.slice(assetsStart)}`)
    : directPath;
}

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

if (!existsSync(join(distDir, 'index.html'))) fail('dist/index.html bulunamadı.');
if (!existsSync(sourceWasm)) fail('tracked source WASM bulunamadı.');
if (!existsSync(sourceGlue)) fail('tracked generated WASM glue bulunamadı.');

const distFiles = walk(distDir);
const javascriptAssets = distFiles.filter((path) => extname(path) === '.js');
const cssAssets = distFiles.filter((path) => extname(path) === '.css');
const wasmAssets = distFiles.filter((path) => extname(path) === '.wasm');

if (javascriptAssets.length === 0) fail('dist içinde JavaScript bundle bulunamadı.');
if (cssAssets.length === 0) fail('dist içinde CSS asset bulunamadı.');
if (wasmAssets.length === 0) fail('dist içinde WASM asset bulunamadı.');
for (const asset of wasmAssets) {
  if (statSync(asset).size === 0) fail(`${relative(root, asset)} 0 byte. `);
}

const indexPath = join(distDir, 'index.html');
const index = readFileSync(indexPath, 'utf8');
if (!/<title>4\/D Bordro<\/title>/i.test(index)) {
  fail('dist/index.html kullanıcıya görünen başlık olarak "4/D Bordro" içermiyor.');
}
if (/(?:AI Studio|Gemini|Google AI)/i.test(index)) {
  fail('dist/index.html eski yapay zekâ/servis markası kalıntısı içeriyor.');
}
if (/navigator\.serviceWorker|serviceWorker\.register|registerSW/i.test(index)) {
  fail('dist/index.html eski metadata taşıyabilecek bir service worker kaydı içeriyor.');
}

const manifestPath = join(distDir, 'manifest.webmanifest');
const faviconPath = join(distDir, 'favicon.svg');
if (!existsSync(manifestPath)) fail('dist manifest.webmanifest bulunamadı.');
if (!existsSync(faviconPath)) fail('dist favicon.svg bulunamadı.');
if (existsSync(manifestPath)) {
  try {
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    if (manifest.name !== '4/D Bordro' || manifest.short_name !== '4/D Bordro') {
      fail('web manifest uygulama adını "4/D Bordro" olarak belirtmiyor.');
    }
  } catch (error) {
    fail(`web manifest JSON olarak okunamadı: ${String(error)}`);
  }
}
if (/(["'`])\/src\//.test(index)) fail('index.html /src/ absolute asset URL içeriyor.');

for (const [, reference] of index.matchAll(/\b(?:src|href)=["']([^"']+)["']/g)) {
  const resolvedReference = localReferencePath(reference, indexPath);
  if (!resolvedReference) {
    fail(`index.html dış veya desteklenmeyen asset referansı içeriyor: ${reference}`);
    continue;
  }
  if (!existsSync(resolvedReference)) {
    fail(`index.html asset referansı dist içinde bulunamadı: ${reference}`);
  }
}

const gluePath = javascriptAssets.find((path) => {
  const source = readFileSync(path, 'utf8');
  return /instantiateStreaming/.test(source) && /\.wasm/.test(source);
});
if (javascriptAssets.some((path) => /navigator\.serviceWorker|serviceWorker\.register|registerSW/i.test(readFileSync(path, 'utf8')))) {
  fail('dist JavaScript bundle içinde eski metadata taşıyabilecek bir service worker kaydı bulundu.');
}
if (!gluePath) {
  fail('Vite output içinde generated WASM glue bundle bulunamadı.');
} else {
  const glue = readFileSync(gluePath, 'utf8');
  const wasmReferences = [...glue.matchAll(/new URL\(["']([^"']+\.wasm)["']/g)].map(
    ([, reference]) => reference
  );
  if (wasmReferences.length === 0) {
    fail('Generated WASM glue içinde relative/import.meta.url WASM referansı bulunamadı.');
  }
  for (const reference of wasmReferences) {
    if (/^(?:https?:)?\/\//i.test(reference) || /\/src\//.test(reference)) {
      fail(`Generated WASM glue güvensiz/yanlış asset URL içeriyor: ${reference}`);
      continue;
    }
    const resolvedReference = localReferencePath(reference, gluePath);
    if (!resolvedReference || !existsSync(resolvedReference)) {
      fail(`Generated WASM glue referansı dist içinde bulunamadı: ${reference}`);
    }
  }
}

const sourceGlueText = readFileSync(sourceGlue, 'utf8');
if (!/new URL\(\s*["']payroll_wasm_bg\.wasm["']\s*,\s*import\.meta\.url\s*\)/.test(sourceGlueText)) {
  fail('tracked generated glue WASM dosyasını import.meta.url ile relative yüklemiyor.');
}

const sourceHash = sha256(sourceWasm);
const matchingWasm = wasmAssets.find((path) => sha256(path) === sourceHash);
if (!matchingWasm) {
  fail('dist WASM asset tracked src/wasm/pkg/payroll_wasm_bg.wasm ile aynı değil.');
}

const netlifyConfigPath = resolve(root, 'netlify.toml');
if (!existsSync(netlifyConfigPath)) {
  fail('netlify.toml bulunamadı.');
} else if (!/Content-Type\s*=\s*["']application\/wasm["']/.test(readFileSync(netlifyConfigPath, 'utf8'))) {
  fail('netlify.toml WASM MIME type application/wasm headerı içermiyor.');
}

if (process.exitCode) process.exit(1);
console.log(
  `web:verify: PASS — ${javascriptAssets.length} JS, ${cssAssets.length} CSS, ${wasmAssets.length} WASM asset; dist WASM tracked artifact ile eşleşiyor.`
);
