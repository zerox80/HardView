use super::common::{dept_from_host, os_short};
use super::config::{app_config_dir, validate_config};
use super::inventory::read_inventory_dir;
use super::test_support::{count, sample_config, sample_data_dir};
use super::{build_devices, build_overview};
use crate::model::Inventory;
use std::fs;

/// Sichert den Agent-Ausgabekontrakt: jede Sample-JSON erfuellt die Pflichtfelder
/// aus shared/schema/inventory.schema.json, der Hostname passt zum Dateinamen und
/// die Datei deserialisiert in das Backend-Modell (kein stiller Drift Agent <-> App).
#[test]
fn sample_inventory_files_conform_to_schema() {
    let dir = sample_data_dir().join("Inventory");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let txt = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&txt)
            .unwrap_or_else(|e| panic!("{}: kein gueltiges JSON: {}", stem, e));

        // Pflichtfelder laut Schema (required: schemaVersion/hostname/collectedAtUtc).
        assert_eq!(
            json["schemaVersion"],
            serde_json::json!(1),
            "{}: schemaVersion",
            stem
        );
        assert!(
            json["collectedAtUtc"].is_string(),
            "{}: collectedAtUtc fehlt",
            stem
        );
        let host = json["hostname"]
            .as_str()
            .unwrap_or_else(|| panic!("{}: hostname fehlt", stem));
        assert!(
            host.eq_ignore_ascii_case(&stem),
            "{}: hostname != Dateiname",
            stem
        );

        // Muss in das Backend-Modell passen und die mediaType-Enum-Domaene einhalten.
        let _inv: Inventory = serde_json::from_str(&txt)
            .unwrap_or_else(|e| panic!("{}: passt nicht zu Inventory: {}", stem, e));
        if let Some(disks) = json["disks"].as_array() {
            for d in disks {
                if let Some(mt) = d["mediaType"].as_str() {
                    assert!(
                        matches!(mt, "SSD" | "HDD" | "SCM" | "Unbekannt"),
                        "{}: ungueltiger mediaType {}",
                        stem,
                        mt
                    );
                }
            }
        }
        checked += 1;
    }
    assert!(
        checked >= 16,
        "mind. 16 Sample-JSONs erwartet, geprueft: {}",
        checked
    );
}

#[test]
fn dept_from_host_matches_exact_tokens() {
    assert_eq!(dept_from_host("WS-IT-07"), "IT");
    assert_eq!(dept_from_host("WS-MARKETING-04"), "Marketing");
    assert_eq!(dept_from_host("WS-GF-01"), "Geschäftsführung");
    // Regression: "SECURITY" enthaelt "IT", darf aber nicht als IT gelten.
    assert_eq!(dept_from_host("WS-SECURITY-01"), "Allgemein");
    // Kein bekanntes Segment -> Allgemein.
    assert_eq!(dept_from_host("LAPTOP-123"), "Allgemein");
}

#[test]
fn os_short_infers_windows_name_from_known_build_without_caption() {
    assert_eq!(os_short("", "10.0.22631"), "Win 11 23H2");
    assert_eq!(os_short("", "19045"), "Win 10 22H2");
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

    let lager = devs.iter().find(|d| d.host == "WS-LAGER-01").unwrap();
    assert_eq!(lager.status, "stale");
    assert!(lager.upgrade_reasons.iter().any(|r| r.contains("HDD")));
    assert!(lager.upgrade_reasons.iter().any(|r| r.contains("Win 10")));

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
    assert_eq!(ov.status.upgrade, 4);
    assert_eq!(ov.upgrade_needed, 5);
    assert_eq!(
        ov.by_dept
            .iter()
            .find(|d| d.dept == "Lager")
            .unwrap()
            .needs_action,
        2
    );
    assert_eq!(ov.current, ov.with_inventory - ov.stale);
    assert!(ov.avg_age_years > 0.0);

    // RAM-Klassen sind zusammenhaengend -> jedes inventarisierte Geraet liegt
    // in genau einem Bucket (Regression gegen die frueheren Luecken 9-15/17-31 GB).
    let ram_sum: i64 = ov.ram_buckets.iter().map(|b| b.count).sum();
    assert_eq!(
        ram_sum, ov.with_inventory,
        "RAM-Buckets decken alle Geräte ab"
    );

    // Invariante: der letzte Age-Bucket ("> max_age_years") muss immer alters-konsistent
    // mit old5 sein, da beide aus derselben age_years > th.max_age_years-Bedingung
    // stammen.
    assert_eq!(ov.age_buckets.last().unwrap().count, ov.old5);
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
        r#"{"schemaVersion":1,"hostname":"WS-GOOD-01","collectedAtUtc":"2026-06-24T00:00:00Z"}"#,
    )
    .unwrap();
    fs::write(
        dir.join("WS-EVIL-01.json"),
        r#"{"schemaVersion":1,"hostname":"WS-GOOD-01","collectedAtUtc":"2026-06-24T00:00:00Z"}"#,
    )
    .unwrap();
    fs::write(
        dir.join("WS-MISSING-01.json"),
        r#"{"collectedAtUtc":"2026-06-24T00:00:00Z"}"#,
    )
    .unwrap();
    fs::write(
        dir.join("WS-NOVERSION-01.json"),
        r#"{"hostname":"WS-NOVERSION-01","collectedAtUtc":"2026-06-24T00:00:00Z"}"#,
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
    let base = sample_data_dir();
    assert!(validate_config(&cfg).is_ok());

    // Ungueltige Pfade blockieren (z. B. System32-Ausbruch)
    cfg.assignments_path = Some("C:\\Windows\\System32\\malicious.json".to_string());
    assert!(validate_config(&cfg).is_err());

    // Client-writable Inventory-Inbox ist kein gueltiger Schreibort.
    cfg.assignments_path = Some(
        base.join("Inventory")
            .join("assignments.json")
            .to_string_lossy()
            .to_string(),
    );
    assert!(validate_config(&cfg).is_err());

    // Syntaktische Ausbrueche, relative Pfade und ADS-aehnliche Namen blockieren.
    let control_path = base.join("control").to_string_lossy().to_string();
    cfg.assignments_path = Some(format!(
        "{}{}..{}control{}assignments.json",
        control_path,
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    ));
    assert!(validate_config(&cfg).is_err());

    cfg.assignments_path = Some("control/assignments.json".to_string());
    assert!(validate_config(&cfg).is_err());

    cfg.assignments_path = Some(
        base.join("control")
            .join("evil:assignments.json")
            .to_string_lossy()
            .to_string(),
    );
    assert!(validate_config(&cfg).is_err());

    // Gueltige Pfade erlauben (Control-Pfad oder AppData)
    cfg.assignments_path = Some(
        base.join("control")
            .join("assignments.json")
            .to_string_lossy()
            .to_string(),
    );
    assert!(validate_config(&cfg).is_ok());

    let valid_path = app_config_dir().join("assignments.json");
    cfg.assignments_path = Some(valid_path.to_string_lossy().to_string());
    assert!(validate_config(&cfg).is_ok());
}
