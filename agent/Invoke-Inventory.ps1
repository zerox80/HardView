#Requires -Version 5.1
<#
.SYNOPSIS
    HardView Hardware-Inventar-Agent. Erhebt einmalig Hardware-/Asset-Metadaten des
    lokalen PCs und legt sie als JSON-Datei auf dem Inventar-Share ab.

.DESCRIPTION
    Teil des internen IT-Asset-Managements (Hardware-Lebenszyklus- und Upgrade-Planung).

    Erhoben werden AUSSCHLIESSLICH Hardware-/Bestandsdaten:
      CPU, RAM (inkl. Slots), Datenträger (SSD/HDD), BIOS/Alter, Betriebssystem,
      Modell/Hersteller/Seriennummer, optional Win11-Readiness (TPM/SecureBoot).
    KEINE Nutzeraktivität, KEIN Dateiinhalt, KEINE Telemetrie, KEIN Netzwerk-Scan.

    Vorgesehen für die woechentliche Ausfuehrung als benannter Scheduled Task
    (Kontext SYSTEM, ohne sichtbares Fenster, niedrige Prioritaet, mit Zeit-Jitter).
    Jeder PC schreibt nur seine eigene Datei "<hostname>.json" -> keine Schreibkonflikte.

.PARAMETER OutputDir
    Zielordner fuer die JSON-Datei (Default: \\FILESERVER\Inventory$\incoming).

.PARAMETER Local
    Schreibt die JSON zusaetzlich in den LogDir-Ordner (fuer Tests, auch ohne Share).

.PARAMETER PassThru
    Gibt das Inventar-Objekt zusaetzlich auf der Pipeline aus (fuer Tests).

.EXAMPLE
    # Normaler (stiller) Lauf, schreibt nach \\FILESERVER\Inventory$\incoming\<host>.json
    powershell -NoProfile -ExecutionPolicy AllSigned -WindowStyle Hidden -File .\Invoke-Inventory.ps1

.EXAMPLE
    # Lokaler Test ohne Share, JSON auf der Konsole ansehen
    .\Invoke-Inventory.ps1 -OutputDir "$env:TEMP\inv" -Local -PassThru -Verbose

.NOTES
    Agent-Version 1.0.0 | PowerShell 5.1 | kein RSAT erforderlich.
