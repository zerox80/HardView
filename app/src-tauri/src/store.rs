//! Datei-Share-Zugriff: Config, Master-CSV, Inventar-JSONs, Zuordnungen.
//! Fuehrt alles zu DeviceFull zusammen und aggregiert die Overview.
use crate::model::*;
use crate::upgrade::{evaluate, fmt_de, DeviceFacts};
use chrono::{DateTime, SecondsFormat, Utc};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const PALETTE: [&str; 8] = [
    "#4f8cff", "#2fd6a6", "#b98cff", "#ff8a4f", "#ffb454", "#5fc9ff", "#ff7a9c", "#7ee081",
];
const ASSIGNMENT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const ASSIGNMENT_LOCK_STALE: Duration = Duration::from_secs(60);

// ------------------------------------------------------------------ Config
pub fn config_path() -> PathBuf {
    app_config_dir().join("config.json")
}

fn app_config_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    Path::new(&base).join("HardView")
}

pub fn default_config() -> Config {
    let data_dir =
        std::env::var("KOWO_DATA_DIR").unwrap_or_else(|_| "G:\\Inventory\\incoming".into());
    let csv = std::env::var("KOWO_CSV")
        .unwrap_or_else(|_| "G:\\Bitlocker\\Rollout_Masterliste.csv".into());
    Config {
        data_dir: data_dir.clone(),
        master_csv_path: csv,
        assignments_path: Some(
            std::env::var("KOWO_ASSIGN").unwrap_or_else(|_| default_assignments_path(&data_dir)),
        ),
        ad_enabled: std::env::var("KOWO_AD").map(|v| v == "1").unwrap_or(false),
        thresholds: Thresholds::default(),
    }
}

pub fn load_config() -> Config {
    let mut cfg = if let Ok(txt) = fs::read_to_string(config_path()) {
        serde_json::from_str::<Config>(&txt).unwrap_or_else(|_| default_config())
    } else {
        default_config()
    };
    // Dev-/Override per Umgebungsvariablen (erleichtert Tests gegen sample-data)
    if let Ok(v) = std::env::var("KOWO_DATA_DIR") {
        cfg.data_dir = v;
    }
    if let Ok(v) = std::env::var("KOWO_CSV") {
        cfg.master_csv_path = v;
    }
    if let Ok(v) = std::env::var("KOWO_ASSIGN") {
        cfg.assignments_path = Some(v);
    }
    if cfg.assignments_path.is_none() {
        cfg.assignments_path = Some(default_assignments_path(&cfg.data_dir));
    }
    if validate_config(&cfg).is_ok() {
        return cfg;
    }

    // Repair legacy configs that put trusted assignments next to inventory JSONs.
    cfg.assignments_path = Some(default_assignments_path(&cfg.data_dir));
    if validate_config(&cfg).is_ok() {
        return cfg;
    }

    let data_dir = "G:\\Inventory\\incoming".to_string();
    Config {
        data_dir: data_dir.clone(),
        master_csv_path: "G:\\Bitlocker\\Rollout_Masterliste.csv".to_string(),
        assignments_path: Some(default_assignments_path(&data_dir)),
        ad_enabled: false,
        thresholds: Thresholds::default(),
    }
}

/// Validiert vom Frontend gelieferte Pfade (Defense-in-Depth gegen einen
/// kompromittierten Renderer): assignmentsPath ist das einzige Schreibziel
/// und muss als .json in einem trusted Control-Pfad oder AppData liegen.
pub fn validate_config(cfg: &Config) -> Result<(), String> {
    let bad = |s: &str| s.trim().is_empty() || s.chars().any(|c| c == '\0' || c.is_control());
    if bad(&cfg.data_dir) {
        return Err("Ungültiger dataDir-Pfad".into());
    }
    if bad(&cfg.master_csv_path) {
        return Err("Ungültiger masterCsvPath".into());
    }
    if let Some(p) = &cfg.assignments_path {
        if bad(p) {
            return Err("Ungültiger assignmentsPath".into());
        }
        if !p.to_lowercase().ends_with(".json") {
            return Err("assignmentsPath muss auf .json enden".into());
        }
        let assignment_path = normalize_path(Path::new(p));
        let data_path = normalize_path(Path::new(&cfg.data_dir));
        let app_dir = normalize_path(&app_config_dir());
        let control_dir = normalize_path(&control_dir_for(&cfg.data_dir));

        if path_starts_with(&assignment_path, &data_path) {
            return Err(
                "assignmentsPath darf nicht im client-beschreibbaren dataDir liegen".into(),
            );
        }
        let allowed = path_starts_with(&assignment_path, &control_dir)
            || path_starts_with(&assignment_path, &app_dir);
        if !allowed {
            return Err("assignmentsPath muss im Control-Ordner oder AppData liegen".into());
        }
    }
    Ok(())
}

