//! Table themes. A theme sets colours, the table pattern and the card enamel;
//! layout, fonts and card geometry are shared, so interface changes apply to
//! every theme. Drawing code asks for a role here, never a literal colour.

use inno_engine::Color as CardColor;
use macroquad::prelude::Color;
use serde::{Deserialize, Serialize};

use super::faces::Set;
use super::sprites::{Enamel, Ground, Pattern, Rgba, Wash, hex};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Black lacquer, gold wire and enamel: the original table.
    #[default]
    Cloisonne,
    /// Blue-and-white ware: cobalt on a cool white ground.
    Porcelain,
    /// Kozo paper, sumi ink and walnut brown, with matte cards.
    Washi,
    /// Ultramarine and gold leaf under a star lattice.
    Lapis,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::Cloisonne, Self::Porcelain, Self::Washi, Self::Lapis];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cloisonne => "Cloisonné",
            Self::Porcelain => "Porcelain",
            Self::Washi => "Washi",
            Self::Lapis => "Lapis",
        }
    }

    /// Accepts the lowercase names used in settings, with or without accents.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "cloisonne" | "cloisonné" => Some(Self::Cloisonne),
            "porcelain" => Some(Self::Porcelain),
            "washi" => Some(Self::Washi),
            "lapis" => Some(Self::Lapis),
            _ => None,
        }
    }

    pub fn palette(self) -> &'static Palette {
        match self {
            Self::Cloisonne => &CLOISONNE,
            Self::Porcelain => &PORCELAIN,
            Self::Washi => &WASHI,
            Self::Lapis => &LAPIS,
        }
    }
}

impl<'de> Deserialize<'de> for Theme {
    /// An unknown name falls back to the default instead of discarding the
    /// other saved settings.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::parse(&String::deserialize(d)?).unwrap_or_default())
    }
}

/// One set's card back and the inks that go with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetPaint {
    /// The back's fill.
    pub back: Color,
    /// The age, name and emblem printed on the back.
    pub on_back: Color,
    /// The set's deck counts and emblems on panels.
    pub ink: Color,
}

/// Fill, edge and text of one control state.
#[derive(Clone, Copy)]
pub struct Control {
    pub fill: Color,
    pub edge: Color,
    pub ink: Color,
}

/// Every colour role the table draws with.
pub struct Palette {
    /// Light themes darken hover and halo effects instead of lightening them.
    pub dark: bool,
    // Table and panels.
    pub table: Color,
    /// Around the table when the window's shape differs from it.
    pub backdrop: Color,
    pub paper: Color,
    pub strip: Color,
    pub hover: Color,
    /// Wire around panels and controls, and its faint inner hairline.
    pub line: Color,
    pub wire: Color,
    pub panel_shadow: Color,
    /// Under popups and tooltips.
    pub shadow: Color,
    /// Dims the table under a card opened full size.
    pub scrim: Color,
    /// Offset shadow under cards on the table.
    pub card_shadow: Color,
    // Text.
    pub ink: Color,
    pub muted: Color,
    /// Small-capitals row labels.
    pub label: Color,
    /// Headings, status and highlighted glyphs.
    pub accent: Color,
    /// Zero counts.
    pub dim: Color,
    /// Special achievement names on their chips.
    pub chip_ink: Color,
    pub error: Color,
    pub you: Color,
    pub rival: Color,
    /// Text selection in setup fields.
    pub selection: Color,
    // Controls.
    pub primary: Control,
    pub primary_hover: Color,
    pub primary_off: Control,
    pub secondary_off: Control,
    /// Cells where a player leads in the icon comparison.
    pub lead: Control,
    /// Legal choices: the outline, and the halo pulsing around it.
    pub glow: Color,
    pub glow_halo: Color,
    // Top bar.
    pub bar: Color,
    pub bar_ink: Color,
    pub bar_muted: Color,
    pub bar_accent: Color,
    pub bar_error: Color,
    pub bar_line: Color,
    /// Thickness of the rule under the bar.
    pub bar_rule: f32,
    pub bar_button: Control,
    pub bar_button_hover: Color,
    pub bar_off: Control,
    // Game log turn headings: the wash at full strength (the current turn),
    // and the heading text for earlier and current turns.
    pub log_wash: Color,
    pub log_head: Color,
    pub log_head_now: Color,
    // Card text and pieces drawn over the enamel.
    pub title_ink: Color,
    pub title_shadow: Color,
    pub card_ink: Color,
    pub demand: Color,
    /// Edges of buried cards peeking out of a splayed stack.
    pub peek_edge: Color,
    /// Card backs and set inks in display order: Base,
    /// Echoes, Figures, Cities, Artifacts, Unseen.
    pub sets: [SetPaint; 6],
    pub back_edge: Color,
    pub enamel: Enamel,
    pub ground: Ground,
}

