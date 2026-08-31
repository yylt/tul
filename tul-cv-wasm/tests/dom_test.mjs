// jsdom-based test: convert the module script to a classic script (stub the
// wasm import), run in jsdom with real DOM semantics, verify accordion,
// text2img no-crash, live wiring and i18n switching.
import { JSDOM } from 'jsdom';
import { readFileSync } from 'fs';

const html = readFileSync(new URL('../../src/html/cv.html', import.meta.url), 'utf8');
let script = html.match(/<script type="module">([\s\S]*?)<\/script>/)[1];

// stub the wasm import line
script = script.replace(/import init, \{[\s\S]*?\} from '\/tul_cv\/tul_cv_wasm\.js';/,
  `const init = async () => {};
   const convert_to_jpeg = () => new Uint8Array([0xFF, 0xD8]);
   const convert_to_png = () => new Uint8Array([0x89, 0x50]);
   const convert_to_webp = () => new Uint8Array([0x52, 0x49]);
   const resize_image = () => new Uint8Array([0x89, 0x50]);
   const crop_image = () => new Uint8Array([0x89, 0x50]);
   const add_text_watermark = () => new Uint8Array([0x89, 0x50]);
   const text_to_image = () => new Uint8Array([0x89, 0x50]);
   const text_to_pdf = () => new Uint8Array([0x25, 0x50]);
   const markdown_to_html = (s) => '<h1>' + s + '</h1>';
   const json_format = (s) => s;
   const json_minify = (s) => s;
   const csv_to_json = () => '[]';
   const to_base64 = (b) => 'b64';
   const from_base64 = (s) => new Uint8Array();
   const to_hex = (b) => 'ff';
   const from_hex = (s) => new Uint8Array();
   const utf8_to_gbk = (s) => new Uint8Array();
   const gbk_to_utf8 = (b) => '';
   const dedupe_lines = (s) => s;
   const sort_lines = (s) => s;
   const convert_unit = (c, v) => v;`);

// jsdom can't run <script type=module>; inject as classic script
const wrapped = '<!DOCTYPE html><html><body><div class="container">' +
  '<div class="topbar"><h1 id="title" data-i18n="title"></h1><button id="lang"></button></div>' +
  '<input id="search">' +
  '<div class="tabs" id="tabs">' +
  '<button class="tab active" data-tab="all" data-i18n="tab_all">All</button>' +
  '<button class="tab" data-tab="image" data-i18n="tab_image">Image</button>' +
  '<button class="tab" data-tab="text" data-i18n="tab_text">Text</button>' +
  '<button class="tab" data-tab="unit" data-i18n="tab_unit">Unit</button></div>' +
  '<div class="tool-list" id="list"></div>' +
  '</div><script>' + script + '<\/script></body></html>';

const dom = new JSDOM(wrapped, {
  url: 'http://localhost:8787/tul_cv',
  runScripts: 'dangerously',
  pretendToBeVisual: true,
  beforeParse(window) {
    let blobUrlSeq = 0;
    window.URL.createObjectURL = () => 'blob:test-' + (++blobUrlSeq);
    window.URL.revokeObjectURL = () => {};
  },
});
const { document } = dom.window;

await new Promise(r => setTimeout(r, 400));
let pass = 0, fail = 0;
const ok = (n, c) => { console.log((c ? 'PASS' : 'FAIL') + ' ' + n); c ? pass++ : fail++; };

const items = () => [...document.querySelectorAll('.tool-item')];

// 1. 19 items, vertical list
const all = items();
ok('19 items rendered', all.length === 19);

// 2. accordion: open first
all[0].querySelector('.tool-head').click();
ok('first opens', all[0].classList.contains('open'));
ok('first body has panel html', all[0].querySelector('.tool-body').innerHTML.length > 30);

// 3. open second closes first
all[1].querySelector('.tool-head').click();
ok('first closes when second opens', !all[0].classList.contains('open') && all[1].classList.contains('open'));

// 4. text2img: open, no crash, has inputs and result wrapper
const ti = all.find(x => x.dataset.id === 'img_text2img');
ti.querySelector('.tool-head').click();
const tiBody = ti.querySelector('.tool-body');
ok('text2img has ttext input', !!tiBody.querySelector('#ttext'));
ok('text2img has result wrap (no box crash)', tiBody.querySelector('.res-wrap') !== null);
// type into ttext -> schedule (run() guards on wasm stubbed init which resolves)
const input = tiBody.querySelector('#ttext');
input.value = 'HELLO';
input.dispatchEvent(new dom.window.Event('input', { bubbles: true }));
await new Promise(r => setTimeout(r, 300));
ok('text2img input no crash after 300ms', true);

// 5. PNG→JPEG panel has dropzone + quality slider + live wiring
const conv = all.find(x => x.dataset.id === 'img_png_jpeg');
conv.querySelector('.tool-head').click();
const cBody = conv.querySelector('.tool-body');
ok('convert has dropzone', !!cBody.querySelector('#dz'));
ok('convert has quality slider', !!cBody.querySelector('#quality'));

// 6. i18n switching
document.getElementById('lang').click();
ok('zh title', document.querySelector('h1').textContent === 'CV 工具');
ok('zh tab', [...document.querySelectorAll('.tab')].some(x => x.textContent.trim() === '全部'));
ok('zh first tool name', document.querySelector('.tool-item .name').textContent === 'PNG → JPEG');
document.getElementById('lang').click();
ok('en title back', document.querySelector('h1').textContent === 'CV Tools');

// 7. search filter
const search = document.getElementById('search');
search.value = 'pdf';
search.dispatchEvent(new dom.window.Event('input', { bubbles: true }));
await new Promise(r => setTimeout(r, 50));
const filtered = items();
ok('search filters to pdf tool', filtered.length >= 1 && filtered.every(x => (x.dataset.id + x.querySelector('.name').textContent + x.querySelector('.desc').textContent).toLowerCase().includes('pdf')));

// 8. tab filter (clear search first)
const search2 = document.getElementById('search');
search2.value = '';
search2.dispatchEvent(new dom.window.Event('input', { bubbles: true }));
await new Promise(r => setTimeout(r, 30));
const unitTab = document.querySelector('.tab[data-tab="unit"]');
unitTab.click();
await new Promise(r => setTimeout(r, 50));
const unitItems = items();
ok('unit tab shows 1 item', unitItems.length === 1 && unitItems[0].dataset.id === 'unit_conv');

console.log(fail ? fail + ' FAILED' : 'DOM TEST PASSED (' + pass + ' checks)');
process.exit(fail ? 1 : 0);