pub fn save_config(cfg: &Config) -> Result<(), String> {
    validate_config(cfg)?;
    let p = config_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let txt = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    atomic_write(&p, &txt)
}

fn default_assignments_path(data_dir: &str) -> String {
    control_dir_for(data_dir)
        .join("assignments.json")
        .to_string_lossy()
        .to_string()
}

fn control_dir_for(data_dir: &str) -> PathBuf {
    let data_path = Path::new(data_dir);
    let base = if data_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("incoming"))
        .unwrap_or(false)
    {
        data_path.parent().unwrap_or(data_path)
    } else {
        data_path.parent().unwrap_or_else(|| Path::new(""))
    };
    base.join("control")
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component.as_os_str());
                }
            }
            _ => out.push(component.as_os_str()),
        }
    }
    out
}

fn path_starts_with(path: &Path, base: &Path) -> bool {
    let path_components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let base_components: Vec<String> = base
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    base_components.len() <= path_components.len()
        && path_components
            .iter()
            .zip(base_components.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

// ------------------------------------------------------------------ Encoding-tolerantes Lesen
/// Liest eine Textdatei. UTF-8 bevorzugt; faellt sonst auf Windows-1252 zurueck
/// (deutsche Umlaute bleiben korrekt), damit ANSI-CSV aus Excel funktioniert.
fn read_text(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
    match String::from_utf8(bytes) {
        Ok(s) => Ok(strip_bom(s)),
        Err(e) => Ok(decode_windows_1252(&e.into_bytes())),
    }
}
fn strip_bom(s: String) -> String {
    s.strip_prefix('\u{feff}')
        .map(|x| x.to_string())
        .unwrap_or(s)
}
fn decode_windows_1252(bytes: &[u8]) -> String {
    const C1: [char; 32] = [
        '\u{20ac}', '\u{0081}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02c6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008d}',
        '\u{017d}', '\u{008f}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02dc}', '\u{2122}', '\u{0161}', '\u{203a}',
        '\u{0153}', '\u{009d}', '\u{017e}', '\u{0178}',
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => C1[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

// ------------------------------------------------------------------ Master-CSV
#[derive(Default, Clone)]
pub struct CsvRow {
    pub user: String,
}

pub fn read_master_csv(path: &str) -> HashMap<String, CsvRow> {
    let mut out = HashMap::new();
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > 20 * 1024 * 1024 {
            // 20 MB Limit
            return out;
        }
    }
    let txt = match read_text(path) {
        Ok(t) => t,
        Err(_) => return out,
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b';')
        .flexible(true)
        .has_headers(true)
        .from_reader(txt.as_bytes());
    // Spaltenindizes aus dem Header bestimmen
    let (mut i_host, mut i_user) = (None, None);
    if let Ok(hdr) = rdr.headers() {
        for (i, h) in hdr.iter().enumerate() {
            match h.trim().to_lowercase().as_str() {
                "computer" => i_host = Some(i),
                "benutzer" => i_user = Some(i),
                _ => {}
            }
        }
    }
    let i_host = match i_host {
        Some(i) => i,
        None => return out,
    };
    for rec in rdr.records().flatten() {
        let host = rec.get(i_host).unwrap_or("").trim();
        if host.is_empty() {
            continue;
        }
        out.insert(
            host.to_uppercase(),
            CsvRow {
                user: i_user
                    .and_then(|i| rec.get(i))
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            },
        );
    }
    out
}

// ------------------------------------------------------------------ Inventar-JSONs
pub fn read_inventory_dir(dir: &str) -> HashMap<String, Inventory> {
    const MAX_INVENTORY_FILES: usize = 50_000; // Schutz vor geflutetem Share (DoS)
    let mut out = HashMap::new();
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return out,
    };
    let mut scanned = 0usize;
    for entry in rd.flatten() {
        scanned += 1;
        if scanned > MAX_INVENTORY_FILES {
            break;
        }
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
        {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if meta.len() > 1024 * 1024 {
                // 1 MB Limit
                continue;
            }
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if stem.eq_ignore_ascii_case("assignments") || stem.eq_ignore_ascii_case("config") {
            continue;
        }
        if !is_valid_host_id(&stem) {
            continue;
        }
        let txt = match read_text(&path.to_string_lossy()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if let Ok(mut inv) = serde_json::from_str::<Inventory>(&txt) {
            let Some(payload_host) = inv
                .hostname
                .as_deref()
                .map(str::trim)
                .filter(|h| !h.is_empty())
            else {
                continue;
            };
            if !payload_host.eq_ignore_ascii_case(&stem) {
                continue;
            }
            inv.hostname = Some(stem.clone());
            out.entry(stem.to_uppercase()).or_insert(inv);
        }
    }
    out
}

fn is_valid_host_id(host: &str) -> bool {
    let len = host.len();
    len > 0
        && len <= 63
        && !host.starts_with('-')
        && !host.ends_with('-')
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn read_known_hosts(cfg: &Config) -> BTreeSet<String> {
    let csv = read_master_csv(&cfg.master_csv_path);
    let inv = read_inventory_dir(&cfg.data_dir);
    known_hosts_from(&csv, &inv)
}

fn known_hosts_from(
    csv: &HashMap<String, CsvRow>,
    inv: &HashMap<String, Inventory>,
) -> BTreeSet<String> {
    let mut hosts = BTreeSet::new();
    hosts.extend(csv.keys().cloned());
    hosts.extend(inv.keys().cloned());
    hosts
}

// ------------------------------------------------------------------ Zuordnungen
pub fn read_assignments(path: &str) -> AssignmentStore {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > 2 * 1024 * 1024 {
            // 2 MB Limit
            return AssignmentStore::default();
        }
    }
    let mut store = read_text(path)
        .ok()
        .and_then(|t| serde_json::from_str::<AssignmentStore>(&t).ok())
        .unwrap_or_default();
    // Schluessel auf Grossschreibung normalisieren (Host-Matching)
    let upper: HashMap<String, AssignmentEntry> = store
        .assignments
        .into_iter()
        .map(|(k, v)| (k.to_uppercase(), v))
        .collect();
    store.assignments = upper;
    store
}

pub fn write_assignment(
    cfg: &Config,
    host: &str,
    user: &str,
    user_display: &str,
    user_dept: &str,
    note: &str,
    by: &str,
) -> Result<(), String> {
    let mut checked_cfg = cfg.clone();
    if checked_cfg.assignments_path.is_none() {
        checked_cfg.assignments_path = Some(default_assignments_path(&checked_cfg.data_dir));
    }
    validate_config(&checked_cfg)?;

    let host_key = host.trim().to_uppercase();
    if !is_valid_host_id(&host_key) {
        return Err("Ungueltiger Hostname".into());
    }
    let known_hosts = read_known_hosts(&checked_cfg);
    if !known_hosts.contains(&host_key) {
        return Err(format!(
            "Geraet '{}' ist nicht in Inventar oder Masterliste vorhanden",
            host_key
        ));
    }

    let path = checked_cfg
        .assignments_path
        .clone()
        .unwrap_or_else(|| default_assignments_path(&checked_cfg.data_dir));
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Control-Ordner konnte nicht erstellt werden: {}", e))?;
    }

    let _lock = acquire_assignment_lock(Path::new(&path))?;
    let mut store = read_assignments(&path);
    let now = now_iso();
    store.version += 1;
    store.updated_at_utc = Some(now.clone());
    store.updated_by = Some(by.to_string());
    store.assignments.insert(
        host_key,
        AssignmentEntry {
            user: user.to_string(),
            user_display: user_display.to_string(),
            dept: user_dept.to_string(),
            confirmed_by: Some(by.to_string()),
            confirmed_at_utc: Some(now),
            note: note.to_string(),
        },
    );
    let txt = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    atomic_write(Path::new(&path), &txt)
}

// ------------------------------------------------------------------ Merge -> DeviceFull
pub fn build_devices(cfg: &Config) -> Vec<DeviceFull> {
    let csv = read_master_csv(&cfg.master_csv_path);
    let inv = read_inventory_dir(&cfg.data_dir);
    let assign = read_assignments(
        cfg.assignments_path
            .as_deref()
            .unwrap_or(&default_assignments_path(&cfg.data_dir)),
    );
    let th = &cfg.thresholds;

    let hosts = known_hosts_from(&csv, &inv);

    hosts
        .into_iter()
        .map(|host| {
            build_one(
                &host,
                csv.get(&host),
                inv.get(&host),
                assign.assignments.get(&host),
                th,
            )
        })
        .collect()
}

fn build_one(
    host: &str,
    csv: Option<&CsvRow>,
    inv: Option<&Inventory>,
    assign: Option<&AssignmentEntry>,
    th: &Thresholds,
) -> DeviceFull {
    let has_inv = inv.is_some();
    let empty = Inventory::default();
    let iv = inv.unwrap_or(&empty);

    // ----- CPU / RAM / Disk -----
    let cpu = iv.cpu.clone().unwrap_or_default();
    let cpu_name = opt_str(&cpu.name, "—");
    let cores = f2i(cpu.cores);
    let threads = f2i(cpu.logical_processors);
    let clock = f2i(cpu.max_clock_mhz);

    let ram = iv.ram.clone().unwrap_or_default();
    let ram_gb = f2i(ram.total_gb);
    let slots_used = f2i(ram.slots_used);
    let slots_total = f2i(ram.slots_total).max(slots_used);
    let ram_sticks: Vec<RamStick> = ram
        .sticks
        .unwrap_or_default()
        .into_iter()
        .map(|s| RamStick {
            capacity_gb: f2i(s.capacity_gb),
            speed_mhz: f2i(s.speed_mhz),
            slot: opt_str(&s.slot, ""),
        })
        .collect();

    let disks = iv.disks.clone().unwrap_or_default();
    let has_ssd = disks.iter().any(|d| eq_ci(&d.media_type, "SSD"));
    let has_hdd = disks.iter().any(|d| eq_ci(&d.media_type, "HDD"));
    let primary = disks
        .iter()
        .max_by_key(|d| d.size_gb.unwrap_or(0.0).round() as i64);
    let primary_is_ssd = primary
        .map(|d| eq_ci(&d.media_type, "SSD"))
        .unwrap_or(false);
    let disk_is_ssd = has_ssd && !has_hdd && primary_is_ssd;
    let disk_type = if has_ssd && has_hdd {
        "Mixed SSD/HDD".to_string()
    } else {
        primary
            .and_then(|d| d.media_type.clone())
            .unwrap_or_else(|| "—".into())
    };
    let disk_gb = primary.map(|d| f2i(d.size_gb)).unwrap_or(0);
    let disk_model = primary.and_then(|d| d.model.clone()).unwrap_or_default();

    // ----- OS / Alter / Last-Seen -----
    let os = iv.os.clone().unwrap_or_default();
    let os_caption = opt_str(&os.caption, "—");
    let os_build = os.version.clone().or(os.build.clone()).unwrap_or_default();
    let os_is_win11 = os_caption.contains("11");
    let age_years = iv.age_years;
    let last_seen_days = iv.collected_at_utc.as_deref().and_then(days_since);

    // ----- Benutzer-Aufloesung -----
    let (user_source, user_display, note, confirmed_by, user_sam) = if let Some(a) = assign {
        let disp = if a.user_display.is_empty() {
            a.user.clone()
        } else {
            a.user_display.clone()
        };
        (
            "manuell bestätigt",
            disp,
            a.note.clone(),
            a.confirmed_by.clone(),
            a.user.clone(),
        )
    } else if let Some(c) = csv.filter(|c| !c.user.is_empty()) {
        (
            "Rollout-Liste",
            c.user.clone(),
            String::new(),
            None,
            String::new(),
        )
    } else if let Some(u) = iv
        .current_user
        .clone()
        .or(iv.last_logged_on_user.clone())
        .filter(|s| !s.is_empty())
    {
        (
            "zuletzt angemeldet",
            strip_domain(&u),
            String::new(),
            None,
            String::new(),
        )
    } else {
        (
            "—",
            "Unbekannt".to_string(),
            String::new(),
            None,
            String::new(),
        )
    };
    let user_source = user_source.to_string();
    let user = user_display.clone();
    let init = initials(&user_display);

    let dept = assign
        .and_then(|a| {
            let dept = a.dept.trim();
            if dept.is_empty() {
                None
            } else {
                Some(dept.to_string())
            }
        })
        .unwrap_or_else(|| dept_from_host(host));

    // ----- Bewertung -----
    let ev = evaluate(
        th,
        &DeviceFacts {
            has_inventory: has_inv,
            ram_gb,
            age_years,
            disk_is_ssd,
            cpu_cores: cores,
            cpu_clock_mhz: clock,
            os_is_win11,
            last_seen_days,
        },
    );

    let age_text = match age_years {
        Some(a) if has_inv => format!("{} J.", fmt_de(a)),
        _ => "—".to_string(),
    };
    let last_seen_text = if !has_inv {
        "nie".to_string()
    } else {
        last_seen_text(last_seen_days)
    };

    DeviceFull {
        host: host.to_string(),
        has_inventory: has_inv,
        status: ev.status,
        status_label: ev.status_label,
        upgrade_reasons: ev.reasons,
        user,
        user_display,
        user_sam,
        user_source,
        dept,
        initials: init,
        avatar_color: avatar_color(host),
        cpu: cpu_name,
        cores,
        cores_text: format!("{} Kerne / {} Threads", cores, threads),
        ram_gb,
        ram_slots_used: slots_used,
        ram_slots_total: slots_total,
        ram_free_slots: (slots_total - slots_used).max(0),
        ram_target_gb: th.target_ram_gb,
        disk_type,
        disk_gb,
        disk_model,
        age_years,
        age_text,
        last_seen_days,
        last_seen_text,
        os_short: os_short(&os_caption, &os_build),
        os_caption,
        os_build,
        chassis: opt_str(&iv.chassis, "—"),
        manufacturer: opt_str(&iv.manufacturer, "—"),
        model: opt_str(&iv.model, ""),
        serial_number: opt_str(&iv.serial_number, "—"),
        bios_version: iv.bios.clone().and_then(|b| b.version).unwrap_or_default(),
        bios_date: iv
            .bios
            .clone()
            .and_then(|b| b.release_date)
            .map(|d| d.split('T').next().unwrap_or("").to_string()),
        gpus: iv.gpus.clone().unwrap_or_default(),
        ip: iv
            .network
            .clone()
            .unwrap_or_default()
            .into_iter()
            .find_map(|n| n.ipv4)
            .unwrap_or_default(),
        mac: iv
            .network
            .clone()
            .unwrap_or_default()
            .into_iter()
            .find_map(|n| n.mac)
            .unwrap_or_default(),
        tpm: iv.win11.clone().and_then(|w| w.tpm_present),
        secure_boot: iv.win11.clone().and_then(|w| w.secure_boot),
        ram_sticks,
        note,
        confirmed_by,
        collected_at_utc: iv.collected_at_utc.clone(),
    }
}

// ------------------------------------------------------------------ Overview
pub fn build_overview(devs: &[DeviceFull], th: &Thresholds) -> Overview {
    let total = devs.len() as i64;
    let with_inv = devs.iter().filter(|d| d.has_inventory).count() as i64;
    let count = |s: &str| devs.iter().filter(|d| d.status == s).count() as i64;
    let (ok, upgrade, stale, missing) = (
        count("ok"),
        count("upgrade"),
        count("stale"),
        count("missing"),
    );
    let aged: Vec<f64> = devs.iter().filter_map(|d| d.age_years).collect();
    let avg = if aged.is_empty() {
        0.0
    } else {
        aged.iter().sum::<f64>() / aged.len() as f64
    };
    let old5 = devs
        .iter()
        .filter(|d| d.age_years.map(|a| a > th.max_age_years).unwrap_or(false))
        .count() as i64;

    let mut dept_map: HashMap<String, (i64, i64)> = HashMap::new();
    for d in devs {
        let e = dept_map.entry(d.dept.clone()).or_insert((0, 0));
        e.0 += 1;
        if d.status == "upgrade" || d.status == "missing" {
            e.1 += 1;
        }
    }
    let mut by_dept: Vec<DeptStat> = dept_map
        .into_iter()
        .map(|(dept, (count, upgrade))| DeptStat {
            dept,
            count,
            upgrade,
        })
        .collect();
    by_dept.sort_by(|a, b| b.count.cmp(&a.count).then(a.dept.cmp(&b.dept)));

    let age_bucket = |lo: f64, hi: f64| aged.iter().filter(|&&a| a >= lo && a < hi).count() as i64;
    let age_buckets = vec![
        Bucket {
            label: "< 2 Jahre".into(),
            count: age_bucket(0.0, 2.0),
        },
        Bucket {
            label: "2–4 Jahre".into(),
            count: age_bucket(2.0, 4.0),
        },
        Bucket {
            label: "4–5 Jahre".into(),
            count: aged.iter().filter(|&&a| (4.0..=5.0).contains(&a)).count() as i64,
        },
        Bucket {
            label: "> 5 Jahre".into(),
            count: aged.iter().filter(|&&a| a > 5.0).count() as i64,
        },
    ];
    let ram_count = |f: &dyn Fn(i64) -> bool| {
        devs.iter()
            .filter(|d| d.has_inventory && f(d.ram_gb))
            .count() as i64
    };
    // Zusammenhaengende Klassen ohne Luecken (12/24 GB etc. fallen sonst durch).
    let ram_buckets = vec![
        Bucket {
            label: "≤ 8 GB".into(),
            count: ram_count(&|g| g <= 8),
        },
        Bucket {
            label: "9–16 GB".into(),
            count: ram_count(&|g| g > 8 && g <= 16),
        },
        Bucket {
            label: "17–32 GB".into(),
            count: ram_count(&|g| g > 16 && g <= 32),
        },
        Bucket {
            label: "> 32 GB".into(),
            count: ram_count(&|g| g > 32),
        },
    ];

    Overview {
        total,
        with_inventory: with_inv,
        stale,
        missing,
        upgrade_needed: upgrade,
        ok,
        current: with_inv - stale,
        avg_age_years: (avg * 10.0).round() / 10.0,
        old5,
        dept_count: by_dept.len() as i64,
        by_dept,
        age_buckets,
        ram_buckets,
        status: StatusCounts {
            ok,
            upgrade,
            stale,
            missing,
        },
    }
}

// ------------------------------------------------------------------ Helpers
fn f2i(x: Option<f64>) -> i64 {
    x.unwrap_or(0.0).round() as i64
}
fn opt_str(o: &Option<String>, dflt: &str) -> String {
    o.clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| dflt.to_string())
}
fn eq_ci(o: &Option<String>, v: &str) -> bool {
    o.as_deref()
        .map(|s| s.eq_ignore_ascii_case(v))
        .unwrap_or(false)
}
fn strip_domain(u: &str) -> String {
    u.rsplit('\\').next().unwrap_or(u).to_string()
}
fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    let mut s = String::new();
    if let Some(p) = parts.first() {
        s.push(p.chars().next().unwrap_or('?'));
    }
    if let Some(p) = parts.get(1) {
        s.push(p.chars().next().unwrap_or(' '));
    }
    if s.is_empty() {
        "?".into()
    } else {
        s.to_uppercase()
    }
}
fn avatar_color(host: &str) -> String {
    let mut n: u32 = 0;
    for b in host.bytes() {
        n = n.wrapping_mul(31).wrapping_add(b as u32);
    }
    PALETTE[(n as usize) % PALETTE.len()].to_string()
}
fn os_short(caption: &str, build: &str) -> String {
    let w = if caption.contains("11") {
        "Win 11"
    } else if caption.contains("10") {
        "Win 10"
    } else {
        caption
    };
    let b = build.rsplit('.').next().unwrap_or(build);
    let label = match b {
        "22631" => "23H2",
        "22621" => "22H2",
        "19045" => "22H2",
        "19044" => "21H2",
        "26100" | "26200" => "24H2",
        _ => "",
    };
    if label.is_empty() {
        w.to_string()
    } else {
        format!("{} {}", w, label)
    }
}
fn last_seen_text(days: Option<i64>) -> String {
    match days {
        None => "—".into(),
        Some(d) if d < 1 => "gerade eben".into(),
        Some(1) => "vor 1 Tag".into(),
        Some(d) => format!("vor {} Tagen", d),
    }
}
/// Leitet die Abteilung aus dem Hostnamen ab (Schema WS-<ABT>-NN).
/// Heuristik fuer Kowobau; per AD-Department spaeter ueberschreibbar.
fn dept_from_host(host: &str) -> String {
    let up = host.to_uppercase();
    let map = [
        ("BUCH", "Buchhaltung"),
        ("VERTRIEB", "Vertrieb"),
        ("MARKETING", "Marketing"),
        ("ENTW", "Entwicklung"),
        ("PERSONAL", "Personal"),
        ("LAGER", "Lager"),
        ("EMPFANG", "Empfang"),
        ("IT", "IT"),
        ("GF", "Geschäftsführung"),
    ];
    for (key, name) in map {
        if up.contains(key) {
            return name.to_string();
        }
    }
    "Allgemein".to_string()
}
fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}
fn days_since(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| (Utc::now() - dt.with_timezone(&Utc)).num_days())
}