impl Palette {
    /// A card colour's enamel as (light, band, dark, tint).
    pub fn card(&self, c: CardColor) -> [Color; 4] {
        self.enamel.card(c).map(rgba)
    }

    /// A card colour named in text: lightened on dark tables, the deep band
    /// colour on light ones.
    pub fn hue(&self, c: CardColor) -> Color {
        let [light, band, ..] = self.card(c);
        if self.dark {
            let t = 0.3;
            Color::new(
                light.r + (1. - light.r) * t,
                light.g + (1. - light.g) * t,
                light.b + (1. - light.b) * t,
                1.,
            )
        } else {
            band
        }
    }

    pub fn set(&self, set: Set) -> SetPaint {
        self.sets[match set {
            Set::Base => 0,
            Set::Echoes => 1,
            Set::Figures => 2,
            Set::Cities => 3,
            Set::Artifacts => 4,
            Set::Unseen => 5,
        }]
    }
}

/// Card backs follow the printed sets: Base tan with brown printing, Echoes blue,
/// Figures green, Cities red, Artifacts purple and Unseen gray. Every theme shares
/// them; only the panel inks change between dark and light tables.
const fn set_paint(back: u32, on_back: u32, ink: u32) -> SetPaint {
    SetPaint {
        back: c(back),
        on_back: c(on_back),
        ink: c(ink),
    }
}

const DARK_SETS: [SetPaint; 6] = [
    set_paint(0xcbb187, 0x3a2413, 0xd8c08f),
    set_paint(0x3567b3, 0xf5f7fb, 0x86b2f0),
    set_paint(0x2e7a3a, 0xf3f8f2, 0x74cf7f),
    set_paint(0xb0262c, 0xfbf1f1, 0xf2858b),
    set_paint(0x702894, 0xf7effb, 0xc99ce6),
    set_paint(0x5b606b, 0xf2f3f5, 0xb9bec8),
];

const LIGHT_SETS: [SetPaint; 6] = [
    set_paint(0xcbb187, 0x3a2413, 0x6b4a26),
    set_paint(0x3567b3, 0xf5f7fb, 0x2c5ea8),
    set_paint(0x2e7a3a, 0xf3f8f2, 0x1f6a2d),
    set_paint(0xb0262c, 0xfbf1f1, 0xa41c20),
    set_paint(0x702894, 0xf7effb, 0x6c2590),
    set_paint(0x5b606b, 0xf2f3f5, 0x4f5561),
];

const fn c(rgb: u32) -> Color {
    rgba(hex(rgb))
}
const fn ca(rgb: u32, a: f32) -> Color {
    let [r, g, b, _] = hex(rgb);
    Color::new(r, g, b, a)
}
pub const fn rgba(c: Rgba) -> Color {
    Color::new(c[0], c[1], c[2], c[3])
}
const fn ea(rgb: u32, a: f32) -> Rgba {
    let [r, g, b, _] = hex(rgb);
    [r, g, b, a]
}
const fn control(fill: u32, edge: u32, ink: u32) -> Control {
    Control {
        fill: c(fill),
        edge: c(edge),
        ink: c(ink),
    }
}

