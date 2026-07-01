use crate::model::{Bucket, DeptStat, DeviceFull, Overview, StatusCounts, Thresholds};
use crate::upgrade::fmt_de;
use std::collections::HashMap;

// RAM-Bucket-Klassen in festen, aneinandergrenzenden Intervallen (keine Luecken
// fuer 12/24 GB etc.). Grenzen sind bewusst ortsfest, damit die Histogramme ueber
// Konfigurationswechsel hinweg vergleichbar bleiben (und Tests nicht bei jeder
// Anpassung brechen).
const RAM_LABELS: [&str; 4] = ["≤ 8 GB", "9–16 GB", "17–32 GB", "> 32 GB"];
fn ram_predicate(i: usize, g: i64) -> bool {
    match i {
        0 => g <= 8,
        1 => g > 8 && g <= 16,
        2 => g > 16 && g <= 32,
        _ => g > 32,
    }
}

// ------------------------------------------------------------------ Overview
pub fn build_overview(devs: &[DeviceFull], th: &Thresholds) -> Overview {
    let total = devs.len() as i64;
    let needs_upgrade = |d: &DeviceFull| {
        d.status == "upgrade" || (d.status == "stale" && !d.upgrade_reasons.is_empty())
    };
    let needs_action = |d: &DeviceFull| needs_upgrade(d) || d.status == "missing";

    // Ein einziger Durchlauf ueber `devs` liefert Status-Tallies, die Abteilungs-
    // Aggregation, die Upgrade-Kandidaten-Summe und die vier RAM-Bucket-Zaehler
    // (statt zuvor neun separater Scans).
    let mut with_inv = 0i64;
    let mut ok = 0i64;
    let mut status_upgrade = 0i64;
    let mut stale = 0i64;
    let mut missing = 0i64;
    let mut needs_upgrade_total = 0i64;
    let mut dept_map: HashMap<String, (i64, i64)> = HashMap::new();
    let mut ram_buckets = [
        Bucket {
            label: RAM_LABELS[0].into(),
            count: 0,
        },
        Bucket {
            label: RAM_LABELS[1].into(),
            count: 0,
        },
        Bucket {
            label: RAM_LABELS[2].into(),
            count: 0,
        },
        Bucket {
            label: RAM_LABELS[3].into(),
            count: 0,
        },
    ];

    for d in devs {
        match d.status.as_str() {
            "ok" => ok += 1,
            "upgrade" => status_upgrade += 1,
            "stale" => stale += 1,
            "missing" => missing += 1,
            _ => {}
        }
        let d_has_inv = d.has_inventory;
        if d_has_inv {
            with_inv += 1;
        }
        if d_has_inv && !d.ram_gb.is_negative() {
            for (i, bucket) in ram_buckets.iter_mut().enumerate() {
                if ram_predicate(i, d.ram_gb) {
                    bucket.count += 1;
                    break;
                }
            }
        }
        if needs_upgrade(d) {
            needs_upgrade_total += 1;
        }
        let e = dept_map.entry(d.dept.clone()).or_insert((0, 0));
        e.0 += 1;
        if needs_action(d) {
            e.1 += 1;
        }
    }
    let upgrade = needs_upgrade_total;

    // `aged` sammeln wir in einem zweiten minimalen Pass (alter kann null sein und
    // benoetigt zudem sum/old5-Berechnungen; in einem universellen Single-Pass
    // wuerde das die Lesbarkeitherabsetzen ohne messbaren Vorteil bei typischen
    // Inventargroessen).
    let aged: Vec<f64> = devs.iter().filter_map(|d| d.age_years).collect();
    let avg = if aged.is_empty() {
        0.0
    } else {
        aged.iter().sum::<f64>() / aged.len() as f64
    };
    let old5 = devs
        .iter()
        .filter(|d| d.age_years.map(|a| a > th.max_age_years).unwrap_or(false))
        .count() as i64;

    let mut by_dept: Vec<DeptStat> = dept_map
        .into_iter()
        .map(|(dept, (count, needs_action))| DeptStat {
            dept,
            count,
            needs_action,
        })
        .collect();
    by_dept.sort_by(|a, b| b.count.cmp(&a.count).then(a.dept.cmp(&b.dept)));

    // Bucket-Grenzen proportional zu max_age_years ableiten (statt fix 2/4/5), damit
    // das Histogramm bei individuellen Schwellwerten zu old5/old_age_label passt.
    // Beim Default (5,0 Jahre) reproduzieren die Faktoren exakt die fruehere feste
    // Aufteilung 2,0/4,0/5,0 Jahre.
    let b1 = th.max_age_years * (2.0 / 5.0);
    let b2 = th.max_age_years * (4.0 / 5.0);
    let b3 = th.max_age_years;
    let age_bucket = |lo: f64, hi: f64| aged.iter().filter(|&&a| a >= lo && a < hi).count() as i64;
    let age_buckets = vec![
        Bucket {
            label: format!("< {} Jahre", fmt_de(b1)),
            count: age_bucket(0.0, b1),
        },
        Bucket {
            label: format!("{}–{} Jahre", fmt_de(b1), fmt_de(b2)),
            count: age_bucket(b1, b2),
        },
        Bucket {
            label: format!("{}–{} Jahre", fmt_de(b2), fmt_de(b3)),
            count: aged.iter().filter(|&&a| a >= b2 && a <= b3).count() as i64,
        },
        Bucket {
            label: format!("> {} Jahre", fmt_de(b3)),
            count: aged.iter().filter(|&&a| a > b3).count() as i64,
        },
    ];

    // `current` = "inventarisierte Geraete mit aktueller Meldung" = withInv - stale.
    // Upgrade-Kandidaten fallen hinein, da sie laut Definition eine gueltige
    // (nicht-stale) Inventarmeldung haben; das entspricht der KPI-Semantik im
    // Frontend "AKTUELL INVENTARISIERT".
    Overview {
        total,
        with_inventory: with_inv,
        stale,
        missing,
        upgrade_needed: upgrade,
        ok,
        current: with_inv - stale,
        avg_age_years: (avg * 10.0).round() / 10.0,
        old5,
        old_age_label: format!("> {} Jahre", fmt_de(th.max_age_years)),
        dept_count: by_dept.len() as i64,
        by_dept,
        age_buckets,
        ram_buckets: ram_buckets.to_vec(),
        status: StatusCounts {
            ok,
            upgrade: status_upgrade,
            stale,
            missing,
        },
    }
}