// ------------------------------------------------------------------ Atomic write
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp-{}-{}", pid, nanos));
    fs::write(&tmp, content).map_err(|e| format!("Schreiben fehlgeschlagen: {}", e))?;
    if let Err(replace_err) = replace_file(&tmp, path) {
        if let Err(write_err) = fs::write(path, content) {
            let _ = fs::remove_file(&tmp);
            return Err(format!(
                "Umbenennen fehlgeschlagen: {}; direktes Schreiben fehlgeschlagen: {}",
                replace_err, write_err
            ));
        }
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

#[cfg(windows)]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let existing: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
    let new: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            new.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::rename(src, dst)
}

struct AssignmentLock {
    path: PathBuf,
}

impl Drop for AssignmentLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_assignment_lock(path: &Path) -> Result<AssignmentLock, String> {
    let lock_path = path.with_extension("lock");
    let start = Instant::now();

    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = writeln!(
                    file,
                    "pid={} createdAtUtc={}",
                    std::process::id(),
                    now_iso()
                );
                return Ok(AssignmentLock { path: lock_path });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if assignment_lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                if start.elapsed() >= ASSIGNMENT_LOCK_TIMEOUT {
                    return Err(
                        "assignments.json wird gerade von einer anderen Instanz geschrieben".into(),
                    );
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!(
                    "Assignment-Lock konnte nicht erstellt werden: {}",
                    e
                ))
            }
        }
    }
}

