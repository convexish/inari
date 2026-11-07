//! WSL's X11 clipboard bridge cannot use miniquad's UTF8-only selection owner.
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
    // A BOM makes clip.exe decode Unicode instead of the Windows OEM code page.
    let bytes: Vec<u8> = std::iter::once(0xfeff_u16)
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    let written = child.stdin.take().unwrap().write_all(&bytes);
    // Always reap the child, even if writing failed; dropping stdin sends EOF.
    let status = child.wait().map_err(|error| error.to_string())?;
    written.map_err(|error| format!("Could not send log to Windows: {error}"))?;
    if !status.success() {
        return Err(format!("Windows clip.exe exited with {status}"));
    }
    Ok(())
}
