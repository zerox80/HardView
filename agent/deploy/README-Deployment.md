# HardView Inventar-Agent — Verteilung (GPO)

Der Agent (`Invoke-Inventory.ps1`) erhebt **wöchentlich, still** die Hardware-Metadaten eines PCs
(CPU, RAM, Datenträger, BIOS-Alter, OS …) und legt sie als `<hostname>.json` im Inventar-Share ab.
Erhoben werden **ausschließlich Bestandsdaten** — keine Nutzeraktivität, keine Dateiinhalte.

## 1. Inventar-Share vorbereiten

Lege einen Ablageordner an, z. B. eine versteckte Freigabe `Inventory$` auf dem Fileserver
(das ist das UNC-Ziel, auf das `G:\Inventory` der IT zeigt). Darunter liegen getrennte Ordner:
`incoming\` für Agent-JSONs und `control\` für App-State.

**NTFS-/Freigabe-Rechte (Least Privilege):**

| Prinzipal | Recht | Zweck |
|---|---|---|
| `Domänen-Computer` | Nur auf `incoming\`: *Ordner auflisten/lesen* **deaktivieren**, *Dateien erstellen / Daten schreiben* + *Schreiben* erlauben (nur „Nur dieser Ordner") | Jeder PC kann eine neue JSON anlegen, aber keine fremden Dateien lesen/auflisten |
| `ERSTELLER-BESITZER` / `CREATOR OWNER` | Nur auf `incoming\`: *Ändern* inkl. Löschen/Replace (nur „Unterordner und Dateien") | Der jeweilige PC kann seine eigene JSON bei späteren Läufen atomar ersetzen |
| IT-Gruppe (z. B. `GG-IT-Admins`) | Lesen auf `incoming\`, Ändern / Vollzugriff auf `control\` | App liest Inventar und schreibt `control\assignments.json` |

> **Warum „Domänen-Computer"?** Der Task läuft als **SYSTEM**; beim Netzwerkzugriff
> authentifiziert sich der PC als Computerkonto `DOMÄNE\PC$`. Diese gehören zu „Domänen-Computer".
> Den Agent-Test mindestens zweimal auf demselben PC ausführen: Der zweite Lauf prüft, ob das
> atomare Ersetzen von `<hostname>.json` mit den gesetzten ACLs wirklich erlaubt ist. Der Agent
> schreibt nicht direkt über eine vorhandene Datei hinweg; fehlende Replace-/Delete-Rechte sind
> deshalb ein bewusst sichtbarer Fehler.

## 2. Wichtiger Hinweis: SYSTEM kennt kein `G:\`

Gemappte Laufwerke (`G:`) sind **benutzergebunden** und existieren im SYSTEM-Kontext nicht.
Der Agent muss per GPO daher mit einem **UNC-Pfad** als Ziel laufen:

```
-OutputDir "\\FILESERVER\Inventory$\incoming"     ✔  (UNC)
-OutputDir "G:\Inventory"                X  (funktioniert NUR interaktiv, nicht als SYSTEM)
```

## 3. Agent-Skript bereitstellen

Lege `Invoke-Inventory.ps1` an einen Ort, den **Domänen-Computer lesen** können — ideal:

```
\\YOUR_DOMAIN.local\NETLOGON\HardView\Invoke-Inventory.ps1
```

(NETLOGON wird automatisch repliziert und ist für alle Computer/Use​r lesbar.)

> **Sicherheit (wichtig):** Der Task läuft als **SYSTEM** und führt dieses Skript auf **jedem**
> Client aus. Wer Schreibzugriff auf `…\NETLOGON\HardView\` hat, erlangt damit SYSTEM-Codeausführung
> auf der gesamten Flotte. Schreibrecht auf diesen Ordner deshalb strikt auf **Domänen-Admins**
> beschränken (Domänen-Computer/-Benutzer: nur Lesen). Zusätzlich das Skript signieren (Abschnitt 6).

## 4. Geplante Aufgabe per GPO verteilen

**Variante A – XML importieren (pro PC / Test):**
```powershell
schtasks /Create /TN "HardView\HardwareInventar" /XML Inventory-ScheduledTask.xml /RU SYSTEM
```
(Vorher die zwei Platzhalter in `Inventory-ScheduledTask.xml` anpassen: Skriptquelle + `\\FILESERVER\Inventory$\incoming`.)

**Variante B – Gruppenrichtlinie (Massenrollout, empfohlen):**
1. GPO „HardView Inventar" erstellen, auf die Client-OU verknüpfen.
2. *Computerkonfiguration → Einstellungen → Systemsteuerungseinstellungen → Geplante Aufgaben*
   → Neu → **Geplante Aufgabe (Windows Vista und höher)**.
3. Werte gemäß `Inventory-ScheduledTask.xml` setzen:
   - Konto: `NT-AUTORITÄT\SYSTEM`, *Mit höchsten Privilegien*, *Unabhängig von Benutzeranmeldung*, *Ausgeblendet*.
   - Trigger: wöchentlich, So 12:00, **Zufallsverzögerung 4 h** (entzerrt ~776 PCs auf dem Share).
   - Aktion: `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`, Argumente:
     `-NoProfile -NonInteractive -ExecutionPolicy AllSigned -WindowStyle Hidden -File "\\YOUR_DOMAIN.local\NETLOGON\HardView\Invoke-Inventory.ps1" -OutputDir "\\FILESERVER\Inventory$\incoming"`
   - Bedingungen: *Nur bei Netzwerkverbindung*; Akku-Optionen aktiv lassen (Laptops sollen melden).

**Variante C – lokal testen:** `Install-InventoryTask.ps1` (als Admin) registriert die Aufgabe auf einem Einzel-PC.

## 5. Verifizieren

```powershell
# Sofortlauf erzwingen und Ergebnis prüfen
Start-ScheduledTask -TaskName 'HardwareInventar' -TaskPath '\HardView\'
Get-ScheduledTaskInfo -TaskName 'HardwareInventar' -TaskPath '\HardView\'   # LastTaskResult = 0
# JSON auf dem Share kontrollieren:
Get-Item "\\FILESERVER\Inventory$\incoming\$env:COMPUTERNAME.json"
# Lokales Agent-Log:
Get-Content "$env:ProgramData\HardView\agent\agent.log" -Tail 20
```
Kein Fenster, kein Toast — der Lauf ist für den Mitarbeiter unsichtbar (stört die Arbeit nicht).

### Fehler `0x1` sichtbar machen

`LastTaskResult = 0x1` bedeutet nur: `powershell.exe` wurde mit Exitcode 1 beendet.
Die eigentliche Ursache steht entweder im TaskScheduler-Eventlog, im Agent-Log oder - wenn
PowerShell das Skript gar nicht erst startet - in einem Diagnose-Wrapper.

```powershell
# TaskScheduler-Events fuer diesen Task aus den letzten 2 Stunden:
Get-WinEvent -FilterHashtable @{
  LogName   = 'Microsoft-Windows-TaskScheduler/Operational'
  StartTime = (Get-Date).AddHours(-2)
} | Where-Object {
  $_.Message -like '*\HardView\HardwareInventar*'
} | Select-Object TimeCreated, Id, LevelDisplayName, Message | Format-List

