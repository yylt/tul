// Smoke test for the core WASM module (run from repo root: node tul-cv-wasm/tests/smoke_core.mjs)
import { readFileSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { createRequire } from 'module';
const require = createRequire('/home/yy/gopath/src/github.com/yylt/tul/');
const { PNG } = require('pngjs');

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(here, '..', 'pkg');
const wasmBytes = readFileSync(path.join(root, 'tul_cv_wasm_bg.wasm'));

globalThis.fetch = async (url) => {
  if (String(url).endsWith('tul_cv_wasm_bg.wasm')) {
    return new Response(wasmBytes, { headers: { 'content-type': 'application/wasm' } });
  }
  throw new Error('no fetch for ' + url);
};

const mod = await import(path.join(root, 'tul_cv_wasm.js'));
await mod.default();
const enc = new TextEncoder();

const checks = [
  ['to_base64', mod.to_base64(enc.encode('hello')) === 'aGVsbG8='],
  ['to_hex', mod.to_hex(enc.encode('AB')) === '4142'],
  ['json_format', mod.json_format('{"a":1}').includes('"a": 1')],
  ['json_minify', mod.json_minify('{ "a" : 1 }') === '{"a":1}'],
  ['markdown_to_html', mod.markdown_to_html('# hi').includes('<h1>')],
  ['dedupe_lines', mod.dedupe_lines('b\na\nb') === 'b\na'],
  ['sort_lines', mod.sort_lines('10\n2\n1', true, false) === '1\n2\n10'],
  ['csv_to_json', mod.csv_to_json('name,age\nalice,30', ',').includes('alice')],
  ['gbk', Array.from(mod.utf8_to_gbk('中文')).join() === '214,208,206,196'],
  ['unit_km_m', mod.convert_unit('length', 1, 'km', 'm') === 1000],
  ['unit_c_f', mod.convert_unit('temperature', 0, 'C', 'F') === 32],
];

const png = new PNG({ width: 16, height: 16 });
for (let i = 0; i < png.data.length; i += 4) { png.data[i] = 255; png.data[i + 3] = 255; }
const pngBuf = PNG.sync.write(png);

const jpg = mod.convert_to_jpeg(new Uint8Array(pngBuf), 85);
checks.push(['jpeg_magic', jpg[0] === 0xFF && jpg[1] === 0xD8]);
const png2 = mod.convert_to_png(new Uint8Array(jpg));
checks.push(['png_magic', png2[0] === 0x89 && png2[1] === 0x50]);
const webp = mod.convert_to_webp(new Uint8Array(pngBuf), 80);
checks.push(['webp_magic', String.fromCharCode(...webp.slice(0, 4)) === 'RIFF']);
const resized = PNG.sync.read(Buffer.from(mod.resize_image(new Uint8Array(jpg), 1024, true, 'png')));
checks.push(['resize_1024', resized.width === 1024 && resized.height === 1024]);
const cropped = PNG.sync.read(Buffer.from(mod.crop_image(new Uint8Array(pngBuf), 0, 0, 4, 4, 'png')));
checks.push(['crop_4x4', cropped.width === 4 && cropped.height === 4]);
checks.push(['watermark', mod.add_text_watermark(new Uint8Array(pngBuf), 'TUL', 96, 'png').length > 0]);
const t2i = PNG.sync.read(Buffer.from(mod.text_to_image('AB', 2)));
checks.push(['text_to_image', t2i.width === 32 && t2i.height === 16]);
const pdf = mod.text_to_pdf('hello\nworld', 'Title');
checks.push(['pdf_magic', String.fromCharCode(...pdf.slice(0, 5)) === '%PDF-']);

let failed = 0;
for (const [name, ok] of checks) {
  console.log((ok ? 'PASS' : 'FAIL') + ' ' + name);
  if (!ok) failed++;
}
if (failed) { console.log(failed + ' checks FAILED'); process.exit(1); }
console.log('CORE SMOKE PASSED');
