#Requires -Version 5.1
<#  Read-only AD-Lookup via System.DirectoryServices (integrierte Windows-Auth, kein RSAT).
    Gibt aktivierte Benutzer als JSON-Array aus: sam, display, dept, mail.
    Wird vom HardView-Backend (ad.rs) ohne Fenster aufgerufen.  #>
$ErrorActionPreference = 'Stop'
try {
    $root = New-Object System.DirectoryServices.DirectoryEntry('LDAP://RootDSE')
    $base = [string]$root.Get('defaultNamingContext')
    $de = New-Object System.DirectoryServices.DirectoryEntry("LDAP://$base")
    $ds = New-Object System.DirectoryServices.DirectorySearcher($de)
    # Aktivierte Personen-/Benutzerobjekte (deaktivierte via UAC-Bit 2 ausgeschlossen)
    $ds.Filter = '(&(objectCategory=person)(objectClass=user)(!(userAccountControl:1.2.840.113556.1.4.803:=2)))'
    $ds.PageSize = 1000
    $ds.SizeLimit = 0
    'sAMAccountName','displayName','department','mail','givenName','sn' | ForEach-Object { [void]$ds.PropertiesToLoad.Add($_) }

    # Obergrenze gegen sehr grosse Verzeichnisse (Speicher/Zeit); die App filtert
    # ohnehin in Rust und schneidet auf 100 Treffer zu.
    $max = 20000
    $list = New-Object System.Collections.Generic.List[object]
    foreach ($r in $ds.FindAll()) {
        $p = $r.Properties
        $sam = if ($p['samaccountname'].Count) { [string]$p['samaccountname'][0] } else { '' }
        if (-not $sam) { continue }
        $disp = if ($p['displayname'].Count) { [string]$p['displayname'][0] }
                elseif ($p['givenname'].Count -or $p['sn'].Count) { (('{0} {1}' -f [string]$p['givenname'][0], [string]$p['sn'][0]).Trim()) }
                else { $sam }
        $dept = if ($p['department'].Count) { [string]$p['department'][0] } else { '' }
        $mail = if ($p['mail'].Count) { [string]$p['mail'][0] } else { '' }
        $list.Add([ordered]@{ sam = $sam; display = $disp; dept = $dept; mail = $mail })
        if ($list.Count -ge $max) { break }
    }
    # Immer als Array ausgeben (auch bei 0/1 Treffern)
    [Console]::Out.Write((ConvertTo-Json -InputObject @($list) -Depth 3 -Compress))
} catch {
    [Console]::Error.WriteLine("AD-Fehler: $($_.Exception.Message)")
    exit 1
}
