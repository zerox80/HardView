use super::assignments::read_assignments;
use super::common::{
    avatar_color, days_since, dept_from_host, eq_ci, f2i, initials, last_seen_text, opt_str,
    os_short, strip_domain,
};
use super::config::default_assignments_path;
use super::facts::{classify_ssd_state, classify_windows_11, is_solid_state_media};
use super::inventory::{known_hosts_from, read_inventory_dir};
use super::master_csv::{read_master_csv, CsvRow};
use crate::model::*;
use crate::upgrade::{evaluate, fmt_de, DeviceFacts};

// ------------------------------------------------------------------ Merge -> DeviceFull
pub fn build_devices(cfg: &Config) -> Vec<DeviceFull> {
    let csv = read_master_csv(&cfg.master_csv_path);
    let inv = read_inventory_dir(&cfg.data_dir);
    let assign = read_assignments(
        cfg.assignments_path
            .as_deref()
            .unwrap_or(&default_assignments_path(&cfg.data_dir)),
    );
    let th = &cfg.thresholds;

    let hosts = known_hosts_from(&csv, &inv);

    hosts
        .into_iter()
        .map(|host| {
            build_one(
                &host,
                csv.get(&host),
                inv.get(&host),
                assign.assignments.get(&host),
                th,
            )
        })
        .collect()
}

