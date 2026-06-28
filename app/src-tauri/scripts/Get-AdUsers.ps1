#Requires -Version 5.1
<#  Read-only AD-Lookup via System.DirectoryServices (integrierte Windows-Auth, kein RSAT).
    Gibt aktivierte Benutzer als JSON-Array aus: sam, display, dept, mail.
    Wird vom HardView-Backend (ad.rs) ohne Fenster aufgerufen.  #>
$ErrorActionPreference = 'Stop'
try {
    function ConvertTo-LdapFilterValue {
        param([AllowNull()] [string] $Value)

        if ($null -eq $Value) { return '' }
        $sb = New-Object System.Text.StringBuilder
        foreach ($ch in $Value.ToCharArray()) {
            $code = [int][char]$ch
            if ($code -eq 0) { [void]$sb.Append('\00') }
            elseif ($code -eq 40) { [void]$sb.Append('\28') }
            elseif ($code -eq 41) { [void]$sb.Append('\29') }
            elseif ($code -eq 42) { [void]$sb.Append('\2a') }
            elseif ($code -eq 92) { [void]$sb.Append('\5c') }
            else { [void]$sb.Append($ch) }
        }
        return $sb.ToString()
    }

    $root = New-Object System.DirectoryServices.DirectoryEntry('LDAP://RootDSE')
    $base = [string]$root.Get('defaultNamingContext')
    $de = New-Object System.DirectoryServices.DirectoryEntry("LDAP://$base")
    $ds = New-Object System.DirectoryServices.DirectorySearcher($de)
    # Aktivierte Personen-/Benutzerobjekte (deaktivierte via UAC-Bit 2 ausgeschlossen)
    $enabledFilter = '(&(objectCategory=person)(objectClass=user)(!(userAccountControl:1.2.840.113556.1.4.803:=2)))'
    $search = [string] $env:HARDVIEW_AD_SEARCH
    if ($null -eq $search) { $search = '' }
    $search = $search.Trim()
    if ([string]::IsNullOrWhiteSpace($search)) {
        $ds.Filter = $enabledFilter
    } else {
        $needle = '*' + (ConvertTo-LdapFilterValue $search) + '*'
        $matchFilter = "(|(sAMAccountName=$needle)(displayName=$needle)(department=$needle)(mail=$needle)(givenName=$needle)(sn=$needle))"
        $ds.Filter = "(&$enabledFilter$matchFilter)"
    }
    $ds.PageSize = 1000
    $ds.SizeLimit = 0
    'sAMAccountName','displayName','department','mail','givenName','sn' | ForEach-Object { [void]$ds.PropertiesToLoad.Add($_) }

    # Obergrenze gegen sehr grosse Verzeichnisse (Speicher/Zeit). Bei Suchtext
    # filtert LDAP bereits serverseitig, deshalb reicht ein kleineres Limit.
    $max = if ([string]::IsNullOrWhiteSpace($search)) { 20000 } else { 500 }
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
