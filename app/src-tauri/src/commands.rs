//! Tauri-Befehle (Bruecke Frontend <-> Backend). Halten Geraeteliste & AD-Cache im State.
use crate::ad;
use crate::model::*;
use crate::store;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

const AD_TTL: Duration = Duration::from_secs(600);

pub struct Inner {
    pub config: Config,
    pub devices: Option<Vec<DeviceFull>>,
    pub ad: Option<(Instant, Vec<AdUser>)>,
}

pub struct AppState {
    pub inner: Mutex<Inner>,
    /// Serialisiert AD-Abfragen, damit nebenlaeufige Aufrufe nicht mehrere
    /// PowerShell-Prozesse starten. Lock-Reihenfolge ist strikt `ad_fetch` vor
    /// `inner` (nie umgekehrt) -> deadlockfrei. `inner` wird darunter nur kurz
    /// fuer Cache-Pruefung/-Update gehalten, nie ueber den Fetch hinweg.
    pub ad_fetch: Mutex<()>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            inner: Mutex::new(Inner {
                config: store::load_config(),
                devices: None,
                ad: None,
            }),
            ad_fetch: Mutex::new(()),
        }
    }
}

fn ensure_devices(inner: &mut Inner) -> &Vec<DeviceFull> {
    if inner.devices.is_none() {
        inner.devices = Some(store::build_devices(&inner.config));
    }
    inner.devices.as_ref().unwrap()
}

#[tauri::command]
pub fn get_devices(state: State<AppState>) -> Result<Vec<DeviceFull>, String> {
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(ensure_devices(&mut inner).clone())
}

#[tauri::command]
pub fn get_device(state: State<AppState>, host: String) -> Result<Option<DeviceFull>, String> {
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    let d = ensure_devices(&mut inner)
        .iter()
        .find(|d| d.host.eq_ignore_ascii_case(&host))
        .cloned();
    Ok(d)
}

#[tauri::command]
pub fn get_overview(state: State<AppState>) -> Result<Overview, String> {
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    let th = inner.config.thresholds.clone();
    let devs = ensure_devices(&mut inner).clone();
    Ok(store::build_overview(&devs, &th))
}

#[tauri::command]
pub fn get_ad_users(state: State<AppState>, search: String) -> Result<Vec<AdUser>, String> {
    let query = search.trim().to_string();
    let q = query.to_lowercase();
    let mut users = Vec::new();

    // AD aktiv? Leere Abfragen nutzen den Gesamtlisten-Cache, Suchabfragen gehen
    // gezielt gegen LDAP, damit sehr grosse Directories keine Treffer abschneiden.
    let (ad_enabled, cached_full, needs_full_fetch) = {
        let inner = state.inner.lock().map_err(|e| e.to_string())?;
        if inner.config.ad_enabled {
            match &inner.ad {
                Some((t, list)) if t.elapsed() < AD_TTL => (true, Some(list.clone()), false),
                _ => (true, None, q.is_empty()),
            }
        } else {
            (false, None, false)
        }
    };

    if ad_enabled {
        if !q.is_empty() {
            let _fetch_guard = state.ad_fetch.lock().map_err(|e| e.to_string())?;
            match ad::fetch_ad_users(&query) {
                Ok(list) => users = list,
                Err(_) => {
                    if let Some(list) = cached_full {
                        users = list;
                    }
                }
            }
        } else if let Some(list) = cached_full {
            users = list;
        } else if needs_full_fetch {
            let _fetch_guard = state.ad_fetch.lock().map_err(|e| e.to_string())?;
            let refreshed = {
                let inner = state.inner.lock().map_err(|e| e.to_string())?;
                if !inner.config.ad_enabled {
                    Some(Vec::new())
                } else if let Some((t, list)) = &inner.ad {
                    if t.elapsed() < AD_TTL {
                        Some(list.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            match refreshed {
                Some(list) => users = list,
                None => {
                    if let Ok(list) = ad::fetch_ad_users("") {
                        let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
                        if inner.config.ad_enabled {
                            inner.ad = Some((Instant::now(), list.clone()));
                            users = list;
                        }
                    }
                }
            }
        }
    }

    // 2) Fallback: eindeutige Benutzer aus den Geraetedaten (CSV/Inventar)
    if users.is_empty() {
        let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
        let devs = ensure_devices(&mut inner).clone();
        let mut seen = std::collections::HashSet::new();
        for d in devs {
            if d.user_display.is_empty() || d.user_display == "Unbekannt" {
                continue;
            }
            let sam = if d.user_sam.is_empty() {
                synth_sam(&d.user_display)
            } else {
                d.user_sam.clone()
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
    }

    if !q.is_empty() {
        users.retain(|u| {
            format!("{} {} {}", u.display, u.sam, u.dept)
                .to_lowercase()
                .contains(&q)
        });
    }
    users.truncate(100);
    Ok(users)
}

#[tauri::command]
pub fn set_assignment(
    state: State<AppState>,
    host: String,
    user: String,
    user_display: String,
    user_dept: Option<String>,
    note: String,
) -> Result<serde_json::Value, String> {
    let config = {
        let inner = state.inner.lock().map_err(|e| e.to_string())?;
        inner.config.clone()
    };
    let by = current_user_domain().0;
    store::write_assignment(
        &config,
        &host,
        &user,
        &user_display,
        user_dept.as_deref().unwrap_or(""),
        &note,
        &by,
    )?;
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    inner.devices = None; // Cache invalidieren -> beim naechsten Lesen neu mergen
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
pub fn refresh(state: State<AppState>) -> Result<serde_json::Value, String> {
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    inner.config = store::load_config();
    inner.devices = None;
    inner.ad = None;
    let n = ensure_devices(&mut inner).len();
    Ok(serde_json::json!({ "ok": true, "count": n }))
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<Config, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(inner.config.clone())
}

#[tauri::command]
pub fn set_settings(state: State<AppState>, config: Config) -> Result<serde_json::Value, String> {
    store::save_config(&config)?;
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    inner.config = config;
    inner.devices = None;
    inner.ad = None;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
pub fn me() -> Result<serde_json::Value, String> {
    let (user, domain) = current_user_domain();
    let name = user.rsplit('\\').next().unwrap_or(&user).to_string();
    let initials: String = name
        .split(['.', ' ', '_'])
        .filter(|s| !s.is_empty())
        .take(2)
        .map(|s| s.chars().next().unwrap_or(' '))
        .collect::<String>()
        .to_uppercase();
    Ok(serde_json::json!({
        "name": name,
        "initials": if initials.is_empty() { "?".into() } else { initials },
        "domain": domain
    }))
}

#[tauri::command]
pub fn export_devices(state: State<AppState>, format: String) -> Result<serde_json::Value, String> {
    if !format.trim().eq_ignore_ascii_case("csv") {
        return Err(format!("Nicht unterstütztes Exportformat: {}", format));
    }
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    let devs = ensure_devices(&mut inner).clone();
    drop(inner);

    let (file, rows) = crate::export::write_devices_csv(&devs)?;
    Ok(serde_json::json!({ "ok": true, "path": file.to_string_lossy(), "rows": rows }))
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

fn current_user_domain() -> (String, String) {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "Unbekannt".into());
    let domain = std::env::var("USERDNSDOMAIN")
        .or_else(|_| std::env::var("USERDOMAIN"))
        .unwrap_or_else(|_| "corp.local".into())
        .to_lowercase();
    let full = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_else(|_| "CORP".into()),
        user
    );
    (full, domain)
}
