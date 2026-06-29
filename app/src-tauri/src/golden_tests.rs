//! Prueft die Bewertungslogik (upgrade::evaluate) gegen die geteilten Golden-Vectors
//! in shared/test-vectors. Derselbe Datensatz wird vom Frontend-Mock geprueft
//! (app/tests/upgrade-parity.test.js) -> Rust und mock.js koennen nicht auseinanderdriften.
use crate::model::Thresholds;
use crate::upgrade::{evaluate, DeviceFacts};

#[derive(serde::Deserialize)]
struct Vectors {
    thresholds: Thresholds,
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    facts: Facts,
    status: String,
    reasons: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Facts {
    has_inventory: bool,
    #[serde(rename = "ramGB")]
    ram_gb: i64,
    age_years: Option<f64>,
    disk_is_ssd: Option<bool>,
    cpu_cores: i64,
    cpu_clock_mhz: i64,
    os_is_win11: Option<bool>,
    last_seen_days: Option<i64>,
}

#[test]
fn evaluate_matches_shared_golden_vectors() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../shared/test-vectors/upgrade-cases.json");
    let txt = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Golden-Vectors nicht lesbar ({}): {}", path.display(), e));
    let vectors: Vectors = serde_json::from_str(&txt).expect("Golden-Vectors nicht parsebar");

    assert!(
        !vectors.cases.is_empty(),
        "Golden-Vectors enthalten keine Faelle"
    );
    for c in &vectors.cases {
        let ev = evaluate(
            &vectors.thresholds,
            &DeviceFacts {
                has_inventory: c.facts.has_inventory,
                ram_gb: c.facts.ram_gb,
                age_years: c.facts.age_years,
                disk_is_ssd: c.facts.disk_is_ssd,
                cpu_cores: c.facts.cpu_cores,
                cpu_clock_mhz: c.facts.cpu_clock_mhz,
                os_is_win11: c.facts.os_is_win11,
                last_seen_days: c.facts.last_seen_days,
            },
        );
        assert_eq!(ev.status, c.status, "Status fuer Fall '{}'", c.name);
        assert_eq!(
            ev.reasons, c.reasons,
            "Begruendungen fuer Fall '{}'",
            c.name
        );
    }
}
