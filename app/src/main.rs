//! Inari: the native desktop table for Innovation with any supported
//! expansion combination, two humans at one machine or one against an AI.
#![forbid(unsafe_code)]
#![cfg_attr(windows, windows_subsystem = "windows")]
mod clipboard;
mod faces;
mod next;
mod session;
mod sprites;
mod strength;
mod table;
mod theme;
mod ui;

use macroquad::prelude::*;

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
    macroquad::Window::from_config(
        Conf {
            window_title: "Inari".into(),
            window_width: 1840,
            window_height: 960,
            high_dpi: true,
            sample_count: 1,
            icon: Some(miniquad::conf::Icon {
                small: mark(16).try_into().unwrap(),
                medium: mark(32).try_into().unwrap(),
                big: mark(64).try_into().unwrap(),
            }),
            ..Default::default()
        },
        ui::run(options),
    );
}

/// The Inari mark in Cloisonné as straight RGBA rows, for the window icon and
/// the Windows file icon (`assets/inari.ico`).
fn mark(px: usize) -> Vec<u8> {
    let middle = theme::Theme::Cloisonne.palette().enamel.age[1];
    sprites::brand(px, middle).bytes()
}

#[cfg(test)]
mod tests {
    use macroquad::prelude::Image;

    fn png(px: usize) -> Vec<u8> {
        let path = std::env::temp_dir().join(format!("inari-mark-{}-{px}.png", std::process::id()));
        let path = path.to_str().unwrap();
        // export_png writes rows bottom-up, as for screen captures.
        let bytes = super::mark(px)
            .chunks(px * 4)
            .rev()
            .flatten()
            .copied()
            .collect();
        Image {
            width: px as u16,
            height: px as u16,
            bytes,
        }
        .export_png(path);
        let png = std::fs::read(path).unwrap();
        std::fs::remove_file(path).unwrap();
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
