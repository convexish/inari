//! Procedural art for the table. A small anti-aliased path rasterizer turns
//! the icon geometry below into RGBA sprites, so the app ships no card or icon
//! images. Geometry uses a 48x48 grid, as in the HTML prototype the design
//! came from. Colours come from the theme's `Enamel` and `Ground`.

use std::f32::consts::{PI, TAU};

use inno_engine::{Color as CardColor, Icon};

use super::faces::{CityIcon, ICONS};

pub type Rgba = [f32; 4];
type Pt = (f32, f32);

pub const fn hex(rgb: u32) -> Rgba {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.,
        ((rgb >> 8) & 0xff) as f32 / 255.,
        (rgb & 0xff) as f32 / 255.,
        1.,
    ]
}
pub const fn alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], a]
}

// ---------------------------------------------------------------- palette

/// A theme's card and tile colours.
pub struct Enamel {
    /// Card colours as (light, band, dark, tint): band shades and the pale body.
    pub red: [Rgba; 4],
    pub yellow: [Rgba; 4],
    pub green: [Rgba; 4],
    pub blue: [Rgba; 4],
    pub purple: [Rgba; 4],
    /// The card's outer rim, then the two hairlines inside it.
    pub rim: Rgba,
    pub rim_dark: Rgba,
    pub rim_light: Rgba,
    /// Wire along the edges of the title band, rails and bottom band.
    pub band_edge: Rgba,
    /// Strength of the white sheen on the card body.
    pub sheen: f32,
    /// Icon tiles: rim, inner hairline, glyph fill and glyph outline.
    pub tile_rim: Rgba,
    pub tile_inner: Rgba,
    pub glyph: Rgba,
    pub glyph_edge: Rgba,
    /// Dark tiles for city icons, echoes and bonuses: highlight, base, rim, ink.
    pub disc_light: Rgba,
    pub disc: Rgba,
    pub disc_rim: Rgba,
    pub disc_ink: Rgba,
    pub hex: Rgba,
    pub hex_rim: Rgba,
    /// Age disc highlight, body and shade, and its rim.
    pub age: [Rgba; 3],
    pub age_rim: Rgba,
}

impl Enamel {
    pub fn card(&self, c: CardColor) -> [Rgba; 4] {
        match c {
            CardColor::Red => self.red,
            CardColor::Yellow => self.yellow,
            CardColor::Green => self.green,
            CardColor::Blue => self.blue,
            CardColor::Purple => self.purple,
        }
    }
}

/// The repeating texture printed on the table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pattern {
    /// Diagonal gold lattice.
    Lattice,
    /// Cracked-ice lattice from blue-and-white porcelain.
    Ice,
    /// Eight-point stars joined into a lattice.
    Stars,
    /// Overlapping wave fans (seigaiha).
    Waves,
}

/// A soft radial wash over the whole table: colour, centre and radii as
/// fractions of the table.
pub struct Wash {
    pub color: Rgba,
    pub center: (f32, f32),
    pub radius: (f32, f32),
}

pub struct Ground {
    pub pattern: Pattern,
    pub table: Rgba,
    pub ink: Rgba,
    pub wash: Option<Wash>,
}

/// Icon enamel as (light, base, dark). Shared by every theme, so an icon
/// looks the same everywhere.
pub fn icon_palette(icon: Icon) -> [Rgba; 3] {
    match icon {
        Icon::Authority => [hex(0xa3a9b1), hex(0x5d636c), hex(0x30343a)],
        Icon::Concept => [hex(0xb27fd2), hex(0x6e3d8e), hex(0x3a1c4e)],
        Icon::Health => [hex(0x5fc08f), hex(0x1f7a52), hex(0x0c3d28)],
        Icon::Prosperity => [hex(0xf6d77c), hex(0xc8951f), hex(0x7a5608)],
        Icon::Industry => [hex(0xe86f6f), hex(0xa8262f), hex(0x5a0e16)],
        Icon::Efficiency => [hex(0x74a0ea), hex(0x2952a3), hex(0x132a5e)],
        Icon::Avatar => [hex(0x62c9cc), hex(0x1f8487), hex(0x0c4446)],
        Icon::Food => [hex(0xb6bd6c), hex(0x737d2c), hex(0x394315)],
    }
}

// --------------------------------------------------------------- geometry

fn circle(cx: f32, cy: f32, r: f32) -> String {
    format!(
        "M{},{} a{r},{r} 0 1,0 {},0 a{r},{r} 0 1,0 {},0 Z",
        cx - r,
        cy,
        2. * r,
        -2. * r
    )
}

fn seg(a: Pt, b: Pt) -> String {
    format!("M{},{} L{},{}", a.0, a.1, b.0, b.1)
}

