//! Under WSL, copies go straight to the Windows clipboard rather than through
//! WSLg's X11 clipboard bridge.
//! Send text directly to Windows, without interpreting log content as commands.
use std::io::Write;
use std::process::{Command, Stdio};

pub fn is_wsl() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WSL_DISTRO_NAME").is_some()
            || std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .is_ok_and(|release| release.to_ascii_lowercase().contains("microsoft")))
}

pub fn copy_windows(text: &str) -> Result<(), String> {
    let mut command = Command::new("clip.exe");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Some WSL shells do not inherit the Windows PATH.
            Command::new("/mnt/c/Windows/System32/clip.exe")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| format!("Could not start Windows clip.exe: {error}"))?
        }
        Err(error) => return Err(format!("Could not start Windows clip.exe: {error}")),
    };
    // clip.exe recognises UTF-16 and so does not use the OEM code page. No
    // BOM: clip.exe would keep it as an invisible first character.
    let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let written = child.stdin.take().unwrap().write_all(&bytes);
    // Always reap the child, even if writing failed; dropping stdin sends EOF.
    let status = child.wait().map_err(|error| error.to_string())?;
    written.map_err(|error| format!("Could not send log to Windows: {error}"))?;
    if !status.success() {
        return Err(format!("Windows clip.exe exited with {status}"));
    }
    Ok(())
}
