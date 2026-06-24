#Requires -Version 5.1
<#
.SYNOPSIS
    Erzeugt realistische Beispieldaten fuer die Entwicklung/Demo der HardView-App:
    eine Master-CSV (wie G:\Bitlocker\Rollout_Masterliste.csv), passende Inventar-JSONs,
    eine assignments.json und eine config.json.

    Bewusst eingebaute Faelle:
      - alte Geraete (> 5 Jahre)            -> "Geraet alt"
      - wenig RAM (8 / 4 GB)                -> "RAM zu klein"
      - reine HDD                           -> "HDD statt SSD"
      - veraltete Meldung (stale)           -> "Agent meldet nicht"
      - in CSV, aber ohne JSON              -> "kein Inventar"
.NOTES
    Idempotent: ueberschreibt den Zielordner-Inhalt.
#>
[CmdletBinding()]
param([string]$Root = $PSScriptRoot)

$invDir = Join-Path $Root 'Inventory'
$controlDir = Join-Path $Root 'control'
New-Item -ItemType Directory -Path $invDir -Force | Out-Null
New-Item -ItemType Directory -Path $controlDir -Force | Out-Null

# host, vorname, nachname, abteilung, cpu, kerne, threads, ramGB, slotsUsed, slotsTotal,
# diskType, diskGB, os, build, ageYears, staleDays, hasInventory, hersteller, modell
$pcs = @(
  @{h='WS-MARKETING-04'; f='Lena';    l='Hoffmann'; d='Marketing';        cpu='Intel Core i5-12500';     c=6;  t=12; ram=16; su=2; st=4; disk='SSD'; dgb=512;  os='Windows 11 Pro'; b='22631'; age=2.1; stale=1;  inv=$true;  mfg='Dell Inc.';      mdl='OptiPlex 7090'}
  @{h='WS-VERTRIEB-11';  f='Markus';  l='Bauer';    d='Vertrieb';         cpu='Intel Core i7-13700';     c=16; t=24; ram=32; su=2; st=4; disk='SSD'; dgb=1024; os='Windows 11 Pro'; b='22631'; age=1.2; stale=0;  inv=$true;  mfg='Lenovo';         mdl='ThinkCentre M90t'}
  @{h='WS-BUCH-02';      f='Sabine';  l='Koehler';  d='Buchhaltung';      cpu='Intel Core i5-10400';     c=6;  t=12; ram=8;  su=1; st=4; disk='SSD'; dgb=256;  os='Windows 10 Pro'; b='19045'; age=4.6; stale=2;  inv=$true;  mfg='HP';             mdl='EliteDesk 800 G6'}
  @{h='WS-IT-07';        f='Daniel';  l='Richter';  d='IT';               cpu='AMD Ryzen 7 5800X';       c=8;  t=16; ram=32; su=2; st=4; disk='SSD'; dgb=1024; os='Windows 11 Pro'; b='22631'; age=3.1; stale=0;  inv=$true;  mfg='Custom';         mdl='Workstation'}
  @{h='WS-ENTW-15';      f='Tobias';  l='Wolf';     d='Entwicklung';      cpu='Intel Core i9-13900K';    c=24; t=32; ram=64; su=4; st=4; disk='SSD'; dgb=2048; os='Windows 11 Pro'; b='22631'; age=0.8; stale=0;  inv=$true;  mfg='Dell Inc.';      mdl='Precision 3660'}
  @{h='WS-PERSONAL-03';  f='Andrea';  l='Schulz';   d='Personal';         cpu='Intel Core i3-10100';     c=4;  t=8;  ram=8;  su=1; st=2; disk='HDD'; dgb=500;  os='Windows 10 Pro'; b='19045'; age=5.3; stale=3;  inv=$true;  mfg='HP';             mdl='ProDesk 400 G6'}
  @{h='WS-VERTRIEB-08';  f='Kevin';   l='Braun';    d='Vertrieb';         cpu='AMD Ryzen 5 5600';        c=6;  t=12; ram=16; su=2; st=2; disk='SSD'; dgb=512;  os='Windows 11 Pro'; b='22621'; age=2.4; stale=1;  inv=$true;  mfg='Lenovo';         mdl='ThinkCentre M75q'}
  @{h='WS-LAGER-01';     f='Petra';   l='Lang';     d='Lager';            cpu='Intel Core i3-8100';      c=4;  t=4;  ram=8;  su=2; st=2; disk='HDD'; dgb=500;  os='Windows 10 Pro'; b='19044'; age=6.7; stale=45; inv=$true;  mfg='Fujitsu';        mdl='Esprimo D538'}
  @{h='WS-ENTW-09';      f='Jonas';   l='Frank';    d='Entwicklung';      cpu='AMD Ryzen 7 7700';        c=8;  t=16; ram=32; su=2; st=4; disk='SSD'; dgb=1024; os='Windows 11 Pro'; b='22631'; age=1.0; stale=0;  inv=$true;  mfg='Dell Inc.';      mdl='Precision 3460'}
  @{h='WS-MARKETING-06'; f='Nina';    l='Albrecht'; d='Marketing';        cpu='Intel Core i5-12500';     c=6;  t=12; ram=16; su=2; st=4; disk='SSD'; dgb=512;  os='Windows 11 Pro'; b='22631'; age=2.0; stale=0;  inv=$true;  mfg='Dell Inc.';      mdl='OptiPlex 7090'}
  @{h='WS-GF-01';        f='Stefan';  l='Klein';    d='Geschaeftsfuehrung';cpu='Intel Core i7-13700';    c=16; t=24; ram=32; su=2; st=4; disk='SSD'; dgb=1024; os='Windows 11 Pro'; b='22631'; age=1.5; stale=0;  inv=$true;  mfg='Lenovo';         mdl='ThinkPad X1 Carbon'}
  @{h='WS-BUCH-05';      f='Claudia'; l='Neumann';  d='Buchhaltung';      cpu='Intel Core i5-10400';     c=6;  t=12; ram=16; su=2; st=4; disk='SSD'; dgb=512;  os='Windows 10 Pro'; b='19045'; age=4.2; stale=4;  inv=$true;  mfg='HP';             mdl='EliteDesk 800 G6'}
  @{h='WS-IT-02';        f='Sven';    l='Hartmann'; d='IT';               cpu='AMD Ryzen 9 5900X';       c=12; t=24; ram=32; su=4; st=4; disk='SSD'; dgb=1024; os='Windows 11 Pro'; b='22631'; age=3.0; stale=0;  inv=$true;  mfg='Custom';         mdl='Workstation'}
  @{h='WS-EMPFANG-01';   f='Julia';   l='Vogt';     d='Empfang';          cpu='Intel Core i3-7100';      c=2;  t=4;  ram=8;  su=2; st=2; disk='HDD'; dgb=250;  os='Windows 10 Pro'; b='19045'; age=7.5; stale=6;  inv=$true;  mfg='Fujitsu';        mdl='Esprimo P558'}
  @{h='WS-VERTRIEB-14';  f='Florian'; l='Maier';    d='Vertrieb';         cpu='Intel Core i5-12500';     c=6;  t=12; ram=16; su=2; st=4; disk='SSD'; dgb=512;  os='Windows 11 Pro'; b='22621'; age=2.3; stale=90; inv=$true;  mfg='Dell Inc.';      mdl='OptiPlex 7090'}
  @{h='WS-ENTW-21';      f='Carolin'; l='Busch';    d='Entwicklung';      cpu='AMD Ryzen 7 5800X';       c=8;  t=16; ram=32; su=2; st=4; disk='SSD'; dgb=1024; os='Windows 11 Pro'; b='22631'; age=3.2; stale=0;  inv=$true;  mfg='Dell Inc.';      mdl='Precision 3650'}
  @{h='WS-BUCH-08';      f='Thomas';  l='Wagner';   d='Buchhaltung';      cpu='Intel Core i5-9500';      c=6;  t=6;  ram=8;  su=2; st=4; disk='HDD'; dgb=1000; os='Windows 10 Pro'; b='19045'; age=5.8; stale=0;  inv=$false; mfg='HP';             mdl='EliteDesk 705 G5'}
  @{h='WS-LAGER-04';     f='Michael'; l='Scholz';   d='Lager';            cpu='Intel Celeron G4900';     c=2;  t=2;  ram=4;  su=1; st=2; disk='HDD'; dgb=500;  os='Windows 10 Pro'; b='19044'; age=6.9; stale=0;  inv=$false; mfg='Fujitsu';        mdl='Esprimo D538'}
)