fn ticks(cx: f32, cy: f32, r1: f32, r2: f32, n: usize) -> String {
    (0..n)
        .map(|i| {
            let a = TAU * i as f32 / n as f32 - PI / 2.;
            seg(
                (cx + r1 * a.cos(), cy + r1 * a.sin()),
                (cx + r2 * a.cos(), cy + r2 * a.sin()),
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn hand(cx: f32, cy: f32, degrees: f32, length: f32) -> String {
    let a = (degrees - 90.).to_radians();
    seg((cx, cy), (cx + length * a.cos(), cy + length * a.sin()))
}

/// Standard icon silhouettes: an even-odd body and detail strokes cut into it.
fn icon_shape(icon: Icon) -> (String, String) {
    match icon {
        Icon::Food => food_shape(),
        Icon::Authority => (
            "M7,42 V10 H10.5 V14 H14.5 V10 H18 V21 H19.5 V17 H22.5 V21 \
             H25.5 V17 H28.5 V21 H30 V10 H33.5 V14 H37.5 V10 H41 V42 Z \
             M20.5,42 V33.5 A3.5,3.5 0 0 1 27.5,33.5 V42 Z \
             M11.3,19 H13.7 V25.5 H11.3 Z M34.3,19 H36.7 V25.5 H34.3 Z"
                .into(),
            String::new(),
        ),
        Icon::Concept => (
            "M24,5.5 C31.5,5.5 37,11 37,18.5 C37,23.5 34,26.6 31.8,29.4 \
             C30.6,31 30.3,32.2 30.3,33.5 H17.7 C17.7,32.2 17.4,31 16.2,29.4 \
             C14,26.6 11,23.5 11,18.5 C11,11 16.5,5.5 24,5.5 Z \
             M17.5,35.3 H30.5 V38.2 H17.5 Z M18.5,39.8 H29.5 V42.6 H18.5 Z \
             M21,44 H27 L26,45.8 H22 Z"
                .into(),
            "M20,29 V22 L22,24.5 L24,21.5 L26,24.5 L28,22 V29".into(),
        ),
        Icon::Health => (
            "M9.5,40.5 C8,25 18,9.5 41,7 C41.5,28 29,40.5 11.5,40.8 Z \
             M10.2,40.2 L5.8,44.6 L7.6,46.2 L12,41.8 Z"
                .into(),
            "M12.5,38.5 C19,30 27,21 37.5,10.5 \
             M20,30.2 L18.6,22.5 M25.8,24 L25,15.5 M31.5,17.8 L31.6,11.6 \
             M20.4,30 L28.5,30.6 M26,23.8 L34.2,23.2"
                .into(),
        ),
        Icon::Prosperity => (
            format!(
                "M10,32 L6,15.5 L16.5,23.5 L24,10.5 L31.5,23.5 L42,15.5 L38,32 Z {} {} {} \
                 M9.5,34.5 H38.5 V40.5 H9.5 Z",
                circle(6., 13.2, 3.),
                circle(24., 8.2, 3.2),
                circle(42., 13.2, 3.)
            ),
            "M16,37.5 h0.01 M24,37.5 h0.01 M32,37.5 h0.01".into(),
        ),
        Icon::Industry => (
            format!(
                "M5,42 V25 L13.5,18.5 V25 L22,18.5 V25 L30.5,18.5 V25 H33 V12.5 H39.5 V42 Z \
                 M9,30 H13 V34 H9 Z M17,30 H21 V34 H17 Z M25,30 H29 V34 H25 Z \
                 M17,37 H21 V42 H17 Z {} {} {}",
                circle(37.5, 7.2, 2.6),
                circle(32., 5.6, 1.9),
                circle(27.6, 6.6, 1.3)
            ),
            String::new(),
        ),
        Icon::Efficiency => (
            format!(
                "{} M21.5,5.5 H26.5 V9.8 H21.5 Z M22.6,8.5 H25.4 V10.5 H22.6 Z",
                circle(24., 26., 16.5)
            ),
            format!(
                "{} {} {} M24,26 h0.01",
                ticks(24., 26., 12., 14.2, 12),
                hand(24., 26., 300., 7.2),
                hand(24., 26., 60., 10.2)
            ),
        ),
        Icon::Avatar => (
            format!(
                "{} M8,43 C8,31 15,25.5 24,25.5 C33,25.5 40,31 40,43 Z",
                circle(24., 15., 8.)
            ),
            String::new(),
        ),
    }
}

/// Placeholder Food emblem: an upright ear of wheat, in olive enamel rather
/// than Health's emerald or Prosperity's gold.
fn food_shape() -> (String, String) {
    (
        "M22.5,17 H25.5 V43 H22.5 Z \
         M24,4 C17,10 19,15 24,18 C29,15 31,10 24,4 Z \
         M10,13 C18,13 21,17 21,23 C14,23 10,19 10,13 Z \
         M38,13 C30,13 27,17 27,23 C34,23 38,19 38,13 Z \
         M10,24 C18,24 21,28 21,34 C14,34 10,30 10,24 Z \
         M38,24 C30,24 27,28 27,34 C34,34 38,30 38,24 Z"
            .into(),
        String::new(),
    )
}

/// Line glyphs: (stroke path, stroke width, optional solid fill path).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Glyph {
    /// Index into `CityIcon::ALL`.
    City(u8),
    /// 1 left, 2 right, 3 up, 4 aslant.
    Splay(u8),
    Achievement,
    Score,
    /// A hand of cards, for the hand count in player summaries.
    Hand,
    Forecast,
    Safe,
    Museum,
    Junk,
    Echo,
    Echoes,
    Cities,
    Unseen,
    Artifacts,
    /// Portrait cameo: the Figures set emblem.
    Figures,
    /// Returning arrows: a karma paragraph.
    Karma,
    /// Sealed scroll: a Decree paragraph.
    Decree,
}

impl Glyph {
    pub fn city(c: CityIcon) -> Self {
        Self::City(CityIcon::ALL.iter().position(|&x| x == c).unwrap_or(0) as u8)
    }
}

fn glyph_shape(g: Glyph) -> (String, f32, String) {
    let s = |d: &str, w: f32| (d.to_owned(), w, String::new());
    match g {
        Glyph::City(i) => match CityIcon::ALL[usize::from(i) % CityIcon::ALL.len()] {
            CityIcon::Plus => s("M24,11 V37 M11,24 H37", 5.2),
            CityIcon::Search => (
                format!("{} M28,28 L38.5,38.5", circle(20.5, 20.5, 10.)),
                4.4,
                String::new(),
            ),
            CityIcon::Flag => (
                "M13,42 V7".into(),
                3.6,
                "M14.5,8.5 C20,5.5 25,12 36,8.5 V25 C25,28.5 20,22 14.5,25 Z".into(),
            ),
            CityIcon::Fountain => (
                "M24,31 V20 M24,17 V9 M24,10 C20,6.5 16.5,9 16,14 M24,10 C28,6.5 31.5,9 32,14 \
                 M14.5,21 C10.5,23 9.5,27 9.5,31 M33.5,21 C37.5,23 38.5,27 38.5,31"
                    .into(),
                3.,
                "M7,33 H41 C41,39.5 34,43 24,43 C14,43 7,39.5 7,33 Z \
                 M14,18.5 H34 C34,22.5 29.5,25 24,25 C18.5,25 14,22.5 14,18.5 Z"
                    .into(),
            ),
            CityIcon::Junk => glyph_shape(Glyph::Junk),
            CityIcon::Left => glyph_shape(Glyph::Splay(1)),
            CityIcon::Right => glyph_shape(Glyph::Splay(2)),
            CityIcon::Up => glyph_shape(Glyph::Splay(3)),
            CityIcon::Unsplay => s(
                "M24,9 V39 M6,24 H18 M13,18.5 L18.5,24 L13,29.5 \
                 M42,24 H30 M35,18.5 L29.5,24 L35,29.5",
                3.8,
            ),
            CityIcon::Uplift => s("M24,41 V17 M15,26 L24,17 L33,26 M11,8.5 H37", 4.4),
        },
        Glyph::Splay(1) => s("M38,24 H11 M20,14.5 L10.5,24 L20,33.5", 4.6),
        Glyph::Splay(2) => s("M10,24 H37 M28,14.5 L37.5,24 L28,33.5", 4.6),
        Glyph::Splay(3) => s("M24,39 V11 M14.5,20.5 L24,11 L33.5,20.5", 4.6),
        Glyph::Splay(_) => s("M12,36 L35,13 M22,12.5 H35.5 V26", 4.6),
        Glyph::Achievement => (
            String::new(),
            0.,
            "M24,5 L29.3,17.6 L43,18.6 L32.6,27.4 L35.8,40.8 L24,33.6 L12.2,40.8 \
             L15.4,27.4 L5,18.6 L18.7,17.6 Z"
                .into(),
        ),
        Glyph::Score => (
            format!("{} {}", circle(24., 24., 16.), circle(24., 24., 10.5)),
            3.4,
            String::new(),
        ),
        Glyph::Hand => s("M11,13 L27,9 L33,35 L17,39 Z M20,12 H37 V40 H20 Z", 3.2),
        Glyph::Forecast => (
            format!(
                "{} M14,41 H34 M17,41 L19.5,33 M31,41 L28.5,33",
                circle(24., 21., 13.)
            ),
            3.2,
            circle(19.5, 16.5, 3.2),
        ),
        Glyph::Safe => (
            format!(
                "M8,9 H40 V39 H8 Z M13,39 V43 M35,39 V43 {} \
                 M24,16.5 V19 M24,29 V31.5 M16.5,24 H19 M29,24 H31.5",
                circle(24., 24., 7.5)
            ),
            3.2,
            String::new(),
        ),
        Glyph::Museum => s(
            "M7,19 L24,8 L41,19 Z M12,23 V36 M20,23 V36 M28,23 V36 M36,23 V36 M7,40 H41",
            3.4,
        ),
        Glyph::Junk => s(
            "M10,15 H38 M19.5,15 V10.5 H28.5 V15 M14,15 L16.3,41 H31.7 L34,15 M21,21 V35 M27,21 V35",
            3.2,
        ),
        Glyph::Echo => (
            "M14,14 C9,20 9,28 14,34 M21,10 C14,18 14,30 21,38 M28,6 C19,16 19,32 28,42".into(),
            3.6,
            circle(36., 24., 4.6),
        ),
        Glyph::Echoes => s(
            "M17,14 C12,20 12,28 17,34 M25,10 C17,18 17,30 25,38 M33,6 C22,16 22,32 33,42",
            4.,
        ),
        Glyph::Cities => (
            String::new(),
            0.,
            "M6,42 V24 H13 V16 H20 V24 H22 V8 H31 V24 H35 V18 H42 V42 Z".into(),
        ),
        Glyph::Unseen => (
            "M5,24 C12,13 36,13 43,24 C36,35 12,35 5,24 Z M10,40 L38,8".into(),
            3.6,
            circle(24., 24., 5.5),
        ),
        Glyph::Figures => (
            "M24,5 C3,5 3,39 24,39 C45,39 45,5 24,5 Z M14,44 H34".into(),
            2.8,
            format!("{} M14,33 C14,22 34,22 34,33 Z", circle(24., 16., 6.)),
        ),
        Glyph::Karma => s(
            "M10,22 A14,14 0 0 1 35,14 L39,19 M39,9 V19 H29 \
             M38,26 A14,14 0 0 1 13,34 L9,29 M9,39 V29 H19",
            3.6,
        ),
        Glyph::Decree => (
            format!(
                "M12,8 H36 V28 M12,8 C4,8 4,16 12,16 V38 H25 \
                 M12,8 V16 M18,16 H30 M18,23 H30 {}",
                circle(34., 33., 6.)
            ),
            2.8,
            "M30,39 L28,46 L34,43 L40,46 L38,39 Z".into(),
        ),
        Glyph::Artifacts => (
            "M20,11.5 C13,11.5 12.5,17 15,21 M28,11.5 C35,11.5 35.5,17 33,21".into(),
            3.4,
            "M24,45 C16.5,38.5 13,31 14,23 C14.7,18.5 18,16 20.5,14.5 V10.5 H18.5 V6.5 \
             H29.5 V10.5 H27.5 V14.5 C30,16 33.3,18.5 34,23 C35,31 31.5,38.5 24,45 Z"
                .into(),
        ),
    }
}

// ------------------------------------------------------------- path parser

#[derive(Debug, Clone)]
struct Sub {
    pts: Vec<Pt>,
    closed: bool,
}

fn tokens(d: &str) -> Vec<Result<char, f32>> {
    let b = d.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() && c != 'e' {
            out.push(Ok(c));
            i += 1;
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut dot = c == '.';
            while i < b.len() {
                let n = b[i] as char;
                if n.is_ascii_digit() || (n == '.' && !dot) {
                    dot |= n == '.';
                    i += 1;
                } else if n == 'e' {
                    i += 1;
                    if i < b.len() && b[i] == b'-' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            out.push(Err(d[start..i].parse().unwrap_or(0.)));
        } else {
            i += 1;
        }
    }
    out
}

fn arc(p0: Pt, r: Pt, phi: f32, large: bool, sweep: bool, p1: Pt, out: &mut Vec<Pt>) {
    let (mut rx, mut ry) = (r.0.abs(), r.1.abs());
    if rx < 1e-6 || ry < 1e-6 {
        out.push(p1);
        return;
    }
    let (s, c) = phi.to_radians().sin_cos();
    let (dx, dy) = ((p0.0 - p1.0) / 2., (p0.1 - p1.1) / 2.);
    let (x1, y1) = (c * dx + s * dy, -s * dx + c * dy);
    let lambda = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lambda > 1. {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let k = (num / den).max(0.).sqrt() * if large == sweep { -1. } else { 1. };
    let (cxp, cyp) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let cx = c * cxp - s * cyp + (p0.0 + p1.0) / 2.;
    let cy = s * cxp + c * cyp + (p0.1 + p1.1) / 2.;
    let angle = |u: Pt, v: Pt| (u.0 * v.1 - u.1 * v.0).atan2(u.0 * v.0 + u.1 * v.1);
    let u = ((x1 - cxp) / rx, (y1 - cyp) / ry);
    let v = ((-x1 - cxp) / rx, (-y1 - cyp) / ry);
    let t0 = angle((1., 0.), u);
    let mut dt = angle(u, v);
    if !sweep && dt > 0. {
        dt -= TAU;
    } else if sweep && dt < 0. {
        dt += TAU;
    }
    let n = ((dt.abs() / (PI / 24.)).ceil() as usize).max(4);
    for i in 1..=n {
        let t = t0 + dt * i as f32 / n as f32;
        let (st, ct) = t.sin_cos();
        out.push((
            cx + rx * ct * c - ry * st * s,
            cy + rx * ct * s + ry * st * c,
        ));
    }
}

/// Flattens an SVG path (M L H V C A Z, absolute or relative) to polylines.
fn parse(d: &str) -> Vec<Sub> {
    let toks = tokens(d);
    let mut subs: Vec<Sub> = Vec::new();
    let mut cur: Pt = (0., 0.);
    let mut start = cur;
    let mut i = 0;
    let mut cmd = 'M';
    let num = |i: &mut usize| -> Option<f32> {
        match toks.get(*i) {
            Some(Err(v)) => {
                *i += 1;
                Some(*v)
            }
            _ => None,
        }
    };
    while i < toks.len() {
        if let Ok(c) = toks[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                if let Some(s) = subs.last_mut() {
                    s.closed = true;
                }
                cur = start;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { (0., 0.) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                cur = (base.0 + x, base.1 + y);
                start = cur;
                subs.push(Sub {
                    pts: vec![cur],
                    closed: false,
                });
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                cur = (base.0 + x, base.1 + y);
                if let Some(s) = subs.last_mut() {
                    s.pts.push(cur);
                }
            }
            'H' => {
                let Some(x) = num(&mut i) else { break };
                cur = (if rel { cur.0 + x } else { x }, cur.1);
                if let Some(s) = subs.last_mut() {
                    s.pts.push(cur);
                }
            }
            'V' => {
                let Some(y) = num(&mut i) else { break };
                cur = (cur.0, if rel { cur.1 + y } else { y });
                if let Some(s) = subs.last_mut() {
                    s.pts.push(cur);
                }
            }
            'C' => {
                let mut v = [0f32; 6];
                for slot in &mut v {
                    let Some(x) = num(&mut i) else { return subs };
                    *slot = x;
                }
                let p0 = cur;
                let p1 = (base.0 + v[0], base.1 + v[1]);
                let p2 = (base.0 + v[2], base.1 + v[3]);
                let p3 = (base.0 + v[4], base.1 + v[5]);
                if let Some(s) = subs.last_mut() {
                    for k in 1..=16 {
                        let t = k as f32 / 16.;
                        let u = 1. - t;
                        let (a, b, c2, d2) = (u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t);
                        s.pts.push((
                            a * p0.0 + b * p1.0 + c2 * p2.0 + d2 * p3.0,
                            a * p0.1 + b * p1.1 + c2 * p2.1 + d2 * p3.1,
                        ));
                    }
                }
                cur = p3;
            }
            'A' => {
                let mut v = [0f32; 7];
                for slot in &mut v {
                    let Some(x) = num(&mut i) else { return subs };
                    *slot = x;
                }
                let end = (base.0 + v[5], base.1 + v[6]);
                if let Some(s) = subs.last_mut() {
                    arc(
                        cur,
                        (v[0], v[1]),
                        v[2],
                        v[3] != 0.,
                        v[4] != 0.,
                        end,
                        &mut s.pts,
                    );
                }
                cur = end;
            }
            _ => i += 1,
        }
    }
    subs
}

// --------------------------------------------------------------- raster

/// Premultiplied RGBA canvas with straight-alpha export.
pub struct Pixmap {
    pub w: usize,
    pub h: usize,
    px: Vec<Rgba>,
}

/// Coverage per pixel, 0..1.
struct Mask(Vec<f32>);

const SUB: usize = 5;

fn signed_area(p: &[Pt]) -> f32 {
    let mut a = 0.;
    for i in 0..p.len() {
        let (x0, y0) = p[i];
        let (x1, y1) = p[(i + 1) % p.len()];
        a += x0 * y1 - x1 * y0;
    }
    a / 2.
}

fn disc_poly(c: Pt, r: f32) -> Vec<Pt> {
    let n = ((r * 4.) as usize).clamp(10, 40);
    (0..n)
        .map(|i| {
            let a = TAU * i as f32 / n as f32;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

impl Pixmap {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            px: vec![[0.; 4]; w * h],
        }
    }

    fn rasterize(&self, polys: &[Vec<Pt>], even_odd: bool) -> Mask {
        let mut cov = vec![0f32; self.w * self.h];
        let edges: Vec<(Pt, Pt)> = polys
            .iter()
            .filter(|p| p.len() > 2)
            .flat_map(|p| (0..p.len()).map(move |i| (p[i], p[(i + 1) % p.len()])))
            .filter(|(a, b)| (a.1 - b.1).abs() > 1e-7)
            .collect();
        let wgt = 1. / SUB as f32;
        let mut xs: Vec<(f32, i32)> = Vec::new();
        for row in 0..self.h {
            let line = &mut cov[row * self.w..(row + 1) * self.w];
            for s in 0..SUB {
                let y = row as f32 + (s as f32 + 0.5) / SUB as f32;
                xs.clear();
                for &((x0, y0), (x1, y1)) in &edges {
                    if (y0 <= y && y < y1) || (y1 <= y && y < y0) {
                        let t = (y - y0) / (y1 - y0);
                        xs.push((x0 + t * (x1 - x0), if y1 > y0 { 1 } else { -1 }));
                    }
                }
                xs.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut wind = 0;
                for k in 0..xs.len() {
                    wind += xs[k].1;
                    let inside = if even_odd { k % 2 == 0 } else { wind != 0 };
                    if inside && k + 1 < xs.len() {
                        add_span(line, xs[k].0, xs[k + 1].0, wgt);
                    }
                }
            }
        }
        for c in &mut cov {
            *c = c.min(1.);
        }
        Mask(cov)
    }

    fn paint(&mut self, mask: &Mask, color: impl Fn(f32, f32) -> Rgba) {
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                let m = mask.0[i];
                if m <= 0. {
                    continue;
                }
                let c = color(x as f32 + 0.5, y as f32 + 0.5);
                let a = c[3] * m;
                let d = &mut self.px[i];
                for k in 0..3 {
                    d[k] = c[k] * a + d[k] * (1. - a);
                }
                d[3] = a + d[3] * (1. - a);
            }
        }
    }

    /// Straight-alpha RGBA8 bytes.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.px {
            let a = p[3];
            for &c in &p[..3] {
                let v = if a > 0. { c / a } else { 0. };
                out.push((v.clamp(0., 1.) * 255. + 0.5) as u8);
            }
            out.push((a.clamp(0., 1.) * 255. + 0.5) as u8);
        }
        out
    }
}

fn add_span(line: &mut [f32], xa: f32, xb: f32, wgt: f32) {
    let w = line.len() as f32;
    let (xa, xb) = (xa.max(0.), xb.min(w));
    if xb <= xa {
        return;
    }
    let (ia, ib) = (xa.floor() as usize, xb.floor() as usize);
    if ia == ib {
        line[ia] += (xb - xa) * wgt;
        return;
    }
    line[ia] += (ia as f32 + 1. - xa) * wgt;
    for c in &mut line[ia + 1..ib] {
        *c += wgt;
    }
    if ib < line.len() {
        line[ib] += (xb - ib as f32) * wgt;
    }
}

/// Maps the 48-unit design grid onto pixels.
#[derive(Clone, Copy)]
struct Xf {
    s: f32,
    dx: f32,
    dy: f32,
}
impl Xf {
    fn grid(size: f32, k: f32) -> Self {
        let s = size * k / 48.;
        let off = size * (1. - k) / 2.;
        Self {
            s,
            dx: off,
            dy: off,
        }
    }
    fn pt(self, p: Pt) -> Pt {
        (self.dx + p.0 * self.s, self.dy + p.1 * self.s)
    }
}

fn fill_polys(d: &str, xf: Xf) -> Vec<Vec<Pt>> {
    parse(d)
        .into_iter()
        .map(|s| s.pts.into_iter().map(|p| xf.pt(p)).collect())
        .collect()
}

/// Stroke outlines with round joins and caps, all wound the same way.
fn stroke_polys(d: &str, xf: Xf, width: f32) -> Vec<Vec<Pt>> {
    let hw = width * xf.s / 2.;
    let mut out = Vec::new();
    for sub in parse(d) {
        let mut pts: Vec<Pt> = sub.pts.into_iter().map(|p| xf.pt(p)).collect();
        if sub.closed && pts.len() > 1 {
            pts.push(pts[0]);
        }
        for p in &pts {
            out.push(disc_poly(*p, hw));
        }
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-4 {
                continue;
            }
            let (nx, ny) = (-dy / len * hw, dx / len * hw);
            out.push(vec![
                (a.0 + nx, a.1 + ny),
                (b.0 + nx, b.1 + ny),
                (b.0 - nx, b.1 - ny),
                (a.0 - nx, a.1 - ny),
            ]);
        }
    }
    for p in &mut out {
        if signed_area(p) < 0. {
            p.reverse();
        }
    }
    out
}

fn rrect_poly(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<Pt> {
    let r = r.min(w / 2.).min(h / 2.).max(0.);
    let mut out = Vec::new();
    let corners = [
        (x + w - r, y + r, -PI / 2.),
        (x + w - r, y + h - r, 0.),
        (x + r, y + h - r, PI / 2.),
        (x + r, y + r, PI),
    ];
    for (cx, cy, a0) in corners {
        for k in 0..=6 {
            let a = a0 + PI / 2. * k as f32 / 6.;
            out.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    out
}

fn hex_poly(size: f32, inset: f32) -> Vec<Pt> {
    let w = size - 2. * inset;
    let h = w * 0.866;
    let c = size / 2.;
    vec![
        (c - w / 2., c),
        (c - w / 4., c - h / 2.),
        (c + w / 4., c - h / 2.),
        (c + w / 2., c),
        (c + w / 4., c + h / 2.),
        (c - w / 4., c + h / 2.),
    ]
}

fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Piecewise-linear gradient through `stops` (position, colour).
fn ramp(stops: &[(f32, Rgba)], t: f32) -> Rgba {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            return lerp(w[0].1, w[1].1, (t - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    stops[stops.len() - 1].1
}

/// Radial gradient centred at (cx, cy) as fractions of the box, reaching the farthest
/// corner, like CSS `radial-gradient(circle at ...)`.
fn radial(w: f32, h: f32, cx: f32, cy: f32, stops: &[(f32, Rgba)]) -> impl Fn(f32, f32) -> Rgba {
    let (ox, oy) = (cx * w, cy * h);
    let far = [(0., 0.), (w, 0.), (0., h), (w, h)]
        .iter()
        .map(|(x, y): &(f32, f32)| ((x - ox).powi(2) + (y - oy).powi(2)).sqrt())
        .fold(0f32, f32::max);
    let stops = stops.to_vec();
    move |x, y| ramp(&stops, ((x - ox).powi(2) + (y - oy).powi(2)).sqrt() / far)
}

// ---------------------------------------------------------------- sprites

fn ring(pm: &mut Pixmap, outer: Vec<Pt>, inner: Vec<Pt>, color: Rgba) {
    let m = pm.rasterize(&[outer, inner], true);
    pm.paint(&m, |_, _| color);
}

fn tile_frame(pm: &mut Pixmap, size: f32, fill: impl Fn(f32, f32) -> Rgba, rim: Rgba, inner: Rgba) {
    let r = size * 0.18;
    let k = size / 36.;
    let body = rrect_poly(0., 0., size, size, r);
    let m = pm.rasterize(std::slice::from_ref(&body), false);
    pm.paint(&m, fill);
    let i1 = rrect_poly(1.5 * k, 1.5 * k, size - 3. * k, size - 3. * k, r - 1.5 * k);
    let i2 = rrect_poly(2.5 * k, 2.5 * k, size - 5. * k, size - 5. * k, r - 2.5 * k);
    ring(pm, i1.clone(), i2, inner);
    ring(pm, body, i1, rim);
}

/// Enamel icon tile with a metal rim and a pale glyph.
pub fn icon_tile(icon: Icon, px: usize, e: &Enamel) -> Pixmap {
    enamel_icon_tile(icon_palette(icon), icon_shape(icon), px, e)
}

/// Food uses the same frame, shading and glyph treatment as the other icons.
pub fn food_tile(px: usize, e: &Enamel) -> Pixmap {
    icon_tile(Icon::Food, px, e)
}

fn enamel_icon_tile(palette: [Rgba; 3], shape: (String, String), px: usize, e: &Enamel) -> Pixmap {
    let size = px as f32;
    let mut pm = Pixmap::new(px, px);
    let [light, base, dark] = palette;
    tile_frame(
        &mut pm,
        size,
        radial(
            size,
            size,
            0.3,
            0.24,
            &[(0., light), (0.55, base), (1., dark)],
        ),
        e.tile_rim,
        e.tile_inner,
    );
    draw_icon(&mut pm, shape, base, Xf::grid(size, 0.62), size < 22., e);
    pm
}

/// A bare icon for inline text: no tile, coloured glyph.
fn draw_icon(
    pm: &mut Pixmap,
    shape: (String, String),
    base: Rgba,
    xf: Xf,
    small: bool,
    e: &Enamel,
) {
    let (body, details) = shape;
    let m = pm.rasterize(&fill_polys(&body, xf), true);
    pm.paint(&m, |_, _| e.glyph);
    let edge = pm.rasterize(
        &stroke_polys(&body, xf, if small { 2.6 } else { 2. }),
        false,
    );
    pm.paint(&edge, |_, _| e.glyph_edge);
    if !details.is_empty() {
        let m = pm.rasterize(&stroke_polys(&details, xf, 2.8), false);
        pm.paint(&m, |_, _| base);
    }
}

fn glyph_into(pm: &mut Pixmap, g: Glyph, xf: Xf, color: Rgba) {
    let (stroke, width, fill) = glyph_shape(g);
    if !fill.is_empty() {
        let m = pm.rasterize(&fill_polys(&fill, xf), true);
        pm.paint(&m, |_, _| color);
    }
    if !stroke.is_empty() {
        let m = pm.rasterize(&stroke_polys(&stroke, xf, width), false);
        pm.paint(&m, |_, _| color);
    }
}

/// Dark enamel tile for city icons, echoes and bonuses.
fn disc(px: usize, e: &Enamel) -> Pixmap {
    let size = px as f32;
    let mut pm = Pixmap::new(px, px);
    tile_frame(
        &mut pm,
        size,
        radial(
            size,
            size,
            0.35,
            0.3,
            &[(0., e.disc_light), (0.7, e.disc), (1., e.disc)],
        ),
        e.disc_rim,
        [0., 0., 0., 0.],
    );
    pm
}

pub fn city_tile(c: CityIcon, px: usize, e: &Enamel) -> Pixmap {
    let mut pm = disc(px, e);
    glyph_into(
        &mut pm,
        Glyph::city(c),
        Xf::grid(px as f32, 0.62),
        e.disc_ink,
    );
    pm
}

pub fn echo_tile(px: usize, e: &Enamel) -> Pixmap {
    let mut pm = disc(px, e);
    glyph_into(&mut pm, Glyph::Echo, Xf::grid(px as f32, 0.66), e.disc_ink);
    pm
}

/// Laurel around an empty centre; the number is drawn as text on top.
pub fn bonus_tile(px: usize, e: &Enamel) -> Pixmap {
    let mut pm = disc(px, e);
    let xf = Xf::grid(px as f32, 1.);
    let r = 17.;
    for side in [1f32, -1.] {
        let flip = |p: Pt| if side < 0. { (48. - p.0, p.1) } else { p };
        let at = |deg: f32| {
            let t = deg.to_radians();
            (24. + r * t.cos(), 24. + r * t.sin())
        };
        let (a, b) = (flip(at(100.)), flip(at(236.)));
        let sweep = if side > 0. { 1 } else { 0 };
        let stem = format!("M{},{} A{r},{r} 0 0 {sweep} {},{}", a.0, a.1, b.0, b.1);
        let m = pm.rasterize(&stroke_polys(&stem, xf, 1.3), false);
        pm.paint(&m, |_, _| e.disc_ink);
        for (deg, rx, ry) in [
            (112., 4.6, 1.9),
            (136., 4.8, 2.),
            (160., 4.8, 2.),
            (184., 4.6, 1.9),
            (208., 4.2, 1.8),
            (230., 3.4, 1.6),
        ] {
            let c = at(deg);
            let rot = (deg + 90. - 38.) * side + if side < 0. { 180. } else { 0. };
            let c = flip(c);
            let (s, co) = rot.to_radians().sin_cos();
            let leaf: Vec<Pt> = (0..16)
                .map(|k| {
                    let t = TAU * k as f32 / 16.;
                    let (ex, ey) = (rx * t.cos(), ry * t.sin());
                    xf.pt((c.0 + ex * co - ey * s, c.1 + ex * s + ey * co))
                })
                .collect();
            let m = pm.rasterize(&[leaf], false);
            pm.paint(&m, |_, _| e.disc_ink);
        }
    }
    pm
}

/// The image slot: a plain dark hexagon with a pale rim.
pub fn hex_tile(px: usize, e: &Enamel) -> Pixmap {
    let size = px as f32;
    let mut pm = Pixmap::new(px, px);
    let outer = hex_poly(size, 1.);
    let m = pm.rasterize(std::slice::from_ref(&outer), false);
    pm.paint(&m, |_, _| e.hex);
    let w = (size * 0.04).max(1.);
    let inner = hex_poly(size, 1. + w);
    ring(&mut pm, outer, inner, e.hex_rim);
    let a = hex_poly(size, size * 0.12);
    let b = hex_poly(size, size * 0.12 + 0.8 * size / 36.);
    ring(&mut pm, a, b, alpha(e.hex_rim, 0.25));
    pm
}

/// Enamel disc behind age numerals.
pub fn age_disc(px: usize, e: &Enamel) -> Pixmap {
    let size = px as f32;
    let mut pm = Pixmap::new(px, px);
    let c = (size / 2., size / 2.);
    let m = pm.rasterize(&[disc_poly(c, size / 2.)], false);
    pm.paint(&m, |_, _| e.age_rim);
    let m = pm.rasterize(&[disc_poly(c, size / 2. - size * 0.05)], false);
    let [light, body, shade] = e.age;
    pm.paint(
        &m,
        radial(
            size,
            size,
            0.35,
            0.3,
            &[(0., light), (0.5, body), (1., shade)],
        ),
    );
    pm
}

/// The Inari mark on a unit square as (centre, radius, colour): one dot per
/// icon colour in a ring, then the `middle` dot.
pub fn brand_dots(middle: Rgba) -> Vec<((f32, f32), f32, Rgba)> {
    let mut dots: Vec<_> = ICONS
        .into_iter()
        .enumerate()
        .map(|(i, icon)| {
            let a = TAU * i as f32 / ICONS.len() as f32 - PI / 2.;
            let c = (0.5 + 0.31 * a.cos(), 0.5 + 0.31 * a.sin());
            (c, 0.11, icon_palette(icon)[1])
        })
        .collect();
    dots.push(((0.5, 0.5), 0.115, middle));
    dots
}

/// The Inari mark as a sprite, for the window and file icons.
pub fn brand(px: usize, middle: Rgba) -> Pixmap {
    let size = px as f32;
    let mut pm = Pixmap::new(px, px);
    for ((x, y), r, color) in brand_dots(middle) {
        let m = pm.rasterize(&[disc_poly((x * size, y * size), r * size)], false);
        pm.paint(&m, |_, _| color);
    }
    pm
}

/// A single-colour line glyph filling the square.
pub fn glyph(g: Glyph, px: usize, color: Rgba) -> Pixmap {
    let mut pm = Pixmap::new(px, px);
    glyph_into(&mut pm, g, Xf::grid(px as f32, 1.), color);
    pm
}

/// Card body in card units (210 x 150) scaled to `w` x `h` pixels: pale
/// enamel with sheen, a wire frame, title band and the rails that join the
/// icon slots.
pub fn card_frame(color: CardColor, city: bool, w: usize, h: usize, e: &Enamel) -> Pixmap {
    let mut pm = Pixmap::new(w, h);
    let s = w as f32 / 210.;
    let (cw, ch) = (w as f32, h as f32);
    let [light, band, dark, tint] = e.card(color);
    let body = rrect_poly(0., 0., cw, ch, 7. * s);
    let card = pm.rasterize(std::slice::from_ref(&body), false);
    pm.paint(&card, |_, _| tint);
    pm.paint(&card, |x, y| {
        let t = (((x - 0.28 * cw) / (1.3 * cw)).powi(2) + ((y - 0.12 * ch) / ch).powi(2)).sqrt();
        [1., 1., 1., (e.sheen * (1. - t / 0.55)).max(0.)]
    });
    let inset = |d: f32| {
        rrect_poly(
            d * s,
            d * s,
            cw - 2. * d * s,
            ch - 2. * d * s,
            (7. - d).max(1.) * s,
        )
    };
    ring(&mut pm, inset(3.2), inset(4.), e.rim_light);
    ring(&mut pm, inset(1.6), inset(3.2), e.rim_dark);
    ring(&mut pm, body.clone(), inset(1.6), e.rim);
    // Bands are clipped to the card's rounded outline.
    let mut band_rect = |x: f32, y: f32, bw: f32, bh: f32, vertical: bool, edges: [bool; 4]| {
        let (x, y, bw, bh) = (x * s, y * s, bw * s, bh * s);
        let poly = vec![(x, y), (x + bw, y), (x + bw, y + bh), (x, y + bh)];
        let mut m = pm.rasterize(&[poly], false);
        for (a, b) in m.0.iter_mut().zip(&card.0) {
            *a *= b;
        }
        pm.paint(&m, |px, py| {
            let t = if vertical {
                (px - x) / bw
            } else {
                (py - y) / bh
            };
            ramp(&[(0., light), (0.45, band), (1., dark)], t)
        });
        let edge = 1.2 * s;
        let lines = [
            (edges[0], (x, y, bw, edge)),
            (edges[1], (x + bw - edge, y, edge, bh)),
            (edges[2], (x, y + bh - edge, bw, edge)),
            (edges[3], (x, y, edge, bh)),
        ];
        for (on, (lx, ly, lw, lh)) in lines {
            if on {
                let mut m = pm.rasterize(
                    &[vec![
                        (lx, ly),
                        (lx + lw, ly),
                        (lx + lw, ly + lh),
                        (lx, ly + lh),
                    ]],
                    false,
                );
                for (a, b) in m.0.iter_mut().zip(&card.0) {
                    *a *= b;
                }
                pm.paint(&m, |_, _| e.band_edge);
            }
        }
    };
    if city {
        band_rect(0., 19., 210., 12., false, [true, false, true, false]);
    } else {
        band_rect(28., 9., 182., 21., false, [true, false, true, true]);
        band_rect(19., 40., 12., 70., true, [false, true, false, true]);
    }
    band_rect(0., 119., 210., 12., false, [true, false, true, false]);
    pm
}

/// Table units after which a pattern repeats, across and down.
pub fn pattern_period(p: Pattern) -> (f32, f32) {
    match p {
        Pattern::Lattice => (42., 42.),
        Pattern::Ice => (160., 160.),
        Pattern::Stars => (60., 60.),
        // Rows 12 apart, alternate rows offset by half a fan.
        Pattern::Waves => (48., 24.),
    }
}

/// Table units covered by one pattern tile: at least two repeats each way,
/// so tiles laid edge to edge continue the pattern.
pub fn pattern_tile_size(p: Pattern) -> (f32, f32) {
    let (pw, ph) = pattern_period(p);
    match p {
        Pattern::Waves => (3. * pw, 4. * ph),
        _ => (2. * pw, 2. * ph),
    }
}

/// Cracked-ice segments within one 160-unit repeat; ends on an edge meet the
/// matching end on the opposite edge.
const ICE: &str = "M40,0 L52,38 M110,0 L96,46 M52,38 L96,46 M0,30 L52,38 M96,46 L128,74 \
    M128,74 L160,30 M52,38 L30,82 M30,82 L0,100 M30,82 L78,92 M78,92 L96,46 M78,92 L128,74 \
    M128,74 L160,100 M78,92 L60,128 M60,128 L40,160 M60,128 L30,82 M78,92 L120,130 \
    M120,130 L110,160 M120,130 L128,74 M120,130 L150,132 M150,132 L160,100";

/// A star of two squares joined to its neighbours, within one 60-unit repeat.
const STAR: &str = "M19,19 H41 V41 H19 Z M30,14.4 L45.6,30 L30,45.6 L14.4,30 Z \
    M30,14.4 V0 M45.6,30 H60 M30,45.6 V60 M14.4,30 H0 \
    M19,19 L0,0 M41,19 L60,0 M41,41 L60,60 M19,41 L0,60";

/// One opaque tile of the table texture at `w` x `h` pixels. Shapes from the
/// neighbouring repeats are drawn too, so strokes cross tile edges cleanly.
pub fn pattern_tile(g: &Ground, w: usize, h: usize) -> Pixmap {
    let mut pm = Pixmap::new(w, h);
    let (tw, th) = pattern_tile_size(g.pattern);
    let s = w as f32 / tw;
    let full = rrect_poly(0., 0., w as f32, h as f32, 0.);
    let m = pm.rasterize(&[full], false);
    pm.paint(&m, |_, _| g.table);
    let at = |dx: f32, dy: f32| Xf {
        s,
        dx: dx * s,
        dy: dy * s,
    };
    // Offsets of every repeat that can reach into the tile.
    let (pw, ph) = pattern_period(g.pattern);
    let repeats = || {
        let (nx, ny) = ((tw / pw) as i32, (th / ph) as i32);
        (-1..=nx).flat_map(move |i| (-1..=ny).map(move |j| (i as f32 * pw, j as f32 * ph)))
    };
    let mut lines = |d: &str, width: f32| {
        let polys: Vec<Vec<Pt>> = repeats()
            .flat_map(|(dx, dy)| stroke_polys(d, at(dx, dy), width))
            .collect();
        let m = pm.rasterize(&polys, false);
        pm.paint(&m, |_, _| g.ink);
    };
    match g.pattern {
        Pattern::Lattice => lines("M0,0 L42,42 M42,0 L0,42", 1.),
        Pattern::Ice => lines(ICE, 1.3),
        Pattern::Stars => {
            lines(STAR, 1.);
            let dots: Vec<Vec<Pt>> = repeats()
                .map(|(dx, dy)| disc_poly(at(dx, dy).pt((30., 30.)), 1.6 * s))
                .collect();
            let m = pm.rasterize(&dots, false);
            pm.paint(&m, |_, _| alpha(g.ink, (g.ink[3] * 3.).min(1.)));
        }
        Pattern::Waves => {
            // Fans of radius 24 in rows 12 apart, alternate rows offset by
            // half a fan. Later rows are drawn over earlier ones.
            for k in -2..=(th / 12.) as i32 + 2 {
                for j in -1..=(tw / pw) as i32 + 1 {
                    let x = if k % 2 == 0 { 0. } else { 24. } + 48. * j as f32;
                    let c = (x * s, 12. * k as f32 * s);
                    let m = pm.rasterize(&[disc_poly(c, 24. * s)], false);
                    pm.paint(&m, |_, _| g.table);
                    let rings: Vec<Vec<Pt>> = [23., 17.5, 12., 6.5]
                        .into_iter()
                        .flat_map(|r| [disc_poly(c, (r + 0.55) * s), disc_poly(c, (r - 0.55) * s)])
                        .collect();
                    let m = pm.rasterize(&rings, true);
                    pm.paint(&m, |_, _| g.ink);
                }
            }
        }
    }
    pm
}

/// The theme's radial wash at `px` x `px / 2` pixels, to be stretched over
/// the table.
pub fn wash(w: &Wash, px: usize) -> Pixmap {
    let (pw, ph) = (px as f32, (px / 2) as f32);
    let mut pm = Pixmap::new(px, px / 2);
    let full = rrect_poly(0., 0., pw, ph, 0.);
    let m = pm.rasterize(&[full], false);
    let (cx, cy) = (w.center.0 * pw, w.center.1 * ph);
    let (rx, ry) = (w.radius.0 * pw, w.radius.1 * ph);
    pm.paint(&m, |x, y| {
        let t = (((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2)).sqrt();
        alpha(w.color, w.color[3] * (1. - t).clamp(0., 1.))
    });
    pm
}

#[cfg(test)]
mod tests {
    use super::super::faces::{COLORS, ICONS};
    use super::*;
    use crate::theme::Theme;

    fn opaque(pm: &Pixmap) -> f32 {
        pm.px.iter().map(|p| p[3]).sum()
    }

    #[test]
    fn squares_and_circles_cover_their_area() {
        let pm = Pixmap::new(40, 40);
        let sq = pm.rasterize(
            &fill_polys(
                "M10,10 H30 V30 H10 Z",
                Xf {
                    s: 1.,
                    dx: 0.,
                    dy: 0.,
                },
            ),
            true,
        );
        assert!((sq.0.iter().sum::<f32>() - 400.).abs() < 1.);
        let c = pm.rasterize(
            &fill_polys(
                &circle(20., 20., 10.),
                Xf {
                    s: 1.,
                    dx: 0.,
                    dy: 0.,
                },
            ),
            true,
        );
        let area = c.0.iter().sum::<f32>();
        assert!((area - PI * 100.).abs() < 6., "{area}");
    }

    #[test]
    fn even_odd_cuts_holes_and_strokes_union() {
        let pm = Pixmap::new(40, 40);
        let xf = Xf {
            s: 1.,
            dx: 0.,
            dy: 0.,
        };
        let ring = pm.rasterize(
            &fill_polys("M0,0 H40 V40 H0 Z M10,10 H30 V30 H10 Z", xf),
            true,
        );
        assert!((ring.0.iter().sum::<f32>() - 1200.).abs() < 1.);
        // Overlapping round-capped strokes never exceed full coverage.
        let s = pm.rasterize(&stroke_polys("M5,20 H35 M20,5 V35", xf, 4.), false);
        assert!(s.0.iter().all(|&c| c <= 1.));
        let area = s.0.iter().sum::<f32>();
        assert!(area > 2. * 30. * 4. - 20. && area < 2. * 34. * 4., "{area}");
    }

    #[test]
    fn every_sprite_draws_something() {
        for theme in Theme::ALL {
            let e = &theme.palette().enamel;
            for icon in ICONS {
                assert!(opaque(&icon_tile(icon, 36, e)) > 36. * 36. * 0.9);
            }
            assert!(opaque(&food_tile(36, e)) > 36. * 36. * 0.9);
            for c in CityIcon::ALL {
                assert!(opaque(&city_tile(c, 36, e)) > 1000.);
            }
            for color in COLORS {
                let frame = card_frame(color, false, 210, 150, e);
                assert!(opaque(&frame) > 210. * 150. * 0.95);
            }
            assert!(opaque(&hex_tile(36, e)) > 600.);
            assert!(opaque(&bonus_tile(36, e)) > 1000.);
            assert!(opaque(&age_disc(26, e)) > 400.);
        }
        assert_eq!(
            glyph(Glyph::Museum, 20, hex(0xefe6cf)).bytes().len(),
            20 * 20 * 4
        );
    }

    #[test]
    fn figures_glyphs_and_food_render_at_inline_and_tile_sizes() {
        for px in [16, 36, 72] {
            for g in [Glyph::Figures, Glyph::Karma, Glyph::Decree] {
                let pm = glyph(g, px, hex(0xffffff));
                assert_eq!(pm.bytes().len(), px * px * 4);
                assert!(opaque(&pm) > (px * px) as f32 * 0.1, "{g:?} at {px}");
                assert!(opaque(&pm) < (px * px) as f32 * 0.7, "{g:?} at {px}");
            }
            for theme in Theme::ALL {
                let e = &theme.palette().enamel;
                let food = food_tile(px, e);
                let blank = enamel_icon_tile(
                    [hex(0xb6bd6c), hex(0x737d2c), hex(0x394315)],
                    (String::new(), String::new()),
                    px,
                    e,
                );
                let marked = food
                    .px
                    .iter()
                    .zip(&blank.px)
                    .filter(|(a, b)| a != b)
                    .count();
                assert!(marked > px * px / 20, "{theme:?} Food at {px}: {marked}");
                assert_eq!(food.bytes().len(), px * px * 4);
                for icon in ICONS {
                    assert_ne!(food.bytes(), icon_tile(icon, px, e).bytes());
                }
            }
        }
    }

    #[test]
    fn table_patterns_are_opaque_and_textured() {
        for theme in Theme::ALL {
            let ground = &theme.palette().ground;
            let (tw, th) = pattern_tile_size(ground.pattern);
            let (w, h) = (tw as usize, th as usize);
            let tile = pattern_tile(ground, w, h);
            assert!(opaque(&tile) > (w * h) as f32 * 0.999, "{theme:?}");
            // The ink differs from the table somewhere in every tile.
            let marked = tile
                .px
                .iter()
                .filter(|p| (p[0] - ground.table[0]).abs() > 0.01)
                .count();
            assert!(marked > w * h / 50, "{theme:?}: {marked}");
        }
    }

    #[test]
    fn pattern_tiles_join_without_seams() {
        // Every strip along a tile's edge matches the strip one repeat
        // inward, so the shapes beyond each edge were drawn and tiles laid
        // edge to edge continue the pattern.
        for theme in Theme::ALL {
            let ground = &theme.palette().ground;
            let (tw, th) = pattern_tile_size(ground.pattern);
            let (pw, ph) = pattern_period(ground.pattern);
            let (w, h, pw, ph) = (tw as usize, th as usize, pw as usize, ph as usize);
            let tile = pattern_tile(ground, w, h);
            let at = |x: usize, y: usize| tile.px[y * w + x];
            let same = |a: Rgba, b: Rgba| (0..3).all(|k| (a[k] - b[k]).abs() < 0.004);
            for y in 0..h {
                for x in 0..pw {
                    assert!(same(at(x, y), at(x + pw, y)), "{theme:?} left ({x}, {y})");
                    let r = w - 1 - x;
                    assert!(same(at(r, y), at(r - pw, y)), "{theme:?} right ({r}, {y})");
                }
            }
            for x in 0..w {
                for y in 0..ph {
                    assert!(same(at(x, y), at(x, y + ph)), "{theme:?} top ({x}, {y})");
                    let b = h - 1 - y;
                    assert!(same(at(x, b), at(x, b - ph)), "{theme:?} bottom ({x}, {b})");
                }
            }
        }
    }

    /// Writes a contact sheet of every sprite in every theme, each on its table
    /// pattern, for visual review:
    /// `SPRITE_SHEET=out.png cargo test -p inari contact_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn contact_sheet() {
        let Ok(path) = std::env::var("SPRITE_SHEET") else {
            return;
        };
        let (w, block) = (900usize, 420usize);
        let h = block * Theme::ALL.len();
        let mut sheet = Pixmap::new(w, h);
        let mut blit = |pm: &Pixmap, x: usize, y: usize| {
            for j in 0..pm.h {
                for i in 0..pm.w {
                    if x + i >= w || y + j >= h {
                        continue;
                    }
                    let s = pm.px[j * pm.w + i];
                    let d = &mut sheet.px[(y + j) * w + x + i];
                    for k in 0..3 {
                        d[k] = s[k] + d[k] * (1. - s[3]);
                    }
                    d[3] = s[3] + d[3] * (1. - s[3]);
                }
            }
        };
        for (t, theme) in Theme::ALL.into_iter().enumerate() {
            let p = theme.palette();
            let e = &p.enamel;
            let top = t * block;
            let (tw, th) = pattern_tile_size(p.ground.pattern);
            let tile = pattern_tile(&p.ground, tw as usize, th as usize);
            for ty in (0..block).step_by(th as usize) {
                for tx in (0..w).step_by(tw as usize) {
                    // Clip the last row of tiles to this theme's block.
                    let mut part = Pixmap::new(tile.w, tile.h.min(block - ty));
                    part.px.copy_from_slice(&tile.px[..part.w * part.h]);
                    blit(&part, tx, top + ty);
                }
            }
            if let Some(w) = &p.ground.wash {
                let mut stretched = Pixmap::new(900, block);
                let small = wash(w, 90);
                for y in 0..block {
                    for x in 0..900 {
                        stretched.px[y * 900 + x] =
                            small.px[(y * small.h / block) * small.w + x / 10];
                    }
                }
                blit(&stretched, 0, top);
            }
            for (i, icon) in ICONS.into_iter().enumerate() {
                blit(&icon_tile(icon, 72, e), 10 + i * 80, top + 10);
                blit(&icon_tile(icon, 36, e), 10 + i * 80, top + 90);
                blit(&icon_tile(icon, 16, e), 50 + i * 80, top + 90);
            }
            let food_x = 10 + ICONS.len() * 80;
            blit(&food_tile(72, e), food_x, top + 10);
            blit(&food_tile(36, e), food_x, top + 90);
            blit(&food_tile(16, e), food_x + 40, top + 90);
            for (i, c) in CityIcon::ALL.into_iter().enumerate() {
                blit(&city_tile(c, 48, e), 10 + i * 56, top + 140);
            }
            blit(&echo_tile(48, e), 10, top + 200);
            blit(&bonus_tile(48, e), 66, top + 200);
            blit(&hex_tile(48, e), 122, top + 200);
            blit(&age_disc(26, e), 180, top + 210);
            let glyphs = [
                Glyph::Splay(1),
                Glyph::Splay(4),
                Glyph::Forecast,
                Glyph::Safe,
                Glyph::Museum,
                Glyph::Junk,
                Glyph::Echoes,
                Glyph::Unseen,
                Glyph::Artifacts,
                Glyph::Cities,
                Glyph::Achievement,
                Glyph::Score,
                Glyph::Hand,
                Glyph::city(CityIcon::Search),
                Glyph::Figures,
                Glyph::Karma,
                Glyph::Decree,
            ];
            let accent = p.accent;
            for (i, g) in glyphs.into_iter().enumerate() {
                let ink = [accent.r, accent.g, accent.b, accent.a];
                blit(&glyph(g, 32, ink), 230 + i * 40, top + 210);
            }
            for (i, color) in COLORS.into_iter().enumerate() {
                blit(
                    &card_frame(color, i == 2, 168, 120, e),
                    10 + i * 176,
                    top + 280,
                );
            }
        }
        // export_png writes rows bottom-up, as for screen captures.
        let bytes = sheet.bytes();
        let flipped = bytes.chunks(w * 4).rev().flatten().copied().collect();
        macroquad::prelude::Image {
            width: w as u16,
            height: h as u16,
            bytes: flipped,
        }
        .export_png(&path);
    }
}
