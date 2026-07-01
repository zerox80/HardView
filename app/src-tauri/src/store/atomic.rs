use super::common::now_iso;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const ASSIGNMENT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
/// Lock-Dateien, die aelter als ASSIGNMENT_LOCK_STALE sind, gelten als "vermutlich
/// verwaist" — wir uebernehmen sie aber NUR dann, wenn wir zusaetzlich bestaetigen
/// koennen, dass der hinterlegte Prozess nicht mehr lebt (PID-Liveness-Check).
/// Ohne diese doppelte Pruefung wuerden wir bei hoher Last einem lebenden Writer
/// sein frisches Lock stehlen (klassische TOCTOU-Luecke zwischen Staleness-Pruefung
/// und remove_file).
const ASSIGNMENT_LOCK_STALE: Duration = Duration::from_secs(60);
// ------------------------------------------------------------------ Atomic write
pub(super) fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let pid = std::process::id();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp-{}-{}", pid, stamp));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("Temporäre Datei konnte nicht angelegt werden: {}", e))?;
    file.write_all(content.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Schreiben fehlgeschlagen: {}", e)
        })?;
    drop(file);
    if let Err(replace_err) = replace_file(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Atomarer Replace fehlgeschlagen: {}", replace_err));
    }
    Ok(())
}

#[cfg(windows)]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let existing: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
    let new: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            new.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::rename(src, dst)
}

pub(super) struct AssignmentLock {
    path: PathBuf,
}

impl Drop for AssignmentLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) fn acquire_assignment_lock(path: &Path) -> Result<AssignmentLock, String> {
    let lock_path = path.with_extension("lock");
    let start = Instant::now();

    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = writeln!(
                    file,
                    "pid={} createdAtUtc={}",
                    std::process::id(),
                    now_iso()
                );
                return Ok(AssignmentLock { path: lock_path });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if can_take_over_stale(&lock_path) {
                    // Atomar auf einen Tombstone verschieben (vs. remove_file + create_new,
                    // was eine TOCTOU-Luecke offen liesse: ein zwischenzeitlich von einem
                    // anderen Writer angelegtes frisches Lock waere geloescht worden).
                    // Renameschlaegt fehl, wenn ein anderer Writer das Lock bereits
                    // uebernommen/ersetzt hat -> wir scheitern hier sicher und proben
                    // die Schleife von vorn (kein fremdes Lock wird gestohlen).
                    let tombstone =
                        lock_path.with_extension(format!("stale-{}", std::process::id()));
                    if fs::rename(&lock_path, &tombstone).is_err() {
                        // Lock wurde inzwischen veraendert -> nicht uebernehmen.
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    let _ = fs::remove_file(&tombstone);
                    continue;
                }
                if start.elapsed() >= ASSIGNMENT_LOCK_TIMEOUT {
                    return Err(
                        "assignments.json wird gerade von einer anderen Instanz geschrieben".into(),
                    );
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!(
                    "Assignment-Lock konnte nicht erstellt werden: {}",
                    e
                ))
            }
        }
    }
}

/// Entscheidet, ob ein bestehendes Lock uebernommen werden darf. Wir kombinieren
/// zwei Bedingungen, um die oben beschriebene TOCTOU-Luecke zu schliessen:
///   (1) Lock-Datei ist aelter als ASSIGNMENT_LOCK_STALE (heuristische Staleness
///       gegen hängengebliebene/abgestürzte Writer).
///   (2) Zusaetzlich lesen wir den hinterlegten PID und pruefen per OS, ob dieser
///       Prozess noch existiert. Nur wenn wir bestaetigen koennen, dass der PID nicht
///       (mehr) aktiv ist, uebernehmen wir. Ist der PID nicht lesbar oder laesst er
///       sich nicht pruefen (z. B. Access-Denied), uebernehmen wir NICHT — wir
///       warten lieber das Timeout ab, als ein evtl. lebendes Lock zu stehlen.
fn can_take_over_stale(path: &Path) -> bool {
    if !assignment_lock_is_stale(path) {
        return false;
    }
    match read_lock_pid(path) {
        Some(pid) => !is_process_alive(pid),
        None => false,
    }
}

fn assignment_lock_is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|age| age >= ASSIGNMENT_LOCK_STALE)
        .unwrap_or(false)
}

fn read_lock_pid(path: &Path) -> Option<u32> {
    let mut content = String::new();
    {
        let mut f = OpenOptions::new().read(true).open(path).ok()?;
        f.read_to_string(&mut content).ok()?;
    }
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("pid=") {
            if let Ok(pid) = rest.trim().parse::<u32>() {
                return Some(pid);
            }
        }
    }
    None
}

/// Prueft, ob ein Prozess mit der angegebenen PID noch aktiv ist. Auf Windows per
/// OpenProcess + GetExitCodeProcess; auf anderen Plattformen koennen wir das nicht
/// portabel pruefen und gehen konservativ davon aus, der Prozess sei noch am Leben
/// (kein Lock-Steal).
#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use std::os::raw::c_void;
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn GetExitCodeProcess(h: *mut c_void, code: *mut u32) -> i32;
        fn CloseHandle(h: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_INVALID_PARAMETER: u32 = 87;
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            // OpenProcess liefert NULL, wenn der Prozess nicht existiert (-> tot, wir
            // duermen uebernehmen) ODER wenn wir keine Berechtigung haben (-> evtl.
            // am Leben, konservativ nicht uebernehmen). Unterscheidung anhand des
            // letzten Fehlers: ERROR_INVALID_PARAMETER bedeutet "PID nicht aktiv".
            let last = GetLastError();
            return last != ERROR_INVALID_PARAMETER;
        }
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        ok != 0 && code == STILL_ACTIVE
    }
}

#[cfg(not(windows))]
fn is_process_alive(_pid: u32) -> bool {
    true
}
