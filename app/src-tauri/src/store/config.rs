use super::atomic::atomic_write;
use crate::model::{Config, Thresholds};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path, PathBuf};

// ------------------------------------------------------------------ Config
pub fn config_path() -> PathBuf {
    app_config_dir().join("config.json")
}

pub(super) fn app_config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("HardView")
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
    let data_path = hardened_config_path(Path::new(&cfg.data_dir), "dataDir")?;
    let _master_csv_path = hardened_config_path(Path::new(&cfg.master_csv_path), "masterCsvPath")?;
    validate_thresholds(&cfg.thresholds)?;
    if let Some(p) = &cfg.assignments_path {
        if bad(p) {
            return Err("Ungültiger assignmentsPath".into());
        }
        if !p.to_lowercase().ends_with(".json") {
            return Err("assignmentsPath muss auf .json enden".into());
        }
        let assignment_path = hardened_config_path(Path::new(p), "assignmentsPath")?;
        let app_dir = hardened_config_path(&app_config_dir(), "AppData")?;
        let control_dir = hardened_config_path(&control_dir_for(&cfg.data_dir), "Control-Ordner")?;

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

fn hardened_config_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{} muss ein absoluter Pfad sein", label));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("{} darf keine '..'-Segmente enthalten", label));
    }
    if path.components().any(|component| match component {
        Component::Normal(value) => suspicious_path_component(value),
        _ => false,
    }) {
        return Err(format!("{} enthaelt ein unsicheres Pfadsegment", label));
    }
    reject_reparse_components(path, label)?;
    Ok(canonicalize_existing_prefix(path).unwrap_or_else(|| normalize_path(path)))
}

fn suspicious_path_component(component: &OsStr) -> bool {
    let value = component.to_string_lossy();
    value.is_empty()
        || value == "."
        || value == ".."
        || value.ends_with([' ', '.'])
        || value
            .chars()
            .any(|ch| matches!(ch, ':' | '"' | '<' | '>' | '|' | '?' | '*'))
}

fn reject_reparse_components(path: &Path, label: &str) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if !matches!(component, Component::Normal(_)) {
            continue;
        }
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if is_reparse_or_symlink(&metadata) {
                return Err(format!(
                    "{} darf keine Symlinks oder Reparse Points enthalten: {}",
                    label,
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn canonicalize_existing_prefix(path: &Path) -> Option<PathBuf> {
    let mut missing: Vec<OsString> = Vec::new();
    let mut cursor = path;
    loop {
        if cursor.exists() {
            let mut out = fs::canonicalize(cursor).ok()?;
            for component in missing.iter().rev() {
                out.push(component);
            }
            return Some(out);
        }
        missing.push(cursor.file_name()?.to_os_string());
        cursor = cursor.parent()?;
    }
}

#[cfg(windows)]
fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn validate_thresholds(th: &Thresholds) -> Result<(), String> {
    if th.min_ram_gb < 0 {
        return Err("minRamGB darf nicht negativ sein".into());
    }
    if !th.max_age_years.is_finite() || th.max_age_years <= 0.0 {
        return Err("maxAgeYears muss groesser als 0 sein".into());
    }
    if th.stale_days <= 0 {
        return Err("staleDays muss groesser als 0 sein".into());
    }
    if th.min_cpu_cores < 0 {
        return Err("minCpuCores darf nicht negativ sein".into());
    }
    if th.min_cpu_clock_mhz < 0 {
        return Err("minCpuClockMhz darf nicht negativ sein".into());
    }
    if th.target_ram_gb <= 0 {
        return Err("targetRamGB muss groesser als 0 sein".into());
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

pub(super) fn default_assignments_path(data_dir: &str) -> String {
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
