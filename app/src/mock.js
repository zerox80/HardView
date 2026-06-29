/* mock.js — NUR fuer Browser-Vorschau / Entwicklung ohne Tauri.
 * Liefert exakt die gleichen Objektformen wie das Rust-Backend (commands.rs),
 * damit app.js identisch fuer Vorschau UND echte App funktioniert.
 * In der echten Tauri-App ist window.__TAURI__ vorhanden und dieser Code wird NICHT genutzt.
 * Die Berechnungslogik hier spiegelt store.rs/upgrade.rs wider (Quelle der Wahrheit = Rust). */
(function () {
  const DEFAULT_THRESHOLDS = { minRamGB: 8, maxAgeYears: 5, staleDays: 30, requireSsd: true, minCpuCores: 4, minCpuClockMhz: 0, targetRamGB: 16 };
  let THRESH = Object.assign({}, DEFAULT_THRESHOLDS);
  const PALETTE = ['#4f8cff', '#2fd6a6', '#b98cff', '#ff8a4f', '#ffb454', '#5fc9ff', '#ff7a9c', '#7ee081'];

  // Roh-PCs (entspricht sample-data). inv=false -> in CSV aber ohne Agent-JSON.
  const PCS = [
    { h:'WS-MARKETING-04', f:'Lena', l:'Hoffmann', d:'Marketing', cpu:'Intel Core i5-12500', c:6, t:12, ram:16, su:2, st:4, disk:'SSD', dgb:512, os:'Windows 11 Pro', b:'22631', age:2.1, stale:1, inv:true, mfg:'Dell Inc.', mdl:'OptiPlex 7090' },
    { h:'WS-VERTRIEB-11', f:'Markus', l:'Bauer', d:'Vertrieb', cpu:'Intel Core i7-13700', c:16, t:24, ram:32, su:2, st:4, disk:'SSD', dgb:1024, os:'Windows 11 Pro', b:'22631', age:1.2, stale:0, inv:true, mfg:'Lenovo', mdl:'ThinkCentre M90t' },
    { h:'WS-BUCH-02', f:'Sabine', l:'Köhler', d:'Buchhaltung', cpu:'Intel Core i5-10400', c:6, t:12, ram:8, su:1, st:4, disk:'SSD', dgb:256, os:'Windows 10 Pro', b:'19045', age:4.6, stale:2, inv:true, mfg:'HP', mdl:'EliteDesk 800 G6' },
    { h:'WS-IT-07', f:'Daniel', l:'Richter', d:'IT', cpu:'AMD Ryzen 7 5800X', c:8, t:16, ram:32, su:2, st:4, disk:'SSD', dgb:1024, os:'Windows 11 Pro', b:'22631', age:3.1, stale:0, inv:true, mfg:'Custom', mdl:'Workstation', assigned:true },
    { h:'WS-ENTW-15', f:'Tobias', l:'Wolf', d:'Entwicklung', cpu:'Intel Core i9-13900K', c:24, t:32, ram:64, su:4, st:4, disk:'SSD', dgb:2048, os:'Windows 11 Pro', b:'22631', age:0.8, stale:0, inv:true, mfg:'Dell Inc.', mdl:'Precision 3660' },
    { h:'WS-PERSONAL-03', f:'Andrea', l:'Schulz', d:'Personal', cpu:'Intel Core i3-10100', c:4, t:8, ram:8, su:1, st:2, disk:'HDD', dgb:500, os:'Windows 10 Pro', b:'19045', age:5.3, stale:3, inv:true, mfg:'HP', mdl:'ProDesk 400 G6' },
    { h:'WS-VERTRIEB-08', f:'Kevin', l:'Braun', d:'Vertrieb', cpu:'AMD Ryzen 5 5600', c:6, t:12, ram:16, su:2, st:2, disk:'SSD', dgb:512, os:'Windows 11 Pro', b:'22621', age:2.4, stale:1, inv:true, mfg:'Lenovo', mdl:'ThinkCentre M75q' },
    { h:'WS-LAGER-01', f:'Petra', l:'Lang', d:'Lager', cpu:'Intel Core i3-8100', c:4, t:4, ram:8, su:2, st:2, disk:'HDD', dgb:500, os:'Windows 10 Pro', b:'19044', age:6.7, stale:45, inv:true, mfg:'Fujitsu', mdl:'Esprimo D538' },
    { h:'WS-ENTW-09', f:'Jonas', l:'Frank', d:'Entwicklung', cpu:'AMD Ryzen 7 7700', c:8, t:16, ram:32, su:2, st:4, disk:'SSD', dgb:1024, os:'Windows 11 Pro', b:'22631', age:1.0, stale:0, inv:true, mfg:'Dell Inc.', mdl:'Precision 3460' },
    { h:'WS-MARKETING-06', f:'Nina', l:'Albrecht', d:'Marketing', cpu:'Intel Core i5-12500', c:6, t:12, ram:16, su:2, st:4, disk:'SSD', dgb:512, os:'Windows 11 Pro', b:'22631', age:2.0, stale:0, inv:true, mfg:'Dell Inc.', mdl:'OptiPlex 7090' },
    { h:'WS-GF-01', f:'Stefan', l:'Klein', d:'Geschäftsführung', cpu:'Intel Core i7-13700', c:16, t:24, ram:32, su:2, st:4, disk:'SSD', dgb:1024, os:'Windows 11 Pro', b:'22631', age:1.5, stale:0, inv:true, mfg:'Lenovo', mdl:'ThinkPad X1 Carbon' },
    { h:'WS-BUCH-05', f:'Claudia', l:'Neumann', d:'Buchhaltung', cpu:'Intel Core i5-10400', c:6, t:12, ram:16, su:2, st:4, disk:'SSD', dgb:512, os:'Windows 10 Pro', b:'19045', age:4.2, stale:4, inv:true, mfg:'HP', mdl:'EliteDesk 800 G6' },
    { h:'WS-IT-02', f:'Sven', l:'Hartmann', d:'IT', cpu:'AMD Ryzen 9 5900X', c:12, t:24, ram:32, su:4, st:4, disk:'SSD', dgb:1024, os:'Windows 11 Pro', b:'22631', age:3.0, stale:0, inv:true, mfg:'Custom', mdl:'Workstation' },
    { h:'WS-EMPFANG-01', f:'Julia', l:'Vogt', d:'Empfang', cpu:'Intel Core i3-7100', c:2, t:4, ram:8, su:2, st:2, disk:'HDD', dgb:250, os:'Windows 10 Pro', b:'19045', age:7.5, stale:6, inv:true, mfg:'Fujitsu', mdl:'Esprimo P558' },
    { h:'WS-VERTRIEB-14', f:'Florian', l:'Maier', d:'Vertrieb', cpu:'Intel Core i5-12500', c:6, t:12, ram:16, su:2, st:4, disk:'SSD', dgb:512, os:'Windows 11 Pro', b:'22621', age:2.3, stale:90, inv:true, mfg:'Dell Inc.', mdl:'OptiPlex 7090' },
    { h:'WS-ENTW-21', f:'Carolin', l:'Busch', d:'Entwicklung', cpu:'AMD Ryzen 7 5800X', c:8, t:16, ram:32, su:2, st:4, disk:'SSD', dgb:1024, os:'Windows 11 Pro', b:'22631', age:3.2, stale:0, inv:true, mfg:'Dell Inc.', mdl:'Precision 3650' },
    { h:'WS-BUCH-08', f:'Thomas', l:'Wagner', d:'Buchhaltung', cpu:'Intel Core i5-9500', c:6, t:6, ram:8, su:2, st:4, disk:'HDD', dgb:1000, os:'Windows 10 Pro', b:'19045', age:5.8, stale:0, inv:false, mfg:'HP', mdl:'EliteDesk 705 G5' },
    { h:'WS-LAGER-04', f:'Michael', l:'Scholz', d:'Lager', cpu:'Intel Celeron G4900', c:2, t:2, ram:4, su:1, st:2, disk:'HDD', dgb:500, os:'Windows 10 Pro', b:'19044', age:6.9, stale:0, inv:false, mfg:'Fujitsu', mdl:'Esprimo D538' }
  ];

  function hashColor(s) { let n = 0; for (let i = 0; i < s.length; i++) n = (n * 31 + s.charCodeAt(i)) >>> 0; return PALETTE[n % PALETTE.length]; }
  function initials(f, l) { return ((f || '?')[0] + (l || '?')[0]).toUpperCase(); }
  function osShort(os, b) { const w = os.includes('11') ? 'Win 11' : os.includes('10') ? 'Win 10' : os; const map = { '22631':'23H2','22621':'22H2','19045':'22H2','19044':'21H2','26100':'24H2','26200':'25H2' }; return w + (map[b] ? ' ' + map[b] : ''); }
  function lastSeen(days) { if (days == null) return '—'; if (days < -1) return 'Zeitstempel in Zukunft'; if (days < 1) return 'gerade eben'; if (days === 1) return 'vor 1 Tag'; return 'vor ' + days + ' Tagen'; }
  // Deutsche Dezimaldarstellung, 1 Nachkommastelle — spiegelt fmt_de() aus upgrade.rs.
  function fmtDe(v) { return Number(v).toFixed(1).replace('.', ','); }
  function intAtLeast(value, fallback, min) {
    const parsed = parseInt(value, 10);
    return Number.isFinite(parsed) ? Math.max(min, parsed) : fallback;
  }
  function numberAtLeast(value, fallback, min) {
    const parsed = parseFloat(value);
    return Number.isFinite(parsed) ? Math.max(min, parsed) : fallback;
  }
  function normalizeThresholds(input, base) {
    const src = Object.assign({}, base || DEFAULT_THRESHOLDS, input || {});
    return {
      minRamGB: intAtLeast(src.minRamGB, DEFAULT_THRESHOLDS.minRamGB, 0),
      maxAgeYears: numberAtLeast(src.maxAgeYears, DEFAULT_THRESHOLDS.maxAgeYears, 0.1),
      staleDays: intAtLeast(src.staleDays, DEFAULT_THRESHOLDS.staleDays, 1),
      requireSsd: !!src.requireSsd,
      minCpuCores: intAtLeast(src.minCpuCores, DEFAULT_THRESHOLDS.minCpuCores, 0),
      minCpuClockMhz: intAtLeast(src.minCpuClockMhz, DEFAULT_THRESHOLDS.minCpuClockMhz, 0),
      targetRamGB: intAtLeast(src.targetRamGB, DEFAULT_THRESHOLDS.targetRamGB, 1)
    };
  }

  // Bewertung eines Geraets aus zusammengefuehrten Fakten. Spiegelt evaluate() aus
  // upgrade.rs exakt (inkl. Reihenfolge, "> 0"-Schutz und Begruendungstexte).
  // Geteilte Golden-Vectors stellen die Parity sicher (shared/test-vectors).
  function evaluate(th, f) {
    if (!f.hasInventory) {
      return { status: 'missing', statusLabel: 'Kein Inventar', reasons: ['Kein Inventar — Agent hat noch nie gemeldet'] };
    }
    const reasons = [];
    if (f.ageYears != null && f.ageYears > th.maxAgeYears) reasons.push('Gerät alt (' + fmtDe(f.ageYears) + ' Jahre)');
    if (f.ramGB > 0 && f.ramGB <= th.minRamGB) reasons.push('RAM knapp (' + f.ramGB + ' GB)');
    if (th.requireSsd && f.diskIsSsd === false) reasons.push('HDD statt SSD');
    if (f.cpuCores > 0 && f.cpuCores < th.minCpuCores) reasons.push('CPU schwach (' + f.cpuCores + ' Kerne)');
    if (th.minCpuClockMhz > 0 && f.cpuClockMhz > 0 && f.cpuClockMhz < th.minCpuClockMhz) reasons.push('CPU-Takt niedrig (' + f.cpuClockMhz + ' MHz)');
    if (f.osIsWin11 === false) reasons.push('Kein Windows 11 (Win 10 EOL)');
    const futureTimestamp = f.lastSeenDays != null && f.lastSeenDays < -1;
    if (futureTimestamp || (f.lastSeenDays != null && f.lastSeenDays > th.staleDays)) {
      return { status: 'stale', statusLabel: futureTimestamp ? 'Unplausibel · Zeitstempel in Zukunft' : 'Veraltet · Agent meldet nicht', reasons };
    }
    if (reasons.length) return { status: 'upgrade', statusLabel: 'Upgrade empfohlen', reasons };
    return { status: 'ok', statusLabel: 'Aktuell · OK', reasons };
  }

  function compute(pc) {
    const hasInv = pc.inv;
    const ev = evaluate(THRESH, {
      hasInventory: hasInv,
      ramGB: pc.ram,
      ageYears: hasInv ? pc.age : null,
      diskIsSsd: pc.disk == null ? null : (pc.disk === 'SSD' || pc.disk === 'SCM'),
      cpuCores: pc.c,
      cpuClockMhz: pc.clock || 0,
      osIsWin11: pc.os ? pc.os.includes('11') : null,
      lastSeenDays: hasInv ? pc.stale : null
    });
    const status = ev.status, statusLabel = ev.statusLabel, reasons = ev.reasons;
    const user = pc.f + ' ' + pc.l;
    return {
      host: pc.h, hasInventory: hasInv, status, statusLabel, upgradeReasons: reasons,
      user, userDisplay: user, userSam: (pc.f + '.' + pc.l).toLowerCase(),
      userSource: pc.assigned ? 'manuell bestätigt' : 'Rollout-Liste',
      dept: pc.d, initials: initials(pc.f, pc.l), avatarColor: hashColor(pc.h),
      cpu: pc.cpu, cores: pc.c, coresText: pc.c + ' Kerne / ' + pc.t + ' Threads',
      ramGB: pc.ram, ramSlotsUsed: pc.su, ramSlotsTotal: pc.st, ramFreeSlots: pc.st - pc.su, ramTargetGB: THRESH.targetRamGB,
      diskType: pc.disk, diskGB: pc.dgb, diskModel: pc.disk === 'SSD' ? 'Samsung SSD 870' : 'Seagate Barracuda',
      ageYears: hasInv ? pc.age : null, ageText: hasInv ? (fmtDe(pc.age) + ' J.') : '—',
      lastSeenDays: hasInv ? pc.stale : null, lastSeenText: hasInv ? lastSeen(pc.stale) : 'nie',
      osShort: osShort(pc.os, pc.b), osCaption: pc.os, osBuild: '10.0.' + pc.b,
      chassis: /ThinkPad|Carbon/.test(pc.mdl) ? 'Laptop' : 'Desktop',
      manufacturer: pc.mfg, model: pc.mdl, serialNumber: 'SN' + pc.h.replace(/\W/g, '').slice(-6),
      biosVersion: '1.12.0', biosDate: hasInv ? new Date(Date.now() - pc.age * 365 * 864e5).toISOString().slice(0, 10) : null,
      gpus: ['Intel UHD Graphics'], ip: '10.4.' + (10 + (pc.h.length % 8)) + '.' + (20 + pc.h.length), mac: '00:1A:2B:3C:4D:5E',
      installDate: null, lastBoot: null, tpm: pc.os.includes('11'), secureBoot: pc.os.includes('11'),
      ramSticks: Array.from({ length: pc.su }, (_, i) => ({ capacityGB: Math.round(pc.ram / pc.su), speedMhz: 3200, slot: 'DIMM' + i })),
      note: pc.assigned ? 'Gerät nach Abteilungswechsel bestätigt.' : '', confirmedBy: pc.assigned ? 'CORP\\T.Administrator' : null,
      collectedAtUtc: hasInv ? new Date(Date.now() - pc.stale * 864e5).toISOString() : null
    };
  }

  function overview(devs) {
    const total = devs.length;
    const withInv = devs.filter(d => d.hasInventory).length;
    const stale = devs.filter(d => d.status === 'stale').length;
    const missing = devs.filter(d => d.status === 'missing').length;
    const needsUpgrade = d => d.status === 'upgrade' || (d.status === 'stale' && (d.upgradeReasons || []).length > 0);
    const needsAction = d => needsUpgrade(d) || d.status === 'missing';
    const statusUpgrade = devs.filter(d => d.status === 'upgrade').length;
    const upgrade = devs.filter(needsUpgrade).length;
    const ok = devs.filter(d => d.status === 'ok').length;
    const aged = devs.filter(d => d.ageYears != null);
    const avgAge = aged.length ? (aged.reduce((a, d) => a + d.ageYears, 0) / aged.length) : 0;
    const old5 = devs.filter(d => d.ageYears != null && d.ageYears > THRESH.maxAgeYears).length;
    const depts = {};
    devs.forEach(d => { (depts[d.dept] = depts[d.dept] || { dept: d.dept, count: 0, upgrade: 0 }); depts[d.dept].count++; if (needsAction(d)) depts[d.dept].upgrade++; });
    const ageBuckets = [
      { label: '< 2 Jahre', count: aged.filter(d => d.ageYears < 2).length },
      { label: '2–4 Jahre', count: aged.filter(d => d.ageYears >= 2 && d.ageYears < 4).length },
      { label: '4–5 Jahre', count: aged.filter(d => d.ageYears >= 4 && d.ageYears <= 5).length },
      { label: '> 5 Jahre', count: aged.filter(d => d.ageYears > 5).length }
    ];
    const withInvDevs = devs.filter(d => d.hasInventory);
    const ramBuckets = [
      { label: '≤ 8 GB', count: withInvDevs.filter(d => d.ramGB <= 8).length },
      { label: '9–16 GB', count: withInvDevs.filter(d => d.ramGB > 8 && d.ramGB <= 16).length },
      { label: '17–32 GB', count: withInvDevs.filter(d => d.ramGB > 16 && d.ramGB <= 32).length },
      { label: '> 32 GB', count: withInvDevs.filter(d => d.ramGB > 32).length }
    ];
    return {
      total, withInventory: withInv, stale, missing, upgradeNeeded: upgrade, ok,
      current: withInv - stale, avgAgeYears: Math.round(avgAge * 10) / 10, old5,
      oldAgeLabel: '> ' + fmtDe(THRESH.maxAgeYears) + ' Jahre',
      deptCount: Object.keys(depts).length,
      byDept: Object.values(depts).sort((a, b) => b.count - a.count),
      ageBuckets, ramBuckets,
      status: { ok, upgrade: statusUpgrade, stale, missing }
    };
  }

  function applyAssignment(d, args) {
    if (!args) return d;
    d.user = args.userDisplay || args.user;
    d.userDisplay = args.userDisplay || args.user;
    d.userSam = args.user || '';
    d.userSource = 'manuell bestätigt';
    d.dept = args.userDept || d.dept;
    d.note = args.note || '';
    d.confirmedBy = 'CORP\\T.Administrator';
    d.initials = (d.userDisplay.split(' ').map(s => s[0]).join('').slice(0, 2)).toUpperCase();
    return d;
  }

  function rebuildDevices() {
    return PCS.map(compute).map(d => applyAssignment(d, ASSIGN[d.host]));
  }

  const AD_USERS = PCS.map(p => ({ sam: (p.f + '.' + p.l).toLowerCase(), display: p.f + ' ' + p.l, dept: p.d, mail: (p.f + '.' + p.l).toLowerCase() + '@example.com' }))
    .concat([
      { sam: 'a.berger', display: 'Anna Berger', dept: 'Marketing', mail: 'a.berger@example.com' },
      { sam: 'm.fischer', display: 'Martin Fischer', dept: 'IT', mail: 'm.fischer@example.com' },
      { sam: 's.weber', display: 'Sophie Weber', dept: 'Personal', mail: 's.weber@example.com' }
    ]);

  const ASSIGN = {};
  let DEVICES = rebuildDevices();
  // Spiegelt die Config-Form des Rust-Backends (commands.rs get_settings/set_settings).
  const SETTINGS = {
    dataDir: 'C:\\(Vorschau)\\Inventory\\incoming',
    masterCsvPath: 'C:\\(Vorschau)\\Rollout_Masterliste.csv',
    assignmentsPath: 'C:\\(Vorschau)\\Inventory\\control\\assignments.json',
    adEnabled: false,
    thresholds: Object.assign({}, THRESH)
  };

  const MOCK = {
    async invoke(cmd, args) {
      await new Promise(r => setTimeout(r, 60)); // kleine Latenz wie echtes Backend
      switch (cmd) {
        case 'get_devices': return DEVICES.map(d => ({ ...d }));
        case 'get_device': return DEVICES.find(d => d.host === args.host) || null;
        case 'get_overview': return overview(DEVICES);
        case 'get_ad_users': {
          const q = (args.search || '').toLowerCase();
          return AD_USERS.filter(u => !q || (u.display + ' ' + u.sam + ' ' + u.dept).toLowerCase().includes(q)).slice(0, 50);
        }
        case 'set_assignment': {
          const d = DEVICES.find(x => x.host === args.host);
          if (!d) throw new Error('Geraet ist nicht in Inventar oder Masterliste vorhanden');
          ASSIGN[args.host] = Object.assign({}, args);
          applyAssignment(d, ASSIGN[args.host]);
          return { ok: true };
        }
        case 'get_settings': return JSON.parse(JSON.stringify(SETTINGS));
        case 'set_settings': {
          const c = (args && args.config) || {};
          if (c.dataDir != null) SETTINGS.dataDir = c.dataDir;
          if (c.masterCsvPath != null) SETTINGS.masterCsvPath = c.masterCsvPath;
          if (c.assignmentsPath !== undefined) SETTINGS.assignmentsPath = c.assignmentsPath;
          SETTINGS.adEnabled = !!c.adEnabled;
          if (c.thresholds) {
            THRESH = normalizeThresholds(c.thresholds, SETTINGS.thresholds);
            SETTINGS.thresholds = Object.assign({}, THRESH);
            DEVICES = rebuildDevices();
          }
          return { ok: true };
        }
        case 'refresh': THRESH = normalizeThresholds(SETTINGS.thresholds, THRESH); DEVICES = rebuildDevices(); return { ok: true, count: DEVICES.length };
        case 'export_devices': return { ok: true, path: '(Vorschau) export.csv', rows: DEVICES.length };
        case 'me': return { name: 'T. Administrator', initials: 'TA', domain: 'corp.local' };
        default: throw new Error('Unbekannter Mock-Befehl: ' + cmd);
      }
    }
  };

  // Browser: an window haengen (echte Vorschau). Node: reine Logik exportieren,
  // damit der Parity-Test (app/tests) evaluate() gegen die Golden-Vectors prueft.
  if (typeof window !== 'undefined') { window.__MOCK__ = MOCK; }
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = { evaluate, normalizeThresholds, DEFAULT_THRESHOLDS };
  }
})();