fn assignment_lock_is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|age| age >= ASSIGNMENT_LOCK_STALE)
        .unwrap_or(false)
}

// ------------------------------------------------------------------ Tests
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> Config {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/sample-data");
        Config {
            data_dir: format!("{}/Inventory", base),
            master_csv_path: format!("{}/Rollout_Masterliste.csv", base),
            assignments_path: Some(format!("{}/control/assignments.json", base)),
            ad_enabled: false,
            thresholds: Thresholds::default(),
        }
    }

    fn count(devs: &[DeviceFull], status: &str) -> usize {
        devs.iter().filter(|d| d.status == status).count()
    }

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "hardview-{}-{}-{}",
            label,
            std::process::id(),
            stamp
        ))
    }

    fn temp_config(root: &std::path::Path) -> Config {
        let data_dir = root.join("incoming");
        let control_dir = root.join("control");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&control_dir).unwrap();
        Config {
            data_dir: data_dir.to_string_lossy().to_string(),
            master_csv_path: root
                .join("Rollout_Masterliste.csv")
                .to_string_lossy()
                .to_string(),
            assignments_path: Some(
                control_dir
                    .join("assignments.json")
                    .to_string_lossy()
                    .to_string(),
            ),
            ad_enabled: false,
            thresholds: Thresholds::default(),
        }
    }

    #[test]
    fn merge_and_classify_sample_data() {
        let cfg = sample_config();
        let devs = build_devices(&cfg);

        // 18 Hosts (CSV) gesamt, 16 mit Inventar-JSON, 2 ohne (kein Agent)
        assert_eq!(devs.len(), 18, "Gesamtzahl Geräte");
        assert_eq!(
            devs.iter().filter(|d| d.has_inventory).count(),
            16,
            "mit Inventar"
        );
        assert_eq!(count(&devs, "missing"), 2, "kein Agent");
        assert_eq!(count(&devs, "stale"), 2, "veraltet/stale");
        assert_eq!(count(&devs, "upgrade"), 4, "Upgrade-Kandidaten");
        assert_eq!(count(&devs, "ok"), 10, "OK");

        // Zuordnung aus assignments.json hat Vorrang
        let it07 = devs.iter().find(|d| d.host == "WS-IT-07").unwrap();
        assert_eq!(it07.user_source, "manuell bestätigt");
        assert_eq!(it07.user_display, "Daniel Richter");
        assert_eq!(it07.dept, "IT");

        // Upgrade-Begründungen korrekt (alt + HDD + wenig RAM + Win10)
        let empfang = devs.iter().find(|d| d.host == "WS-EMPFANG-01").unwrap();
        assert_eq!(empfang.status, "upgrade");
        assert!(empfang.upgrade_reasons.iter().any(|r| r.contains("HDD")));
        assert!(empfang.upgrade_reasons.iter().any(|r| r.contains("alt")));

        // Host ohne JSON -> missing + "nie"
        let buch08 = devs.iter().find(|d| d.host == "WS-BUCH-08").unwrap();
        assert!(!buch08.has_inventory);
        assert_eq!(buch08.last_seen_text, "nie");
    }

    #[test]
    fn overview_aggregates() {
        let cfg = sample_config();
        let devs = build_devices(&cfg);
        let ov = build_overview(&devs, &cfg.thresholds);
        assert_eq!(ov.total, 18);
        assert_eq!(ov.dept_count, 9);
        assert_eq!(ov.upgrade_needed, 4);
        assert_eq!(ov.current, ov.with_inventory - ov.stale);
        assert!(ov.avg_age_years > 0.0);

        // RAM-Klassen sind zusammenhaengend -> jedes inventarisierte Geraet liegt
        // in genau einem Bucket (Regression gegen die frueheren Luecken 9-15/17-31 GB).
        let ram_sum: i64 = ov.ram_buckets.iter().map(|b| b.count).sum();
        assert_eq!(
            ram_sum, ov.with_inventory,
            "RAM-Buckets decken alle Geräte ab"
        );
    }

    #[test]
    fn inventory_reader_rejects_hostname_spoofing() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "hardview-inv-test-{}-{}",
            std::process::id(),
            stamp
        ));
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("WS-GOOD-01.json"),
            r#"{"hostname":"WS-GOOD-01","collectedAtUtc":"2026-06-24T00:00:00Z"}"#,
        )
        .unwrap();
        fs::write(
            dir.join("WS-EVIL-01.json"),
            r#"{"hostname":"WS-GOOD-01","collectedAtUtc":"2026-06-24T00:00:00Z"}"#,
        )
        .unwrap();
        fs::write(
            dir.join("WS-MISSING-01.json"),
            r#"{"collectedAtUtc":"2026-06-24T00:00:00Z"}"#,
        )
        .unwrap();

        let inv = read_inventory_dir(&dir.to_string_lossy());
        assert_eq!(inv.len(), 1);
        assert!(inv.contains_key("WS-GOOD-01"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn config_path_validation() {
        let mut cfg = sample_config();
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/sample-data");
        assert!(validate_config(&cfg).is_ok());

        // Ungueltige Pfade blockieren (z. B. System32-Ausbruch)
        cfg.assignments_path = Some("C:\\Windows\\System32\\malicious.json".to_string());
        assert!(validate_config(&cfg).is_err());

        // Client-writable Inventory-Inbox ist kein gueltiger Schreibort.
        cfg.assignments_path = Some(format!("{}/Inventory/assignments.json", base));
        assert!(validate_config(&cfg).is_err());

        // Gueltige Pfade erlauben (Control-Pfad oder AppData)
        cfg.assignments_path = Some(format!("{}/control/assignments.json", base));
        assert!(validate_config(&cfg).is_ok());

        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        let valid_path = Path::new(&appdata)
            .join("HardView")
            .join("assignments.json");
        cfg.assignments_path = Some(valid_path.to_string_lossy().to_string());
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn windows_1252_master_csv_is_decoded() {
        let root = unique_temp_dir("csv-1252");
        fs::create_dir_all(&root).unwrap();
        let csv_path = root.join("Rollout_Masterliste.csv");
        fs::write(
            &csv_path,
            b"Computer;Benutzer;Datum;Status\nWS-UMLAUT-01;M\xfcller;24.06.2026;best\xe4tigt\n",
        )
        .unwrap();

        let rows = read_master_csv(&csv_path.to_string_lossy());
        let row = rows.get("WS-UMLAUT-01").unwrap();
        assert_eq!(row.user, format!("M{}ller", '\u{fc}'));
        assert_eq!(
            decode_windows_1252(b"best\xe4tigt"),
            format!("best{}tigt", '\u{e4}')
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn assignments_do_not_create_phantom_devices() {
        let root = unique_temp_dir("phantom-assignment");
        let cfg = temp_config(&root);
        fs::write(
            &cfg.master_csv_path,
            "Computer;Benutzer\nWS-KNOWN-01;CSV User\n",
        )
        .unwrap();
        fs::write(
            cfg.assignments_path.as_deref().unwrap(),
            r#"{
  "version": 1,
  "assignments": {
    "WS-KNOWN-01": {
      "user": "known",
      "userDisplay": "Known User",
      "dept": "IT",
      "note": ""
    },
    "WS-PHANTOM-99": {
      "user": "ghost",
      "userDisplay": "Ghost User",
      "dept": "Ghost",
      "note": ""
    }
  }
}"#,
        )
        .unwrap();

        let devs = build_devices(&cfg);
        assert_eq!(devs.len(), 1);
        assert!(devs.iter().all(|d| d.host != "WS-PHANTOM-99"));
        let known = devs.iter().find(|d| d.host == "WS-KNOWN-01").unwrap();
        assert_eq!(known.user_display, "Known User");
        assert_eq!(known.dept, "IT");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_assignment_validates_known_host_and_persists_dept() {
        let root = unique_temp_dir("write-assignment");
        let cfg = temp_config(&root);
        fs::write(&cfg.master_csv_path, "Computer;Benutzer\nWS-KNOWN-01;\n").unwrap();

        let err = write_assignment(
            &cfg,
            "WS-UNKNOWN-01",
            "KOWOBAU\\ghost",
            "Ghost User",
            "IT",
            "",
            "tester",
        )
        .unwrap_err();
        assert!(err.contains("nicht in Inventar"));

        write_assignment(
            &cfg,
            "ws-known-01",
            "KOWOBAU\\jsmith",
            "Jane Smith",
            "IT",
            "confirmed",
            "tester",
        )
        .unwrap();

        let store = read_assignments(cfg.assignments_path.as_deref().unwrap());
        let entry = store.assignments.get("WS-KNOWN-01").unwrap();
        assert_eq!(entry.user, "KOWOBAU\\jsmith");
        assert_eq!(entry.user_display, "Jane Smith");
        assert_eq!(entry.dept, "IT");
        assert_eq!(entry.note, "confirmed");
        assert_eq!(store.version, 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mixed_ssd_hdd_is_not_treated_as_full_ssd() {
        let root = unique_temp_dir("mixed-disk");
        let cfg = temp_config(&root);
        fs::write(&cfg.master_csv_path, "Computer;Benutzer\nWS-MIX-01;\n").unwrap();
        fs::write(
            Path::new(&cfg.data_dir).join("WS-MIX-01.json"),
            format!(
                r#"{{
  "hostname": "WS-MIX-01",
  "collectedAtUtc": "{}",
  "ageYears": 1.0,
  "cpu": {{"cores": 4, "logicalProcessors": 8, "maxClockMhz": 3000}},
  "ram": {{"totalGB": 16, "slotsUsed": 1, "slotsTotal": 2}},
  "disks": [
    {{"mediaType": "SSD", "sizeGB": 256, "model": "Fast SSD"}},
    {{"mediaType": "HDD", "sizeGB": 512, "model": "Bulk HDD"}}
  ],
  "os": {{"caption": "Microsoft Windows 11 Pro", "version": "10.0.22631"}}
}}"#,
                now_iso()
            ),
        )
        .unwrap();

        let devs = build_devices(&cfg);
        let dev = devs.iter().find(|d| d.host == "WS-MIX-01").unwrap();
        assert_eq!(dev.disk_type, "Mixed SSD/HDD");
        assert_eq!(dev.status, "upgrade");
        assert!(dev.upgrade_reasons.iter().any(|r| r.contains("HDD")));

        let _ = fs::remove_dir_all(root);
    }
}
