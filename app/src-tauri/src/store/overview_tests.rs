use super::common::now_iso;
use super::test_support::{temp_config, unique_temp_dir};
use super::{build_devices, build_overview};
use std::fs;
use std::path::Path;

#[test]
fn age_buckets_scale_with_custom_max_age_threshold() {
    let root = unique_temp_dir("age-buckets-custom-threshold");
    let mut cfg = temp_config(&root);
    cfg.thresholds.max_age_years = 10.0;
    fs::write(
        &cfg.master_csv_path,
        "Computer;Benutzer\nWS-AGE-01;\nWS-AGE-02;\nWS-AGE-03;\nWS-AGE-04;\n",
    )
    .unwrap();
    for (host, age) in [
        ("WS-AGE-01", 2.0),
        ("WS-AGE-02", 6.0),
        ("WS-AGE-03", 9.0),
        ("WS-AGE-04", 12.0),
    ] {
        fs::write(
            Path::new(&cfg.data_dir).join(format!("{}.json", host)),
            format!(
                r#"{{
  "schemaVersion": 1,
  "hostname": "{host}",
  "collectedAtUtc": "{now}",
  "ageYears": {age},
  "cpu": {{"cores": 4, "logicalProcessors": 8, "maxClockMhz": 3000}},
  "ram": {{"totalGB": 16, "slotsUsed": 1, "slotsTotal": 2}},
  "disks": [{{"mediaType": "SSD", "sizeGB": 512}}],
  "os": {{"caption": "Microsoft Windows 11 Pro", "version": "10.0.22631"}}
}}"#,
                host = host,
                age = age,
                now = now_iso()
            ),
        )
        .unwrap();
    }

    let devs = build_devices(&cfg);
    let ov = build_overview(&devs, &cfg.thresholds);

    // Bei max_age_years = 10.0 liegen die Grenzen bei 4,0 / 8,0 / 10,0 Jahren statt
    // der frueher fixen 2/4/5 -> die vier Testgeraete (2/6/9/12 Jahre) landen in vier
    // unterschiedlichen Buckets.
    let counts: Vec<i64> = ov.age_buckets.iter().map(|b| b.count).collect();
    assert_eq!(counts, vec![1, 1, 1, 1]);
    assert_eq!(ov.age_buckets[0].label, "< 4,0 Jahre");
    assert_eq!(ov.age_buckets[3].label, "> 10,0 Jahre");

    // Invariante: der letzte Age-Bucket ("> max_age_years") muss immer alters-konsistent
    // mit old5 sein, da beide aus derselben age_years > th.max_age_years-Bedingung
    // stammen — hier mit nicht-Default-Schwellwert geprueft.
    assert_eq!(ov.age_buckets.last().unwrap().count, ov.old5);
    assert_eq!(ov.old5, 1);
    assert_eq!(ov.old_age_label, "> 10,0 Jahre");

    let _ = fs::remove_dir_all(root);
}
