'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../src/html/ip_details.js'), 'utf8');

// Only the DOM operations used by ip_details.js; HTML parsing is deliberately forbidden.
class Element {
  constructor(tagName) {
    this.tagName = tagName;
    this.children = [];
    this.dataset = {};
    this.hidden = false;
    this.attributes = {};
    this.text = '';
  }
  appendChild(child) { this.children.push(child); return child; }
  replaceChildren(...children) { this.children = children; this.text = ''; }
  insertRow() { return this.appendChild(new Element('tr')); }
  insertCell() { return this.appendChild(new Element('td')); }
  setAttribute(name, value) { this.attributes[name] = value; }
  set textContent(value) { this.text = String(value); this.children = []; }
  get textContent() { return this.text + this.children.map(child => child.textContent).join(''); }
  set innerHTML(_) { throw new Error('innerHTML must not be used for IP details'); }
}

function descendants(element, tagName) {
  return element.children.flatMap(child => [
    ...(child.tagName === tagName ? [child] : []), ...descendants(child, tagName)
  ]);
}

async function run({ values = [], search = '', ok = true, fetchError, mapError = false } = {}) {
  const section = new Element('section');
  section.hidden = true;
  const head = new Element('head');
  const form = new Element('form');
  form.hidden = true;
  const input = new Element('input');
  const requests = [];
  const calls = [];
  const map = {
    setView(center, zoom) { calls.push(['view', [...center], zoom]); return this; },
    fitBounds(bounds, options) { calls.push(['bounds', bounds, options.maxZoom]); }
  };
  const L = {
    map(element) { calls.push(['map', element]); return map; },
    tileLayer(url, options) {
      calls.push(['tiles', url, options]);
      return { addTo(target) { assert.equal(target, map); } };
    },
    marker(center) {
      calls.push(['marker', [...center]]);
      return {
        addTo(target) { assert.equal(target, map); return this; },
        bindPopup(node) { calls.push(['popup', node]); }
      };
    },
    circle(center, options) {
      calls.push(['circle', [...center], options.radius]);
      return {
        addTo(target) { assert.equal(target, map); return this; },
        getBounds() { return 'circle-bounds'; }
      };
    }
  };
  head.appendChild = child => {
    head.children.push(child);
    if (child.tagName === 'script') queueMicrotask(() => mapError ? child.onerror() : child.onload());
    return child;
  };
  const context = {
    I18N: { zh: {}, en: {} }, lang: 'en', URLSearchParams, AbortSignal,
    location: { search }, L,
    document: {
      head,
      getElementById(id) {
        assert.ok(['ip-details', 'ip-lookup', 'lookup-ip'].includes(id));
        return { 'ip-details': section, 'ip-lookup': form, 'lookup-ip': input }[id];
      },
      createElement(tagName) { return new Element(tagName); },
      createTextNode(text) { const node = new Element('#text'); node.textContent = text; return node; }
    },
    async fetch(url, options) {
      requests.push({ url, options });
      if (fetchError) throw fetchError;
      return { ok, async json() { return values; } };
    },
    // The host page calls renderIpFields from apply; translations are static text.
    apply() {
      function translate(element) {
        if (element.dataset.i18n) element.textContent = context.I18N[context.lang][element.dataset.i18n];
        element.children.forEach(translate);
      }
      translate(section);
      context.renderIpFields();
    }
  };
  await vm.runInNewContext(source, context, { filename: 'ip_details.js' });
  return {
    section, head, form, input, requests, calls,
    rows() {
      return new Map(descendants(section, 'tr').map(row => row.children.map(cell => cell.textContent)));
    },
    language(lang) { context.lang = lang; context.apply(); }
  };
}

