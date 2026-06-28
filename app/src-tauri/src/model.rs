//! Datenmodelle: (1) Roh-Inventar aus den Agent-JSONs, (2) Config/Zuordnungen,
//! (3) die an das Frontend gelieferten DTOs (DeviceFull, Overview, AdUser).
use serde::{Deserialize, Serialize};

// =================== Roh-Inventar (Agent-JSON) ===================
#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Inventory {
    pub schema_version: Option<i64>,
    pub hostname: Option<String>,
    pub collected_at_utc: Option<String>,
    pub current_user: Option<String>,
    pub last_logged_on_user: Option<String>,
    pub chassis: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub bios: Option<Bios>,
    pub age_years: Option<f64>,
    pub cpu: Option<CpuInv>,
    pub ram: Option<RamInv>,
    pub disks: Option<Vec<DiskInv>>,
    pub gpus: Option<Vec<String>>,
    pub os: Option<OsInv>,
    pub win11: Option<Win11Inv>,
    pub network: Option<Vec<NetInv>>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Bios {
    pub version: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct CpuInv {
    pub name: Option<String>,
    pub cores: Option<f64>,
    pub logical_processors: Option<f64>,
    pub max_clock_mhz: Option<f64>,
    pub sockets: Option<f64>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct RamInv {
    #[serde(rename = "totalGB")]
    pub total_gb: Option<f64>,
    pub slots_used: Option<f64>,
    pub slots_total: Option<f64>,
    pub sticks: Option<Vec<StickInv>>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct StickInv {
    #[serde(rename = "capacityGB")]
    pub capacity_gb: Option<f64>,
    pub speed_mhz: Option<f64>,
    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
    pub slot: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct DiskInv {
    pub model: Option<String>,
    #[serde(rename = "sizeGB")]
    pub size_gb: Option<f64>,
    pub media_type: Option<String>,
    pub bus_type: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct OsInv {
    pub caption: Option<String>,
    pub version: Option<String>,
    pub build: Option<String>,
    pub install_date_utc: Option<String>,
    pub last_boot_utc: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Win11Inv {
    pub tpm_present: Option<bool>,
    pub tpm_version: Option<String>,
    pub secure_boot: Option<bool>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct NetInv {
    pub mac: Option<String>,
    pub ipv4: Option<String>,
}

// =================== Config & Zuordnungen ===================
#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Thresholds {
    #[serde(rename = "minRamGB")]
    pub min_ram_gb: i64,
    pub max_age_years: f64,
    pub stale_days: i64,
    pub require_ssd: bool,
    pub min_cpu_cores: i64,
    #[serde(default)]
    pub min_cpu_clock_mhz: i64,
    #[serde(rename = "targetRamGB")]
    pub target_ram_gb: i64,
}
impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            min_ram_gb: 8,
            max_age_years: 5.0,
            stale_days: 30,
            require_ssd: true,
            min_cpu_cores: 4,
            min_cpu_clock_mhz: 0,
            target_ram_gb: 16,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub data_dir: String,
    pub master_csv_path: String,
    #[serde(default)]
    pub assignments_path: Option<String>,
    #[serde(default)]
    pub ad_enabled: bool,
    #[serde(default)]
    pub thresholds: Thresholds,
}

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentEntry {
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub user_display: String,
    #[serde(default)]
    pub dept: String,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at_utc: Option<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentStore {
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub updated_at_utc: Option<String>,
    #[serde(default)]
    pub updated_by: Option<String>,
    #[serde(default)]
    pub assignments: std::collections::HashMap<String, AssignmentEntry>,
}

// =================== Frontend-DTOs ===================
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RamStick {
    #[serde(rename = "capacityGB")]
    pub capacity_gb: i64,
    pub speed_mhz: i64,
    pub slot: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFull {
    pub host: String,
    pub has_inventory: bool,
    pub status: String,
    pub status_label: String,
    pub upgrade_reasons: Vec<String>,
    pub user: String,
    pub user_display: String,
    pub user_sam: String,
    pub user_source: String,
    pub dept: String,
    pub initials: String,
    pub avatar_color: String,
    pub cpu: String,
    pub cores: i64,
    pub cores_text: String,
    #[serde(rename = "ramGB")]
    pub ram_gb: i64,
    pub ram_slots_used: i64,
    pub ram_slots_total: i64,
    pub ram_free_slots: i64,
    #[serde(rename = "ramTargetGB")]
    pub ram_target_gb: i64,
    pub disk_type: String,
    #[serde(rename = "diskGB")]
    pub disk_gb: i64,
    pub disk_model: String,
    pub age_years: Option<f64>,
    pub age_text: String,
    pub last_seen_days: Option<i64>,
    pub last_seen_text: String,
    pub os_short: String,
    pub os_caption: String,
    pub os_build: String,
    pub chassis: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub bios_version: String,
    pub bios_date: Option<String>,
    pub gpus: Vec<String>,
    pub ip: String,
    pub mac: String,
    pub tpm: Option<bool>,
    pub secure_boot: Option<bool>,
    pub ram_sticks: Vec<RamStick>,
    pub note: String,
    pub confirmed_by: Option<String>,
    pub collected_at_utc: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub label: String,
    pub count: i64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeptStat {
    pub dept: String,
    pub count: i64,
    pub upgrade: i64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatusCounts {
    pub ok: i64,
    pub upgrade: i64,
    pub stale: i64,
    pub missing: i64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub total: i64,
    pub with_inventory: i64,
    pub stale: i64,
    pub missing: i64,
    pub upgrade_needed: i64,
    pub ok: i64,
    pub current: i64,
    pub avg_age_years: f64,
    pub old5: i64,
    pub dept_count: i64,
    pub by_dept: Vec<DeptStat>,
    pub age_buckets: Vec<Bucket>,
    pub ram_buckets: Vec<Bucket>,
    pub status: StatusCounts,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdUser {
    pub sam: String,
    pub display: String,
    #[serde(default)]
    pub dept: String,
    #[serde(default)]
    pub mail: String,
}
