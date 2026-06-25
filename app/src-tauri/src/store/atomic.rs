use super::common::now_iso;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const ASSIGNMENT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
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
                if assignment_lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
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

fn assignment_lock_is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|age| age >= ASSIGNMENT_LOCK_STALE)
        .unwrap_or(false)
}
