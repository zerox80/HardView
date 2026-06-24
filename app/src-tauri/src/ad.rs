//! Read-only AD-Lookup via eingebettetes PowerShell-Skript (System.DirectoryServices,
//! integrierte Windows-Auth). Kein RSAT noetig. Ergebnis wird gecacht.
use crate::model::AdUser;
use std::process::Command;
use std::time::{Duration, Instant};

const SCRIPT: &str = include_str!("../scripts/Get-AdUsers.ps1");
const AD_QUERY_TIMEOUT: Duration = Duration::from_secs(15);

/// Schreibt das eingebettete Skript in den Temp-Ordner und fuehrt es aus.
/// Gibt die geparste Benutzerliste zurueck (alle aktivierten User, in Rust gefiltert).
pub fn fetch_ad_users() -> Result<Vec<AdUser>, String> {
    use std::io::{Read, Write};
    use std::process::Stdio;

    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    let powershell_path = format!(
        "{}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        system_root
    );
    let mut cmd = Command::new(powershell_path);
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", "-"]);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    no_window(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("PowerShell-Start fehlgeschlagen: {}", e))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "PowerShell stdin konnte nicht geöffnet werden".to_string())?;
        stdin
            .write_all(SCRIPT.as_bytes())
            .map_err(|e| format!("Fehler beim Schreiben des AD-Skripts: {}", e))?;
    }

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "PowerShell stdout konnte nicht geoeffnet werden".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "PowerShell stderr konnte nicht geoeffnet werden".to_string())?;
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stderr.read_to_end(&mut buf).map(|_| buf)
    });

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("AD-Abfrage fehlgeschlagen: {}", e))?
        {
            break status;
        }
        if started.elapsed() >= AD_QUERY_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err("AD-Abfrage hat das Timeout ueberschritten".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| "AD-stdout konnte nicht gelesen werden".to_string())?
        .map_err(|e| format!("AD-stdout konnte nicht gelesen werden: {}", e))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "AD-stderr konnte nicht gelesen werden".to_string())?
        .map_err(|e| format!("AD-stderr konnte nicht gelesen werden: {}", e))?;

    if !status.success() {
        return Err(format!(
            "AD-Abfrage fehlgeschlagen: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&stdout);
    let txt = stdout.trim();
    if txt.is_empty() {
        return Ok(Vec::new());
    }
    // ConvertTo-Json liefert bei genau 1 Objekt ein Objekt statt Array -> beides versuchen.
    serde_json::from_str::<Vec<AdUser>>(txt)
        .or_else(|_| serde_json::from_str::<AdUser>(txt).map(|u| vec![u]))
        .map_err(|e| format!("AD-Antwort nicht lesbar: {}", e))
}

#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}
