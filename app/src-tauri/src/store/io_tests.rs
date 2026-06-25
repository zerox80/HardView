use super::assignments::{read_assignments, write_assignment};
use super::atomic::atomic_write;
use super::build_devices;
use super::common::now_iso;
use super::config::validate_config;
use super::master_csv::read_master_csv;
use super::test_support::{sample_config, temp_config, unique_temp_dir};
use super::text::decode_windows_1252;
use std::fs;
use std::path::Path;

#[test]
fn atomic_write_fails_closed_when_replace_fails() {
    let root = unique_temp_dir("atomic-replace");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target.json");
    fs::create_dir_all(&target).unwrap();

    let err = atomic_write(&target, "{}").unwrap_err();
    assert!(err.contains("Atomarer Replace"));
    assert!(
        target.is_dir(),
        "existing target directory must remain intact"
    );
    let leftovers: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("target.tmp-")
        })
        .collect();
    assert!(leftovers.is_empty(), "temporary file should be removed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn config_threshold_validation() {
    let mut cfg = sample_config();
    assert!(validate_config(&cfg).is_ok());

    cfg.thresholds.target_ram_gb = 0;
    assert!(validate_config(&cfg).is_err());

    cfg = sample_config();
    cfg.thresholds.stale_days = 0;
    assert!(validate_config(&cfg).is_err());

    cfg = sample_config();
    cfg.thresholds.max_age_years = f64::NAN;
    assert!(validate_config(&cfg).is_err());

    cfg = sample_config();
    cfg.thresholds.min_cpu_clock_mhz = -1;
    assert!(validate_config(&cfg).is_err());
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
