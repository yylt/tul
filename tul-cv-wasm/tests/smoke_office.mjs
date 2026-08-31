// Smoke test for the office WASM module (run from repo root: node tul-cv-wasm/tests/smoke_office.mjs)
import { readFileSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(here, '..', 'office', 'pkg');
const wasmBytes = readFileSync(path.join(root, 'tul_cv_office_wasm_bg.wasm'));

globalThis.fetch = async (url) => {
  if (String(url).endsWith('tul_cv_office_wasm_bg.wasm')) {
    return new Response(wasmBytes, { headers: { 'content-type': 'application/wasm' } });
  }
  throw new Error('no fetch for ' + url);
};

const mod = await import(path.join(root, 'tul_cv_office_wasm.js'));
await mod.default();

const docx = mod.text_to_docx('hello\ndocx test');
console.log('docx magic:', String.fromCharCode(...docx.slice(0, 2)), 'len:', docx.length);

try {
  mod.xlsx_to_csv(new Uint8Array([1, 2, 3]), '');
  console.log('xlsx: UNEXPECTED SUCCESS');
  process.exit(1);
} catch (e) {
  console.log('xlsx error (expected):', String(e).slice(0, 80));
}

console.log('OFFICE SMOKE PASSED');

// --- xlsx parsing with a real file ---
import { readFileSync as read } from 'fs';
const xlsx = read(new URL('./test.xlsx', import.meta.url));
const csv = mod.xlsx_to_csv(new Uint8Array(xlsx), '');
console.log('xlsx csv:', JSON.stringify(csv));
const json = mod.xlsx_to_json(new Uint8Array(xlsx), '');
console.log('xlsx json:', json);
if (!csv.includes('alice') || !csv.includes('30')) { console.log('XLSX CSV FAIL'); process.exit(1); }
if (!json.includes('"name":"alice"')) { console.log('XLSX JSON FAIL'); process.exit(1); }
console.log('XLSX PARSE PASSED');