const sample = {
  ip: '203.0.113.8',
  data: {
    city: { names: { en: 'Example City', 'zh-CN': '示例城市' }, geoname_id: 123 },
    continent: { code: 'AS', names: { en: 'Asia', 'zh-CN': '亚洲' } },
    country: { iso_code: 'CN', names: { en: 'China', 'zh-CN': '中国' } },
    registered_country: { iso_code: 'CN' },
    subdivisions: [{ iso_code: '11', names: { en: 'Example Province' } }],
    postal: { code: '100000' },
    asn: { autonomous_system_number: 64500, autonomous_system_organization: 'Example Network', as_domain: 'example.test' },
    proxy: { is_proxy: false, is_vpn: false, is_tor: false, is_hosting: true, is_cdn: false, is_school: false, is_anonymous: false }
  }
};

test('top-right lookup uses a native GET form and safely prefills query IP', async () => {
  const html = fs.readFileSync(path.join(__dirname, '../src/html/index.html'), 'utf8');
  assert.match(html, /<form[^>]*id="ip-lookup"[^>]*action="\/"[^>]*method="get"[^>]*hidden>/);
  assert.match(html, /<input[^>]*name="ip"[^>]*required>/);
  assert.match(html, /<button[^>]*type="submit"[^>]*data-i18n="ip_lookup"/);
  const ip = '2001:db8::1';
  const result = await run({ search: '?ip=' + encodeURIComponent(ip) });
  assert.equal(result.form.hidden, false);
  assert.equal(result.input.value, ip);
  assert.equal((await run()).input.value, '');
});

test('query IP is decoded then encoded once, with no query when absent', async () => {
  const ip = '2001:db8::1%eth0 &x=<tag>+?';
  const result = await run({ search: '?other=1&ip=' + encodeURIComponent(ip) });
  assert.equal(result.requests[0].url, '/tul_ip_details?ip=' + encodeURIComponent(ip));
  assert.equal(result.requests[0].options.cache, 'no-store');
  assert.ok(result.requests[0].options.signal instanceof AbortSignal);
  assert.equal((await run()).requests[0].url, '/tul_ip_details');
});

test('sample fields and false flags render and re-render in English and Chinese', async () => {
  const result = await run({ values: [sample] });
  const rows = result.rows();
  for (const [label, value] of [
    ['IP Address', sample.ip], ['City / GeoNames ID', '123'],
    ['Continent / Code', 'AS'], ['Country / ISO Code', 'CN'],
    ['Registered Country / ISO Code', 'CN'], ['Subdivisions / 1 / ISO Code', '11'],
    ['Postal / Code', '100000'], ['ASN / AS Number', '64500'],
    ['ASN / AS Organization', 'Example Network'], ['ASN / AS Domain', 'example.test'],
    ['Network Flags / Hosting / Datacenter', 'Yes']
  ]) assert.equal(rows.get(label), value, label);
  for (const flag of ['Proxy', 'VPN', 'Tor', 'CDN', 'School Network', 'Anonymous Network']) {
    assert.equal(rows.get('Network Flags / ' + flag), 'No', flag);
  }
  assert.match(rows.get('City / Names'), /^Example City \(en\)/);
  assert.equal(result.section.hidden, false);
  assert.equal(descendants(result.section, 'p')[0].hidden, true);
  assert.equal(descendants(result.section, 'h2')[0].textContent, 'IP Details');
  result.language('zh');
  assert.equal(descendants(result.section, 'h2')[0].textContent, 'IP 详情');
  assert.equal(result.rows().get('网络属性 / 代理'), '否');
  assert.equal(result.rows().get('网络属性 / 托管 / 数据中心'), '是');
  assert.match(result.rows().get('城市 / 名称'), /^示例城市 \(zh-CN\)/);
  result.language('en');
  assert.deepEqual(result.rows(), rows);
});

test('untrusted IP and field values remain text, including marker popup', async () => {
  const payload = '<img src=x onerror=alert(1)>';
  const result = await run({ values: [{ ip: payload, data: {
    city: { names: { en: payload } }, [payload]: payload,
    location: { latitude: 0, longitude: 0 }
  } }] });
  assert.equal(descendants(result.section, 'h3')[0].textContent, payload);
  assert.equal(result.rows().get(payload), payload);
  assert.equal(result.rows().get('City / Names'), payload + ' (en)');
  const popup = result.calls.find(call => call[0] === 'popup')[1];
  assert.equal(popup.tagName, '#text');
  assert.equal(popup.textContent, payload);
  assert.equal(descendants(result.section, 'img').length, 0);
});

