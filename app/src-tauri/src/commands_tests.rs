use crate::identity::synth_sam;

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
