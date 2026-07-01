#Requires -Version 5.1
<#
.SYNOPSIS
    Entfernt veraltete HardView-Inventar-JSONs aus der Agent-Inbox.

.DESCRIPTION
    Bereinigt nur erwartete Agent-Dateien im Format <hostname>.json, deren JSON-Hostname
    zum Dateinamen passt und deren collectedAtUtc aelter als die Retention ist.
    Control-Dateien, Assignments und andere Artefakte werden nicht geloescht. Mit -WhatIf
    laesst sich der Lauf gefahrlos pruefen.

.PARAMETER InventoryDir
    Pfad zum incoming-Ordner, z. B. \\FILESERVER\Inventory$\incoming.

.PARAMETER RetentionDays
    Mindestalter in Tagen, bezogen auf collectedAtUtc. Default: 180.

.EXAMPLE
    .\Remove-StaleInventory.ps1 -InventoryDir '\\FILESERVER\Inventory$\incoming' -RetentionDays 180 -WhatIf
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory)]
    [string] $InventoryDir,

    [ValidateRange(30,3650)]
    [int] $RetentionDays = 180
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $InventoryDir -PathType Container)) {
    throw "Inventar-Ordner nicht gefunden: $InventoryDir"
}

$cutoff = [System.DateTimeOffset]::UtcNow.AddDays(-1 * $RetentionDays)
$removed = 0
$failed = 0

function Test-StaleInventoryJsonFile {
    param(
        [System.IO.FileInfo] $File,
        [System.DateTimeOffset] $Cutoff
    )

    if ($File.Name -in @('assignments.json', 'config.json')) {
        return $false
    }

    if ($File.Name -notmatch '^[A-Za-z0-9-]+\.json$') {
        return $false
    }

    try {
        $json = Get-Content -LiteralPath $File.FullName -Raw -ErrorAction Stop | ConvertFrom-Json -ErrorAction Stop
    } catch {
        return $false
    }

    $inventoryHost = [string] $json.hostname
    $collectedAtUtcValue = $json.collectedAtUtc
    $schemaVersion = $json.schemaVersion
    if ([string]::IsNullOrWhiteSpace($inventoryHost) -or $null -eq $collectedAtUtcValue -or [string]::IsNullOrWhiteSpace([string] $collectedAtUtcValue)) {
        return $false
    }
    # SchemaVersion: nur positiv-int akzeptieren (kein String wie "abc", kein
    # unbedachter zukuenftiger Major-Sprung, der eine andere Alterssemantik haben
    # koennte). Aktuell bekannt: Schema 1.
    if ($null -eq $schemaVersion -or $schemaVersion -isnot [int] -or $schemaVersion -lt 1 -or $schemaVersion -gt 1) {
        return $false
    }
    if ($inventoryHost -ine $File.BaseName) {
        return $false
    }

    try {
        if ($collectedAtUtcValue -is [datetime]) {
            $collectedAtUtc = [System.DateTimeOffset]::new(([datetime] $collectedAtUtcValue).ToUniversalTime())
        } else {
            $styles = [System.Globalization.DateTimeStyles]::AssumeUniversal -bor [System.Globalization.DateTimeStyles]::AdjustToUniversal
            $collectedAtUtc = [System.DateTimeOffset]::Parse([string] $collectedAtUtcValue, [System.Globalization.CultureInfo]::InvariantCulture, $styles)
        }
    } catch {
        return $false
    }

    return $collectedAtUtc -lt $Cutoff
}

Get-ChildItem -LiteralPath $InventoryDir -File -Filter '*.json' | Where-Object {
    Test-StaleInventoryJsonFile -File $_ -Cutoff $cutoff
} | ForEach-Object {
    if ($PSCmdlet.ShouldProcess($_.FullName, 'Remove stale inventory JSON')) {
        # Per-Datei-Fehler (Share-Sperre, Read-Only ohne Loeschrechte, gelocktes File
        # waehrend File.Replace) duerfen den gesamten Bereinigungslauf nicht abbrechen.
        # Mit $ErrorActionPreference='Stop' wuerde ein einziger Fehler sonst die ganze
        # Schleife abbrechen und die restlichen Dateien unberuehrt lassen.
        try {
            Remove-Item -LiteralPath $_.FullName -Force -ErrorAction Stop
            $removed += 1
        } catch {
            $failed += 1
            Write-Warning ("Konnte '{0}' nicht entfernen: {1}" -f $_.FullName, $_.Exception.Message)
        }
    }
}

if ($failed -gt 0) {
    Write-Host ("Entfernte Inventar-Dateien: {0} ({1} nicht loeschbar, siehe Warnings)" -f $removed, $failed)
} else {
    Write-Host ("Entfernte Inventar-Dateien: {0}" -f $removed)
}
