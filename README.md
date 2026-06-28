# HardView

Windows-Hardware-Inventarisierung und Asset-Zuordnung — dateibasiert, kein Server erforderlich.

**Kernfragen:** Welche PCs sind veraltet? Wer hat zu wenig RAM, eine HDD oder eine schwache CPU? Wo muss die IT aufrüsten? — plus eine Desktop-App zur Zuordnung von PCs an Active-Directory-Benutzer.

## Funktionsweise

```
PC-Agent (PowerShell)  ──1×/Woche, still──▶  Inventar-Share (\\FILESERVER\Inventory$\incoming)
  Get-CimInstance → <hostname>.json                         │
                                                            ├─ control\assignments.json (nur IT)
Rollout_Masterliste.csv (Geräte↔Benutzer) ─────────────┐   │
                                                        ▼   ▼
                          HardView Desktop-App (Tauri + Rust)
                          liest CSV + JSONs, merged, bewertet, ordnet zu
                          read-only AD-Lookup (LDAP, integrierte Auth)
```

## Features

- **Agent (PowerShell 5.1):** Erhebt Hardware-Snapshots (CPU, RAM, Datenträger, BIOS-Alter, OS) per WMI/CIM — kein RSAT, kein Internet, keine Admin-Abhängigkeiten im Client. Läuft wöchentlich still als SYSTEM-Task und schreibt `<hostname>.json` auf den Share.
- **Desktop-App (Tauri + Rust + Vanilla JS):** Liest alle JSONs und die Rollout-Masterliste (CSV), wertet Upgrade-Bedarf aus (RAM, Alter, SSD, CPU-Kerne, Win-11-Fähigkeit) und zeigt Tabelle, KPIs, Dashboard und Export.
- **Zuordnung:** PCs können AD-Benutzern zugeordnet werden; Zuordnungen werden atomar in `assignments.json` auf dem Share gespeichert.
- **AD-Integration:** Read-only AD-Lookup per integrierter Windows-Auth (kein RSAT). Fällt automatisch auf CSV-Namen zurück wenn AD nicht erreichbar ist.
- **Einstellungen:** Datenpfade, Bewertungs-Schwellwerte (RAM, Alter, SSD, …) und AD-Aktivierung direkt in der App konfigurierbar.

## Projektstruktur

```
agent/
  Invoke-Inventory.ps1            Inventar-Agent (WMI/CIM → JSON), PS 5.1
  Invoke-InventoryDebug.ps1       temporaerer GPO/Scheduled-Task Debug-Wrapper
  deploy/                         GPO-Rollout: Installer, Task-XML, Deployment-Anleitung
app/
  src/                            Frontend (Vanilla JS, keine externen Abhängigkeiten)
    index.html · styles.css · app.js · mock.js
  src-tauri/                      Rust-Backend
    src/  model.rs · store.rs · upgrade.rs · ad.rs · commands.rs · lib.rs
    scripts/Get-AdUsers.ps1       AD-Lookup (System.DirectoryServices)
    tauri.conf.json · Cargo.toml · capabilities/
  dev-server.js                   Statischer Dev-Server für Browser-Vorschau
shared/
  schema/inventory.schema.json    JSON-Schema der Agent-Ausgabe
  sample-data/                    Demo-Dataset (CSV + 16 JSONs + assignments) + Generator
  docs/SETUP.md                   Build, Konfiguration, Betrieb
```

## Voraussetzungen

- **Rust** (stable) + **Cargo**
- **Node.js ≥ 18** + **npm**
- **PowerShell 5.1** (Standard in Windows 10/11)
- **Windows** — Agent und App sind Windows-spezifisch (WMI, LDAP, MSI-Bundle)

## Schnellstart

**Frontend im Browser ansehen** (ohne Rust-Build):
```sh
node app/dev-server.js
# → http://localhost:5599  (nutzt mock.js)
```

**App mit Demo-Daten starten:**
```powershell
cd app
$sample = (Resolve-Path '..\shared\sample-data').Path
$env:HARDVIEW_DATA_DIR = Join-Path $sample 'Inventory'
$env:HARDVIEW_CSV = Join-Path $sample 'Rollout_Masterliste.csv'
$env:HARDVIEW_ASSIGN = Join-Path $sample 'control\assignments.json'
npm ci
npm run dev
```

