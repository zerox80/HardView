use crate::model::{Config, DeviceFull, Thresholds};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn sample_data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../shared/sample-data")
        .canonicalize()
        .unwrap()
}

pub(super) fn sample_config() -> Config {
    let base = sample_data_dir();
    Config {
        data_dir: base.join("Inventory").to_string_lossy().to_string(),
        master_csv_path: base
            .join("Rollout_Masterliste.csv")
            .to_string_lossy()
            .to_string(),
        assignments_path: Some(
            base.join("control")
                .join("assignments.json")
                .to_string_lossy()
                .to_string(),
        ),
        ad_enabled: false,
        thresholds: Thresholds::default(),
    }
}

pub(super) fn count(devs: &[DeviceFull], status: &str) -> usize {
    devs.iter().filter(|d| d.status == status).count()
}

pub(super) fn unique_temp_dir(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "hardview-{}-{}-{}",
        label,
        std::process::id(),
        stamp
    ))
}

pub(super) fn temp_config(root: &Path) -> Config {
    let data_dir = root.join("incoming");
    let control_dir = root.join("control");
    fs::create_dir_all(&data_dir).unwrap();
    fs::create_dir_all(&control_dir).unwrap();
    Config {
        data_dir: data_dir.to_string_lossy().to_string(),
        master_csv_path: root
            .join("Rollout_Masterliste.csv")
            .to_string_lossy()
            .to_string(),
        assignments_path: Some(
            control_dir
                .join("assignments.json")
                .to_string_lossy()
                .to_string(),
        ),
        ad_enabled: false,
        thresholds: Thresholds::default(),
    }
}
