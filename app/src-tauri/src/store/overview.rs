use crate::model::{Bucket, DeptStat, DeviceFull, Overview, StatusCounts, Thresholds};
use crate::upgrade::fmt_de;
use std::collections::HashMap;

// ------------------------------------------------------------------ Overview
pub fn build_overview(devs: &[DeviceFull], th: &Thresholds) -> Overview {
    let total = devs.len() as i64;
    let with_inv = devs.iter().filter(|d| d.has_inventory).count() as i64;
    let count = |s: &str| devs.iter().filter(|d| d.status == s).count() as i64;
    let needs_upgrade = |d: &DeviceFull| {
        d.status == "upgrade" || (d.status == "stale" && !d.upgrade_reasons.is_empty())
    };
    let needs_action = |d: &DeviceFull| needs_upgrade(d) || d.status == "missing";
    let (ok, status_upgrade, stale, missing) = (
        count("ok"),
        count("upgrade"),
        count("stale"),
        count("missing"),
    );
    let upgrade = devs.iter().filter(|d| needs_upgrade(d)).count() as i64;
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

    let mut dept_map: HashMap<String, (i64, i64)> = HashMap::new();
    for d in devs {
        let e = dept_map.entry(d.dept.clone()).or_insert((0, 0));
        e.0 += 1;
        if needs_action(d) {
            e.1 += 1;
        }
    }
    let mut by_dept: Vec<DeptStat> = dept_map
        .into_iter()
        .map(|(dept, (count, upgrade))| DeptStat {
            dept,
            count,
            upgrade,
        })
        .collect();
    by_dept.sort_by(|a, b| b.count.cmp(&a.count).then(a.dept.cmp(&b.dept)));

    let age_bucket = |lo: f64, hi: f64| aged.iter().filter(|&&a| a >= lo && a < hi).count() as i64;
    let age_buckets = vec![
        Bucket {
            label: "< 2 Jahre".into(),
            count: age_bucket(0.0, 2.0),
        },
        Bucket {
            label: "2–4 Jahre".into(),
            count: age_bucket(2.0, 4.0),
        },
        Bucket {
            label: "4–5 Jahre".into(),
            count: aged.iter().filter(|&&a| (4.0..=5.0).contains(&a)).count() as i64,
        },
        Bucket {
            label: "> 5 Jahre".into(),
            count: aged.iter().filter(|&&a| a > 5.0).count() as i64,
        },
    ];
    let ram_count = |f: &dyn Fn(i64) -> bool| {
        devs.iter()
            .filter(|d| d.has_inventory && f(d.ram_gb))
            .count() as i64
    };
    // Zusammenhaengende Klassen ohne Luecken (12/24 GB etc. fallen sonst durch).
    let ram_buckets = vec![
        Bucket {
            label: "≤ 8 GB".into(),
            count: ram_count(&|g| g <= 8),
        },
        Bucket {
            label: "9–16 GB".into(),
            count: ram_count(&|g| g > 8 && g <= 16),
        },
        Bucket {
            label: "17–32 GB".into(),
            count: ram_count(&|g| g > 16 && g <= 32),
        },
        Bucket {
            label: "> 32 GB".into(),
            count: ram_count(&|g| g > 32),
        },
    ];

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
        ram_buckets,
        status: StatusCounts {
            ok,
            upgrade: status_upgrade,
            stale,
            missing,
        },
    }
}
