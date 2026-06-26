# HardView — Produktiv-Setup (Checkliste)

## A. Inventar-Share
1. Ordner/Freigabe anlegen, z. B. `\\FILESERVER\Inventory$` (versteckt), darunter:
   - `incoming\` für Agent-JSONs (`<hostname>.json`)
   - `control\` für App-State (`assignments.json`)
2. ACL setzen (Least Privilege):
   - `Domänen-Computer`: in `incoming\` *Dateien erstellen / Daten schreiben* (nur dieser Ordner), **kein** Auflisten/Lesen fremder Dateien, kein Zugriff auf `control\`.
   - `ERSTELLER-BESITZER` / `CREATOR OWNER`: in `incoming\` *Ändern* für Unterordner und Dateien, damit jeder PC seine eigene JSON bei späteren Läufen ersetzen kann.
   - IT-Gruppe: Lesen auf `incoming\`, Ändern/Vollzugriff auf `control\`.
3. Optional eine leere `control\assignments.json` ablegen — sie wird sonst beim ersten Schreiben erzeugt.
4. Agent auf einem Test-PC zweimal starten; der zweite Lauf prüft die Rechte zum Überschreiben von `<hostname>.json`.

## B. Agent ausrollen
Siehe [`agent/deploy/README-Deployment.md`](../../agent/deploy/README-Deployment.md):
`Invoke-Inventory.ps1` nach `\\YOUR_DOMAIN.local\NETLOGON\HardView\` kopieren, GPO-Task (SYSTEM, wöchentlich,
ausgeblendet, Zufallsverzögerung) verteilen, Ziel = **UNC** (`\\FILESERVER\Inventory$\incoming`, nicht `G:`).
Verifizieren über `agent.log` und die erzeugte `<hostname>.json`.

## C. Desktop-App ausliefern
1. `cd app && npm ci && npm run build`
2. MSI aus `app/src-tauri/target/release/bundle/msi/` an die IT-Arbeitsplätze verteilen.
3. `%APPDATA%\HardView\config.json` setzen (pro IT-Rechner oder via GPO/Logon-Skript):
   ```json
   {
     "dataDir": "G:\\Inventory\\incoming",
     "masterCsvPath": "G:\\Bitlocker\\Rollout_Masterliste.csv",
     "assignmentsPath": "G:\\Inventory\\control\\assignments.json",
     "adEnabled": true,
     "thresholds": { "minRamGB": 8, "maxAgeYears": 5, "staleDays": 30, "requireSsd": true, "minCpuCores": 4, "minCpuClockMhz": 0, "targetRamGB": 16 }
   }
   ```
   (Die IT-Arbeitsplätze dürfen hier `G:` nutzen — die App läuft interaktiv mit gemapptem Laufwerk.)
   Alternativ lassen sich diese Werte direkt in der App unter **Einstellungen** pflegen (schreibt dieselbe
   `config.json`; `assignmentsPath` wird dabei serverseitig auf Control-Ordner/AppData beschränkt).

## D. AD aktivieren
- `adEnabled: true` setzen (in `config.json` oder in der App unter **Einstellungen**). Die App ruft
  `Get-AdUsers.ps1` (eingebettet) per integrierter Auth auf —
  **kein RSAT** nötig, read-only. Liefert Name/Abteilung/Mail für den Zuordnungs-Dialog.
- Voraussetzung: Der IT-Rechner hat Sicht auf einen `YOUR_DOMAIN.local`-DC (im LAN/VPN).
- Ist AD nicht erreichbar oder `adEnabled:false`, fällt die Benutzerauswahl automatisch auf die
  eindeutigen Namen aus CSV/Inventar zurück (App bleibt funktionsfähig).

## E. Betrieb
- **Aktualisieren**-Button liest Share/CSV neu; lokaler SQLite-Cache ist nicht nötig (JSONs sind klein).
- **Export** schreibt eine CSV (UTF-8 BOM, `;`) nach `…\Documents\HardView-Export-<Zeit>.csv` für die Beschaffung.
- **Zuordnung** schreibt mit Share-Lock atomar in `control\assignments.json` (version-hochzählend).

## Troubleshooting

> `LastTaskResult = 0x1`: TaskScheduler-Eventlog und `%ProgramData%\HardView\agent\agent.log`
> pruefen; fuer echte stdout/stderr-Ausgaben `Install-InventoryTask.ps1 -DebugLog` nutzen
> (siehe Deployment-Doku).

| Symptom | Ursache / Lösung |
|---|---|
| Agent-JSON fehlt | Task lief als SYSTEM gegen `G:` → auf UNC umstellen; Share-ACL für `Domänen-Computer` prüfen |
| `tpm`/`secureBoot` = null | Agent nicht elevated gelaufen; als SYSTEM (GPO-Task) füllen sich die Werte |
| App zeigt „kein Agent" | PC in CSV, aber keine `<hostname>.json` (Agent noch nicht/zuletzt nicht gelaufen) |
| AD-Picker leer/Fallback | kein DC erreichbar oder `adEnabled:false` → CSV-Namen werden genutzt |
| Umlaute in CSV falsch | CSV ist ANSI → wird automatisch als Windows-1252 gelesen; sonst als UTF-8 speichern |
