#Requires -Version 5.1
<#
.SYNOPSIS
    Kowobau Hardware-Inventar-Agent. Erhebt einmalig Hardware-/Asset-Metadaten des
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
    [string] $LogDir = (Join-Path $env:ProgramData 'Kowobau\HardwareInventar')
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
        Add-Content -LiteralPath $log -Value $line -Encoding UTF8
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
$hostname = $env:COMPUTERNAME
if ($cs -and $cs.Name) { $hostname = $cs.Name }
$hostname = $hostname -replace '[^a-zA-Z0-9-]', ''
$domain = if ($cs) { $cs.Domain } else { $env:USERDNSDOMAIN }

# --- Benutzer (interaktiv + zuletzt angemeldet; unter SYSTEM ist UserName ggf. leer)
$currentUser = if ($cs) { $cs.UserName } else { $null }
$lastUser = $null
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
$slotsTotal = if ($memArr) { [int](@($memArr)[0].MemoryDevices) } else { $sticks.Count }
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
    $dd = Get-CimSafe -Class 'Win32_DiskDrive'
    foreach ($d in @($dd)) {
        $media = 'Unbekannt'
        if ($d.Model -match 'SSD|NVMe') { $media = 'SSD' }
        $disks += [ordered]@{
            model     = if ($d.Model) { $d.Model.Trim() } else { $null }
            sizeGB    = [math]::Round([int64]$d.Size / 1GB, 0)
            mediaType = $media
            busType   = [string]$d.InterfaceType
        }
    }
}

# --- GPU
$gpus = @()
if ($gpu) { foreach ($g in @($gpu)) { if ($g.Name) { $gpus += $g.Name.Trim() } } }

# --- BIOS-Datum & Alter (Hauptsignal fuer das PC-Alter)
$biosDate = if ($bios) { To-IsoUtc $bios.ReleaseDate } else { $null }
$ageYears = $null
$ageSource = $null
if ($bios -and $bios.ReleaseDate) {
    $ageYears = [math]::Round(((Get-Date).ToUniversalTime() - ([datetime]$bios.ReleaseDate).ToUniversalTime()).TotalDays / 365.25, 1)
    $ageSource = 'bios'
} elseif ($os -and $os.InstallDate) {
    $ageYears = [math]::Round(((Get-Date).ToUniversalTime() - ([datetime]$os.InstallDate).ToUniversalTime()).TotalDays / 365.25, 1)
    $ageSource = 'osInstall'
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

# --- Win11-Readiness (optional, best effort; unter SYSTEM meist verfuegbar)
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
    if (-not (Test-Path -LiteralPath $Dir)) { New-Item -ItemType Directory -Path $Dir -Force | Out-Null }
    $final = Join-Path $Dir $Name
    $tmp   = Join-Path $Dir ('{0}.{1}.tmp' -f $Name, ([guid]::NewGuid().ToString('N').Substring(0,8)))
    # UTF-8 ohne BOM (gut fuer Rust/serde_json)
    [System.IO.File]::WriteAllText($tmp, $Content, (New-Object System.Text.UTF8Encoding($false)))
    try {
        Move-Item -LiteralPath $tmp -Destination $final -Force -ErrorAction Stop
    } catch {
        # Restriktive Shares erlauben manchmal Schreiben, aber kein Replace/Delete.
        # Dann wird die bekannte Zieldatei direkt ueberschrieben.
        try {
            [System.IO.File]::WriteAllText($final, $Content, (New-Object System.Text.UTF8Encoding($false)))
        } finally {
            Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
        }
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
