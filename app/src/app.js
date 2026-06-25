/* app.js — HardView UI. Reiner Renderer; Datenquelle ist invoke() (Tauri-Backend oder mock.js). */
(function () {
  'use strict';

  const TAURI = window.__TAURI__;
  async function invoke(cmd, args) {
    if (TAURI && TAURI.core && TAURI.core.invoke) return TAURI.core.invoke(cmd, args || {});
    return window.__MOCK__.invoke(cmd, args || {});
  }

  // ---------------- helpers ----------------
  const $ = (s) => document.querySelector(s);
  function el(tag, attrs) {
    const e = document.createElement(tag);
    const kids = Array.prototype.slice.call(arguments, 2);
    if (attrs) for (const k in attrs) {
      const v = attrs[k];
      if (v == null) continue;
      if (k === 'class') e.className = v;
      else if (k === 'style' && typeof v === 'object') Object.assign(e.style, v);
      else if (k.indexOf('on') === 0 && typeof v === 'function') e.addEventListener(k.slice(2).toLowerCase(), v);
      else e.setAttribute(k, v);
    }
    flatten(kids).forEach((kid) => {
      if (kid == null || kid === false) return;
      e.appendChild(typeof kid === 'object' ? kid : document.createTextNode(String(kid)));
    });
    return e;
  }
  function flatten(a) { return a.reduce((o, v) => o.concat(Array.isArray(v) ? flatten(v) : v), []); }
  // Statisches SVG-Markup sicher zu DOM-Knoten parsen (ersetzt den früheren innerHTML-Pfad in el()).
  function svg(markup) {
    const doc = new DOMParser().parseFromString(markup, 'image/svg+xml');
    return document.importNode(doc.documentElement, true);
  }
  function ramColor(gb) { return gb <= 8 ? 'var(--red)' : gb < 16 ? 'var(--amber)' : 'var(--blue)'; }
  function ageColor(y) { return y == null ? 'var(--muted-2)' : y > 5 ? 'var(--red)' : y > 4 ? 'var(--amber)' : 'var(--text-2)'; }
  function toast(msg) {
    const m = $('#toastMount'); m.innerHTML = '';
    const t = el('div', { class: 'toast' }, msg); m.appendChild(t);
    setTimeout(() => { if (t.parentNode) t.remove(); }, 2600);
  }

  // ---------------- state ----------------
  const state = { view: 'inventar', devices: [], overview: null, settings: null, q: '', filter: 'all', sort: 'host', dir: 'asc', selected: null };
  const DEFAULT_THRESHOLDS = { minRamGB: 8, maxAgeYears: 5, staleDays: 30, requireSsd: true, minCpuCores: 4, minCpuClockMhz: 0, targetRamGB: 16 };

  const VIEWS = {
    inventar:  { title: 'Geräte-Inventar', sub: 'Hardware-Bestand aller Arbeitsplätze · wöchentliche Inventarisierung', list: true, filter: 'all' },
    warnungen: { title: 'Upgrade-Kandidaten', sub: 'Geräte, bei denen die IT aufrüsten oder ersetzen sollte', list: true, filter: 'upgrade' },
    dashboard: { title: 'Dashboard', sub: 'Überblick über Alter, Arbeitsspeicher und Status des Bestands', list: false },
    gruppen:   { title: 'Abteilungen', sub: 'Geräte und Upgrade-Bedarf je Abteilung', list: false },
    berichte:  { title: 'Berichte', sub: 'Export für Beschaffung und Lebenszyklus-Planung', list: false },
    einstellungen: { title: 'Einstellungen', sub: 'Datenquellen, Active Directory und Bewertungs-Schwellen', list: false }
  };

  // ---------------- data ----------------
  async function loadData() {
    try {
      const [devices, overview, me, settings] = await Promise.all([
        invoke('get_devices'), invoke('get_overview'),
        invoke('me').catch(() => null), invoke('get_settings').catch(() => null)
      ]);
      state.devices = devices || [];
      state.overview = overview;
      state.settings = settings;
      if (me) { $('#meName').textContent = me.name; $('#meInitials').textContent = me.initials; }
      updateAdStatus(settings, me);
      window.HardView.renderAll();
    } catch (e) { toast('Fehler beim Laden: ' + e); console.error(e); }
  }

  // Sidebar-AD-Status ehrlich aus der Konfiguration ableiten (statt statisch „verbunden").
  function updateAdStatus(settings, me) {
    const dot = $('#adDot'), text = $('#adText'), sub = $('#adSub');
    const enabled = !!(settings && settings.adEnabled);
    if (dot) dot.classList.toggle('off', !enabled);
    if (text) text.textContent = enabled ? 'AD aktiv' : 'CSV-Fallback (AD aus)';
    if (sub && me && me.domain) sub.textContent = me.domain;
  }

  function visible() {
    const isUpgradeCandidate = (d) => d.status === 'upgrade' || (d.status === 'stale' && (d.upgradeReasons || []).length > 0);
    let list = state.devices.filter((d) => {
      if (state.filter !== 'all') {
        if (state.filter === 'veraltet') { if (!(d.status === 'stale' || d.status === 'missing')) return false; }
        else if (state.filter === 'upgrade') { if (!isUpgradeCandidate(d)) return false; }
        else if (d.status !== state.filter) return false;
      }
      if (state.q) {
        const hay = (d.host + ' ' + d.user + ' ' + d.cpu + ' ' + d.dept).toLowerCase();
        if (hay.indexOf(state.q) === -1) return false;
      }
      return true;
    });
    const dir = state.dir === 'asc' ? 1 : -1;
    const rank = { ok: 0, upgrade: 1, stale: 2, missing: 3 };
    list.sort((a, b) => {
      let av, bv;
      switch (state.sort) {
        case 'user': av = a.user.toLowerCase(); bv = b.user.toLowerCase(); break;
        case 'cpu': av = a.cpu.toLowerCase(); bv = b.cpu.toLowerCase(); break;
        case 'ram': av = a.ramGB; bv = b.ramGB; break;
        case 'age': av = a.ageYears == null ? -1 : a.ageYears; bv = b.ageYears == null ? -1 : b.ageYears; break;
        case 'status': av = rank[a.status]; bv = rank[b.status]; break;
        default: av = a.host.toLowerCase(); bv = b.host.toLowerCase();
      }
      if (av < bv) return -1 * dir; if (av > bv) return 1 * dir;
      return a.host < b.host ? -1 : 1;
    });
    return list;
  }

  // ---------------- render: KPIs ----------------
  function renderKpis() {
    const o = state.overview; if (!o) return;
    const host = $('#kpis'); host.innerHTML = '';
    const card = (label, val, sub, cls, unit) => el('div', { class: 'kpi' },
      el('div', { class: 'k-label' }, label),
      el('div', { class: 'k-val ' + (cls || '') }, String(val), unit ? el('span', { class: 'k-unit' }, unit) : null),
      el('div', { class: 'k-sub' }, sub)
    );
    host.appendChild(card('GERÄTE GESAMT', o.total, 'in ' + o.deptCount + ' Abteilungen'));
    host.appendChild(card('AKTUELL INVENTARISIERT', o.current, (o.missing + o.stale) + ' ohne aktuelle Meldung', 'green'));
    host.appendChild(card('UPGRADE NÖTIG', o.upgradeNeeded, 'RAM · Alter · SSD · Win 11', 'amber'));
    host.appendChild(card('Ø ALTER', String(o.avgAgeYears).replace('.', ','), o.old5 + ' Geräte ≥ 5 Jahre', '', ' J.'));
    $('#navWarnBadge').textContent = o.upgradeNeeded;
  }

  // ---------------- render: segments ----------------
  function renderSegs() {
    const o = state.overview; if (!o) return;
    const host = $('#segs'); host.innerHTML = '';
    const defs = [
      ['all', 'Alle', o.total],
      ['ok', 'OK', o.status.ok],
      ['upgrade', 'Upgrade', o.upgradeNeeded],
      ['veraltet', 'Veraltet', o.status.stale + o.status.missing]
    ];
    defs.forEach(([key, label, count]) => {
      const seg = el('div', { class: 'seg' + (state.filter === key ? ' active' : ''), onclick: () => { state.filter = key; window.HardView.renderAll(); } },
        label, el('span', { class: 'cnt' }, count));
      host.appendChild(seg);
    });
    const vis = visible().length;
    $('#resultCount').textContent = vis + ' von ' + o.total;
  }

  // ---------------- render: table ----------------
  const HEADS = [
    { label: '', key: 'status', align: 'center' },
    { label: 'Hostname', key: 'host' },
    { label: 'Benutzer (AD)', key: 'user' },
    { label: 'Prozessor', key: 'cpu' },
    { label: 'Arbeitsspeicher', key: 'ram' },
    { label: 'Alter', key: 'age', align: 'right' }
  ];
  function renderThead() {
    const host = $('#thead'); host.innerHTML = '';
    HEADS.forEach((h) => {
      const sorted = state.sort === h.key;
      const arrow = sorted ? (state.dir === 'asc' ? '  ↑' : '  ↓') : '';
      host.appendChild(el('div', {
        class: 'th sortable' + (sorted ? ' sorted' : '') + (h.align === 'right' ? ' right' : h.align === 'center' ? ' center' : ''),
        onclick: () => { if (state.sort === h.key) state.dir = state.dir === 'asc' ? 'desc' : 'asc'; else { state.sort = h.key; state.dir = 'asc'; } window.HardView.renderAll(); }
      }, h.label + arrow));
    });
  }
  function renderRows() {
    const host = $('#tbody'); host.innerHTML = '';
    const list = visible();
    if (!list.length) { host.appendChild(el('div', { class: 'empty' }, 'Keine Geräte für diese Filterung.')); return; }
    list.forEach((d) => {
      const ramTarget = Math.max(1, Number(d.ramTargetGB) || DEFAULT_THRESHOLDS.targetRamGB);
      const ramPct = Math.min(100, Math.max(0, Math.round((d.ramGB / ramTarget) * 100)));
      const row = el('div', {
        class: 'row grid-cols' + (d.status === 'upgrade' ? ' warn' : '') + (state.selected === d.host ? ' sel' : '') + (d.status === 'stale' || d.status === 'missing' ? ' dim' : ''),
        onclick: () => { state.selected = d.host; renderRows(); renderDrawer(); }
      },
        el('div', { style: { display: 'flex', justifyContent: 'center' } }, el('div', { class: 'dot ' + d.status })),
        el('div', { class: 'cell-min' },
          el('div', { class: 'cell-host' }, d.host),
          el('div', { class: 'cell-sub' }, d.osShort)),
        el('div', { class: 'user-cell' },
          el('div', { class: 'avatar', style: { background: d.avatarColor } }, d.initials),
          el('div', { class: 'cell-min' },
            el('div', { class: 'cell-main dfe' }, d.user),
            el('div', { class: 'cell-sub' }, d.dept))),
        el('div', { class: 'cell-min' },
          el('div', { class: 'cell-main dfe' }, d.cpu),
          el('div', { class: 'cell-sub' }, d.cores + ' Kerne')),
        el('div', { class: 'cell-min' },
          el('div', { class: 'ram-top' },
            el('span', { class: 'ram-val' }, d.ramGB + ' GB'),
            el('span', { class: 'ram-chip', style: { color: d.ramFreeSlots > 0 ? 'var(--green)' : 'var(--muted-2)' } }, d.ramFreeSlots > 0 ? (d.ramFreeSlots + ' Slot frei') : 'voll')),
          el('div', { class: 'bar' }, el('div', { style: { width: ramPct + '%', background: ramColor(d.ramGB) } }))),
        el('div', { class: 'cell-right', style: { color: ageColor(d.ageYears) } },
          el('div', {}, d.ageText),
          el('div', { class: 'cell-sub', style: { textAlign: 'right' } }, d.lastSeenText))
      );
      host.appendChild(row);
    });
  }

  // ---------------- render: detail drawer ----------------
  function statusClass(s) { return s; }
  function renderDrawer() {
    const mount = $('#drawerMount'); mount.innerHTML = '';
    if (!state.selected) return;
    const d = state.devices.find((x) => x.host === state.selected);
    if (!d) return;
    const close = () => { state.selected = null; mount.innerHTML = ''; renderRows(); };

    const specs = [
      ['Hersteller / Modell', (d.manufacturer || '—') + ' ' + (d.model || '')],
      ['Prozessor', d.cpu],
      ['Kerne / Threads', d.coresText],
      ['Arbeitsspeicher', d.ramGB + ' GB (' + d.ramSlotsUsed + '/' + d.ramSlotsTotal + ' Slots belegt)'],
      ['Datenträger', d.diskType + ' · ' + d.diskGB + ' GB'],
      ['Grafik', (d.gpus && d.gpus[0]) || '—'],
      ['Betriebssystem', d.osCaption + ' (' + d.osBuild + ')'],
      ['Seriennummer', d.serialNumber || '—'],
      ['BIOS-Datum', d.biosDate || '—'],
      ['Alter', d.ageText],
      ['Letzte Inventarisierung', d.lastSeenText],
      ['IP-Adresse', d.ip || '—']
    ];

    const reasonBox = d.upgradeReasons && d.upgradeReasons.length
      ? el('div', { class: 'reason-box' + (d.status === 'missing' ? ' crit' : '') },
          el('div', { style: { flex: 'none', marginTop: '1px' } }, svg('<svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M8 1.6 15 14H1L8 1.6Z" stroke="' + (d.status === 'missing' ? '#ff5d6c' : '#ffb454') + '" stroke-width="1.3" stroke-linejoin="round"/><line x1="8" y1="6.2" x2="8" y2="9.6" stroke="' + (d.status === 'missing' ? '#ff5d6c' : '#ffb454') + '" stroke-width="1.3"/><circle cx="8" cy="11.6" r="0.85" fill="' + (d.status === 'missing' ? '#ff5d6c' : '#ffb454') + '"/></svg>')),
          el('ul', {}, d.upgradeReasons.map((r) => el('li', {}, r))))
      : el('div', { class: 'reason-box', style: { background: 'rgba(47,214,166,.08)', borderColor: 'rgba(47,214,166,.25)' } },
          el('div', { style: { fontSize: '12px', color: '#9fe3cd', lineHeight: '1.5' } }, '✓ Keine Auffälligkeiten — Gerät entspricht den Vorgaben.'));

    const drawer = el('div', { class: 'drawer' },
      el('div', { class: 'drawer-head' },
        el('div', { class: 'top' },
          el('div', { class: 'id' },
            el('div', { class: 'dot-lg ' + statusClass(d.status), style: { background: 'currentColor' } }),
            el('div', {},
              el('div', { class: 'host' }, d.host),
              el('div', { class: 'status', style: { color: dotColor(d.status) } }, d.statusLabel))),
          el('div', { class: 'iconbtn', onclick: close }, svg('<svg width="13" height="13" viewBox="0 0 13 13"><line x1="1" y1="1" x2="12" y2="12" stroke="currentColor" stroke-width="1.4"/><line x1="12" y1="1" x2="1" y2="12" stroke="currentColor" stroke-width="1.4"/></svg>')))),
      el('div', { class: 'drawer-body' },
        el('div', { class: 'ad-box' },
          el('div', { class: 'av', style: { background: d.avatarColor } }, d.initials),
          el('div', {},
            el('div', { class: 'u' }, d.user),
            el('div', { class: 'd' }, d.dept + ' · ' + d.userSource)),
          el('div', { class: 'btn chg', onclick: () => window.HardView.openAssign(d.host) }, 'Ändern')),
        el('div', { class: 'section-label' }, 'Bewertung'),
        reasonBox,
        el('div', { class: 'section-label' }, 'Spezifikationen'),
        el('div', { class: 'specs' }, specs.map(([k, v]) => el('div', { class: 'spec' }, el('span', { class: 'k' }, k), el('span', { class: 'v' }, v))))),
      el('div', { class: 'drawer-foot' },
        el('div', { class: 'btn btn-primary', onclick: () => window.HardView.openAssign(d.host) }, 'Benutzer zuordnen'),
        el('div', { class: 'btn', onclick: close }, 'Schließen'))
    );
    // dot-lg color via currentColor trick
    drawer.querySelector('.dot-lg').style.color = dotColor(d.status);

    mount.appendChild(el('div', { class: 'scrim', onclick: close }));
    mount.appendChild(drawer);
  }
  function dotColor(s) { return s === 'ok' ? 'var(--green)' : s === 'upgrade' ? 'var(--amber)' : s === 'missing' ? 'var(--red)' : 'var(--muted-2)'; }


  window.HardView = {
    $,
    DEFAULT_THRESHOLDS,
    TAURI,
    VIEWS,
    el,
    invoke,
    loadData,
    renderDrawer,
    renderKpis,
    renderRows,
    renderSegs,
    renderThead,
    state,
    svg,
    toast
  };
})();
