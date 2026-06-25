use super::text::read_text;
use std::collections::HashMap;
use std::fs;

// ------------------------------------------------------------------ Master-CSV
#[derive(Default, Clone)]
pub struct CsvRow {
    pub user: String,
}

pub fn read_master_csv(path: &str) -> HashMap<String, CsvRow> {
    let mut out = HashMap::new();
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > 20 * 1024 * 1024 {
            // 20 MB Limit
            return out;
        }
    }
    let txt = match read_text(path) {
        Ok(t) => t,
        Err(_) => return out,
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b';')
        .flexible(true)
        .has_headers(true)
        .from_reader(txt.as_bytes());
    // Spaltenindizes aus dem Header bestimmen
    let (mut i_host, mut i_user) = (None, None);
    if let Ok(hdr) = rdr.headers() {
        for (i, h) in hdr.iter().enumerate() {
            match h.trim().to_lowercase().as_str() {
                "computer" => i_host = Some(i),
                "benutzer" => i_user = Some(i),
                _ => {}
            }
        }
    }
    let i_host = match i_host {
        Some(i) => i,
        None => return out,
    };
    for rec in rdr.records().flatten() {
        let host = rec.get(i_host).unwrap_or("").trim();
        if host.is_empty() {
            continue;
        }
        out.insert(
            host.to_uppercase(),
            CsvRow {
                user: i_user
                    .and_then(|i| rec.get(i))
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            },
        );
    }
    out
}
