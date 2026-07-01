//! Tauri-Befehle (Bruecke Frontend <-> Backend). Halten Geraeteliste & AD-Cache im State.
use crate::ad;
use crate::ad_users::{fallback_users_from_devices, filter_and_truncate};
use crate::identity::current_user_domain;
use crate::model::*;
use crate::store;
use std::collections::BTreeSet;
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
    let devs = ensure_devices(&mut inner);
    Ok(store::build_overview(devs, &th))
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
                Err(e) => {
                    eprintln!("[hardview] AD-Suche fehlgeschlagen ('{query}'): {e}");
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
                None => match ad::fetch_ad_users("") {
                    Ok(list) => {
                        let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
                        if inner.config.ad_enabled {
                            inner.ad = Some((Instant::now(), list.clone()));
                            users = list;
                        }
                    }
                    Err(e) => {
                        eprintln!("[hardview] AD-Vollabruf fehlgeschlagen: {e}");
                    }
                },
            }
        }
    }

    // 2) Fallback: eindeutige Benutzer aus den Geraetedaten (CSV/Inventar)
    if users.is_empty() {
        let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
        let devs = ensure_devices(&mut inner);
        users = fallback_users_from_devices(devs);
    }

    Ok(filter_and_truncate(users, &q))
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
    let host_key = host.trim().to_uppercase();
    let (config, known_hosts) = {
        let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
        let config = inner.config.clone();
        let known_hosts: BTreeSet<String> = ensure_devices(&mut inner)
            .iter()
            .map(|d| d.host.clone())
            .collect();
        (config, known_hosts)
    };
    let by = current_user_domain().0;
    store::write_assignment_for_known_hosts(
        &config,
        &known_hosts,
        store::AssignmentWrite {
            host: &host,
            user: &user,
            user_display: &user_display,
            user_dept: user_dept.as_deref().unwrap_or(""),
            note: &note,
            by: &by,
        },
    )?;
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    let updated = inner.devices.as_mut().and_then(|devs| {
        devs.iter_mut()
            .find(|d| d.host.eq_ignore_ascii_case(&host_key))
            .map(|d| {
                store::apply_manual_assignment(
                    d,
                    &user,
                    &user_display,
                    user_dept.as_deref().unwrap_or(""),
                    &note,
                    &by,
                );
                d.clone()
            })
    });
    Ok(serde_json::json!({ "ok": true, "device": updated }))
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