fn build_one(
    host: &str,
    csv: Option<&CsvRow>,
    inv: Option<&Inventory>,
    assign: Option<&AssignmentEntry>,
    th: &Thresholds,
) -> DeviceFull {
    let has_inv = inv.is_some();
    let empty = Inventory::default();
    let iv = inv.unwrap_or(&empty);

    // ----- CPU / RAM / Disk -----
    let cpu = iv.cpu.clone().unwrap_or_default();
    let cpu_name = opt_str(&cpu.name, "—");
    let cores = f2i(cpu.cores);
    let threads = f2i(cpu.logical_processors);
    let clock = f2i(cpu.max_clock_mhz);

    let ram = iv.ram.clone().unwrap_or_default();
    let ram_gb = f2i(ram.total_gb);
    let slots_used = f2i(ram.slots_used);
    let slots_total = f2i(ram.slots_total).max(slots_used);
    let ram_sticks: Vec<RamStick> = ram
        .sticks
        .unwrap_or_default()
        .into_iter()
        .map(|s| RamStick {
            capacity_gb: f2i(s.capacity_gb),
            speed_mhz: f2i(s.speed_mhz),
            slot: opt_str(&s.slot, ""),
        })
        .collect();

    let disks = iv.disks.clone().unwrap_or_default();
    let has_ssd = disks.iter().any(|d| is_solid_state_media(&d.media_type));
    let has_hdd = disks.iter().any(|d| eq_ci(&d.media_type, "HDD"));
    let primary = disks
        .iter()
        .max_by_key(|d| d.size_gb.unwrap_or(0.0).round() as i64);
    let disk_is_ssd = classify_ssd_state(primary, has_hdd);
    let disk_type = if has_ssd && has_hdd {
        "Mixed SSD/HDD".to_string()
    } else {
        primary
            .and_then(|d| d.media_type.clone())
            .unwrap_or_else(|| "—".into())
    };
    let disk_gb = primary.map(|d| f2i(d.size_gb)).unwrap_or(0);
    let disk_model = primary.and_then(|d| d.model.clone()).unwrap_or_default();

    // ----- OS / Alter / Last-Seen -----
    let os = iv.os.clone().unwrap_or_default();
    let os_caption = opt_str(&os.caption, "—");
    let os_build = os
        .version
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| os.build.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .unwrap_or_default()
        .to_string();
    let os_is_win11 = classify_windows_11(&os_caption, &os_build);
    let age_years = iv.age_years;
    let last_seen_days = iv.collected_at_utc.as_deref().and_then(days_since);

    // ----- Benutzer-Aufloesung -----
    let (user_source, user_display, note, confirmed_by, user_sam) = if let Some(a) = assign {
        let disp = if a.user_display.is_empty() {
            a.user.clone()
        } else {
            a.user_display.clone()
        };
        (
            "manuell bestätigt",
            disp,
            a.note.clone(),
            a.confirmed_by.clone(),
            a.user.clone(),
        )
    } else if let Some(c) = csv.filter(|c| !c.user.is_empty()) {
        (
            "Rollout-Liste",
            c.user.clone(),
            String::new(),
            None,
            String::new(),
        )
    } else if let Some(u) = iv
        .current_user
        .clone()
        .or(iv.last_logged_on_user.clone())
        .filter(|s| !s.is_empty())
    {
        (
            "zuletzt angemeldet",
            strip_domain(&u),
            String::new(),
            None,
            String::new(),
        )
    } else {
        (
            "—",
            "Unbekannt".to_string(),
            String::new(),
            None,
            String::new(),
        )
    };
    let user_source = user_source.to_string();
    let user = user_display.clone();
    let init = initials(&user_display);

    let dept = assign
        .and_then(|a| {
            let dept = a.dept.trim();
            if dept.is_empty() {
                None
            } else {
                Some(dept.to_string())
            }
        })
        .unwrap_or_else(|| dept_from_host(host));

    // ----- Bewertung -----
    let ev = evaluate(
        th,
        &DeviceFacts {
            has_inventory: has_inv,
            ram_gb,
            age_years,
            disk_is_ssd,
            cpu_cores: cores,
            cpu_clock_mhz: clock,
            os_is_win11,
            last_seen_days,
        },
    );

    let age_text = match age_years {
        Some(a) if has_inv => format!("{} J.", fmt_de(a)),
        _ => "—".to_string(),
    };
    let last_seen_text = if !has_inv {
        "nie".to_string()
    } else {
        last_seen_text(last_seen_days)
    };

    let network = iv.network.clone().unwrap_or_default();
    let bios = iv.bios.clone();
    let win11 = iv.win11.clone();

    DeviceFull {
        host: host.to_string(),
        has_inventory: has_inv,
        status: ev.status,
        status_label: ev.status_label,
        upgrade_reasons: ev.reasons,
        user,
        user_display,
        user_sam,
        user_source,
        dept,
        initials: init,
        avatar_color: avatar_color(host),
        cpu: cpu_name,
        cores,
        cores_text: format!("{} Kerne / {} Threads", cores, threads),
        ram_gb,
        ram_slots_used: slots_used,
        ram_slots_total: slots_total,
        ram_free_slots: (slots_total - slots_used).max(0),
        ram_target_gb: th.target_ram_gb,
        disk_type,
        disk_gb,
        disk_model,
        age_years,
        age_text,
        last_seen_days,
        last_seen_text,
        os_short: os_short(&os_caption, &os_build),
        os_caption,
        os_build,
        chassis: opt_str(&iv.chassis, "—"),
        manufacturer: opt_str(&iv.manufacturer, "—"),
        model: opt_str(&iv.model, ""),
        serial_number: opt_str(&iv.serial_number, "—"),
        bios_version: bios.clone().and_then(|b| b.version).unwrap_or_default(),
        bios_date: bios
            .and_then(|b| b.release_date)
            .map(|d| d.split('T').next().unwrap_or("").to_string()),
        gpus: iv.gpus.clone().unwrap_or_default(),
        ip: network
            .iter()
            .find_map(|n| n.ipv4.clone())
            .unwrap_or_default(),
        mac: network
            .iter()
            .find_map(|n| n.mac.clone())
            .unwrap_or_default(),
        tpm: win11.as_ref().and_then(|w| w.tpm_present),
        secure_boot: win11.and_then(|w| w.secure_boot),
        ram_sticks,
        note,
        confirmed_by,
        collected_at_utc: iv.collected_at_utc.clone(),
    }
}

pub fn apply_manual_assignment(
    d: &mut DeviceFull,
    user: &str,
    user_display: &str,
    user_dept: &str,
    note: &str,
    confirmed_by: &str,
) {
    let display = if user_display.trim().is_empty() {
        user.to_string()
    } else {
        user_display.to_string()
    };
    d.user = display.clone();
    d.user_display = display.clone();
    d.user_sam = user.to_string();
    d.user_source = "manuell bestätigt".to_string();
    d.dept = if user_dept.trim().is_empty() {
        dept_from_host(&d.host)
    } else {
        user_dept.to_string()
    };
    d.initials = initials(&display);
    d.note = note.to_string();
    d.confirmed_by = Some(confirmed_by.to_string());
}
