#Requires -Version 5.1
#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Registriert den HardView-Inventar-Agent als geplante Aufgabe (woechentlich, still, SYSTEM).
    Fuer lokale Tests/Einzelinstallation. Massenverteilung erfolgt per GPO (siehe README-Deployment.md).

.DESCRIPTION
    Legt die Aufgabe "HardView\HardwareInventar" an:
      - Trigger: woechentlich, mit Zufallsverzoegerung (entzerrt 776 PCs auf dem Share)
      - Kontext: SYSTEM, ohne Fenster, niedrige Prioritaet
      - laeuft auch ohne angemeldeten Benutzer und auf Akku (Laptops)

.PARAMETER ScriptPath
    Pfad zu Invoke-Inventory.ps1 (Default: ..\Invoke-Inventory.ps1 relativ zu diesem Skript).

.PARAMETER OutputDir
    Inventar-Inbox, in die die JSON geschrieben wird (Default: \\FILESERVER\Inventory$\incoming).

.PARAMETER DayOfWeek
    Wochentag des Laufs (Default: Sunday).

.PARAMETER At
    Uhrzeit (Default: 12:00). Mit RandomDelay verteilt sich der tatsaechliche Start.

.PARAMETER RandomDelayHours
    Maximale Zufallsverzoegerung in Stunden (Default: 4).

.PARAMETER Uninstall
    Entfernt die Aufgabe wieder.

.PARAMETER AllowUnsignedForTest
    Erlaubt RemoteSigned nur fuer lokale Labortests mit unsigniertem Agent-Skript.

.PARAMETER DebugLog
    Registriert den Task mit einem Diagnose-Wrapper, der stdout/stderr/Verbose und
    Terminierungsfehler nach DebugLogPath schreibt. Nur zur Fehlersuche verwenden.

.EXAMPLE
    .\Install-InventoryTask.ps1 -OutputDir '\\fileserver\Inventory$\incoming'
.EXAMPLE
    .\Install-InventoryTask.ps1 -ExecutionPolicy RemoteSigned -AllowUnsignedForTest -OutputDir "$env:TEMP\inv" -DebugLog
.EXAMPLE
    .\Install-InventoryTask.ps1 -Uninstall
#>
[CmdletBinding()]
param(
    [string]   $ScriptPath = (Join-Path (Split-Path $PSScriptRoot -Parent) 'Invoke-Inventory.ps1'),
    [string]   $OutputDir = '\\FILESERVER\Inventory$\incoming',
    [string]   $DayOfWeek = 'Sunday',
    [datetime] $At = '12:00',
    [int]      $RandomDelayHours = 4,
    [string]   $TaskName = 'HardwareInventar',
    [string]   $TaskPath = '\HardView\',
    [ValidateSet('AllSigned','RemoteSigned')]
    [string]   $ExecutionPolicy = 'AllSigned',
    [switch]   $AllowUnsignedForTest,
    [switch]   $DebugLog,
    [string]   $DebugLogPath = (Join-Path $env:ProgramData 'HardView\agent\task-debug.log'),
    [switch]   $Uninstall
)

$ErrorActionPreference = 'Stop'

function ConvertTo-SingleQuotedLiteral {
    param([Parameter(Mandatory)] [string] $Value)
    return "'" + ($Value -replace "'", "''") + "'"
}

# CommandLineToArgvW (vom Task-Scheduler-Host fuer den Prozessstart genutzt) interpretiert
# eine ungerade Anzahl Backslashes direkt vor einem schliessenden Anfuehrungszeichen als
# Escape fuer das Quote selbst -> das Argument wuerde nicht korrekt geschlossen. Ein
# trailing Backslash-Lauf muss deshalb verdoppelt werden, bevor er in "..." eingebettet wird.
function ConvertTo-SafeDoubleQuotedArg {
    param([Parameter(Mandatory)] [string] $Value)
    return ($Value -replace '(\\+)$', '$1$1')
}

if ($Uninstall) {
    Unregister-ScheduledTask -TaskName $TaskName -TaskPath $TaskPath -Confirm:$false -ErrorAction SilentlyContinue
    Write-Host "Aufgabe '$TaskPath$TaskName' entfernt."
    return
}

if (-not (Test-Path -LiteralPath $ScriptPath)) {
    throw "Agent-Skript nicht gefunden: $ScriptPath"
}

if ($ScriptPath -match '"|[\x00-\x1F]' -or $OutputDir -match '"|[\x00-\x1F]' -or $DebugLogPath -match '"|[\x00-\x1F]') {
    throw 'ScriptPath, OutputDir und DebugLogPath duerfen keine Anfuehrungszeichen oder Steuerzeichen enthalten.'
}

# Der Task laeuft als SYSTEM aus einer Netzwerkfreigabe; AllSigned ist deshalb
# der sichere Produktions-Default. RemoteSigned ist nur mit expliziter Testfreigabe erlaubt.
if ($ExecutionPolicy -eq 'AllSigned') {
    $signature = Get-AuthenticodeSignature -LiteralPath $ScriptPath
    if ($signature.Status -ne 'Valid') {
        throw ("Agent-Skript ist nicht gueltig signiert ({0}). Fuer lokale Labortests RemoteSigned mit -AllowUnsignedForTest verwenden." -f $signature.Status)
    }
} elseif (-not $AllowUnsignedForTest) {
    throw 'RemoteSigned ist nur mit -AllowUnsignedForTest erlaubt.'
}