/// The original enamel card colours, shared by every theme but Washi.
const RED: [Rgba; 4] = [hex(0xbf4250), hex(0x8e1d2a), hex(0x4f0b14), hex(0xe4b1ad)];
const YELLOW: [Rgba; 4] = [hex(0xc4911f), hex(0x8f6208), hex(0x4f3604), hex(0xe8cf8c)];
const GREEN: [Rgba; 4] = [hex(0x36906a), hex(0x1b6448), hex(0x0a3825), hex(0xb0d2bd)];
const BLUE: [Rgba; 4] = [hex(0x3d6ab8), hex(0x1f4588), hex(0x0d2150), hex(0xb3c4e3)];
const PURPLE: [Rgba; 4] = [hex(0x8352a3), hex(0x56286f), hex(0x2e103b), hex(0xcdb6da)];

static CLOISONNE: Palette = Palette {
    dark: true,
    table: c(0x121318),
    backdrop: c(0x08090b),
    paper: c(0x1b1d23),
    strip: c(0x23252d),
    hover: c(0x2c2f38),
    line: c(0x8c7238),
    wire: ca(0xd4b059, 0.28),
    panel_shadow: ca(0x000000, 0.35),
    shadow: ca(0x000000, 0.45),
    scrim: ca(0x000000, 0.6),
    card_shadow: c(0x000000),
    ink: c(0xece3cc),
    muted: c(0xa69c86),
    label: c(0xb9a77c),
    accent: c(0xe2bd5c),
    dim: c(0x5a564e),
    chip_ink: c(0xc9bea5),
    error: c(0xee7a72),
    you: c(0x7fa7ee),
    rival: c(0xee7a72),
    selection: c(0x5c4e2c),
    primary: control(0xefe6cf, 0xe2bd5c, 0x1b1d23),
    primary_hover: c(0xffffff),
    primary_off: control(0x5c584e, 0x5c584e, 0x969082),
    secondary_off: control(0x1d1f25, 0x423c2e, 0x6c665a),
    lead: control(0x34322d, 0x969080, 0xece3cc),
    glow: c(0xf6f0e0),
    glow_halo: c(0xfff5db),
    bar: c(0x0e0f13),
    bar_ink: c(0xece3cc),
    bar_muted: c(0xa69c86),
    bar_accent: c(0xe2bd5c),
    bar_error: c(0xee7a72),
    bar_line: c(0x8c7238),
    bar_rule: 1.,
    bar_button: control(0x23252d, 0x8c7238, 0xece3cc),
    bar_button_hover: c(0x2c2f38),
    bar_off: control(0x1d1f25, 0x423c2e, 0x6c665a),
    log_wash: ca(0xd4b059, 0.42),
    log_head: c(0xf0d48a),
    log_head_now: c(0xfbefc6),
    title_ink: c(0xfbefc6),
    title_shadow: ca(0x000000, 0.55),
    card_ink: c(0x2a2117),
    demand: c(0x8a1b27),
    peek_edge: c(0xb8934a),
    sets: DARK_SETS,
    back_edge: ca(0xc9a34a, 0.9),
    enamel: Enamel {
        red: RED,
        yellow: YELLOW,
        green: GREEN,
        blue: BLUE,
        purple: PURPLE,
        rim: hex(0xd9b45e),
        rim_dark: ea(0x281806, 0.55),
        rim_light: ea(0xe9cd80, 0.55),
        band_edge: hex(0xd9b45e),
        sheen: 0.5,
        tile_rim: hex(0xe3c06a),
        tile_inner: ea(0x281806, 0.5),
        glyph: hex(0xfbefc6),
        glyph_edge: hex(0x6b4d12),
        disc_light: hex(0x3a3d47),
        disc: hex(0x1d1f26),
        disc_rim: hex(0xd9b45e),
        disc_ink: hex(0xf3e9d2),
        hex: hex(0x16161b),
        hex_rim: hex(0xefe6cf),
        age: [hex(0xfffaf0), hex(0xefe6cf), hex(0xb9ad94)],
        age_rim: ea(0x000000, 0.45),
    },
    ground: Ground {
        pattern: Pattern::Lattice,
        table: hex(0x121318),
        ink: ea(0xd4b059, 0.05),
        wash: None,
    },
};

