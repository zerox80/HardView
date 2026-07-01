//! Reine Hilfsfunktionen fuer `get_ad_users`: CSV/Inventar-Fallback-Liste bauen und
//! Suchfilter + Truncate anwenden. Die zustandsbehaftete Cache/TTL/Fetch-Orchestrierung
//! (inkl. `ad_fetch`-vor-`inner`-Lock-Reihenfolge) bleibt bewusst in `commands.rs`.
use crate::identity::synth_sam;
use crate::model::{AdUser, DeviceFull};
use std::collections::HashSet;

/// Baut die Fallback-Benutzerliste aus Geraetedaten (CSV/Inventar), wenn AD deaktiviert
/// oder keine AD-Antwort verfuegbar ist. Dedupliziert nach (synthetisiertem oder echtem)
/// SAM und sortiert nach Anzeigename. Kollidieren zwei synthetisierte SAMs (z.B. zwei
/// Personen mit gleichem Anzeigenamen in unterschiedlichen Abteilungen), wird der Host
/// als Disambiguator angehaengt statt den zweiten Eintrag stillschweigend zu verwerfen.
pub(crate) fn fallback_users_from_devices(devs: &[DeviceFull]) -> Vec<AdUser> {
    let mut seen = HashSet::new();
    let mut users: Vec<AdUser> = Vec::new();
    for d in devs {
        if d.user_display.is_empty() || d.user_display == "Unbekannt" {
            continue;
        }
        let sam = if !d.user_sam.is_empty() {
            d.user_sam.clone()
        } else {
            let base = synth_sam(&d.user_display);
            if seen.contains(&base) {
                format!("{base}.{}", d.host.to_lowercase())
            } else {
                base
            }
        };
        if seen.insert(sam.clone()) {
            users.push(AdUser {
                sam,
                display: d.user_display.clone(),
                dept: d.dept.clone(),
                mail: String::new(),
            });
        }
    }
    users.sort_by(|a, b| a.display.cmp(&b.display));
    users
}

/// Filtert per Case-insensitive Substring-Suche ueber Anzeigename/SAM/Abteilung/Mail
/// (nur wenn `query_lower` nicht leer ist) und kappt das Ergebnis auf maximal 100 Treffer.
/// `query_lower` muss bereits kleingeschrieben sein.
pub(crate) fn filter_and_truncate(mut users: Vec<AdUser>, query_lower: &str) -> Vec<AdUser> {
    if !query_lower.is_empty() {
        users.retain(|u| {
            format!("{} {} {} {}", u.display, u.sam, u.dept, u.mail)
                .to_lowercase()
                .contains(query_lower)
        });
    }
    users.truncate(100);
    users
}