test('missing, incomplete, out-of-range and nonnumeric coordinates do not load maps', async () => {
  for (const location of [undefined, {}, { latitude: 1 }, { latitude: 91, longitude: 1 },
    { latitude: 1, longitude: -181 }, { latitude: '1', longitude: 1 }, { latitude: NaN, longitude: 0 }]) {
    const result = await run({ values: [{ ip: sample.ip, data: { location } }] });
    assert.equal(result.head.children.length, 0);
    assert.equal(result.calls.length, 0);
    assert.equal(descendants(result.section, 'article').length, 1);
  }
});

test('valid coordinates load Leaflet and OSM, marker, accuracy circle and translated note', async () => {
  const result = await run({ values: [{ ...sample, data: { ...sample.data,
    location: { latitude: 39.9, longitude: 116.4, accuracy_radius: 20, time_zone: 'Asia/Shanghai' }
  } }] });
  assert.deepEqual(result.head.children.map(node => node.tagName), ['link', 'script']);
  assert.equal(result.head.children[0].href, 'https://unpkg.com/leaflet@1.9.4/dist/leaflet.css');
  assert.equal(result.head.children[1].src, 'https://unpkg.com/leaflet@1.9.4/dist/leaflet.js');
  assert.deepEqual(result.calls.find(call => call[0] === 'view'), ['view', [39.9, 116.4], 10]);
  assert.deepEqual(result.calls.find(call => call[0] === 'marker'), ['marker', [39.9, 116.4]]);
  assert.deepEqual(result.calls.find(call => call[0] === 'circle'), ['circle', [39.9, 116.4], 20000]);
  assert.deepEqual(result.calls.find(call => call[0] === 'bounds'), ['bounds', 'circle-bounds', 10]);
  const tiles = result.calls.find(call => call[0] === 'tiles');
  assert.equal(tiles[1], 'https://tile.openstreetmap.org/{z}/{x}/{y}.png');
  assert.match(tiles[2].attribution, /OpenStreetMap/);
  assert.equal(tiles[2].maxZoom, 19);
  assert.equal(result.rows().get('Location / Time Zone'), 'Asia/Shanghai');
  assert.match(result.section.textContent, /IP geolocation is approximate/);
  result.language('zh');
  assert.match(result.section.textContent, /IP 地理位置为估算位置/);
});

test('empty arrays and invalid entries show an empty state in both languages', async () => {
  for (const values of [[], [null, {}, { ip: 'x' }]]) {
    const result = await run({ values });
    assert.match(result.section.textContent, /No IP details found\./);
    assert.equal(descendants(result.section, 'p')[0].hidden, false);
    assert.equal(result.head.children.length, 0);
    result.language('zh');
    assert.match(result.section.textContent, /未查询到 IP 详情/);
  }
});

test('network, HTTP and malformed-response errors show translated error state', async () => {
  for (const options of [{ fetchError: new Error('offline') }, { ok: false }, { values: {} }]) {
    const result = await run(options);
    assert.match(result.section.textContent, /Unable to load IP details/);
    assert.equal(descendants(result.section, 'p')[0].hidden, false);
    assert.equal(result.head.children.length, 0);
    result.language('zh');
    assert.match(result.section.textContent, /IP 详情加载失败/);
  }
});

test('Leaflet load failure preserves fields and shows translated map error', async () => {
  const result = await run({ mapError: true, values: [{ ...sample,
    data: { location: { latitude: 0, longitude: 0 } }
  }] });
  assert.equal(result.rows().get('IP Address'), sample.ip);
  assert.match(result.section.textContent, /Unable to load the map/);
  assert.equal(result.calls.length, 0);
  result.language('zh');
  assert.match(result.section.textContent, /地图加载失败/);
});
