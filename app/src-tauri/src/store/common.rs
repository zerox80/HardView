use chrono::{DateTime, SecondsFormat, Utc};

const PALETTE: [&str; 8] = [
    "#4f8cff", "#2fd6a6", "#b98cff", "#ff8a4f", "#ffb454", "#5fc9ff", "#ff7a9c", "#7ee081",
];
// ------------------------------------------------------------------ Helpers
pub(super) fn f2i(x: Option<f64>) -> i64 {
    x.unwrap_or(0.0).round() as i64
}
pub(super) fn opt_str(o: &Option<String>, dflt: &str) -> String {
    o.clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| dflt.to_string())
}
pub(super) fn eq_ci(o: &Option<String>, v: &str) -> bool {
    o.as_deref()
        .map(|s| s.eq_ignore_ascii_case(v))
        .unwrap_or(false)
}
pub(super) fn strip_domain(u: &str) -> String {
    u.rsplit('\\').next().unwrap_or(u).to_string()
}
pub(super) fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    let mut s = String::new();
    if let Some(p) = parts.first() {
        s.push(p.chars().next().unwrap_or('?'));
    }
    if let Some(p) = parts.get(1) {
        s.push(p.chars().next().unwrap_or(' '));
    }
    if s.is_empty() {
        "?".into()
    } else {
        s.to_uppercase()
    }
}
pub(super) fn avatar_color(host: &str) -> String {
    let mut n: u32 = 0;
    for b in host.bytes() {
        n = n.wrapping_mul(31).wrapping_add(b as u32);
    }
    PALETTE[(n as usize) % PALETTE.len()].to_string()
}
pub(super) fn os_short(caption: &str, build: &str) -> String {
    let w = if caption.contains("11") {
        "Win 11"
    } else if caption.contains("10") {
        "Win 10"
    } else {
        caption
    };
    let b = build.rsplit('.').next().unwrap_or(build);
    let label = match b {
        "22631" => "23H2",
        "22621" => "22H2",
        "19045" => "22H2",
        "19044" => "21H2",
        "26100" | "26200" => "24H2",
        _ => "",
    };
    if label.is_empty() {
        w.to_string()
    } else {
        format!("{} {}", w, label)
    }
}
pub(super) fn last_seen_text(days: Option<i64>) -> String {
    match days {
        None => "—".into(),
        Some(d) if d < 1 => "gerade eben".into(),
        Some(1) => "vor 1 Tag".into(),
        Some(d) => format!("vor {} Tagen", d),
    }
}
/// Leitet die Abteilung aus dem Hostnamen ab (Schema WS-<ABT>-NN).
/// Heuristik fuer Kowobau; per AD-Department spaeter ueberschreibbar.
pub(super) fn dept_from_host(host: &str) -> String {
    let up = host.to_uppercase();
    let map = [
        ("BUCH", "Buchhaltung"),
        ("VERTRIEB", "Vertrieb"),
        ("MARKETING", "Marketing"),
        ("ENTW", "Entwicklung"),
        ("PERSONAL", "Personal"),
        ("LAGER", "Lager"),
        ("EMPFANG", "Empfang"),
        ("IT", "IT"),
        ("GF", "Geschäftsführung"),
    ];
    for (key, name) in map {
        if up.contains(key) {
            return name.to_string();
        }
    }
    "Allgemein".to_string()
}
pub(super) fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}
pub(super) fn days_since(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| (Utc::now() - dt.with_timezone(&Utc)).num_days())
}
