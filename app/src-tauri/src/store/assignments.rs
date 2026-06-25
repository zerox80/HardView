use super::atomic::{acquire_assignment_lock, atomic_write};
use super::common::now_iso;
use super::config::{default_assignments_path, validate_config};
use super::inventory::{is_valid_host_id, read_known_hosts};
use super::text::read_text;
use crate::model::{AssignmentEntry, AssignmentStore, Config};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
