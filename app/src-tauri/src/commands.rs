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
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            inner: Mutex::new(Inner {
                config: store::load_config(),
                devices: None,
                ad: None,
            }),
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
    let q = search.to_lowercase();

    let (ad_enabled, cached, needs_fetch) = {
        let inner = state.inner.lock().map_err(|e| e.to_string())?;
        if inner.config.ad_enabled {
            let fresh = matches!(&inner.ad, Some((t, _)) if t.elapsed() < AD_TTL);
            let cached = inner.ad.as_ref().map(|(_, list)| list.clone());
            (true, if fresh { cached.clone() } else { None }, !fresh)
        } else {
            (false, None, false)
        }
    };

    // 1) AD aktiviert -> echtes Lookup mit Cache. Der externe Prozess laeuft
    // bewusst ohne globalen State-Lock, damit die App bedienbar bleibt.
    let mut users: Vec<AdUser> = cached.unwrap_or_default();
    if ad_enabled && needs_fetch {
        match ad::fetch_ad_users() {
            Ok(list) => {
                let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
                if inner.config.ad_enabled {
                    inner.ad = Some((Instant::now(), list.clone()));
                    users = list;
                } else {
                    users.clear();
                }
            }
            Err(_) => {
                let inner = state.inner.lock().map_err(|e| e.to_string())?;
                if let Some((_, list)) = &inner.ad {
                    users = list.clone();
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
                d.user_display.to_lowercase().replace(' ', ".")
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
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    let by = current_user_domain().0;
    store::write_assignment(
        &inner.config,
        &host,
        &user,
        &user_display,
        user_dept.as_deref().unwrap_or(""),
        &note,
        &by,
    )?;
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
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    store::save_config(&config)?;
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
    let _ = format; // aktuell nur CSV
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    let devs = ensure_devices(&mut inner).clone();

    let mut csv = String::from(
        "Hostname;Benutzer;Quelle;Abteilung;Status;Begruendungen;CPU;Kerne;RAM_GB;Datentraeger;Groesse_GB;Alter_Jahre;Betriebssystem;Letzte_Inventarisierung;Seriennummer;Modell\r\n",
    );
    // CSV-Härtung: jedes Feld wird zitiert (RFC 4180) und gegen Formel-Injection
    // abgesichert. Werte stammen z. T. aus nicht vertrauenswürdigen Agent-JSONs;
    // ein führendes = + - @ (oder Tab) würde Excel/Calc als Formel auswerten
    // (DDE → Codeausführung auf dem IT-Arbeitsplatz beim Öffnen des Exports).
    let esc = |s: &str| {
        let cleaned = s.replace(['\r', '\n'], " ");
        let guarded = if cleaned.starts_with(['=', '+', '-', '@', '\t']) {
            format!("'{}", cleaned)
        } else {
            cleaned
        };
        format!("\"{}\"", guarded.replace('"', "\"\""))
    };
    for d in &devs {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{}\r\n",
            esc(&d.host),
            esc(&d.user_display),
            esc(&d.user_source),
            esc(&d.dept),
            esc(&d.status_label),
            esc(&d.upgrade_reasons.join(" | ")),
            esc(&d.cpu),
            d.cores,
            d.ram_gb,
            esc(&d.disk_type),
            d.disk_gb,
            d.age_years
                .map(|a| format!("{:.1}", a).replace('.', ","))
                .unwrap_or_default(),
            esc(&d.os_caption),
            esc(&d.last_seen_text),
            esc(&d.serial_number),
            esc(&format!("{} {}", d.manufacturer, d.model)),
        ));
    }

    let docs = std::env::var("USERPROFILE")
        .map(|p| std::path::Path::new(&p).join("Documents"))
        .unwrap_or_else(|_| std::env::temp_dir());
    let _ = std::fs::create_dir_all(&docs);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let file = docs.join(format!("HardView-Export-{}.csv", stamp));
    // CSV als UTF-8 mit BOM, damit Excel Umlaute korrekt anzeigt
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(csv.as_bytes());
    std::fs::write(&file, bytes).map_err(|e| format!("Export fehlgeschlagen: {}", e))?;

    Ok(serde_json::json!({ "ok": true, "path": file.to_string_lossy(), "rows": devs.len() }))
}

fn current_user_domain() -> (String, String) {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "Unbekannt".into());
    let domain = std::env::var("USERDNSDOMAIN")
        .or_else(|_| std::env::var("USERDOMAIN"))
        .unwrap_or_else(|_| "kowobau.local".into())
        .to_lowercase();
    let full = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_else(|_| "KOWOBAU".into()),
        user
    );
    (full, domain)
}
