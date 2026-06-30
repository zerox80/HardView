/* view-model.js - pure UI state helpers shared by browser code and Node tests. */
(function (root, factory) {
  'use strict';
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  if (root) root.HardViewViewModel = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  'use strict';

  const STATUS_RANK = { ok: 0, upgrade: 1, stale: 2, missing: 3 };

  function lower(value) {
    return value == null ? '' : String(value).toLowerCase();
  }

  // Deutsche Dezimaldarstellung, 1 Nachkommastelle - spiegelt fmt_de() aus upgrade.rs.
  function fmtDe(value) {
    return Number(value).toFixed(1).replace('.', ',');
  }

  function isUpgradeCandidate(device) {
    const reasons = device.upgradeReasons || device.upgrade_reasons || [];
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

  function sortValue(device, key) {
    switch (key) {
      case 'user': return lower(device.user);
      case 'cpu': return lower(device.cpu);
      case 'ram': return Number(device.ramGB) || 0;
      case 'age': return device.ageYears == null ? -1 : Number(device.ageYears);
      case 'status': return STATUS_RANK[device.status] == null ? 99 : STATUS_RANK[device.status];
      default: return lower(device.host);
    }
  }

  function compareHost(a, b) {
    const av = lower(a.host);
    const bv = lower(b.host);
    if (av < bv) return -1;
    if (av > bv) return 1;
    return 0;
  }

  function visibleDevices(devices, state) {
    const viewState = state || {};
    const filter = viewState.filter || 'all';
    const query = lower(viewState.q).trim();
    const sort = viewState.sort || 'host';
    const dir = viewState.dir === 'desc' ? -1 : 1;

    return (devices || [])
      .filter((device) => matchesFilter(device, filter) && matchesQuery(device, query))
      .sort((a, b) => {
        const av = sortValue(a, sort);
        const bv = sortValue(b, sort);
        if (av < bv) return -1 * dir;
        if (av > bv) return 1 * dir;
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
