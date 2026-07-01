/// Ermittelt die angemeldete Windows-Identitaet fuer den Zuordnungs-Audit-Trail
/// (`confirmedBy`/`updatedBy` in assignments.json) und die Sidebar-Anzeige.
/// Liest die SAM-kompatible Kennung ("DOMAENE\Benutzer") und die DNS-Domaene direkt aus dem
/// authentifizierten Token/System (statt aus `%USERNAME%`/`%USERDOMAIN%`-Umgebungsvariablen,
/// die vor dem Start des Prozesses beliebig gesetzt werden koennen). Faellt bei Fehlern
/// (z. B. nicht domaenengebundener Rechner) je Feld unabhaengig auf die bisherige,
/// umgebungsvariablenbasierte Ermittlung zurueck.
pub(crate) fn current_user_domain() -> (String, String) {
    let full = windows_sam_compatible_name().unwrap_or_else(env_full_identity);
    let domain = windows_dns_domain().unwrap_or_else(env_dns_domain);
    (full, domain)
}

fn env_full_identity() -> String {
    format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_else(|_| "CORP".into()),
        std::env::var("USERNAME").unwrap_or_else(|_| "Unbekannt".into())
    )
}

fn env_dns_domain() -> String {
    std::env::var("USERDNSDOMAIN")
        .or_else(|_| std::env::var("USERDOMAIN"))
        .unwrap_or_else(|_| "corp.local".into())
        .to_lowercase()
}

/// SAM-kompatible Anmeldekennung ("DOMAENE\Benutzer") ueber `GetUserNameExW`, direkt aus dem
/// Sicherheitstoken des laufenden Prozesses — nicht aus einer Umgebungsvariable ableitbar.
#[cfg(windows)]
fn windows_sam_compatible_name() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Security::Authentication::Identity::{
        GetUserNameExW, NameSamCompatible,
    };

    let mut buffer = [0u16; 256];
    let mut len = buffer.len() as u32;
    let ok = unsafe { GetUserNameExW(NameSamCompatible, buffer.as_mut_ptr(), &mut len) };
    if !ok || len == 0 {
        return None;
    }
    let name = OsString::from_wide(&buffer[..len as usize])
        .to_string_lossy()
        .into_owned();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(not(windows))]
fn windows_sam_compatible_name() -> Option<String> {
    None
}

/// DNS-Domaene des Rechners ueber `GetComputerNameExW` (nur Anzeige in der Sidebar).
/// Auf einem nicht domaenengebundenen Rechner liefert die API erfolgreich einen leeren
/// String zurueck — das behandeln wir wie einen Fehlschlag, um auf den bisherigen
/// "corp.local"-Default auszuweichen statt eine leere Sidebar-Zeile zu zeigen.
#[cfg(windows)]
fn windows_dns_domain() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::{
        ComputerNameDnsDomain, GetComputerNameExW,
    };

    let mut buffer = [0u16; 256];
    let mut len = buffer.len() as u32;
    let ok = unsafe { GetComputerNameExW(ComputerNameDnsDomain, buffer.as_mut_ptr(), &mut len) };
    if ok == 0 {
        return None;
    }
    let name = OsString::from_wide(&buffer[..len as usize])
        .to_string_lossy()
        .into_owned();
    if name.is_empty() {
        None
    } else {
        Some(name.to_lowercase())
    }
}

#[cfg(not(windows))]
fn windows_dns_domain() -> Option<String> {
    None
}

/// Leitet aus einem Anzeigenamen einen plausiblen SAM-Account ab — nur als
/// CSV-Fallback, wenn kein AD verfuegbar ist. Deutsche Umlaute werden
/// transliteriert, damit der Wert ASCII-stabil und deterministisch bleibt.
pub(crate) fn synth_sam(display: &str) -> String {
    let mut sam = String::new();
    for ch in display.chars() {
        match ch {
            'ä' | 'Ä' => sam.push_str("ae"),
            'ö' | 'Ö' => sam.push_str("oe"),
            'ü' | 'Ü' => sam.push_str("ue"),
            'ß' => sam.push_str("ss"),
            ' ' => sam.push('.'),
            c if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') => sam.push(c),
            _ => {}
        }
    }
    sam.to_lowercase()
}
