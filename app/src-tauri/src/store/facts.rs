use super::common::eq_ci;
use crate::model::DiskInv;

pub(super) fn is_solid_state_media(media_type: &Option<String>) -> bool {
    eq_ci(media_type, "SSD") || eq_ci(media_type, "SCM")
}

pub(super) fn classify_ssd_state(primary: Option<&DiskInv>, has_hdd: bool) -> Option<bool> {
    if has_hdd {
        return Some(false);
    }
    let media = primary.and_then(|d| d.media_type.as_ref())?;
    if media.eq_ignore_ascii_case("HDD") {
        Some(false)
    } else if media.eq_ignore_ascii_case("SSD") || media.eq_ignore_ascii_case("SCM") {
        Some(true)
    } else {
        None
    }
}

pub(super) fn classify_windows_11(caption: &str, build: &str) -> Option<bool> {
    let lower = caption.to_lowercase();
    if lower.contains("windows 11") {
        return Some(true);
    }
    if lower.contains("windows 10") {
        return Some(false);
    }
    if let Some(build_no) = build.rsplit('.').next().and_then(|b| b.parse::<i64>().ok()) {
        if build_no >= 22_000 {
            return Some(true);
        }
        if build_no >= 10_000 {
            return Some(false);
        }
    }
    None
}
