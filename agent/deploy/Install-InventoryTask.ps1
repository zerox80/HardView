#Requires -Version 5.1
#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Registriert den HardView-Inventar-Agent als geplante Aufgabe (woechentlich, still, SYSTEM).
    Fuer lokale Tests/Einzelinstallation. Massenverteilung erfolgt per GPO (siehe README-Deployment.md).

.DESCRIPTION
    Legt die Aufgabe "Kowobau\HardwareInventar" an:
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

.EXAMPLE
    .\Install-InventoryTask.ps1 -OutputDir '\\fileserver\Inventory$\incoming'
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
    [string]   $TaskPath = '\Kowobau\',
    [ValidateSet('AllSigned','RemoteSigned')]
    [string]   $ExecutionPolicy = 'AllSigned',
    [switch]   $Uninstall
)

$ErrorActionPreference = 'Stop'

if ($Uninstall) {
    Unregister-ScheduledTask -TaskName $TaskName -TaskPath $TaskPath -Confirm:$false -ErrorAction SilentlyContinue
    Write-Host "Aufgabe '$TaskPath$TaskName' entfernt."
    return
}

if (-not (Test-Path -LiteralPath $ScriptPath)) {
    throw "Agent-Skript nicht gefunden: $ScriptPath"
}

# Der Task laeuft als SYSTEM aus einer Netzwerkfreigabe; AllSigned ist deshalb
# der sichere Produktions-Default. RemoteSigned nur fuer lokale Tests verwenden.
$arg = '-NoProfile -NonInteractive -ExecutionPolicy {0} -WindowStyle Hidden -File "{1}" -OutputDir "{2}"' -f $ExecutionPolicy, $ScriptPath, $OutputDir

$powerShellPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
if (-not (Test-Path -LiteralPath $powerShellPath)) {
    throw "PowerShell nicht gefunden: $powerShellPath"
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
    -Description 'Kowobau IT-Asset-Management: woechentliche Hardware-Inventarisierung (nur Hardware-Metadaten).' `
    -Force | Out-Null

Write-Host "Aufgabe '$TaskPath$TaskName' registriert."
Write-Host ("  Skript : {0}" -f $ScriptPath)
Write-Host ("  Ziel   : {0}" -f $OutputDir)
Write-Host ("  Lauf   : {0} {1} (+ bis {2}h Zufallsverzoegerung), Kontext SYSTEM, ohne Fenster" -f $DayOfWeek, $At.ToString('HH:mm'), $RandomDelayHours)
Write-Host ""
Write-Host "Sofort testen:  Start-ScheduledTask -TaskName '$TaskName' -TaskPath '$TaskPath'"
