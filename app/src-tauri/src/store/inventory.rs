use super::master_csv::{read_master_csv, CsvRow};
use super::text::read_text;
use crate::model::{Config, Inventory};
use std::collections::{BTreeSet, HashMap};
use std::fs;

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
            if inv.schema_version != Some(1) {
                continue;
            }
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

pub(super) fn is_valid_host_id(host: &str) -> bool {
    let len = host.len();
    len > 0
        && len <= 63
        && !host.starts_with('-')
        && !host.ends_with('-')
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub(super) fn read_known_hosts(cfg: &Config) -> BTreeSet<String> {
    let csv = read_master_csv(&cfg.master_csv_path);
    let inv = read_inventory_dir(&cfg.data_dir);
    known_hosts_from(&csv, &inv)
}

pub(super) fn known_hosts_from(
    csv: &HashMap<String, CsvRow>,
    inv: &HashMap<String, Inventory>,
) -> BTreeSet<String> {
    let mut hosts = BTreeSet::new();
    hosts.extend(csv.keys().cloned());
    hosts.extend(inv.keys().cloned());
    hosts
}
