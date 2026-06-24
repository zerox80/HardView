//! Bewertung eines Geraets: Status (ok/upgrade/stale/missing) + Begruendungen.
//! Quelle der Wahrheit fuer die Logik (mock.js spiegelt dies fuer die Vorschau).
use crate::model::Thresholds;

pub struct Eval {
    pub status: String,
    pub status_label: String,
    pub reasons: Vec<String>,
}

/// Zusammengefuehrte Fakten eines Geraets, die in die Bewertung einfliessen.
/// Buendelt die Eingaben von `evaluate`, damit die Signatur schlank bleibt.
pub struct DeviceFacts {
    pub has_inventory: bool,
    pub ram_gb: i64,
    pub age_years: Option<f64>,
    pub disk_is_ssd: bool,
    pub cpu_cores: i64,
    pub cpu_clock_mhz: i64,
    pub os_is_win11: bool,
    pub last_seen_days: Option<i64>,
}

/// Bewertet ein Geraet anhand der zusammengefuehrten Fakten.
pub fn evaluate(th: &Thresholds, f: &DeviceFacts) -> Eval {
    if !f.has_inventory {
        return Eval {
            status: "missing".into(),
            status_label: "Kein Inventar".into(),
            reasons: vec!["Kein Inventar — Agent hat noch nie gemeldet".into()],
        };
    }

    let mut reasons: Vec<String> = Vec::new();
    if let Some(a) = f.age_years {
        if a > th.max_age_years {
            reasons.push(format!("Gerät alt ({} Jahre)", fmt_de(a)));
        }
    }
    if f.ram_gb > 0 && f.ram_gb <= th.min_ram_gb {
        reasons.push(format!("RAM knapp ({} GB)", f.ram_gb));
    }
    if th.require_ssd && !f.disk_is_ssd {
        reasons.push("HDD statt SSD".into());
    }
    if f.cpu_cores > 0 && f.cpu_cores < th.min_cpu_cores {
        reasons.push(format!("CPU schwach ({} Kerne)", f.cpu_cores));
    }
    if th.min_cpu_clock_mhz > 0 && f.cpu_clock_mhz > 0 && f.cpu_clock_mhz < th.min_cpu_clock_mhz {
        reasons.push(format!("CPU-Takt niedrig ({} MHz)", f.cpu_clock_mhz));
    }
    if !f.os_is_win11 {
        reasons.push("Kein Windows 11 (Win 10 EOL)".into());
    }

    let stale = matches!(f.last_seen_days, Some(d) if d > th.stale_days);
    if stale {
        Eval {
            status: "stale".into(),
            status_label: "Veraltet · Agent meldet nicht".into(),
            reasons,
        }
    } else if !reasons.is_empty() {
        Eval {
            status: "upgrade".into(),
            status_label: "Upgrade empfohlen".into(),
            reasons,
        }
    } else {
        Eval {
            status: "ok".into(),
            status_label: "Aktuell · OK".into(),
            reasons,
        }
    }
}

/// Deutsche Dezimaldarstellung (Punkt -> Komma), 1 Nachkommastelle.
pub fn fmt_de(v: f64) -> String {
    format!("{:.1}", v).replace('.', ",")
}
