//! CSV-Export der Geraeteliste. RFC-4180-Quoting, Haertung gegen Formel-Injection
//! und UTF-8 mit BOM (damit Excel Umlaute korrekt anzeigt).
use crate::model::DeviceFull;
use crate::upgrade::fmt_de;
use std::path::PathBuf;

/// Serialisiert die Geraete als CSV, schreibt sie in den Documents-Ordner des
/// Benutzers und liefert (Pfad, Zeilenzahl) zurueck.
pub fn write_devices_csv(devs: &[DeviceFull]) -> Result<(PathBuf, usize), String> {
    let csv = build_csv(devs);

    let docs = std::env::var("USERPROFILE")
        .map(|p| std::path::Path::new(&p).join("Documents"))
        .unwrap_or_else(|_| std::env::temp_dir());
    let _ = std::fs::create_dir_all(&docs);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let file = docs.join(format!("HardView-Export-{}.csv", stamp));

    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(csv.as_bytes());
    std::fs::write(&file, bytes).map_err(|e| format!("Export fehlgeschlagen: {}", e))?;
    Ok((file, devs.len()))
}

fn build_csv(devs: &[DeviceFull]) -> String {
    let mut csv = String::from(
        "Hostname;Benutzer;Quelle;Abteilung;Status;Begruendungen;CPU;Kerne;RAM_GB;Datentraeger;Groesse_GB;Alter_Jahre;Betriebssystem;Letzte_Inventarisierung;Seriennummer;Modell\r\n",
    );
    for d in devs {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{}\r\n",
            esc(&d.host),
            esc(&d.user_display),
            esc(&d.user_source),
            esc(&d.dept),
            esc(&d.status_label),
            esc(&d.upgrade_reasons.join(" | ")),
            esc(&d.cpu),
            d.cores,
            d.ram_gb,
            esc(&d.disk_type),
            d.disk_gb,
            d.age_years.map(fmt_de).unwrap_or_default(),
            esc(&d.os_caption),
            esc(&d.last_seen_text),
            esc(&d.serial_number),
            esc(&format!("{} {}", d.manufacturer, d.model)),
        ));
    }
    csv
}

/// Zitiert ein Feld (RFC 4180) und entschaerft Formel-Injection: Werte stammen z. T.
/// aus nicht vertrauenswuerdigen Agent-JSONs; ein fuehrendes = + - @ (oder Tab) wuerde
/// Excel/Calc als Formel auswerten (DDE -> Codeausfuehrung beim Oeffnen des Exports).
fn esc(s: &str) -> String {
    let cleaned = s.replace(['\r', '\n'], " ");
    let guarded = if cleaned.starts_with(['=', '+', '-', '@', '\t']) {
        format!("'{}", cleaned)
    } else {
        cleaned
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::esc;

    #[test]
    fn esc_quotes_and_doubles_inner_quotes() {
        assert_eq!(esc("normal"), "\"normal\"");
        // Innere Quotes werden RFC-4180-konform verdoppelt.
        assert_eq!(esc("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn esc_neutralizes_formula_injection() {
        // Fuehrende Formel-Trigger werden mit ' entschaerft (kein Excel/Calc-Eval).
        assert_eq!(esc("=1+1"), "\"'=1+1\"");
        assert_eq!(esc("+cmd"), "\"'+cmd\"");
        assert_eq!(esc("-2"), "\"'-2\"");
        assert_eq!(esc("@SUM(A1)"), "\"'@SUM(A1)\"");
        assert_eq!(esc("\tTab"), "\"'\tTab\"");
        // Klassischer DDE-Payload bleibt als Text erhalten, nicht als Formel.
        assert_eq!(esc("=cmd|'/c calc'!A1"), "\"'=cmd|'/c calc'!A1\"");
    }

    #[test]
    fn esc_strips_newlines_to_keep_one_row_per_device() {
        // CR/LF wuerden sonst die Zeilenstruktur (und damit die Spalten) zerstoeren;
        // beide werden einzeln durch ein Leerzeichen ersetzt (CRLF -> zwei Spaces).
        assert_eq!(esc("Zeile1\r\nZeile2"), "\"Zeile1  Zeile2\"");
        assert_eq!(esc("a\nb"), "\"a b\"");
        assert!(!esc("a\r\nb").contains('\n') && !esc("a\r\nb").contains('\r'));
    }
}
