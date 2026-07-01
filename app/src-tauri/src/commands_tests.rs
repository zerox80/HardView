use crate::identity::{current_user_domain, synth_sam};

#[test]
fn current_user_domain_never_returns_blank_identity() {
    // Kann den exakten Wert nicht pruefen (haengt vom ausfuehrenden Konto ab, z. B.
    // GitHub-Actions-Runner statt Domaenen-Benutzer) - stellt aber sicher, dass die
    // Win32-Ermittlung (oder ihr Umgebungsvariablen-Fallback) nie leer/panisch ist,
    // sondern immer eine anzeigbare "DOMAENE\Benutzer"-Kennung liefert.
    let (full, domain) = current_user_domain();
    assert!(!full.trim().is_empty(), "Identitaet darf nicht leer sein");
    assert!(
        full.contains('\\'),
        "erwarte DOMAENE\\Benutzer-Format: {}",
        full
    );
    assert!(!domain.trim().is_empty(), "Domaene darf nicht leer sein");
}

#[test]
fn synth_sam_transliterates_umlauts() {
    assert_eq!(synth_sam("Jürgen Müller"), "juergen.mueller");
    assert_eq!(synth_sam("Björn Öztürk"), "bjoern.oeztuerk");
    assert_eq!(synth_sam("Weiß"), "weiss");
    assert_eq!(synth_sam("Änne Ärmel"), "aenne.aermel");
}

#[test]
fn synth_sam_filters_unsupported_chars_and_is_deterministic() {
    assert_eq!(synth_sam("O'Brien"), "obrien");
    assert_eq!(synth_sam("Anna-Lena_K (Gast)"), "anna-lena_k.gast");
    assert_eq!(synth_sam("Test User"), synth_sam("Test User"));
}
