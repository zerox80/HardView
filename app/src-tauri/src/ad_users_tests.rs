use crate::ad_users::{fallback_users_from_devices, filter_and_truncate};
use crate::model::{AdUser, DeviceFull};

fn device(host: &str, user_display: &str, user_sam: &str, dept: &str) -> DeviceFull {
    DeviceFull {
        host: host.into(),
        user_display: user_display.into(),
        user_sam: user_sam.into(),
        dept: dept.into(),
        ..Default::default()
    }
}

fn user(sam: &str, display: &str, dept: &str, mail: &str) -> AdUser {
    AdUser {
        sam: sam.into(),
        display: display.into(),
        dept: dept.into(),
        mail: mail.into(),
    }
}

#[test]
fn fallback_users_from_devices_skips_empty_and_unbekannt() {
    let devs = vec![
        device("WS-A", "", "", "IT"),
        device("WS-B", "Unbekannt", "", "IT"),
        device("WS-C", "Anna Berger", "a.berger", "IT"),
    ];
    let users = fallback_users_from_devices(&devs);
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].display, "Anna Berger");
}

#[test]
fn fallback_users_from_devices_dedupes_by_real_sam() {
    let devs = vec![
        // Gleicher echter SAM (z.B. dieselbe Person auf zwei Geraeten) -> dedupliziert.
        device("WS-A", "Anna Berger", "a.berger", "IT"),
        device("WS-B", "Anna Berger", "a.berger", "IT"),
    ];
    let users = fallback_users_from_devices(&devs);
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].sam, "a.berger");
}

#[test]
fn fallback_users_from_devices_disambiguates_synthesized_sam_collision() {
    let devs = vec![
        // Kein user_sam -> wird aus dem Anzeigenamen synthetisiert.
        device("WS-A", "Jürgen Müller", "", "IT"),
        // Zweites Geraet mit gleichem synthetisiertem SAM (andere Person, anderer Host)
        // -> darf NICHT verworfen werden, sondern wird per Host disambiguiert.
        device("WS-B", "Jürgen Müller", "", "Marketing"),
        // Eigener echter SAM -> eigener Eintrag.
        device("WS-C", "Anna Berger", "a.berger", "IT"),
    ];
    let users = fallback_users_from_devices(&devs);
    assert_eq!(users.len(), 3);
    let first = users.iter().find(|u| u.sam == "juergen.mueller").unwrap();
    assert_eq!(first.dept, "IT");
    let second = users
        .iter()
        .find(|u| u.sam == "juergen.mueller.ws-b")
        .unwrap();
    assert_eq!(second.dept, "Marketing");
    assert!(users.iter().any(|u| u.sam == "a.berger"));
}

#[test]
fn fallback_users_from_devices_sorts_by_display_name() {
    let devs = vec![
        device("WS-A", "Zoe Wagner", "z.wagner", "IT"),
        device("WS-B", "Anna Berger", "a.berger", "IT"),
        device("WS-C", "Markus Bauer", "m.bauer", "Vertrieb"),
    ];
    let users = fallback_users_from_devices(&devs);
    let names: Vec<&str> = users.iter().map(|u| u.display.as_str()).collect();
    assert_eq!(names, vec!["Anna Berger", "Markus Bauer", "Zoe Wagner"]);
}

#[test]
fn filter_and_truncate_is_noop_for_empty_query() {
    let users = vec![
        user("z.wagner", "Zoe Wagner", "IT", "z.wagner@example.com"),
        user(
            "a.berger",
            "Anna Berger",
            "Marketing",
            "a.berger@example.com",
        ),
    ];
    let result = filter_and_truncate(users.clone(), "");
    assert_eq!(result.len(), users.len());
    assert_eq!(result[0].display, users[0].display);
    assert_eq!(result[1].display, users[1].display);
}

#[test]
fn filter_and_truncate_matches_case_insensitively_across_all_fields() {
    let users = vec![
        user(
            "a.berger",
            "Anna Berger",
            "Marketing",
            "a.berger@example.com",
        ),
        user("m.bauer", "Markus Bauer", "Vertrieb", "m.bauer@example.com"),
    ];
    // Treffer ueber Abteilung (Marketing), Gross-/Kleinschreibung ignoriert.
    let by_dept = filter_and_truncate(users.clone(), "marketing");
    assert_eq!(by_dept.len(), 1);
    assert_eq!(by_dept[0].sam, "a.berger");

    // Treffer ueber SAM.
    let by_sam = filter_and_truncate(users, "m.bauer");
    assert_eq!(by_sam.len(), 1);
    assert_eq!(by_sam[0].sam, "m.bauer");
}

#[test]
fn filter_and_truncate_caps_result_at_100() {
    let users: Vec<AdUser> = (0..150)
        .map(|i| user(&format!("u{i}"), &format!("User {i}"), "IT", ""))
        .collect();
    let result = filter_and_truncate(users, "");
    assert_eq!(result.len(), 100);
}