#>
[CmdletBinding()]
param(
    [string] $OutputDir = '\\FILESERVER\Inventory$\incoming',
    [switch] $Local,
    [switch] $PassThru,
    [string] $LogDir = (Join-Path $env:ProgramData 'HardView\agent')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$AgentVersion  = '1.0.0'
$SchemaVersion = 1

# ----------------------------------------------------------------------------- helpers
$script:CollectionErrors = New-Object System.Collections.Generic.List[string]

function Write-InvLog {
    param([string]$Message, [ValidateSet('INFO','WARN','ERROR')] [string]$Level = 'INFO')
    $line = ('{0} [{1}] {2}' -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'), $Level, $Message)
    Write-Verbose $line
    try {
        if (-not (Test-Path -LiteralPath $LogDir)) { New-Item -ItemType Directory -Path $LogDir -Force | Out-Null }
        $log = Join-Path $LogDir 'agent.log'
        # einfache Rotation bei > 512 KB
        if ((Test-Path $log) -and ((Get-Item $log).Length -gt 512KB)) {
            Move-Item $log (Join-Path $LogDir 'agent.1.log') -Force
        }
        # PS 5.1 -Encoding UTF8 wuerde BOM schreiben -> nach Rotation gemischter
        # BOM-Zustand. Daher explizit UTF-8 ohne BOM anhaengen.
        [System.IO.File]::AppendAllText($log, ($line + "`n"), (New-Object System.Text.UTF8Encoding($false)))
    } catch { }
}

# Sammelt eine CIM-Klasse ab und protokolliert Fehler, statt das Skript abzubrechen.
function Get-CimSafe {
    param(
        [Parameter(Mandatory)] [string] $Class,
        [string] $Namespace = 'root\cimv2',
        [string] $Filter
    )
    try {
        $p = @{ ClassName = $Class; Namespace = $Namespace; ErrorAction = 'Stop' }
        if ($Filter) { $p['Filter'] = $Filter }
        return Get-CimInstance @p
    } catch {
        $msg = ('{0}: {1}' -f $Class, $_.Exception.Message)
        $script:CollectionErrors.Add($msg)
        Write-InvLog $msg 'WARN'
        return $null
    }
}

function To-IsoUtc {
    param($DateTime)
    if ($null -eq $DateTime) { return $null }
    try { return ([datetime]$DateTime).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ') }
    catch { return $null }
}

function ConvertTo-ChassisName {
    param([int[]] $Codes)
    if (-not $Codes) { return 'Unbekannt' }
    $c = $Codes[0]
    $laptop = 8,9,10,11,12,14,18,21,30,31,32
    if ($laptop -contains $c) { return 'Laptop' }
    if ($c -eq 13) { return 'All-in-One' }
    if (23 -eq $c) { return 'Server/Rack' }
    return 'Desktop'
}

# ----------------------------------------------------------------------------- collect
Write-InvLog ("Inventarlauf gestartet (Agent {0}, Schema {1})" -f $AgentVersion, $SchemaVersion)

$cs   = Get-CimSafe -Class 'Win32_ComputerSystem'
$bios = Get-CimSafe -Class 'Win32_BIOS'
$os   = Get-CimSafe -Class 'Win32_OperatingSystem'
$encl = Get-CimSafe -Class 'Win32_SystemEnclosure'
$cpus = Get-CimSafe -Class 'Win32_Processor'
$mem  = Get-CimSafe -Class 'Win32_PhysicalMemory'
$memArr = Get-CimSafe -Class 'Win32_PhysicalMemoryArray'
$gpu  = Get-CimSafe -Class 'Win32_VideoController'

# --- Hostname / Domaene
# Vorzugsweise den DNS-Hostnamen verwenden (Win32_ComputerSystem.Name liefert nur den
# NetBIOS-Namen <=15 Zeichen; laenere Win11-Hostnamen wuerden sonst im Dateinamen
# und damit beim Host-Match mit AD-Eintraegen trunkiert werden).
$hostname = if ($cs -and $cs.DNSHostName) { [string]$cs.DNSHostName }
            elseif ($cs -and $cs.Name) { [string]$cs.Name }
            else { $env:COMPUTERNAME }
$hostname = $hostname -replace '[^a-zA-Z0-9-]', ''
$domain = if ($cs) { $cs.Domain } else { $env:USERDNSDOMAIN }

# --- Benutzer (interaktiv + zuletzt angemeldet; unter SYSTEM ist UserName ggf. leer)
$currentUser = if ($cs) { $cs.UserName } else { $null }
$lastUser = $null
# Hinweis: LastLoggedOnUser unter LogonUI wird NUR bei aktiviertem AutoLogon zuverlaessig
# gesetzt. Ohne AutoLogon bleibt der Wert leer oder veraltet; das Feld "lastLoggedOnUser"
# ist daher ein best-effort-Signal und nicht als kanonische "zuletzt angemeldet"-Quelle
# zu verstehen (zuverlaessig ist das 4624 Security-Log bzw. Win32_LoggedOnUser).
try {
    $lu = Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Authentication\LogonUI' `
            -Name 'LastLoggedOnUser' -ErrorAction Stop
    $lastUser = $lu.LastLoggedOnUser
} catch { }

# --- CPU
$cpu0 = if ($cpus) { @($cpus)[0] } else { $null }
$cpuObj = [ordered]@{
    name              = if ($cpu0) { ($cpu0.Name -replace '\s+', ' ').Trim() } else { $null }
    cores             = if ($cpu0) { [int]$cpu0.NumberOfCores } else { $null }
    logicalProcessors = if ($cpu0) { [int]$cpu0.NumberOfLogicalProcessors } else { $null }
    maxClockMhz       = if ($cpu0) { [int]$cpu0.MaxClockSpeed } else { $null }
    sockets           = if ($cpus) { @($cpus).Count } else { $null }
}

# --- RAM
$sticks = @()
$totalBytes = 0
if ($mem) {
    foreach ($m in @($mem)) {
        $totalBytes += [int64]$m.Capacity
        $sticks += [ordered]@{
            capacityGB   = [math]::Round([int64]$m.Capacity / 1GB, 0)
            speedMhz     = [int]$m.Speed
            manufacturer = if ($m.Manufacturer) { $m.Manufacturer.Trim() } else { $null }
            partNumber   = if ($m.PartNumber)   { $m.PartNumber.Trim() }   else { $null }
            slot         = $m.DeviceLocator
        }
    }
}
# SlotsTotal ueber alle PhysicalMemoryArray-Instanzen summieren — bei
# Multi-Socket-Servern gibt es mehrere Arrays, und nur das erste zu nehmen wuerde
# systematisch zu wenig Slots melden. Fallback: Anzahl belegter Slots.
$slotsTotal = if ($memArr) {
    (@($memArr) | Measure-Object -Property MemoryDevices -Sum).Sum
} else { $sticks.Count }
if (-not $slotsTotal -or $slotsTotal -lt $sticks.Count) { $slotsTotal = $sticks.Count }
$ramObj = [ordered]@{
    totalGB   = [math]::Round($totalBytes / 1GB, 0)
    slotsUsed = $sticks.Count
    slotsTotal= $slotsTotal
    sticks    = $sticks
}

# --- Datentraeger (SSD/HDD). Bevorzugt Get-PhysicalDisk (MediaType), sonst Fallback.
$disks = @()
try {
    $pd = Get-PhysicalDisk -ErrorAction Stop
    foreach ($d in @($pd)) {
        $media = switch ([string]$d.MediaType) {
            'SSD' { 'SSD' }; 'HDD' { 'HDD' }; 'SCM' { 'SCM' } default { 'Unbekannt' }
        }
        # NVMe ueber BusType erkennen, falls MediaType unspezifisch
        if ($media -eq 'Unbekannt' -and "$($d.BusType)" -eq 'NVMe') { $media = 'SSD' }
        $disks += [ordered]@{
            model     = if ($d.FriendlyName) { $d.FriendlyName.Trim() } else { $null }
            sizeGB    = [math]::Round([int64]$d.Size / 1GB, 0)
            mediaType = $media
            busType   = [string]$d.BusType
        }
    }
} catch {
    $script:CollectionErrors.Add(('Get-PhysicalDisk: {0}' -f $_.Exception.Message))
    # Bevor wir auf den unzuverlaessigen Win32_DiskDrive-Modellstring-Regex
    # zurueckfallen, probieren wir zusaetzlich das Storage-Sub-Namespace
    # (root\Microsoft\Windows\Storage / MSFT_PhysicalDisk). Dort ist MediaType
    # verfuegbar, auch wenn das Top-Level-Cmdlet gesperrt war (z. B. Server 2016).
    $storageDisks = $null
    try {
        $storageDisks = Get-CimInstance -Namespace 'root\Microsoft\Windows\Storage' `
            -ClassName MSFT_PhysicalDisk -ErrorAction Stop
    } catch {
        $storageDisks = $null
    }
    if ($storageDisks) {
        foreach ($d in @($storageDisks)) {
            $media = switch ([int]$d.MediaType) {
                3 { 'HDD' } 4 { 'SSD' } 5 { 'SCM' } default { 'Unbekannt' }
            }
            if ($media -eq 'Unbekannt' -and "$($d.BusType)" -eq 'NVMe') { $media = 'SSD' }
            $disks += [ordered]@{
                model     = if ($d.FriendlyName) { $d.FriendlyName.Trim() } else { $null }
                sizeGB    = [math]::Round([int64]$d.Size / 1GB, 0)
                mediaType = $media
                busType   = [string]$d.BusType
            }
        }
    } else {
        $dd = Get-CimSafe -Class 'Win32_DiskDrive'
        if ($dd) {
            foreach ($d in @($dd)) {
                $media = 'Unbekannt'
                # Modellstring-Heuristik nur als letzter Ausweg — viele SATA/NVMe-SSDs
                # enthalten kein "SSD"/"NVMe" im Modellnamen und werden sonst falsch
                # klassifiziert (zentrales Disk-Feld der Upgrade-Bewertung).
                if ($d.Model -match 'SSD|NVMe') { $media = 'SSD' }
                elseif ("$($d.InterfaceType)" -eq 'NVMe') { $media = 'SSD' }
                $disks += [ordered]@{
                    model     = if ($d.Model) { $d.Model.Trim() } else { $null }
                    sizeGB    = [math]::Round([int64]$d.Size / 1GB, 0)
                    mediaType = $media
                    busType   = [string]$d.InterfaceType
                }
            }
        }
    }
}

# --- GPU
$gpus = @()
if ($gpu) { foreach ($g in @($gpu)) { if ($g.Name) { $gpus += $g.Name.Trim() } } }

# --- BIOS-Datum & Alter
# Alter = das AELTERE (fruehere) Signal aus BIOS-Releasedatum und OS-Installdatum.
# So verjuengt weder ein BIOS-/Firmware-Update (setzt das BIOS-Datum nach vorn)
# noch eine Windows-Neuinstallation (setzt das OS-Datum nach vorn) das Geraet:
# wir nehmen jeweils den frueheren Nachweis, also das groessere Alter.
$biosDate = if ($bios) { To-IsoUtc $bios.ReleaseDate } else { $null }
$ageYears = $null
$ageSource = $null
$nowUtc = (Get-Date).ToUniversalTime()
$biosAge = if ($bios -and $bios.ReleaseDate) { ($nowUtc - ([datetime]$bios.ReleaseDate).ToUniversalTime()).TotalDays / 365.25 } else { $null }
$osAge   = if ($os   -and $os.InstallDate)   { ($nowUtc - ([datetime]$os.InstallDate).ToUniversalTime()).TotalDays / 365.25 } else { $null }
if ($null -ne $biosAge -and ($null -eq $osAge -or $biosAge -ge $osAge)) {
    $ageYears = [math]::Round($biosAge, 1)
    $ageSource = 'bios'
} elseif ($null -ne $osAge) {
    $ageYears = [math]::Round($osAge, 1)
    $ageSource = 'osInstall'
}
# Sanity-Bounds: negatives Alter (BIOS-Datum in der Zukunft) oder absurde Werte
# (defektes DateTime.MinValue → ~424 Jahre) verfaelschen Bewertungen und Buckets.
# Wir setzen fuer unzulaessige Werte $null und markieren die Quelle als unplausibel.
if ($null -ne $ageYears -and ($ageYears -lt 0 -or $ageYears -gt 30)) {
    $script:CollectionErrors.Add(('ageYears unplausibel ({0}); BIOS/InstallDate-Datum verworfen' -f $ageYears))
    $ageYears = $null
    $ageSource = $null
}

# --- OS
$osObj = [ordered]@{
    caption       = if ($os) { $os.Caption } else { $null }
    version       = if ($os) { $os.Version } else { $null }
    build         = if ($os) { $os.BuildNumber } else { $null }
    installDateUtc= if ($os) { To-IsoUtc $os.InstallDate } else { $null }
    lastBootUtc   = if ($os) { To-IsoUtc $os.LastBootUpTime } else { $null }
    architecture  = if ($os) { $os.OSArchitecture } else { $null }
}

# --- Win11-Preflight (TPM/SecureBoot, best effort; KEINE vollstaendige Readiness!)
# Benannt als tpmSecureBootPreflight, damit Downstream-Konsumenten nicht
# versehentlich tpmPresent+secureBoot als Win11-Ready lesen. Eine vollstaendige
# Win11-Readiness erfordert CPU-Family, min. 2 Cores, 4+ GB RAM, 64+ GB Storage
# neben TPM 2.0; diese vollstaendige Pruefung obliegt der Planungs-Logik, nicht
# dem Agent.
$tpmPresent = $null; $tpmVersion = $null; $secureBoot = $null
try {
    $tpm = Get-CimInstance -Namespace 'root\cimv2\Security\MicrosoftTpm' -ClassName 'Win32_Tpm' -ErrorAction Stop
    if ($tpm) {
        $tpmPresent = [bool]$tpm.IsEnabled_InitialValue
        if ($tpm.SpecVersion) { $tpmVersion = ($tpm.SpecVersion -split ',')[0].Trim() }
    }
} catch { }
try { $secureBoot = [bool](Confirm-SecureBootUEFI -ErrorAction Stop) } catch { }

# --- Netzwerk (erste aktive Adapter-Konfiguration)
$net = @()
$nics = Get-CimSafe -Class 'Win32_NetworkAdapterConfiguration' -Filter 'IPEnabled = TRUE'
if ($nics) {
    foreach ($n in @($nics)) {
        $ipv4 = $null
        if ($n.IPAddress) { $ipv4 = (@($n.IPAddress) | Where-Object { $_ -match '^\d{1,3}(\.\d{1,3}){3}$' } | Select-Object -First 1) }
        $net += [ordered]@{ mac = $n.MACAddress; ipv4 = $ipv4 }
    }
}

# ----------------------------------------------------------------------------- assemble
$inventory = [ordered]@{
    schemaVersion    = $SchemaVersion
    agentVersion     = $AgentVersion
    collectedAtUtc   = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    hostname         = $hostname
    domain           = $domain
    currentUser      = $currentUser
    lastLoggedOnUser = $lastUser
    chassis          = if ($encl) { ConvertTo-ChassisName -Codes @($encl)[0].ChassisTypes } else { 'Unbekannt' }
    manufacturer     = if ($cs)   { $cs.Manufacturer } else { $null }
    model            = if ($cs)   { $cs.Model } else { $null }
    serialNumber     = if ($bios) { $bios.SerialNumber } else { $null }
    bios             = [ordered]@{
        version     = if ($bios) { (@($bios.SMBIOSBIOSVersion) -join ' ') } else { $null }
        releaseDate = $biosDate
    }
    ageYears         = $ageYears
    ageSource        = $ageSource
    cpu              = $cpuObj
    ram              = $ramObj
    disks            = $disks
    gpus             = $gpus
    os               = $osObj
    # Hinweis: Feldname bleibt "win11" zwecks Schema-Kompatibilitaet, enthaelt aber
# nur TPM/SecureBoot-Preflight — keine vollstaendige Readiness (CPU/RAM/Size fehlen).
win11            = [ordered]@{
        tpmPresent = $tpmPresent
        tpmVersion = $tpmVersion
        secureBoot = $secureBoot
    }
    network          = $net
    collectionErrors = @($script:CollectionErrors)
}

$json = $inventory | ConvertTo-Json -Depth 6

# ----------------------------------------------------------------------------- write (atomic)
function Write-AtomicJson {
    param([string]$Dir, [string]$Name, [string]$Content)
    if ($Name -notmatch '^[A-Za-z0-9-]+\.json$') {
        throw "Ungueltiger Inventar-Dateiname: $Name"
    }
    if (-not (Test-Path -LiteralPath $Dir)) { New-Item -ItemType Directory -Path $Dir -Force | Out-Null }
    $final = Join-Path $Dir $Name
    $tmp   = Join-Path $Dir ('{0}.{1}.tmp' -f $Name, ([guid]::NewGuid().ToString('N')))
    $backup = Join-Path $Dir ('{0}.{1}.bak' -f $Name, ([guid]::NewGuid().ToString('N')))
    # UTF-8 ohne BOM (gut fuer Rust/serde_json)
    [System.IO.File]::WriteAllText($tmp, $Content, (New-Object System.Text.UTF8Encoding($false)))
    try {
        if (Test-Path -LiteralPath $final) {
            [System.IO.File]::Replace($tmp, $final, $backup, $true)
            Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue
        } else {
            Move-Item -LiteralPath $tmp -Destination $final -ErrorAction Stop
        }
    } catch {
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
        throw ("Atomarer Replace nach '{0}' fehlgeschlagen: {1}" -f $final, $_.Exception.Message)
    }
    return $final
}

$fileName = ('{0}.json' -f $hostname)
$written = @()

try {
    $written += Write-AtomicJson -Dir $OutputDir -Name $fileName -Content $json
    Write-InvLog ("JSON geschrieben: {0} ({1} Fehler beim Sammeln)" -f $written[-1], $script:CollectionErrors.Count)
} catch {
    Write-InvLog ("Schreiben auf '{0}' fehlgeschlagen: {1}" -f $OutputDir, $_.Exception.Message) 'ERROR'
    if (-not $Local) { throw }
}

if ($Local) {
    try { $written += Write-AtomicJson -Dir $LogDir -Name $fileName -Content $json } catch { }
}

Write-InvLog 'Inventarlauf beendet.'

if ($PassThru) { $inventory }
