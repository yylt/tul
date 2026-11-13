Object.assign(I18N.zh, {
  ip_lookup: '查询', ip_details: 'IP 详情', ip_loading: '正在查询 IP 详情…', ip_error: 'IP 详情加载失败，请稍后重试。',
  ip_empty: '未查询到 IP 详情。', ip_map_error: '地图加载失败，位置数据仍可查看。',
  ip_approximate: 'IP 地理位置为估算位置，并非设备的精确位置。'
});
Object.assign(I18N.en, {
  ip_lookup: 'Look up', ip_details: 'IP Details', ip_loading: 'Loading IP details…', ip_error: 'Unable to load IP details. Please try again later.',
  ip_empty: 'No IP details found.', ip_map_error: 'Unable to load the map. Location details remain available.',
  ip_approximate: 'IP geolocation is approximate, not the exact location of a device.'
});
var detailLabels = {
  ip: ['IP 地址', 'IP Address'], city: ['城市', 'City'], continent: ['洲', 'Continent'],
  country: ['国家', 'Country'], registered_country: ['注册国家', 'Registered Country'],
  subdivisions: ['行政区划', 'Subdivisions'], location: ['位置', 'Location'], postal: ['邮政', 'Postal'],
  names: ['名称', 'Names'], geoname_id: ['GeoNames ID', 'GeoNames ID'], iso_code: ['ISO 代码', 'ISO Code'],
  code: ['代码', 'Code'], latitude: ['纬度', 'Latitude'], longitude: ['经度', 'Longitude'],
  accuracy_radius: ['精度半径 (km)', 'Accuracy Radius (km)'], metro_code: ['都市区代码', 'Metro Code'],
  time_zone: ['时区', 'Time Zone'], asn: ['自治系统', 'ASN'],
  autonomous_system_number: ['AS 编号', 'AS Number'], autonomous_system_organization: ['AS 组织', 'AS Organization'],
  as_domain: ['AS 域名', 'AS Domain'], proxy: ['网络属性', 'Network Flags'],
  is_proxy: ['代理', 'Proxy'], is_vpn: ['VPN', 'VPN'], is_tor: ['Tor', 'Tor'],
  is_hosting: ['托管 / 数据中心', 'Hosting / Datacenter'], is_cdn: ['CDN', 'CDN'],
  is_school: ['学校网络', 'School Network'], is_anonymous: ['匿名网络', 'Anonymous Network']
};
document.getElementById('ip-lookup').hidden = false;
document.getElementById('lookup-ip').value = new URLSearchParams(location.search).get('ip') || '';
var detailSection = document.getElementById('ip-details');
detailSection.hidden = false;
var detailTitle = document.createElement('h2');
detailTitle.dataset.i18n = 'ip_details';
detailSection.appendChild(detailTitle);
var detailStatus = document.createElement('p');
detailStatus.className = 'ip-status';
detailStatus.dataset.i18n = 'ip_loading';
detailSection.appendChild(detailStatus);
var detailEntries = [];
function renderIpFields() {
  var chinese = lang === 'zh';
  detailEntries.forEach(function (entry) {
    entry.table.replaceChildren();
    function rows(value, path) {
      if (value === null || value === undefined) return;
      if (typeof value === 'object' && !Array.isArray(value) && path[path.length - 1] === 'names') {
        value = Object.entries(value).sort(function (a, b) {
          var preferred = chinese ? 'zh-CN' : 'en';
          return Number(b[0] === preferred) - Number(a[0] === preferred);
        }).map(function (pair) { return pair[1] + ' (' + pair[0] + ')'; }).join(' / ');
      } else if (typeof value === 'object') {
        Object.entries(value).forEach(function (pair) { rows(pair[1], path.concat(pair[0])); });
        return;
      }
      var row = entry.table.insertRow();
      row.insertCell().textContent = path.map(function (key) {
        return detailLabels[key] ? detailLabels[key][chinese ? 0 : 1] : (/^\d+$/.test(key) ? Number(key) + 1 : key);
      }).join(' / ');
      row.insertCell().textContent = typeof value === 'boolean' ? (value ? (chinese ? '是' : 'Yes') : (chinese ? '否' : 'No')) : String(value);
    }
    rows(entry.value.ip, ['ip']);
    rows(entry.value.data, []);
  });
}
function loadLeaflet() {
  var css = document.createElement('link');
  css.rel = 'stylesheet';
  css.href = 'https://unpkg.com/leaflet@1.9.4/dist/leaflet.css';
  document.head.appendChild(css);
  return new Promise(function (resolve, reject) {
    var script = document.createElement('script');
    script.src = 'https://unpkg.com/leaflet@1.9.4/dist/leaflet.js';
    script.onload = resolve;
    script.onerror = reject;
    document.head.appendChild(script);
  });
}
async function loadIpDetails() {
  var params = new URLSearchParams(location.search);
  var endpoint = '/tul_ip_details' + (params.has('ip') ? '?ip=' + encodeURIComponent(params.get('ip')) : '');
  try {
    var response = await fetch(endpoint, { cache: 'no-store', signal: AbortSignal.timeout(15000) });
    if (!response.ok) throw new Error('IP lookup failed');
    var values = await response.json();
    if (!Array.isArray(values)) throw new Error('Invalid IP lookup response');
    values.forEach(function (value) {
      if (!value || typeof value !== 'object' || !value.data || typeof value.data !== 'object') return;
      var article = document.createElement('article');
      var heading = document.createElement('h3');
      heading.textContent = value.ip || '';
      article.appendChild(heading);
      var table = document.createElement('table');
      article.appendChild(table);
      detailSection.appendChild(article);
      detailEntries.push({ value: value, table: table, article: article });
    });
    detailStatus.hidden = detailEntries.length > 0;
    detailStatus.dataset.i18n = 'ip_empty';
    apply();
  } catch (_) {
    detailStatus.dataset.i18n = 'ip_error';
    apply();
    return;
  }
  var locations = detailEntries.filter(function (entry) {
    var loc = entry.value.data.location || {};
    return Number.isFinite(loc.latitude) && Math.abs(loc.latitude) <= 90 &&
      Number.isFinite(loc.longitude) && Math.abs(loc.longitude) <= 180;
  });
  if (!locations.length) return;
  try {
    await loadLeaflet();
    locations.forEach(function (entry) {
      var loc = entry.value.data.location;
      var mapElement = document.createElement('div');
      mapElement.className = 'ip-map';
      mapElement.setAttribute('aria-label', 'IP: ' + entry.value.ip);
      entry.article.appendChild(mapElement);
      var center = [loc.latitude, loc.longitude];
      var map = L.map(mapElement).setView(center, 10);
      L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
        maxZoom: 19, attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'
      }).addTo(map);
      L.marker(center).addTo(map).bindPopup(document.createTextNode(String(entry.value.ip || '')));
      if (Number.isFinite(loc.accuracy_radius) && loc.accuracy_radius > 0) {
        var circle = L.circle(center, { radius: loc.accuracy_radius * 1000 }).addTo(map);
        map.fitBounds(circle.getBounds(), { maxZoom: 10 });
      }
      var note = document.createElement('p');
      note.dataset.i18n = 'ip_approximate';
      entry.article.appendChild(note);
    });
  } catch (_) {
    var error = document.createElement('p');
    error.dataset.i18n = 'ip_map_error';
    detailSection.appendChild(error);
  }
  apply();
}
loadIpDetails();
