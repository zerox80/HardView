use super::atomic::{acquire_assignment_lock, atomic_write};
use super::common::now_iso;
use super::config::{default_assignments_path, validate_config};
use super::inventory::is_valid_host_id;
use super::text::read_text;
use crate::model::{AssignmentEntry, AssignmentStore, Config};
use std::collections::{BTreeSet, HashMap};
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

pub struct AssignmentWrite<'a> {
    pub host: &'a str,
    pub user: &'a str,
    pub user_display: &'a str,
    pub user_dept: &'a str,
    pub note: &'a str,
    pub by: &'a str,
}

pub fn write_assignment_for_known_hosts(
    cfg: &Config,
    known_hosts: &BTreeSet<String>,
    write: AssignmentWrite<'_>,
) -> Result<(), String> {
    let checked_cfg = checked_assignment_config(cfg)?;
    persist_assignment(&checked_cfg, known_hosts, write)
}

fn checked_assignment_config(cfg: &Config) -> Result<Config, String> {
    let mut checked_cfg = cfg.clone();
    if checked_cfg.assignments_path.is_none() {
        checked_cfg.assignments_path = Some(default_assignments_path(&checked_cfg.data_dir));
    }
    validate_config(&checked_cfg)?;
    Ok(checked_cfg)
}

fn persist_assignment(
    checked_cfg: &Config,
    known_hosts: &BTreeSet<String>,
    write: AssignmentWrite<'_>,
) -> Result<(), String> {
    let host_key = write.host.trim().to_uppercase();
    if !is_valid_host_id(&host_key) {
        return Err("Ungueltiger Hostname".into());
    }
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
    store.updated_by = Some(write.by.to_string());
    store.assignments.insert(
        host_key,
        AssignmentEntry {
            user: write.user.to_string(),
            user_display: write.user_display.to_string(),
            dept: write.user_dept.to_string(),
            confirmed_by: Some(write.by.to_string()),
            confirmed_at_utc: Some(now),
            note: write.note.to_string(),
        },
    );
    let txt = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    atomic_write(Path::new(&path), &txt)
}