# Agent-Log, falls Invoke-Inventory.ps1 bereits gestartet wurde:
Get-Content "$env:ProgramData\HardView\agent\agent.log" -Tail 80
```

Fuer einen lokalen Test-PC kann der Installer den Task temporaer mit Debug-Logging registrieren:

```powershell
# Labortest mit unsigniertem Skript:
.\Install-InventoryTask.ps1 -ExecutionPolicy RemoteSigned -AllowUnsignedForTest -OutputDir "$env:TEMP\inv" -DebugLog

# Produktivnaher Test mit signiertem Skript und echtem Share:
.\Install-InventoryTask.ps1 -ScriptPath "\\YOUR_DOMAIN.local\NETLOGON\HardView\Invoke-Inventory.ps1" -OutputDir "\\FILESERVER\Inventory$\incoming" -DebugLog

Start-ScheduledTask -TaskName 'HardwareInventar' -TaskPath '\HardView\'
Start-Sleep -Seconds 10
Get-ScheduledTaskInfo -TaskName 'HardwareInventar' -TaskPath '\HardView\'
Get-Content "$env:ProgramData\HardView\agent\task-debug.log" -Tail 160
```

Der Debug-Wrapper laeuft weiterhin als SYSTEM und protokolliert Identitaet, PowerShell-Version,
Skriptpfad, Zielordner, Verbose-Ausgaben und die vollstaendige PowerShell-Exception. Nach der
Fehlersuche den Task wieder ohne `-DebugLog` registrieren.

## 6. Erzwungen: Skript signieren (+ `AllSigned`)

Da der Agent als **SYSTEM** aus einer Netzwerkfreigabe läuft, sollte seine Integrität erzwungen
werden, statt sich allein auf die Ordner-ACL zu verlassen. Skript mit einem internen
Code-Signing-Zertifikat signieren:
```powershell
Set-AuthenticodeSignature .\Invoke-Inventory.ps1 -Certificate (Get-ChildItem Cert:\CurrentUser\My -CodeSigning)[0]
```
Die mitgelieferte Task-XML verwendet `AllSigned`. `Install-InventoryTask.ps1` prüft bei `AllSigned`
vor der Registrierung eine gültige Authenticode-Signatur und bricht bei einem unsignierten oder
manipulierten Agent-Skript ab. Für lokale, unsignierte Labortests ist der Testmodus explizit:
```powershell
.\Install-InventoryTask.ps1 -ExecutionPolicy RemoteSigned -AllowUnsignedForTest -OutputDir "$env:TEMP\inv"
```
Produktiv bleibt `AllSigned`; so führt ein manipuliertes oder untergeschobenes Skript **nicht** mehr aus.

## 7. Transparenz / Datenschutz

Der Agent ist eine **benannte, dokumentierte** IT-Aufgabe (`HardView\HardwareInventar`) und erhebt nur
Hardware-/Bestandsdaten zur Lebenszyklusplanung. Bei Mitbestimmung empfiehlt sich eine kurze
Information an den Betriebsrat (Zweck: Hardware-Upgrade-Planung; keine Verhaltens-/Leistungskontrolle).

Die JSONs enthalten gerätebezogene Daten mit Personenbezug (Seriennummer, MAC/IP, zuletzt
angemeldeter Benutzer). Veraltete `<hostname>.json` ausgemusterter Geräte regelmäßig vom Share
entfernen, und den Zugriff per ACL (Abschnitt 1) auf die IT beschränken. Das Cleanup-Skript
löscht nur Agent-JSONs, deren `hostname` zum Dateinamen passt:
```powershell
.\Remove-StaleInventory.ps1 -InventoryDir "\\FILESERVER\Inventory$\incoming" -RetentionDays 180 -WhatIf
.\Remove-StaleInventory.ps1 -InventoryDir "\\FILESERVER\Inventory$\incoming" -RetentionDays 180
```
