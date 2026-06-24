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
      renderAll();
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
    let list = state.devices.filter((d) => {
      if (state.filter !== 'all') {
        if (state.filter === 'veraltet') { if (!(d.status === 'stale' || d.status === 'missing')) return false; }
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
      ['upgrade', 'Upgrade', o.status.upgrade],
      ['veraltet', 'Veraltet', o.status.stale + o.status.missing]
    ];
    defs.forEach(([key, label, count]) => {
      const seg = el('div', { class: 'seg' + (state.filter === key ? ' active' : ''), onclick: () => { state.filter = key; renderAll(); } },
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
        onclick: () => { if (state.sort === h.key) state.dir = state.dir === 'asc' ? 'desc' : 'asc'; else { state.sort = h.key; state.dir = 'asc'; } renderAll(); }
      }, h.label + arrow));
    });
  }
  function renderRows() {
    const host = $('#tbody'); host.innerHTML = '';
    const list = visible();
    if (!list.length) { host.appendChild(el('div', { class: 'empty' }, 'Keine Geräte für diese Filterung.')); return; }
    list.forEach((d) => {
      const ramPct = Math.min(100, Math.round((d.ramGB / d.ramTargetGB) * 100));
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
          el('div', { class: 'btn chg', onclick: () => openAssign(d.host) }, 'Ändern')),
        el('div', { class: 'section-label' }, 'Bewertung'),
        reasonBox,
        el('div', { class: 'section-label' }, 'Spezifikationen'),
        el('div', { class: 'specs' }, specs.map(([k, v]) => el('div', { class: 'spec' }, el('span', { class: 'k' }, k), el('span', { class: 'v' }, v))))),
      el('div', { class: 'drawer-foot' },
        el('div', { class: 'btn btn-primary', onclick: () => openAssign(d.host) }, 'Benutzer zuordnen'),
        el('div', { class: 'btn', onclick: close }, 'Schließen'))
    );
    // dot-lg color via currentColor trick
    drawer.querySelector('.dot-lg').style.color = dotColor(d.status);

    mount.appendChild(el('div', { class: 'scrim', onclick: close }));
    mount.appendChild(drawer);
  }
  function dotColor(s) { return s === 'ok' ? 'var(--green)' : s === 'upgrade' ? 'var(--amber)' : s === 'missing' ? 'var(--red)' : 'var(--muted-2)'; }

  // ---------------- Zuordnungs-Modal ----------------
  async function openAssign(host) {
    const mount = $('#modalMount');
    let selected = null, users = [], note = '';
    let searchTimer = null, searchSeq = 0;
    const close = () => { mount.innerHTML = ''; };

    async function loadUsers(q) {
      const seq = ++searchSeq;
      try {
        const result = await invoke('get_ad_users', { search: q || '' });
        if (seq !== searchSeq) return;
        users = result;
        renderList();
      } catch (e) {
        if (seq !== searchSeq) return;
        users = [];
        renderList();
        toast('AD-Suche fehlgeschlagen: ' + e);
      }
    }
    function scheduleLoadUsers(q) {
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => loadUsers(q), 250);
    }
    function renderList() {
      const list = $('#adList'); if (!list) return; list.innerHTML = '';
      if (!users.length) { list.appendChild(el('div', { class: 'empty', style: { padding: '24px' } }, 'Keine Treffer.')); return; }
      users.forEach((u) => {
        list.appendChild(el('div', { class: 'ad-item' + (selected && selected.sam === u.sam ? ' sel' : ''), onclick: () => { selected = u; renderList(); } },
          el('div', { class: 'av', style: { background: colorFor(u.sam) } }, inits(u.display)),
          el('div', {}, el('div', { class: 'u' }, u.display), el('div', { class: 'd' }, u.dept + ' · ' + u.sam))));
      });
    }

    const modal = el('div', { class: 'modal-scrim', onclick: (e) => { if (e.target.classList.contains('modal-scrim')) close(); } },
      el('div', { class: 'modal' },
        el('div', { class: 'modal-head' },
          el('div', {}, el('div', { class: 't' }, 'Benutzer zuordnen'), el('div', { class: 's' }, host)),
          el('div', { class: 'iconbtn', onclick: close }, svg('<svg width="13" height="13" viewBox="0 0 13 13"><line x1="1" y1="1" x2="12" y2="12" stroke="currentColor" stroke-width="1.4"/><line x1="12" y1="1" x2="1" y2="12" stroke="currentColor" stroke-width="1.4"/></svg>'))),
        el('div', { class: 'modal-body' },
          el('div', { class: 'ad-search' },
            el('div', {}, svg('<svg width="15" height="15" viewBox="0 0 15 15" fill="none"><circle cx="6.5" cy="6.5" r="4.6" stroke="#5f6776" stroke-width="1.3"/><line x1="10" y1="10" x2="13.5" y2="13.5" stroke="#5f6776" stroke-width="1.3" stroke-linecap="round"/></svg>')),
            el('input', { id: 'adSearch', placeholder: 'AD-Benutzer suchen …', autocomplete: 'off', oninput: (e) => scheduleLoadUsers(e.target.value) })),
          el('div', { class: 'ad-list', id: 'adList' }),
          el('textarea', { class: 'note-input', id: 'noteInput', placeholder: 'Notiz (optional) …', oninput: (e) => { note = e.target.value; } })),
        el('div', { class: 'modal-foot' },
          el('div', { class: 'btn', onclick: close }, 'Abbrechen'),
          el('div', { class: 'btn btn-primary', onclick: async () => {
            if (!selected) { toast('Bitte einen Benutzer auswählen.'); return; }
            try {
              await invoke('set_assignment', { host, user: selected.sam, userDisplay: selected.display, userDept: selected.dept || '', note });
              toast('Zugeordnet: ' + selected.display + ' → ' + host);
              close();
              await loadData();
              state.selected = host; renderDrawer();
            } catch (e) { toast('Speichern fehlgeschlagen: ' + e); }
          } }, 'Speichern'))));
    mount.innerHTML = ''; mount.appendChild(modal);
    loadUsers('');
  }
  function inits(name) {
    const p = (name || '').trim().split(/\s+/).filter(Boolean);
    return (((p[0] || '?')[0] || '?') + ((p[1] || '')[0] || '')).toUpperCase();
  }
  function colorFor(s) { const PAL = ['#4f8cff', '#2fd6a6', '#b98cff', '#ff8a4f', '#ffb454', '#5fc9ff', '#ff7a9c', '#7ee081']; let n = 0; for (let i = 0; i < s.length; i++) n = (n * 31 + s.charCodeAt(i)) >>> 0; return PAL[n % PAL.length]; }

  // ---------------- Dashboard / Abteilungen / Berichte ----------------
  function distBars(items, colorFn) {
    const max = Math.max(1, ...items.map((i) => i.count));
    return items.map((i) => el('div', { class: 'distrow' },
      el('div', { class: 'lbl' }, i.label || i.dept),
      el('div', { class: 'track' }, el('div', { style: { width: Math.round((i.count / max) * 100) + '%', background: colorFn ? colorFn(i) : 'var(--blue)' } })),
      el('div', { class: 'num' }, i.count)));
  }
  function renderDash() {
    const host = $('#dashView'); host.innerHTML = '';

    if (state.view === 'einstellungen') { renderSettings(); return; }

    const o = state.overview; if (!o) return;

    if (state.view === 'berichte') {
      host.appendChild(el('div', { class: 'panel' },
        el('h3', {}, 'Export & Berichte'),
        el('p', { style: { color: 'var(--muted)', fontSize: '13px', margin: '0 0 16px' } }, 'Erzeugt eine CSV mit allen Geräten, Spezifikationen, Status und Upgrade-Begründungen — geeignet für Excel und die Beschaffung.'),
        el('div', { style: { display: 'flex', gap: '10px' } },
          el('div', { class: 'btn btn-primary', onclick: doExport }, 'Vollständige Liste exportieren (CSV)'),
          el('div', { class: 'btn', onclick: () => { state.view = 'warnungen'; applyView(); } }, 'Nur Upgrade-Kandidaten ansehen'))));
      return;
    }

    if (state.view === 'gruppen') {
      host.appendChild(el('div', { class: 'panel' },
        el('h3', {}, 'Geräte je Abteilung'),
        distBars(o.byDept, (i) => i.upgrade > 0 ? 'var(--amber)' : 'var(--blue)')));
      host.appendChild(el('div', { style: { height: '14px' } }));
      host.appendChild(el('div', { class: 'panel' },
        el('h3', {}, 'Upgrade-Bedarf je Abteilung'),
        distBars(o.byDept.map((d) => ({ label: d.dept, count: d.upgrade })), () => 'var(--amber)')));
      return;
    }

    // dashboard
    const grid = el('div', { class: 'dash-grid' },
      el('div', { class: 'panel' }, el('h3', {}, 'Altersverteilung'), distBars(o.ageBuckets, (i) => /5/.test(i.label) && />/.test(i.label) ? 'var(--red)' : 'var(--blue)')),
      el('div', { class: 'panel' }, el('h3', {}, 'Arbeitsspeicher'), distBars(o.ramBuckets, (i) => /≤/.test(i.label) ? 'var(--amber)' : 'var(--green)')));
    host.appendChild(grid);
    host.appendChild(el('div', { style: { height: '14px' } }));
    host.appendChild(el('div', { class: 'panel' },
      el('h3', {}, 'Status & Abteilungen'),
      el('div', { style: { display: 'flex', gap: '8px', marginBottom: '16px', flexWrap: 'wrap' } },
        el('span', { class: 'tag ok' }, 'OK ' + o.status.ok),
        el('span', { class: 'tag upgrade' }, 'Upgrade ' + o.status.upgrade),
        el('span', { class: 'tag stale' }, 'Veraltet ' + o.status.stale),
        el('span', { class: 'tag missing' }, 'Kein Agent ' + o.status.missing)),
      distBars(o.byDept, (i) => i.upgrade > 0 ? 'var(--amber)' : 'var(--blue)')));
  }

  // ---------------- Einstellungen ----------------
  function renderSettings() {
    const host = $('#dashView'); host.innerHTML = '';
    const c = state.settings;
    if (!c) {
      host.appendChild(el('div', { class: 'panel' }, 'Einstellungen sind hier nicht verfügbar (Vorschau-Modus oder Ladefehler).'));
      return;
    }
    const th = c.thresholds || {};
    const txt = (v) => el('input', { class: 'set-input', type: 'text', value: v == null ? '' : String(v), autocomplete: 'off', spellcheck: 'false' });
    const num = (v, step) => el('input', { class: 'set-input', type: 'number', step: step || '1', min: '0', value: v == null ? '' : String(v), autocomplete: 'off' });
    const chk = (v) => { const e = el('input', { class: 'set-check', type: 'checkbox' }); e.checked = !!v; return e; };
    const field = (label, input, hint) => el('div', { class: 'set-field' }, el('label', {}, label), input, hint ? el('div', { class: 'set-hint' }, hint) : null);

    const iDataDir = txt(c.dataDir), iCsv = txt(c.masterCsvPath), iAssign = txt(c.assignmentsPath);
    const iAd = chk(c.adEnabled);
    const iRam = num(th.minRamGB), iAge = num(th.maxAgeYears, '0.1'), iStale = num(th.staleDays);
    const iCores = num(th.minCpuCores), iClock = num(th.minCpuClockMhz, '100'), iTarget = num(th.targetRamGB);
    const iSsd = chk(th.requireSsd);

    const save = async () => {
      const config = {
        dataDir: iDataDir.value.trim(),
        masterCsvPath: iCsv.value.trim(),
        assignmentsPath: iAssign.value.trim() || null,
        adEnabled: iAd.checked,
        thresholds: {
          minRamGB: parseInt(iRam.value, 10) || 0,
          maxAgeYears: parseFloat(iAge.value) || 0,
          staleDays: parseInt(iStale.value, 10) || 0,
          requireSsd: iSsd.checked,
          minCpuCores: parseInt(iCores.value, 10) || 0,
          minCpuClockMhz: parseInt(iClock.value, 10) || 0,
          targetRamGB: parseInt(iTarget.value, 10) || 0
        }
      };
      try {
        await invoke('set_settings', { config });
        toast('Einstellungen gespeichert.');
        await loadData();
        state.view = 'einstellungen'; applyView();
      } catch (e) { toast('Speichern fehlgeschlagen: ' + e); }
    };

    host.appendChild(el('div', { class: 'panel' },
      el('h3', {}, 'Datenquellen'),
      field('Inventar-Ordner (dataDir)', iDataDir, 'Client-beschreibbarer Inbox-Ordner, z. B. G:\\Inventory\\incoming'),
      field('Master-CSV (Rollout-Liste)', iCsv, 'z. B. G:\\Bitlocker\\Rollout_Masterliste.csv'),
      field('Zuordnungen (assignments.json)', iAssign, 'Nur-IT Control-Ordner, z. B. G:\\Inventory\\control\\assignments.json'),
      el('div', { class: 'set-check-row' }, iAd, el('label', {}, 'Active Directory aktivieren (read-only Lookup, kein RSAT)'))));

    host.appendChild(el('div', { style: { height: '14px' } }));

    host.appendChild(el('div', { class: 'panel' },
      el('h3', {}, 'Bewertungs-Schwellen'),
      el('div', { class: 'set-grid' },
        field('Min. RAM (GB)', iRam),
        field('Max. Alter (Jahre)', iAge),
        field('Veraltet ab (Tage)', iStale),
        field('Min. CPU-Kerne', iCores),
        field('Min. CPU-Takt (MHz, 0 = aus)', iClock),
        field('Ziel-RAM (GB)', iTarget)),
      el('div', { class: 'set-check-row' }, iSsd, el('label', {}, 'SSD erforderlich (HDD ⇒ Upgrade empfohlen)'))));

    host.appendChild(el('div', { style: { height: '14px' } }));
    host.appendChild(el('div', { style: { display: 'flex', gap: '10px' } },
      el('div', { class: 'btn btn-primary', onclick: save }, 'Speichern'),
      el('div', { class: 'btn', onclick: () => renderSettings() }, 'Zurücksetzen')));
  }

  async function doExport() {
    try { const r = await invoke('export_devices', { format: 'csv' }); toast('Exportiert (' + r.rows + ' Zeilen): ' + r.path); }
    catch (e) { toast('Export fehlgeschlagen: ' + e); }
  }

  // ---------------- view switching ----------------
  function applyView() {
    const v = VIEWS[state.view];
    $('#viewTitle').textContent = v.title;
    $('#viewSubtitle').textContent = v.sub;
    document.querySelectorAll('#nav .nav-item').forEach((n) => {
      const active = n.getAttribute('data-view') === state.view;
      n.classList.toggle('active', active);
      let bar = n.querySelector('.bar');
      if (active && !bar) n.insertBefore(el('span', { class: 'bar' }), n.firstChild);
      if (!active && bar) bar.remove();
    });
    if (v.list) {
      if (v.filter) state.filter = v.filter;
      $('#filters').classList.remove('hidden');
      $('#tableView').classList.remove('hidden');
      $('#dashView').classList.add('hidden');
      renderSegs(); renderThead(); renderRows();
    } else {
      $('#filters').classList.add('hidden');
      $('#tableView').classList.add('hidden');
      $('#dashView').classList.remove('hidden');
      renderDash();
    }
  }

  function renderAll() { renderKpis(); applyView(); }

  // ---------------- wire up ----------------
  function wire() {
    document.querySelectorAll('#nav .nav-item').forEach((n) => {
      n.addEventListener('click', () => { state.view = n.getAttribute('data-view'); state.selected = null; $('#drawerMount').innerHTML = ''; applyView(); });
    });
    $('#searchInput').addEventListener('input', (e) => { state.q = e.target.value.toLowerCase().trim(); renderSegs(); renderRows(); });
    $('#refreshBtn').addEventListener('click', async () => {
      $('#refreshLabel').textContent = 'Lädt …';
      try { await invoke('refresh'); await loadData(); toast('Daten aktualisiert.'); } catch (e) { toast('Aktualisieren fehlgeschlagen: ' + e); }
      $('#refreshLabel').textContent = 'Aktualisieren';
    });
    $('#exportBtn').addEventListener('click', doExport);

    // Fenstersteuerung (nur in Tauri aktiv)
    document.querySelectorAll('.winbtn').forEach((b) => b.addEventListener('click', async () => {
      if (!(TAURI && TAURI.window)) return;
      const w = TAURI.window.getCurrentWindow();
      const a = b.getAttribute('data-win');
      if (a === 'min') w.minimize(); else if (a === 'max') w.toggleMaximize(); else w.close();
    }));

    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') { state.selected = null; $('#drawerMount').innerHTML = ''; $('#modalMount').innerHTML = ''; renderRows(); } });
  }

  wire();
  loadData();
})();
