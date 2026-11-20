//! Inari: the native desktop table for Innovation with any supported
//! expansion combination, two humans at one machine or one against an AI.
#![forbid(unsafe_code)]
#![cfg_attr(windows, windows_subsystem = "windows")]
mod clipboard;
mod engine;
mod faces;
mod gfx;
mod session;
mod sprites;
mod strength;
mod table;
mod theme;
mod ui;

use eframe::egui;

/// The release tag the build was made from (set by the release workflow), or
/// "dev" for local builds.
pub const VERSION: &str = match option_env!("INARI_VERSION") {
    Some(version) => version,
    None => "dev",
};

fn main() {
    // Windows keeps setup, networks and saves beside the exe.
    if cfg!(windows)
        && let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let _ = std::env::set_current_dir(dir);
    }
    let options = match session::Options::parse(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let icon = egui::IconData {
        rgba: mark(64),
        width: 64,
        height: 64,
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Inari")
            // Wayland compositors match the window to its desktop entry by this.
            .with_app_id("inari")
            .with_inner_size([gfx::W, gfx::H])
            .with_min_inner_size([gfx::W / 4., gfx::H / 4.])
            .with_icon(icon),
        event_loop_builder: x11_under_wsl(),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Inari",
        native,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc, options)))),
    );
    if let Err(error) = result {
        eprintln!("Inari could not open its window: {error}");
        std::process::exit(1);
    }
}

/// WSLg's compositor (Weston 9) segfaults when a native Wayland window is
/// maximized from its Windows title bar, closing every window with it.
/// Under WSL the table goes through XWayland; real Linux desktops such as
/// Hyprland keep native Wayland.
#[cfg(target_os = "linux")]
fn x11_under_wsl() -> Option<eframe::EventLoopBuilderHook> {
    use winit::platform::x11::EventLoopBuilderExtX11;
    let x11 = clipboard::is_wsl() && std::env::var_os("DISPLAY").is_some();
    x11.then(|| -> eframe::EventLoopBuilderHook {
        Box::new(|builder| {
            builder.with_x11();
        })
    })
}
#[cfg(not(target_os = "linux"))]
fn x11_under_wsl() -> Option<eframe::EventLoopBuilderHook> {
    None
}

/// The Inari mark in Cloisonné as straight RGBA rows, for the window icon and
/// the Windows file icon (`assets/inari.ico`).
fn mark(px: usize) -> Vec<u8> {
    let middle = theme::Theme::Cloisonne.palette().enamel.age[1];
    sprites::brand(px, middle).bytes()
}

#[cfg(test)]
mod tests {
    fn png(px: usize) -> Vec<u8> {
        let path = std::env::temp_dir().join(format!("inari-mark-{}-{px}.png", std::process::id()));
        super::gfx::write_png(&path, px, px, &super::mark(px)).unwrap();
        let png = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        png
    }

    /// The Windows build links `assets/inari.ico`. After changing the mark,
    /// rewrite it with `INARI_ICO=1 cargo test -p inari icon_file`.
    #[test]
    fn icon_file() {
        let sizes = [16, 24, 32, 48, 64, 256];
        let pngs: Vec<_> = sizes.into_iter().map(png).collect();
        let mut ico = vec![0, 0, 1, 0];
        ico.extend((sizes.len() as u16).to_le_bytes());
        let mut offset = 6 + 16 * sizes.len();
        for (px, png) in sizes.into_iter().zip(&pngs) {
            // A side of 0 means 256; one plane, 32 bits per pixel.
            let side = (px % 256) as u8;
            ico.extend([side, side, 0, 0, 1, 0, 32, 0]);
            ico.extend((png.len() as u32).to_le_bytes());
            ico.extend((offset as u32).to_le_bytes());
            offset += png.len();
        }
        ico.extend(pngs.concat());
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/inari.ico");
        if std::env::var_os("INARI_ICO").is_some() {
            std::fs::write(path, &ico).unwrap();
        }
        let stale = std::fs::read(path).ok() != Some(ico);
        assert!(!stale, "assets/inari.ico is stale: rerun with INARI_ICO=1");
    }
}