# --- Master-CSV (gleiches Format wie G:\Bitlocker\Rollout_Masterliste.csv: ; -getrennt) ---
$csvLines = @('Datum;Computer;Benutzer;Status')
foreach ($p in $pcs) {
    $datum = (Get-Date).AddDays(-1 * [int]($p.age * 365)).ToString('dd.MM.yyyy')
    $csvLines += ('{0};{1};{2} {3};Aktiv' -f $datum, $p.h, $p.f, $p.l)
}
# UTF-8 (ANSI-kompatibel reicht; deutsche Umlaute vermieden in Beispieldaten)
[System.IO.File]::WriteAllLines((Join-Path $Root 'Rollout_Masterliste.csv'), $csvLines, (New-Object System.Text.UTF8Encoding($false)))

# --- Inventar-JSONs ---
$now = (Get-Date).ToUniversalTime()
$count = 0
foreach ($p in $pcs) {
    if (-not $p.inv) { continue }
    $collected = $now.AddDays(-1 * $p.stale)
    $biosDate  = $now.AddYears(-1 * [int]$p.age).AddDays(-1 * (($p.age - [int]$p.age) * 365))
    $sticks = @()
    $per = [math]::Round($p.ram / $p.su, 0)
    for ($i = 0; $i -lt $p.su; $i++) {
        $sticks += [ordered]@{ capacityGB=$per; speedMhz=3200; manufacturer='Samsung'; partNumber='M471A1K43DB1'; slot=("DIMM{0}" -f $i) }
    }
    $obj = [ordered]@{
        schemaVersion=1; agentVersion='1.0.0'
        collectedAtUtc=$collected.ToString('yyyy-MM-ddTHH:mm:ssZ')
        hostname=$p.h; domain='kowobau.local'
        currentUser=("KOWOBAU\{0}.{1}" -f $p.f, $p.l); lastLoggedOnUser=("KOWOBAU\{0}.{1}" -f $p.f, $p.l)
        chassis=$(if ($p.mdl -match 'ThinkPad|Carbon|Laptop|Book') {'Laptop'} else {'Desktop'})
        manufacturer=$p.mfg; model=$p.mdl; serialNumber=('SN' + ($p.h -replace '\W','').Substring([math]::Max(0,($p.h -replace '\W','').Length-6)))
        bios=[ordered]@{ version='1.12.0'; releaseDate=$biosDate.ToString('yyyy-MM-ddTHH:mm:ssZ') }
        ageYears=$p.age; ageSource='bios'
        cpu=[ordered]@{ name=$p.cpu; cores=$p.c; logicalProcessors=$p.t; maxClockMhz=$(if($p.cpu -match 'i3|Celeron'){2400}else{3200}); sockets=1 }
        ram=[ordered]@{ totalGB=$p.ram; slotsUsed=$p.su; slotsTotal=$p.st; sticks=$sticks }
        disks=@([ordered]@{ model=$(if($p.disk -eq 'SSD'){'Samsung SSD 870'}else{'Seagate Barracuda'}); sizeGB=$p.dgb; mediaType=$p.disk; busType=$(if($p.disk -eq 'SSD'){'SATA'}else{'SATA'}) })
        gpus=@('Intel UHD Graphics')
        os=[ordered]@{ caption=$p.os; version=('10.0.' + $p.b); build=$p.b; installDateUtc=$biosDate.AddDays(14).ToString('yyyy-MM-ddTHH:mm:ssZ'); lastBootUtc=$now.AddDays(-2).ToString('yyyy-MM-ddTHH:mm:ssZ'); architecture='64-Bit' }
        win11=[ordered]@{ tpmPresent=$($p.os -match '11'); tpmVersion=$(if($p.os -match '11'){'2.0'}else{'1.2'}); secureBoot=$($p.os -match '11') }
        network=@([ordered]@{ mac='00:1A:2B:3C:4D:5E'; ipv4=('10.4.' + (Get-Random -Min 10 -Max 20) + '.' + (Get-Random -Min 2 -Max 250)) })
        collectionErrors=@()
    }
    $json = $obj | ConvertTo-Json -Depth 6
    [System.IO.File]::WriteAllText((Join-Path $invDir ($p.h + '.json')), $json, (New-Object System.Text.UTF8Encoding($false)))
    $count++
}