static PORCELAIN: Palette = Palette {
    dark: false,
    table: c(0xe4e9f0),
    backdrop: c(0xc9d1dd),
    paper: c(0xfbfcfe),
    strip: c(0xeef2f8),
    hover: c(0xdde6f4),
    line: c(0x2e4f93),
    wire: ca(0x2e4f93, 0.22),
    panel_shadow: ca(0x16295a, 0.10),
    shadow: ca(0x142346, 0.25),
    scrim: ca(0x0f1932, 0.55),
    card_shadow: ca(0x142346, 0.30),
    ink: c(0x141c33),
    muted: c(0x525d77),
    label: c(0x2a4b8f),
    accent: c(0x1d4597),
    dim: c(0xa7afc0),
    chip_ink: c(0x2a4b8f),
    error: c(0xb3261e),
    you: c(0x1d55c4),
    rival: c(0xb8302a),
    selection: c(0xbcd0f7),
    primary: control(0x1f3f86, 0x14306e, 0xffffff),
    primary_hover: c(0x2b52a6),
    primary_off: control(0xc3cbd9, 0xc3cbd9, 0x6f7a90),
    secondary_off: control(0xf2f4f8, 0xc3cbd9, 0x8a93a6),
    lead: control(0xe3eaf6, 0x8fa3c8, 0x141c33),
    glow: c(0x16306c),
    glow_halo: c(0x1f4597),
    bar: c(0x16295a),
    bar_ink: c(0xeef2fa),
    bar_muted: c(0xa9b6d3),
    bar_accent: c(0xbcd0f7),
    bar_error: c(0xffb4ab),
    bar_line: c(0x0d1c40),
    bar_rule: 3.,
    bar_button: control(0x22386e, 0x5f78b0, 0xeef2fa),
    bar_button_hover: c(0x2d4686),
    bar_off: control(0x1a2d60, 0x3a5288, 0x7d8db3),
    log_wash: ca(0x2e4f93, 0.26),
    log_head: c(0x2a4b8f),
    log_head_now: c(0x0f2558),
    title_ink: c(0xffffff),
    title_shadow: ca(0x050a1e, 0.55),
    card_ink: c(0x1b1e29),
    demand: c(0x8a1b27),
    peek_edge: c(0x98a3b6),
    sets: LIGHT_SETS,
    back_edge: ca(0x98a3b6, 0.95),
    enamel: Enamel {
        red: RED,
        yellow: YELLOW,
        green: GREEN,
        blue: BLUE,
        purple: PURPLE,
        rim: hex(0x6e7a90),
        rim_dark: ea(0xffffff, 0.55),
        rim_light: ea(0xffffff, 0.75),
        band_edge: hex(0xd3dae6),
        sheen: 0.55,
        tile_rim: hex(0xe3e8f0),
        tile_inner: ea(0x0c1630, 0.45),
        glyph: hex(0xfdfdff),
        glyph_edge: hex(0x1b2440),
        disc_light: hex(0x34466f),
        disc: hex(0x17213d),
        disc_rim: hex(0xb5bfd0),
        disc_ink: hex(0xeef2fa),
        hex: hex(0x18213a),
        hex_rim: hex(0xe8edf5),
        age: [hex(0xffffff), hex(0xf1f4f9), hex(0xbdc6d4)],
        age_rim: ea(0x121c38, 0.45),
    },
    ground: Ground {
        pattern: Pattern::Ice,
        table: hex(0xe4e9f0),
        ink: ea(0x264896, 0.13),
        wash: Some(Wash {
            color: ea(0xffffff, 0.6),
            center: (0.5, 0.),
            radius: (0.78, 0.46),
        }),
    },
};

