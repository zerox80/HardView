/* view-model.js - pure UI state helpers shared by browser code and Node tests. */
(function (root, factory) {
  'use strict';
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  if (root) root.HardViewViewModel = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  'use strict';

  const { fmtDe } = (typeof window !== 'undefined' && window.HVShared)
    ? window.HVShared
    : require('./shared.js');

  const STATUS_RANK = { ok: 0, upgrade: 1, stale: 2, missing: 3 };

  function lower(value) {
    return value == null ? '' : String(value).toLowerCase();
  }

  function isUpgradeCandidate(device) {
    const reasons = device.upgradeReasons || [];
    return device.status === 'upgrade' || (device.status === 'stale' && reasons.length > 0);
  }

  function matchesFilter(device, filter) {
    if (!filter || filter === 'all') return true;
    if (filter === 'veraltet') return device.status === 'stale' || device.status === 'missing';
    if (filter === 'upgrade') return isUpgradeCandidate(device);
    return device.status === filter;
  }

  function matchesQuery(device, query) {
    if (!query) return true;
    const haystack = [device.host, device.user, device.cpu, device.dept].map(lower).join(' ');
    return haystack.indexOf(query) !== -1;
  }

  // Locale-bewusste deutsche Kollation (Umlaute wie ä/ö/ü korrekt einsortieren,
  // Ziffern numerisch: "WS-AB-2" vor "WS-AB-10"). Wird fuer Host-, User- und
  // CPU-Vergleiche genutzt; numerische Schluessel vergleichen wir direkt.
  const LOCALE_OPTS = { numeric: true, sensitivity: 'base' };
  function compareText(a, b) {
    return String(a).localeCompare(String(b), 'de', LOCALE_OPTS);
  }
  function compareHost(a, b) {
    return compareText(lower(a.host), lower(b.host));
  }

  function visibleDevices(devices, state) {
    const viewState = state || {};
    const filter = viewState.filter || 'all';
    const query = lower(viewState.q).trim();
    const sort = viewState.sort || 'host';
    const dir = viewState.dir === 'desc' ? -1 : 1;

    // Sortier-Schluessel pro Geraet einmal vorausberechnen (kein O(n log n) lower-
    // casing je Vergleich) — Strings direkt via localeCompare in der Sort-Methode.
    const cache = new Map();
    const keyOf = (d) => {
      if (cache.has(d)) return cache.get(d);
      let key;
      switch (sort) {
        case 'ram': key = Number(d.ramGB) || 0; break;
        case 'age': key = d.ageYears == null ? -1 : Number(d.ageYears); break;
        case 'status': key = STATUS_RANK[d.status] == null ? 99 : STATUS_RANK[d.status]; break;
        case 'user': key = lower(d.user); break;
        case 'cpu': key = lower(d.cpu); break;
        default: key = lower(d.host); break;
      }
      cache.set(d, key);
      return key;
    };

    return (devices || [])
      .filter((device) => matchesFilter(device, filter) && matchesQuery(device, query))
      .sort((a, b) => {
        const av = keyOf(a);
        const bv = keyOf(b);
        let cmp;
        if (typeof av === 'number' && typeof bv === 'number') {
          cmp = av < bv ? -1 : av > bv ? 1 : 0;
        } else {
          cmp = compareText(av, bv);
        }
        if (cmp !== 0) return cmp * dir;
        return compareHost(a, b);
      });
  }

  function applyViewChange(state, views, nextView) {
    const previousView = state.view;
    const changed = previousView !== nextView;
    state.view = nextView;
    state.selected = null;
    if (changed) {
      const view = views && views[nextView];
      if (view && view.list && view.filter) state.filter = view.filter;
    }
    return state;
  }

  return {
    applyViewChange,
    fmtDe,
    isUpgradeCandidate,
    visibleDevices
  };
});
