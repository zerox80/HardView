/* shared.js — kleine Helfer, die von mehreren Frontend-Dateien genutzt werden
 * (Avatar-Farbe, deutsche Zahlenformatierung). Browser: an window.HVShared gehaengt
 * (muss vor mock.js/view-model.js/app-panels.js geladen werden). Node: als
 * CommonJS-Modul exportiert, damit Tests dieselbe Implementierung nutzen. */
(function () {
  'use strict';

  const PALETTE = ['#4f8cff', '#2fd6a6', '#b98cff', '#ff8a4f', '#ffb454', '#5fc9ff', '#ff7a9c', '#7ee081'];

  function hashColor(s) {
    let n = 0;
    for (let i = 0; i < s.length; i++) n = (n * 31 + s.charCodeAt(i)) >>> 0;
    return PALETTE[n % PALETTE.length];
  }

  // Deutsche Dezimaldarstellung, 1 Nachkommastelle — spiegelt fmt_de() aus upgrade.rs.
  function fmtDe(v) {
    return Number(v).toFixed(1).replace('.', ',');
  }

  const api = { PALETTE, hashColor, fmtDe };
  if (typeof window !== 'undefined') { window.HVShared = api; }
  if (typeof module !== 'undefined' && module.exports) { module.exports = api; }
})();