static WASHI: Palette = Palette {
    dark: false,
    table: c(0xe8e2d5),
    backdrop: c(0xd6cebf),
    paper: c(0xf9f6ef),
    strip: c(0xf1ebe0),
    hover: c(0xe7ddcc),
    line: c(0x3a302a),
    wire: ca(0x744e32, 0.30),
    panel_shadow: ca(0x3c2814, 0.10),
    shadow: ca(0x28190a, 0.25),
    scrim: ca(0x1e140a, 0.55),
    card_shadow: ca(0x2b241f, 0.35),
    ink: c(0x1f1915),
    muted: c(0x675c53),
    label: c(0x6e4630),
    accent: c(0x74452a),
    dim: c(0xb8ae9f),
    chip_ink: c(0x6e4630),
    error: c(0xa8322a),
    you: c(0x22508f),
    rival: c(0xa8322a),
    selection: c(0xe2cdb4),
    primary: control(0x5c3a24, 0x3d2516, 0xfbf3e6),
    primary_hover: c(0x74492e),
    primary_off: control(0xcfc5b5, 0xcfc5b5, 0x7d7266),
    secondary_off: control(0xf4efe6, 0xcfc5b5, 0x968b7e),
    lead: control(0xefe5d4, 0xa8896c, 0x1f1915),
    glow: c(0x1f1915),
    glow_halo: c(0x744828),
    bar: c(0x261b14),
    bar_ink: c(0xf3ede2),
    bar_muted: c(0xb9ab9b),
    bar_accent: c(0xd6a878),
    bar_error: c(0xffb4a8),
    bar_line: c(0x7a5034),
    bar_rule: 3.,
    bar_button: control(0x34261d, 0x7a5a40, 0xf3ede2),
    bar_button_hover: c(0x463427),
    bar_off: control(0x2c2018, 0x4e3a2b, 0x8a7a6a),
    log_wash: ca(0x744828, 0.24),
    log_head: c(0x6e4630),
    log_head_now: c(0x3d2516),
    title_ink: c(0xfffaf0),
    title_shadow: ca(0x140a05, 0.45),
    card_ink: c(0x231c16),
    demand: c(0x9a2216),
    peek_edge: c(0x2b241f),
    sets: LIGHT_SETS,
    back_edge: ca(0x2b241f, 0.9),
    enamel: Enamel {
        // Traditional Japanese pigments, each keeping its colour's hue.
        red: [hex(0xcf4a3c), hex(0xa3271e), hex(0x5c120c), hex(0xefc3b8)],
        yellow: [hex(0xdba52a), hex(0x8c6205), hex(0x5a3f04), hex(0xf2db9c)],
        green: [hex(0x4f9a72), hex(0x286a49), hex(0x0f3523), hex(0xc4decb)],
        blue: [hex(0x4a6fae), hex(0x1f3f7a), hex(0x0c1f45), hex(0xc5d1e7)],
        purple: [hex(0x8a5ca8), hex(0x5a3377), hex(0x2c1340), hex(0xd9c8e3)],
        rim: hex(0x2b241f),
        rim_dark: ea(0xfffaf0, 0.65),
        rim_light: ea(0x2b241f, 0.25),
        band_edge: hex(0x2b241f),
        sheen: 0.16,
        tile_rim: hex(0x2b241f),
        tile_inner: ea(0xffffff, 0.22),
        glyph: hex(0xfffaf0),
        glyph_edge: hex(0x2b241f),
        disc_light: hex(0x3e352e),
        disc: hex(0x221c18),
        disc_rim: hex(0xa07448),
        disc_ink: hex(0xf6efe2),
        hex: hex(0x231d19),
        hex_rim: hex(0xf2ece0),
        age: [hex(0xffffff), hex(0xf8f3e9), hex(0xddd3c2)],
        age_rim: hex(0x2b241f),
    },
    ground: Ground {
        pattern: Pattern::Waves,
        table: hex(0xe8e2d5),
        ink: ea(0x68462c, 0.13),
        wash: None,
    },
};

