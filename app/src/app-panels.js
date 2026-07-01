/* app-panels.js — HardView modal, dashboard, settings and shell wiring. */
(function () {
  'use strict';
  const H = window.HardView;
  const { $, DEFAULT_THRESHOLDS, VIEWS, el, invoke, loadData, renderDrawer,
    renderKpis, renderList, renderRows, state, svg, toast, ViewModel } = H;
  // TAURI ist eine Funktion (zur-Laufzeit-Lookup, kein Snapshot) — gibt die API oder null.
  const tapi = H.TAURI;
  const { hashColor: colorFor } = window.HVShared;

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
              const result = await invoke('set_assignment', { host, user: selected.sam, userDisplay: selected.display, userDept: selected.dept || '', note });
              toast('Zugeordnet: ' + selected.display + ' → ' + host);
              close();
              if (result && result.device) {
                const i = state.devices.findIndex((d) => d.host === result.device.host);
                if (i >= 0) state.devices[i] = result.device; else state.devices.push(result.device);
                state.overview = await invoke('get_overview');
                renderKpis(); applyView();
              } else {
                await loadData();
              }
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
  // ---------------- Dashboard / Abteilungen / Berichte ----------------
  function distBars(items, colorFn) {
    const max = Math.max(1, ...items.map((i) => i.count));
    return items.map((i) => el('div', { class: 'distrow' },
      el('div', { class: 'lbl' }, i.label || i.dept),
      el('div', { class: 'track' }, el('div', { style: { width: Math.round((i.count / max) * 100) + '%', background: colorFn ? colorFn(i) : 'var(--blue)' } })), el('div', { class: 'num' }, i.count)));
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
          el('div', { class: 'btn', onclick: () => switchView('warnungen') }, 'Nur Upgrade-Kandidaten ansehen'))));
      return;
    }

    if (state.view === 'gruppen') {
      host.appendChild(el('div', { class: 'panel' },
        el('h3', {}, 'Geräte je Abteilung'),
        distBars(o.byDept, (i) => i.needsAction > 0 ? 'var(--amber)' : 'var(--blue)')));
      host.appendChild(el('div', { style: { height: '14px' } }));
      host.appendChild(el('div', { class: 'panel' },
        el('h3', {}, 'Upgrade-Bedarf je Abteilung'),
        distBars(o.byDept.map((d) => ({ label: d.dept, count: d.needsAction })), () => 'var(--amber)')));
      return;
    }

    // dashboard
    // Farbe positional statt per Label-Regex bestimmen: der letzte Age-Bucket ist per
    // Konstruktion (overview.rs/mock.js) immer der "ueber Schwellwert"-Bucket, der
    // erste RAM-Bucket immer der "wenig RAM"-Bucket.
    const grid = el('div', { class: 'dash-grid' },
      el('div', { class: 'panel' }, el('h3', {}, 'Altersverteilung'), distBars(o.ageBuckets, (i) => i === o.ageBuckets[o.ageBuckets.length - 1] ? 'var(--red)' : 'var(--blue)')),
      el('div', { class: 'panel' }, el('h3', {}, 'Arbeitsspeicher'), distBars(o.ramBuckets, (i) => i === o.ramBuckets[0] ? 'var(--amber)' : 'var(--green)')));
    host.appendChild(grid);
    host.appendChild(el('div', { style: { height: '14px' } }));
    host.appendChild(el('div', { class: 'panel' },
      el('h3', {}, 'Status & Abteilungen'),
      el('div', { style: { display: 'flex', gap: '8px', marginBottom: '16px', flexWrap: 'wrap' } },
        el('span', { class: 'tag ok' }, 'OK ' + o.status.ok),
        el('span', { class: 'tag upgrade' }, 'Upgrade ' + o.upgradeNeeded),
        el('span', { class: 'tag stale' }, 'Veraltet ' + o.status.stale),
        el('span', { class: 'tag missing' }, 'Kein Agent ' + o.status.missing)),
      distBars(o.byDept, (i) => i.needsAction > 0 ? 'var(--amber)' : 'var(--blue)')));
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
    const num = (v, step, min) => el('input', { class: 'set-input', type: 'number', step: step || '1', min: min == null ? '0' : String(min), value: v == null ? '' : String(v), autocomplete: 'off' });
    const chk = (v) => { const e = el('input', { class: 'set-check', type: 'checkbox' }); e.checked = !!v; return e; };
    const field = (label, input, hint) => el('div', { class: 'set-field' }, el('label', {}, label), input, hint ? el('div', { class: 'set-hint' }, hint) : null);

    const iDataDir = txt(c.dataDir), iCsv = txt(c.masterCsvPath), iAssign = txt(c.assignmentsPath);
    const iAd = chk(c.adEnabled);
    const iRam = num(th.minRamGB), iAge = num(th.maxAgeYears, '0.1', '0.1'), iStale = num(th.staleDays, '1', '1');
    const iCores = num(th.minCpuCores), iClock = num(th.minCpuClockMhz, '100'), iTarget = num(th.targetRamGB, '1', '1');
    const iSsd = chk(th.requireSsd);

    const save = async () => {
      const intValue = (input, fallback, min) => {
        const parsed = parseInt(input.value, 10);
        return Number.isFinite(parsed) ? Math.max(min, parsed) : fallback;
      };
      const floatValue = (input, fallback, min) => {
        const parsed = parseFloat(input.value.replace(',', '.'));
        return Number.isFinite(parsed) ? Math.max(min, parsed) : fallback;
      };
      const config = {
        dataDir: iDataDir.value.trim(),
        masterCsvPath: iCsv.value.trim(),
        assignmentsPath: iAssign.value.trim() || null,
        adEnabled: iAd.checked,
        thresholds: {
          minRamGB: intValue(iRam, DEFAULT_THRESHOLDS.minRamGB, 0),
          maxAgeYears: floatValue(iAge, DEFAULT_THRESHOLDS.maxAgeYears, 0.1),
          staleDays: intValue(iStale, DEFAULT_THRESHOLDS.staleDays, 1),
          requireSsd: iSsd.checked,
          minCpuCores: intValue(iCores, DEFAULT_THRESHOLDS.minCpuCores, 0),
          minCpuClockMhz: intValue(iClock, DEFAULT_THRESHOLDS.minCpuClockMhz, 0),
          targetRamGB: intValue(iTarget, DEFAULT_THRESHOLDS.targetRamGB, 1)
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
      $('#filters').classList.remove('hidden');
      $('#tableView').classList.remove('hidden');
      $('#dashView').classList.add('hidden');
      renderList();
    } else {
      $('#filters').classList.add('hidden');
      $('#tableView').classList.add('hidden');
      $('#dashView').classList.remove('hidden');
      renderDash();
    }
  }

  function switchView(view) {
    ViewModel.applyViewChange(state, VIEWS, view);
    $('#drawerMount').innerHTML = '';
    applyView();
  }

  function renderAll() { renderKpis(); applyView(); }

  // ---------------- wire up ----------------
  function wire() {
    document.querySelectorAll('#nav .nav-item').forEach((n) => {
      n.addEventListener('click', () => switchView(n.getAttribute('data-view')));
    });
    let searchTimer = null;
    $('#searchInput').addEventListener('input', (e) => {
      state.q = e.target.value.toLowerCase().trim();
      clearTimeout(searchTimer);
      searchTimer = setTimeout(renderList, 150);
    });
    let refreshing = false;
    $('#refreshBtn').addEventListener('click', async () => {
      if (refreshing) return;
      refreshing = true;
      const btn = $('#refreshBtn');
      btn.classList.add('busy'); $('#refreshLabel').textContent = 'Lädt …';
      try { await invoke('refresh'); await loadData(); toast('Daten aktualisiert.'); } catch (e) { toast('Aktualisieren fehlgeschlagen: ' + e); }
      $('#refreshLabel').textContent = 'Aktualisieren'; btn.classList.remove('busy'); refreshing = false;
    });
    $('#exportBtn').addEventListener('click', doExport);

    // Fenstersteuerung (nur in Tauri aktiv)
    document.querySelectorAll('.winbtn').forEach((b) => b.addEventListener('click', async () => {
      const t = tapi();
      if (!(t && t.window)) return;
      const w = t.window.getCurrentWindow();
      const a = b.getAttribute('data-win');
      if (a === 'min') w.minimize(); else if (a === 'max') w.toggleMaximize(); else w.close();
    }));

    // Escape schliesst nur die oberste Schicht: zuerst ein offenes Modal, dann den Drawer.
    document.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape') return;
      const modal = $('#modalMount');
      if (modal && modal.children.length) { modal.innerHTML = ''; return; }
      const drawer = $('#drawerMount');
      if (drawer && drawer.children.length) { state.selected = null; drawer.innerHTML = ''; renderRows(); }
    });
  }

  H.openAssign = openAssign;
  H.renderAll = renderAll;

  wire();
  loadData();
})();
