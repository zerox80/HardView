use std::fs;

// ------------------------------------------------------------------ Encoding-tolerantes Lesen
/// Liest eine Textdatei. UTF-8 bevorzugt; faellt sonst auf Windows-1252 zurueck
/// (deutsche Umlaute bleiben korrekt), damit ANSI-CSV aus Excel funktioniert.
pub(super) fn read_text(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
    match String::from_utf8(bytes) {
        Ok(s) => Ok(strip_bom(s)),
        Err(e) => Ok(decode_windows_1252(&e.into_bytes())),
    }
}
fn strip_bom(s: String) -> String {
    s.strip_prefix('\u{feff}')
        .map(|x| x.to_string())
        .unwrap_or(s)
}
pub(super) fn decode_windows_1252(bytes: &[u8]) -> String {
    const C1: [char; 32] = [
        '\u{20ac}', '\u{0081}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02c6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008d}',
        '\u{017d}', '\u{008f}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02dc}', '\u{2122}', '\u{0161}', '\u{203a}',
        '\u{0153}', '\u{009d}', '\u{017e}', '\u{0178}',
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => C1[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}