$windowsDir = [System.Environment]::GetFolderPath([System.Environment+SpecialFolder]::Windows)
if ([string]::IsNullOrWhiteSpace($windowsDir)) {
    throw 'Windows-Verzeichnis konnte nicht ermittelt werden.'
}
$powerShellPath = Join-Path $windowsDir 'System32\WindowsPowerShell\v1.0\powershell.exe'
if (-not (Test-Path -LiteralPath $powerShellPath)) {
    throw "PowerShell nicht gefunden: $powerShellPath"
}

if ($DebugLog) {
    $scriptLiteral = ConvertTo-SingleQuotedLiteral $ScriptPath
    $outputLiteral = ConvertTo-SingleQuotedLiteral $OutputDir
    $logLiteral = ConvertTo-SingleQuotedLiteral $DebugLogPath
    $debugCommand = @"
`$ErrorActionPreference = 'Stop'
`$VerbosePreference = 'Continue'
`$DebugPreference = 'Continue'
`$scriptPath = $scriptLiteral
`$outputDir = $outputLiteral
`$log = $logLiteral

function Add-HardViewDebugLog {
    param([AllowNull()] `$Value)
    try {
        `$text = if (`$null -eq `$Value) { '<null>' } else { [string] `$Value }
        Add-Content -LiteralPath `$log -Value `$text -Encoding UTF8
    } catch { }
}

try {
    `$logDir = Split-Path -Parent `$log
    if (-not [string]::IsNullOrWhiteSpace(`$logDir)) {
        New-Item -ItemType Directory -Path `$logDir -Force | Out-Null
    }
    Add-HardViewDebugLog ('=== HardView inventory task debug {0} ===' -f (Get-Date).ToString('o'))
    Add-HardViewDebugLog ('Identity  : {0}' -f [System.Security.Principal.WindowsIdentity]::GetCurrent().Name)
    Add-HardViewDebugLog ('ScriptPath: {0}' -f `$scriptPath)
    Add-HardViewDebugLog ('OutputDir : {0}' -f `$outputDir)
    Add-HardViewDebugLog ('PSVersion : {0}' -f `$PSVersionTable.PSVersion)
    & `$scriptPath -OutputDir `$outputDir -Verbose *>&1 | ForEach-Object {
        Add-HardViewDebugLog ([string] `$_)
    }
    Add-HardViewDebugLog 'ExitCode  : 0'
    exit 0
} catch {
    Add-HardViewDebugLog ('ERROR     : {0}' -f `$_.Exception.Message)
    Add-HardViewDebugLog (`$_ | Format-List * -Force | Out-String)
    if (`$_.ScriptStackTrace) { Add-HardViewDebugLog ([string] `$_.ScriptStackTrace) }
    exit 1
}
"@
    $encodedDebugCommand = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($debugCommand))
    $arg = '-NoProfile -NonInteractive -ExecutionPolicy {0} -WindowStyle Hidden -EncodedCommand {1}' -f $ExecutionPolicy, $encodedDebugCommand
} else {
    $arg = '-NoProfile -NonInteractive -ExecutionPolicy {0} -WindowStyle Hidden -File "{1}" -OutputDir "{2}"' -f $ExecutionPolicy, (ConvertTo-SafeDoubleQuotedArg $ScriptPath), (ConvertTo-SafeDoubleQuotedArg $OutputDir)
}

$action = New-ScheduledTaskAction -Execute $powerShellPath -Argument $arg
$trigger = New-ScheduledTaskTrigger -Weekly -DaysOfWeek $DayOfWeek -At $At -RandomDelay (New-TimeSpan -Hours $RandomDelayHours)
$principal = New-ScheduledTaskPrincipal -UserId 'S-1-5-18' -LogonType ServiceAccount -RunLevel Highest  # SYSTEM
$settings = New-ScheduledTaskSettingsSet `
    -Hidden `
    -StartWhenAvailable `
    -DontStopOnIdleEnd `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Hours 1) `
    -MultipleInstances IgnoreNew

# Niedrige Prozesspriotitaet (unauffaellig); Restart bei Fehlschlag
$settings.Priority = 7

Register-ScheduledTask -TaskName $TaskName -TaskPath $TaskPath `
    -Action $action -Trigger $trigger -Principal $principal -Settings $settings `
    -Description 'HardView IT-Asset-Management: woechentliche Hardware-Inventarisierung (nur Hardware-Metadaten).' `
    -Force | Out-Null

Write-Host "Aufgabe '$TaskPath$TaskName' registriert."
Write-Host ("  Skript : {0}" -f $ScriptPath)
Write-Host ("  Ziel   : {0}" -f $OutputDir)
if ($DebugLog) {
    Write-Host ("  Debug  : {0}" -f $DebugLogPath)
}
Write-Host ("  Lauf   : {0} {1} (+ bis {2}h Zufallsverzoegerung), Kontext SYSTEM, ohne Fenster" -f $DayOfWeek, $At.ToString('HH:mm'), $RandomDelayHours)
Write-Host ""
Write-Host "Sofort testen:  Start-ScheduledTask -TaskName '$TaskName' -TaskPath '$TaskPath'"