static LAPIS: Palette = Palette {
    dark: true,
    table: c(0x0c1636),
    backdrop: c(0x050a1c),
    paper: c(0x121f47),
    strip: c(0x1a2a5a),
    hover: c(0x24376e),
    line: c(0xb8923e),
    wire: ca(0xdeb85c, 0.30),
    panel_shadow: ca(0x00000a, 0.45),
    shadow: ca(0x00000a, 0.5),
    scrim: ca(0x00000a, 0.6),
    card_shadow: ca(0x00000c, 0.9),
    ink: c(0xf2ead4),
    muted: c(0xaab3cc),
    label: c(0xd9bd74),
    accent: c(0xefc65f),
    dim: c(0x56618a),
    chip_ink: c(0xcfc6ad),
    error: c(0xff9584),
    you: c(0x9ec3ff),
    rival: c(0xff9584),
    selection: c(0x5a4a26),
    primary: control(0xf4ead0, 0xefc65f, 0x0c1636),
    primary_hover: c(0xffffff),
    primary_off: control(0x4a557a, 0x4a557a, 0x8c95b3),
    secondary_off: control(0x142150, 0x3a4670, 0x6a759b),
    lead: control(0x22336a, 0x9aa6c8, 0xf2ead4),
    glow: c(0xfff4d6),
    glow_halo: c(0xffecbe),
    bar: c(0x070e27),
    bar_ink: c(0xefe6cf),
    bar_muted: c(0xaab3cc),
    bar_accent: c(0xefc65f),
    bar_error: c(0xff9584),
    bar_line: c(0xb8923e),
    bar_rule: 1.,
    bar_button: control(0x1a2a5a, 0xb8923e, 0xefe6cf),
    bar_button_hover: c(0x24376e),
    bar_off: control(0x142150, 0x3a4670, 0x6a759b),
    log_wash: ca(0xe2be64, 0.40),
    log_head: c(0xf0d48a),
    log_head_now: c(0xfff1c9),
    title_ink: c(0xfff3cf),
    title_shadow: ca(0x000000, 0.55),
    card_ink: c(0x2a2117),
    demand: c(0x8a1b27),
    peek_edge: c(0xc9a24e),
    sets: DARK_SETS,
    back_edge: ca(0xe2be64, 0.9),
    enamel: Enamel {
        red: RED,
        yellow: YELLOW,
        green: GREEN,
        blue: BLUE,
        purple: PURPLE,
        rim: hex(0xe3bf66),
        rim_dark: ea(0x1e1204, 0.55),
        rim_light: ea(0xf0d68c, 0.6),
        band_edge: hex(0xe3bf66),
        sheen: 0.5,
        tile_rim: hex(0xecc96f),
        tile_inner: ea(0x1e1204, 0.5),
        glyph: hex(0xfff3cf),
        glyph_edge: hex(0x6b4d12),
        disc_light: hex(0x2b3c7c),
        disc: hex(0x101a44),
        disc_rim: hex(0xe3bf66),
        disc_ink: hex(0xf6edd5),
        hex: hex(0x0b1230),
        hex_rim: hex(0xf1e8d0),
        age: [hex(0xfffaf0), hex(0xf1e8d0), hex(0xbfb093)],
        age_rim: ea(0x000000, 0.5),
    },
    ground: Ground {
        pattern: Pattern::Stars,
        table: hex(0x0c1636),
        ink: ea(0xe2be64, 0.11),
        wash: Some(Wash {
            color: ea(0x3c60d2, 0.30),
            center: (0.5, 0.38),
            radius: (0.61, 0.5),
        }),
    },
};

#[cfg(test)]
mod tests {
    use super::super::faces::COLORS;
    use super::*;

