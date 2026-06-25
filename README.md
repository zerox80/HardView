# Kowo-Agent · HardView

Hardware-Inventarisierung & Asset-Zuordnung für die Kowobau-IT (Domäne xxx).

Ziel: **Wie alt sind die PCs? Wer hat zu wenig RAM / eine HDD / eine schwache CPU? Wo muss die IT
aufrüsten?** — plus eine schöne Desktop-App, in der man PCs den AD-Benutzern zuordnet.

Drei Bausteine, **dateibasiert, kein Server**:

```
 PC-Agent (PowerShell)  ──1×/Woche, still──▶  Inventar-Share (\\FILESERVER\Inventory$\incoming)
   Get-CimInstance → <hostname>.json               untrusted <hostname>.json
                                                              │
                                                              ├─ control\assignments.json (nur IT)
 Rollout_Masterliste.csv (Geräte↔User) ───────────────┐      │
                                                       ▼      ▼
                              HardView Desktop-App (Tauri + Rust)
                              liest CSV + JSONs, merged, bewertet, ordnet zu
                              read-only AD-Lookup (LDAP, integrierte Auth)
```

## Projektstruktur

```
agent/
  Invoke-Inventory.ps1            Inventar-Agent (WMI/CIM → JSON), PS 5.1, kein RSAT
  deploy/                         GPO-Verteilung: Installer, Task-XML, README-Deployment.md
app/
  src/                            Frontend (HardView, aus Claude Design portiert; Vanilla JS, 0 Abhängigkeiten)
    index.html · styles.css · app.js · mock.js
  src-tauri/                      Rust-Backend
    src/ model.rs · store.rs · upgrade.rs · ad.rs · commands.rs · lib.rs
    scripts/Get-AdUsers.ps1       AD-Lookup (System.DirectoryServices)
    tauri.conf.json · Cargo.toml · capabilities/
  dev-server.js                   statischer Server nur für Browser-Vorschau
shared/
  schema/inventory.schema.json    JSON-Schema der Agent-Ausgabe
  sample-data/                    Demo-Datensatz (CSV + 16 JSONs + assignments + config) + Generator
  docs/SETUP.md                   Build, Konfiguration, Betrieb
```

## Schnellstart (Entwicklung)

**Frontend im Browser ansehen** (ohne Rust):
```
node app/dev-server.js     →  http://localhost:5599   (nutzt mock.js)
```

**Echte App mit Demo-Daten starten:**
```powershell
cd app
$env:KOWO_DATA_DIR='..\shared\sample-data\Inventory'
$env:KOWO_CSV='..\shared\sample-data\Rollout_Masterliste.csv'
$env:KOWO_ASSIGN='..\shared\sample-data\control\assignments.json'
npm ci
npm run dev
```

**Installer/MSI bauen:** `cd app; npm ci; npm run build` → `app/src-tauri/target/release/bundle/`.

**Agent lokal testen:** `agent\Invoke-Inventory.ps1 -OutputDir "$env:TEMP\inv" -Local -PassThru`

**Backend-Tests:** `cd app/src-tauri; cargo test`

**Repo-Checks:** `cd app; npm run check` sowie `cd app/src-tauri; cargo fmt --all -- --check; cargo test --all; cargo clippy --all-targets -- -D warnings`

## Konfiguration

Reihenfolge: `%APPDATA%\HardView\config.json` → Defaults → **Umgebungsvariablen** (`KOWO_DATA_DIR`,
`KOWO_CSV`, `KOWO_ASSIGN`, `KOWO_AD=1`) überschreiben (praktisch für Tests).
Alternativ direkt in der App unter **Einstellungen** pflegbar (schreibt dieselbe `config.json`;
Pfade werden serverseitig validiert, `assignmentsPath` bleibt auf Control-Ordner/AppData beschränkt).

Produktiv zeigt `dataDir` auf den client-beschreibbaren Inbox-Ordner, z. B. `G:\Inventory\incoming`.
`assignmentsPath` zeigt getrennt davon auf den nur für IT beschreibbaren Control-Ordner, z. B.
`G:\Inventory\control\assignments.json`; `masterCsvPath` zeigt auf `G:\Bitlocker\Rollout_Masterliste.csv`.
Upgrade-Schwellen (`minRamGB`, `maxAgeYears`, `requireSsd`, `staleDays`, `minCpuCores`, `targetRamGB`)
stehen in `config.json` und sind anpassbar.

## Bewertungsregeln (Default)

`RAM ≤ 8 GB` · `Alter > 5 J.` · `HDD/Mixed SSD-HDD` · `CPU < 4 Kerne` · `kein Windows 11 (Win 10 EOL)`
→ **Upgrade empfohlen**. Keine frische Meldung (> 30 Tage) bzw. gar kein JSON → **Veraltet / kein Agent**.

## Verteilung

Agent per **GPO als Scheduled Task** (SYSTEM, wöchentlich, ausgeblendet, Zufallsverzögerung).
Details, Share-ACLs und der SYSTEM-/UNC-Hinweis: [`agent/deploy/README-Deployment.md`](agent/deploy/README-Deployment.md).

## Stand der Umsetzung

| Teil | Status |
|---|---|
| Agent (PowerShell) | ✅ gebaut, auf realem PC getestet (0 Sammelfehler) |
| JSON-Schema + Sample-Daten | ✅ |
| Frontend (HardView-Port) | ✅ im Browser verifiziert (Tabelle, KPIs, Drawer, Zuordnung, Dashboard, Einstellungen) |
| Rust-Backend | ✅ `cargo test` (10 Tests) + `cargo clippy -D warnings` grün; Merge & Bewertung gegen Sample-Daten getestet |
| CI | ✅ GitHub Actions: fmt + clippy (-D warnings) + Tests (Windows), PowerShell-Parser und JS-Syntaxcheck |
| Tauri-App | ✅ kompiliert, gestartet, zeigt echte Daten (Screenshot in `app/hardview-screenshot.png`) |
| AD-Lookup | ✅ Code korrekt; Live-Test offen (Build-Laptop ohne DC-Verbindung; Fallback auf CSV greift) |
| GPO-Deployment-Paket | ✅ Installer + Task-XML + Anleitung |

> **Design-Anpassung:** Das Claude-Design suggerierte *Live-Monitoring* (Online/Offline, Echtzeit-Last).
> Da real **wöchentliche Snapshots** erhoben werden, wurde die Optik 1:1 übernommen, die Semantik aber
> ehrlich auf Snapshots umgedeutet: „Online/Warnung/Offline" → „OK / Upgrade nötig / Veraltet",
> Live-Auslastung → Bestands-/Alters-/Upgrade-Bewertung.