**Installer / MSI bauen:**
```sh
cd app && npm ci && npm run build
# → app/src-tauri/target/release/bundle/
```

**Agent lokal testen:**
```powershell
agent\Invoke-Inventory.ps1 -OutputDir "$env:TEMP\inv" -Local -PassThru
```

**Backend-Tests:**
```sh
cd app/src-tauri && cargo test
```

**Alle CI-Checks:**
```sh
# Frontend
cd app && npm run check
# Backend
cd app/src-tauri && cargo fmt --all -- --check && cargo test --all && cargo clippy --all-targets -- -D warnings
```

## Konfiguration

Reihenfolge: `%APPDATA%\HardView\config.json` → Defaults → Umgebungsvariablen (nützlich für Tests):

| Variable | Beschreibung |
|---|---|
| `HARDVIEW_DATA_DIR` | Pfad zum `incoming\`-Ordner mit Agent-JSONs |
| `HARDVIEW_CSV` | Pfad zur Rollout-Masterliste (CSV) |
| `HARDVIEW_ASSIGN` | Pfad zur `assignments.json` |
| `HARDVIEW_AD=1` | AD-Lookup aktivieren |

Alle Einstellungen können auch direkt in der App unter **Einstellungen** gepflegt werden (schreibt dieselbe `config.json`; `assignmentsPath` wird serverseitig auf Control-Ordner/AppData beschränkt).

**Produktivbetrieb:**
- `dataDir` → client-beschreibbarer Inbox-Ordner, z. B. `\\FILESERVER\Inventory$\incoming`
- `assignmentsPath` → IT-only Control-Ordner, z. B. `\\FILESERVER\Inventory$\control\assignments.json`
- `masterCsvPath` → Rollout-Masterliste

Upgrade-Schwellen (`minRamGB`, `maxAgeYears`, `requireSsd`, `staleDays`, `minCpuCores`, `targetRamGB`) stehen in `config.json` und sind anpassbar.

## Bewertungsregeln (Standard)

| Kriterium | Schwelle |
|---|---|
| RAM | ≤ 8 GB |
| Alter | > 5 Jahre |
| Speicher | HDD oder gemischte SSD+HDD |
| CPU | < 4 Kerne |
| Betriebssystem | kein Windows 11 (Win 10 EOL) |
| Aktualität | keine Meldung seit > 30 Tagen oder kein JSON vorhanden |

Alle Schwellen sind in `config.json` anpassbar.

## Deployment

Agent per **GPO als Scheduled Task** (SYSTEM, wöchentlich, ausgeblendet, Zufallsverzögerung). Vollständige Anleitung zu Share-ACLs, UNC-Pfad-Anforderungen und GPO-Einrichtung: [`agent/deploy/README-Deployment.md`](agent/deploy/README-Deployment.md).

## Umsetzungsstand

| Komponente | Status |
|---|---|
| Agent (PowerShell) | ✅ Gebaut, auf realem PC getestet (0 Sammelfehler) |
| JSON-Schema + Sample-Daten | ✅ |
| Frontend | ✅ Im Browser verifiziert (Tabelle, KPIs, Drawer, Zuordnung, Dashboard, Einstellungen) |
| Rust-Backend | ✅ `cargo test` (10 Tests) + `cargo clippy -D warnings` grün; Merge & Bewertung gegen Sample-Daten getestet |
| CI | ✅ GitHub Actions: fmt + clippy (-D warnings) + Tests (Windows), PowerShell-Parser und JS-Syntaxcheck |
| Tauri-App | ✅ Kompiliert, gestartet, zeigt echte Daten (Screenshot: `app/hardview-screenshot.png`) |
| AD-Lookup | ✅ Code korrekt; Live-Test steht aus (kein DC am Build-Laptop; CSV-Fallback greift) |
| GPO-Deployment-Paket | ✅ Installer + Task-XML + Anleitung |
