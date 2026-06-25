use super::config::{app_config_dir, validate_config};
use super::inventory::read_inventory_dir;
use super::test_support::{count, sample_config, sample_data_dir};
use super::{build_devices, build_overview};
use std::fs;

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
            .upgrade,
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