# --- assignments.json (eine manuelle Bestaetigung als Demo der Quellen-Prioritaet) ---
$assign = [ordered]@{
    version=3; updatedAtUtc=$now.ToString('yyyy-MM-ddTHH:mm:ssZ'); updatedBy='KOWOBAU\T.Administrator'
    assignments=[ordered]@{
        'WS-IT-07'=[ordered]@{ user='Daniel.Richter'; userDisplay='Daniel Richter'; confirmedBy='KOWOBAU\T.Administrator'; confirmedAtUtc=$now.AddDays(-5).ToString('yyyy-MM-ddTHH:mm:ssZ'); note='Geraet nach Abteilungswechsel bestaetigt.' }
    }
}
[System.IO.File]::WriteAllText((Join-Path $controlDir 'assignments.json'), ($assign | ConvertTo-Json -Depth 6), (New-Object System.Text.UTF8Encoding($false)))

# --- config.json (Schwellen + Pfade; Dev-Variante zeigt auf diesen Sample-Ordner) ---
$cfg = [ordered]@{
    version=1
    dataDir=$invDir
    masterCsvPath=(Join-Path $Root 'Rollout_Masterliste.csv')
    assignmentsPath=(Join-Path $controlDir 'assignments.json')
    adEnabled=$false
    thresholds=[ordered]@{ minRamGB=8; maxAgeYears=5; staleDays=30; requireSsd=$true; minCpuCores=4; minCpuClockMhz=2000; targetRamGB=16 }
}
[System.IO.File]::WriteAllText((Join-Path $Root 'config.json'), ($cfg | ConvertTo-Json -Depth 6), (New-Object System.Text.UTF8Encoding($false)))

Write-Host ("Erzeugt: {0} Inventar-JSONs, {1} CSV-Zeilen, control\assignments.json, config.json in {2}" -f $count, ($csvLines.Count - 1), $Root)
