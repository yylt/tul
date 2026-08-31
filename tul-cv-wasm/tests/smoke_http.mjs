// End-to-end: load /tul_cv page, extract the glue import path, fetch the
// glue from the worker, and drive it with the wasm fetched the way the
// browser would (new URL('tul_cv_wasm_bg.wasm', import.meta.url)).
// Usage: WORKER_URL=http://localhost:8787 node tul-cv-wasm/tests/smoke_http.mjs
import { mkdirSync, writeFileSync } from 'fs';
import path from 'path';
import { fileURLToPath, pathToFileURL } from 'url';

const base = process.env.WORKER_URL || 'http://localhost:8787';
const here = path.dirname(fileURLToPath(import.meta.url));

async function get(p) {
  const r = await fetch(base + p);
  if (!r.ok) throw new Error(p + ' -> ' + r.status);
  return r;
}

// 1. page loads and references the absolute asset path
const page = await (await get('/tul_cv')).text();
const m = page.match(/from '(\/tul_cv\/tul_cv_wasm\.js)'/);
if (!m) throw new Error('page does not import /tul_cv/tul_cv_wasm.js');
console.log('page import:', m[1]);

// 2. glue js served
const glueSrc = await (await get(m[1])).text();
console.log('glue bytes:', glueSrc.length);

// 3. glue resolves wasm via new URL('tul_cv_wasm_bg.wasm', import.meta.url)
//    -> relative to /tul_cv/tul_cv_wasm.js
const wasmPath = m[1].replace(/[^/]+$/, 'tul_cv_wasm_bg.wasm');
console.log('wasm path:', wasmPath);
const wasmBytes = new Uint8Array(await (await get(wasmPath)).arrayBuffer());
console.log('wasm bytes:', wasmBytes.length, 'magic:', String.fromCharCode(...wasmBytes.slice(0, 4)));

// 4. drive the glue in Node with a fetch shim; the glue fetches the wasm by
//    resolving 'tul_cv_wasm_bg.wasm' against its own URL (/tul_cv/...)
globalThis.fetch = async (url) => {
  const u = String(url);
  if (u.endsWith('tul_cv_wasm_bg.wasm')) {
    return new Response(wasmBytes, { headers: { 'content-type': 'application/wasm' } });
  }
  throw new Error('unexpected fetch: ' + u);
};

// write the served glue to a temp file whose URL ends with /tul_cv_wasm.js,
// mirroring how the browser sees import.meta.url
import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
const glueDir = mkdtempSync(path.join(tmpdir(), 'tulcv-'));
const glueFile = path.join(glueDir, 'tul_cv_wasm.js');
writeFileSync(glueFile, glueSrc);
const mod = await import(pathToFileURL(glueFile).href);
await mod.default();
const out = mod.convert_unit('length', 1, 'km', 'm');
if (out !== 1000) throw new Error('convert_unit failed: ' + out);
console.log('convert_unit km->m:', out);
console.log('HTTP E2E SMOKE PASSED');
