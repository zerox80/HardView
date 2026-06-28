//! CSV-Export der Geraeteliste. RFC-4180-Quoting, Haertung gegen Formel-Injection
//! und UTF-8 mit BOM (damit Excel Umlaute korrekt anzeigt).
use crate::model::DeviceFull;
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
            d.age_years
                .map(|a| format!("{:.1}", a).replace('.', ","))
                .unwrap_or_default(),
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