    fn luminance(c: Color) -> f32 {
        let f = |v: f32| {
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
    }

    /// WCAG 2 contrast of `fg` composited over the opaque `bg`.
    fn contrast(fg: Color, bg: Color) -> f32 {
        let a = fg.a;
        let over = Color::new(
            fg.r * a + bg.r * (1. - a),
            fg.g * a + bg.g * (1. - a),
            fg.b * a + bg.b * (1. - a),
            1.,
        );
        let (x, y) = (luminance(over), luminance(bg));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_theme_keeps_text_legible() {
        for theme in Theme::ALL {
            let p = theme.palette();
            let e = &p.enamel;
            let mut text = vec![
                ("text on panels", p.ink, p.paper),
                ("secondary text", p.muted, p.paper),
                ("labels", p.label, p.paper),
                ("accent", p.accent, p.paper),
                ("chip names", p.chip_ink, p.strip),
                ("you", p.you, p.paper),
                ("rival", p.rival, p.paper),
                ("errors", p.error, p.paper),
                ("text on the table", p.ink, p.table),
                ("buttons", p.ink, p.strip),
                ("hovered buttons", p.ink, p.hover),
                ("primary buttons", p.primary.ink, p.primary.fill),
                ("hovered primary", p.primary.ink, p.primary_hover),
                ("leading cells", p.lead.ink, p.lead.fill),
                ("selected text", p.ink, p.selection),
                ("top bar", p.bar_ink, p.bar),
                ("top bar status", p.bar_accent, p.bar),
                ("top bar muted", p.bar_muted, p.bar),
                ("top bar error", p.bar_error, p.bar),
                ("bar buttons", p.bar_button.ink, p.bar_button.fill),
                ("log headings", p.log_head, p.paper),
                ("current log heading", p.log_head_now, p.paper),
            ];
            for set in p.sets {
                text.push(("printing on card backs", set.on_back, set.back));
                text.push(("deck counts", set.ink, p.paper));
            }
            for color in COLORS {
                text.push(("colour names", p.hue(color), p.paper));
            }
            for card in [e.red, e.yellow, e.green, e.blue, e.purple] {
                text.push(("card text", p.card_ink, rgba(card[3])));
                text.push(("I DEMAND", p.demand, rgba(card[3])));
                text.push(("card titles and chips", p.title_ink, rgba(card[1])));
                text.push(("age numerals", rgba(card[2]), rgba(e.age[1])));
            }
            for (role, fg, bg) in text {
                let ratio = contrast(fg, bg);
                assert!(ratio >= 4.5, "{theme:?} {role}: {ratio:.2}");
            }
            for (role, fg, bg) in [
                ("legal outline", p.glow, p.table),
                ("card rim", rgba(e.rim), p.table),
                ("panel wire", p.line, p.table),
            ] {
                let ratio = contrast(fg, bg);
                assert!(ratio >= 3., "{theme:?} {role}: {ratio:.2}");
            }
        }
    }

    #[test]
    fn set_backs_follow_the_printed_set_colours() {
        // Hue in degrees and saturation, HSV.
        let hsv = |c: Color| {
            let max = c.r.max(c.g).max(c.b);
            let min = c.r.min(c.g).min(c.b);
            let delta = max - min;
            let sector = if delta == 0. {
                0.
            } else if max == c.r {
                (c.g - c.b) / delta
            } else if max == c.g {
                (c.b - c.r) / delta + 2.
            } else {
                (c.r - c.g) / delta + 4.
            };
            ((sector * 60.).rem_euclid(360.), delta / max)
        };
        let near = |hue: f32, target: f32| {
            let d = (hue - target).abs();
            d.min(360. - d) <= 20.
        };
        for theme in Theme::ALL {
            let p = theme.palette();
            for (set, target) in [
                (Set::Base, 38.),
                (Set::Echoes, 217.),
                (Set::Figures, 128.),
                (Set::Cities, 358.),
                (Set::Artifacts, 280.),
            ] {
                let (hue, saturation) = hsv(p.set(set).back);
                assert!(near(hue, target), "{theme:?} {set:?} back hue {hue:.0}");
                assert!(saturation > 0.3, "{theme:?} {set:?} back is too gray");
            }
            // Base backs are tan: light, with dark printing.
            assert!(luminance(p.set(Set::Base).back) > 0.35);
            // Unseen backs are gray.
            assert!(hsv(p.set(Set::Unseen).back).1 < 0.2);
        }
    }

    #[test]
    fn theme_names_parse_back() {
        for theme in Theme::ALL {
            assert_eq!(Theme::parse(theme.name()), Some(theme));
            let json = serde_json::to_string(&theme).unwrap();
            assert_eq!(serde_json::from_str::<Theme>(&json).unwrap(), theme);
        }
        assert_eq!(Theme::parse("Cloisonne"), Some(Theme::Cloisonne));
        assert_eq!(Theme::parse("inari"), None);
        assert_eq!(
            serde_json::from_str::<Theme>("\"inari\"").unwrap(),
            Theme::Cloisonne
        );
    }
}
