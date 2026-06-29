/// Leitet aus einem Anzeigenamen einen plausiblen SAM-Account ab — nur als
/// CSV-Fallback, wenn kein AD verfuegbar ist. Deutsche Umlaute werden
/// transliteriert, damit der Wert ASCII-stabil und deterministisch bleibt.
pub(crate) fn synth_sam(display: &str) -> String {
    let mut sam = String::new();
    for ch in display.chars() {
        match ch {
            'ä' | 'Ä' => sam.push_str("ae"),
            'ö' | 'Ö' => sam.push_str("oe"),
            'ü' | 'Ü' => sam.push_str("ue"),
            'ß' => sam.push_str("ss"),
            ' ' => sam.push('.'),
            c if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') => sam.push(c),
            _ => {}
        }
    }
    sam.to_lowercase()
}
