//! The table: procedurally drawn enamel cards, player summaries of zone
//! counts, an icon comparison and a natural-order game log.
//! Everything is drawn in a fixed 1840x960 space; no card art is loaded.
//! Colours come from the current theme (see theme.rs).
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use inno_engine::{Color as CardColor, Icon, Special, Splay};
use macroquad::prelude::*;

use super::faces::{
    COLORS, CityIcon, FaceId, ICONS, Lead, Set, backs, face, faces, icon_named, names, paragraphs,
    stack_index,
};
use super::session::{DATA_DIR, Game, Options, Settings, fresh_seed, parse_seed};
use super::sprites::{self, Glyph, Pixmap};
use super::strength;
use super::table::{
    DeckRow, Entry, Held, Outcome, Seg, StackView, decree_text, name, plain, title_case,
};
use super::theme::{Control, Palette, Theme, rgba};

const W: f32 = 1840.;
const H: f32 = 960.;

/// The theme drawn this frame; `run` sets it from the settings.
fn pal() -> &'static Palette {
    THEME.get().palette()
}

// Table geometry, in the 210x150 card units of the prototype.
const MAIN_X: f32 = 10.;
const MAIN_W: f32 = 1416.;
const SIDE_X: f32 = 1438.;
const SIDE_W: f32 = 392.;
/// Card design units; faces scale to any size from these.
const CARD_W: f32 = 210.;
const CARD_H: f32 = 150.;
/// Board cards nearly fill their row: taller with five columns, and as wide
/// as six columns allow with the artifact on display.
const BOARD_H5: f32 = 163.;
const BOARD_H6: f32 = 158.;
/// The top row keeps a clear gap below the 40-unit title bar.
const OPP_Y: f32 = 52.;
const MID_Y: f32 = 243.;
const MID_H: f32 = 100.;
const OWN_Y: f32 = 349.;
/// Below a board card: room for splay peeks, then the buried-card strip.
const STRIP_GAP: f32 = 10.;
const HAND_Y: f32 = 548.;
const HAND_W: f32 = 166.;
const HAND_H: f32 = 118.;
const HAND_GAP: f32 = 4.;
/// Hand rows visible at once; larger hands scroll.
const HAND_ROWS: usize = 2;
const CHOICE_Y: f32 = 794.;
const CHOICE_H: f32 = 158.;
const ICONS_H: f32 = 132.;
const TABS_Y: f32 = OPP_Y + ICONS_H + 8.;
const LOG_Y: f32 = TABS_Y + 34.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    Body,
    Bold,
    Title,
}

thread_local! {
    static FONTS: RefCell<[Option<Font>; 3]> = const { RefCell::new([None, None, None]) };
    /// Physical pixels per layout unit: text and sprites are rasterized at this density.
    static PX: Cell<f32> = const { Cell::new(1.) };
    static THEME: Cell<Theme> = const { Cell::new(Theme::Cloisonne) };
}

/// The design's fonts ship with the app (SIL Open Font License; see
/// engine/assets/fonts), so text looks the same on every platform.
async fn load_fonts() {
    let load = |bytes: &[u8]| load_ttf_font_from_bytes(bytes).ok();
    let fonts = [
        load(include_bytes!("../assets/fonts/AlegreyaSans-Medium.ttf")),
        load(include_bytes!("../assets/fonts/AlegreyaSans-Bold.ttf")),
        load(include_bytes!("../assets/fonts/Cinzel-Bold.ttf")),
    ];
    FONTS.with(|slot| *slot.borrow_mut() = fonts);
}

fn with_face<R>(face: Face, f: impl FnOnce(Option<&Font>) -> R) -> R {
    FONTS.with(|fonts| {
        let fonts = fonts.borrow();
        let font = match face {
            Face::Body => fonts[0].as_ref(),
            Face::Bold => fonts[1].as_ref().or(fonts[0].as_ref()),
            Face::Title => fonts[2].as_ref().or(fonts[1].as_ref()),
        };
        f(font)
    })
}
fn font_px(size: f32) -> (u16, f32) {
    let px = (size * PX.get()).round().clamp(1., 400.);
    (px as u16, size / px)
}
fn face_of(bold: bool) -> Face {
    if bold { Face::Bold } else { Face::Body }
}
fn measure(text: &str, size: f32, bold: bool) -> f32 {
    measure_face(text, size, face_of(bold))
}
fn measure_face(text: &str, size: f32, face: Face) -> f32 {
    let (px, scale) = font_px(size);
    with_face(face, |font| measure_text(text, font, px, scale).width)
}
fn draw_str(text: &str, x: f32, y: f32, size: f32, color: Color, bold: bool) {
    draw_face(text, x, y, size, color, face_of(bold));
}
/// Cinzel capitals for titles, names and numerals.
fn title(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_face(text, x, y, size, color, Face::Title);
}
fn centered_title(text: &str, cx: f32, y: f32, size: f32, color: Color) {
    title(
        text,
        cx - measure_face(text, size, Face::Title) / 2.,
        y,
        size,
        color,
    );
}
fn draw_face(text: &str, x: f32, y: f32, size: f32, color: Color, face: Face) {
    let (px, scale) = font_px(size);
    with_face(face, |font| {
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font,
                font_size: px,
                font_scale: scale,
                color,
                ..Default::default()
            },
        );
    });
}
fn label(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_str(text, x, y, size, color, false);
}
fn heading(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_str(text, x, y, size, color, true);
}
fn centered(text: &str, cx: f32, y: f32, size: f32, color: Color, bold: bool) {
    draw_str(
        text,
        cx - measure(text, size, bold) / 2.,
        y,
        size,
        color,
        bold,
    );
}
fn shorten(text: &str, width: f32, size: f32) -> String {
    if measure(text, size, false) <= width {
        return text.into();
    }
    let mut s = text.to_owned();
    while !s.is_empty() && measure(&format!("{s}..."), size, false) > width {
        s.pop();
    }
    format!("{s}...")
}
fn lines(text: &str, width: f32, size: f32) -> Vec<String> {
    let mut out = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let next = if line.is_empty() {
                word.into()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&next, size, false) > width {
                out.push(line);
                line = word.into();
            } else {
                line = next;
            }
        }
        out.push(line);
    }
    out
}
fn wrapped(text: &str, r: Rect, size: f32, color: Color) -> f32 {
    let mut y = r.y;
    for line in lines(text, r.w, size) {
        if y + size > r.bottom() {
            break;
        }
        label(&line, r.x, y + size, size, color);
        y += size * 1.3;
    }
    y - r.y
}
// ------------------------------------------------------------------ shapes

fn inset(r: Rect, d: f32) -> Rect {
    Rect::new(r.x + d, r.y + d, r.w - 2. * d, r.h - 2. * d)
}
/// A rounded rectangle drawn as one triangle fan, so translucent colours
/// shade evenly.
fn rrect(r: Rect, radius: f32, c: Color) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let rad = radius.min(r.w / 2.).min(r.h / 2.).max(0.);
    if rad < 0.5 {
        draw_rectangle(r.x, r.y, r.w, r.h, c);
        return;
    }
    // Seven points round each corner, clockwise from the top right.
    const STEPS: usize = 7;
    let corners = [
        (r.right() - rad, r.y + rad, -FRAC_PI_2),
        (r.right() - rad, r.bottom() - rad, 0.),
        (r.x + rad, r.bottom() - rad, FRAC_PI_2),
        (r.x + rad, r.y + rad, PI),
    ];
    let point = |i: usize| {
        let (cx, cy, a0) = corners[i / STEPS % 4];
        let a = a0 + FRAC_PI_2 * (i % STEPS) as f32 / (STEPS - 1) as f32;
        vec2(cx + rad * a.cos(), cy + rad * a.sin())
    };
    for i in 0..4 * STEPS {
        draw_triangle(r.center(), point(i), point(i + 1), c);
    }
}
fn boxed(r: Rect, radius: f32, fill: Color, edge: Color) {
    rrect(r, radius, edge);
    rrect(inset(r, 1.), radius - 1., fill);
}
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        1.,
    )
}
/// Card colour names as readable text.
fn hue(c: CardColor) -> Color {
    pal().hue(c)
}
/// The card's deep enamel band.
fn band(c: CardColor) -> Color {
    pal().card(c)[1]
}
/// The card's darkest enamel, for age numerals.
fn shade(c: CardColor) -> Color {
    pal().card(c)[2]
}
/// The card's pale enamel body.
fn tint(c: CardColor) -> Color {
    pal().card(c)[3]
}
fn splay_glyph(splay: Splay) -> Option<Glyph> {
    match splay {
        Splay::None => None,
        Splay::Left => Some(Glyph::Splay(1)),
        Splay::Right => Some(Glyph::Splay(2)),
        Splay::Up => Some(Glyph::Splay(3)),
        Splay::Aslant => Some(Glyph::Splay(4)),
    }
}
/// A panel with a wire rim and a faint inner hairline, as in the prototype.
fn panel(r: Rect) {
    draw_rectangle(r.x + 2., r.y + 4., r.w, r.h, pal().panel_shadow);
    boxed(r, 6., pal().paper, pal().line);
    draw_rectangle_lines(r.x + 3., r.y + 3., r.w - 6., r.h - 6., 0.8, pal().wire);
}
/// The theme's table: its colour, a repeating pattern and an optional wash.
fn table_background(art: &Art) {
    let p = pal();
    draw_rectangle(0., 0., W, H, p.table);
    let (tw, th) = sprites::pattern_tile_size(p.ground.pattern);
    let key = Key::Pattern(phys(tw), phys(th));
    let mut y = 0.;
    while y < H {
        let mut x = 0.;
        while x < W {
            art.sprite(key, Rect::new(x, y, tw, th));
            x += tw;
        }
        y += th;
    }
    if p.ground.wash.is_some() {
        art.sprite(Key::Wash, Rect::new(0., 0., W, H));
    }
}
/// Small capitals label in the muted gold of the prototype.
fn caps(text: &str, x: f32, y: f32, size: f32) {
    title(text, x, y, size, pal().label);
}

// ------------------------------------------------------------------- input

#[derive(Clone, Copy)]
enum Style {
    Primary,
    Secondary,
    Bar,
}

struct Input {
    mouse: Vec2,
    click: bool,
    /// The left button is down (dragging).
    held: bool,
    /// Right button: opens a card full size.
    right_click: bool,
    scroll: f32,
    keys: bool,
}
impl Input {
    fn read() -> Self {
        let scale = (screen_width() / W).min(screen_height() / H);
        let origin = vec2(
            (screen_width() - W * scale) / 2.,
            (screen_height() - H * scale) / 2.,
        );
        Self {
            mouse: (Vec2::from(mouse_position()) - origin) / scale,
            click: is_mouse_button_pressed(MouseButton::Left),
            held: is_mouse_button_down(MouseButton::Left),
            right_click: is_mouse_button_pressed(MouseButton::Right),
            scroll: mouse_wheel().1,
            keys: true,
        }
    }
    /// Stands in for the real input under a modal dialog.
    fn blocked() -> Self {
        Self {
            mouse: vec2(-1e5, -1e5),
            click: false,
            held: false,
            right_click: false,
            scroll: 0.,
            keys: false,
        }
    }
    fn button(&self, text: &str, r: Rect, enabled: bool, style: Style) -> bool {
        let hover = enabled && r.contains(self.mouse);
        let p = pal();
        let Control { fill, edge, ink } = match (enabled, style, hover) {
            (false, Style::Primary, _) => p.primary_off,
            (false, Style::Secondary, _) => p.secondary_off,
            (false, Style::Bar, _) => p.bar_off,
            (true, Style::Primary, true) => Control {
                fill: p.primary_hover,
                ..p.primary
            },
            (true, Style::Primary, false) => p.primary,
            (true, Style::Secondary, true) => Control {
                fill: p.hover,
                edge: p.accent,
                ink: p.ink,
            },
            (true, Style::Secondary, false) => Control {
                fill: p.strip,
                edge: p.line,
                ink: p.ink,
            },
            (true, Style::Bar, true) => Control {
                fill: p.bar_button_hover,
                edge: p.bar_accent,
                ..p.bar_button
            },
            (true, Style::Bar, false) => p.bar_button,
        };
        boxed(r, 5., fill, edge);
        let size = if r.h < 34. { 14. } else { 16. };
        let bold = !matches!(style, Style::Secondary);
        let text = shorten(text, r.w - 16., size);
        centered(
            &text,
            r.center().x,
            r.y + r.h / 2. + size * 0.36,
            size,
            ink,
            bold,
        );
        enabled && hover && self.click
    }
}

// --------------------------------------------------------------- card art

/// Sprite cache key; sizes are physical pixels so sprites stay crisp at any
/// window scale.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Icon(Icon, u16),
    City(u8, u16),
    Echo(u16),
    Food(u16),
    Bonus(u16),
    Hex(u16),
    Age(u16),
    Glyph(Glyph, u16, [u8; 4]),
    Frame(CardColor, bool, u16, u16),
    /// One tile of the table pattern, width and height in pixels.
    Pattern(u16, u16),
    Wash,
}

fn build(key: Key) -> Pixmap {
    let e = &pal().enamel;
    match key {
        Key::Icon(icon, px) => sprites::icon_tile(icon, px.into(), e),
        Key::City(i, px) => sprites::city_tile(CityIcon::ALL[usize::from(i)], px.into(), e),
        Key::Echo(px) => sprites::echo_tile(px.into(), e),
        Key::Food(px) => sprites::food_tile(px.into(), e),
        Key::Bonus(px) => sprites::bonus_tile(px.into(), e),
        Key::Hex(px) => sprites::hex_tile(px.into(), e),
        Key::Age(px) => sprites::age_disc(px.into(), e),
        Key::Glyph(g, px, c) => sprites::glyph(g, px.into(), c.map(|v| f32::from(v) / 255.)),
        Key::Frame(color, city, w, h) => sprites::card_frame(color, city, w.into(), h.into(), e),
        Key::Pattern(w, h) => sprites::pattern_tile(&pal().ground, w.into(), h.into()),
        Key::Wash => match &pal().ground.wash {
            Some(wash) => sprites::wash(wash, 96),
            None => Pixmap::new(1, 1),
        },
    }
}

/// Physical pixels for a length in layout units.
fn phys(units: f32) -> u16 {
    (units * PX.get()).round().clamp(2., 2400.) as u16
}

/// One laid-out run of card text. Colours are looked up when drawn, so
/// cached layouts follow the theme.
#[derive(Clone)]
enum Fx {
    Word(String),
    /// The I DEMAND or I COMPEL lead, in capitals and the demand colour.
    Demand(String),
    Icon(Icon),
    Age(u8),
    Hex,
    Echo,
    /// Food, the Age 0 icon.
    Food,
    /// A boxed value with no number: "every [ ] value" (Archimedes).
    Box,
    /// A paragraph mark such as a karma or Decree.
    Mark(Glyph),
}

/// A card's effect text wrapped for one card size.
struct FxLayout {
    size: f32,
    items: Vec<(f32, f32, Fx)>,
}

/// Procedural card and icon art, rasterized on demand and cached.
#[derive(Default)]
struct Art {
    density: f32,
    theme: Theme,
    /// Martian Internet's text follows the rule the match plays.
    martian_internet_printed: bool,
    sprites: RefCell<HashMap<Key, Texture2D>>,
    /// Layout coordinates and font sizes are in logical units, not pixels.
    layouts: RefCell<HashMap<(FaceId, u32), FxLayout>>,
    titles: RefCell<HashMap<(FaceId, u32), TitleLayout>>,
}

/// A card title's lines and font size for one card width, in logical units.
struct TitleLayout {
    lines: Vec<String>,
    size: f32,
}

/// Slot origins in card units: top left, bottom left, centre and right, then
/// the Cities top centre and top right.
const SLOTS: [(f32, f32); 6] = [
    (7., 7.),
    (7., 107.),
    (87., 107.),
    (167., 107.),
    (87., 7.),
    (167., 7.),
];
const SLOT: f32 = 36.;
/// Title sizes in card units: full size, the size below which a title
/// breaks onto two lines, and the largest size two lines fit in the band.
const TITLE_SIZE: f32 = 12.5;
const TITLE_SPLIT_BELOW: f32 = 8.;
const TITLE_TWO_LINE_MAX: f32 = 9.;
/// The effect box sits right of the left icon column, as on real cards.
const FX_BOX: Rect = Rect {
    x: 49.,
    y: 33.,
    w: 154.,
    h: 70.,
};

impl Art {
    /// Drop obsolete textures before drawing the next frame. Keeping only the
    /// current density also invalidates layouts measured with the old fonts;
    /// a new theme repaints every sprite.
    fn begin_frame(&mut self, density: f32, martian_internet_printed: bool) {
        if self.density != density {
            self.sprites.get_mut().clear();
            self.layouts.get_mut().clear();
            self.titles.get_mut().clear();
            self.density = density;
        }
        if self.martian_internet_printed != martian_internet_printed {
            self.layouts.get_mut().clear();
            self.martian_internet_printed = martian_internet_printed;
        }
        if self.theme != THEME.get() {
            self.sprites.get_mut().clear();
            self.theme = THEME.get();
        }
        PX.set(density);
    }

    fn sprite(&self, key: Key, r: Rect) {
        let mut cache = self.sprites.borrow_mut();
        let t = cache.entry(key).or_insert_with(|| {
            let pm = build(key);
            let t = Texture2D::from_rgba8(pm.w as u16, pm.h as u16, &pm.bytes());
            t.set_filter(FilterMode::Linear);
            t
        });
        draw_texture_ex(
            t,
            r.x,
            r.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(r.size()),
                ..Default::default()
            },
        );
    }
    fn icon(&self, icon: Icon, x: f32, y: f32, s: f32) {
        self.sprite(Key::Icon(icon, phys(s)), Rect::new(x, y, s, s));
    }
    fn glyph(&self, g: Glyph, x: f32, y: f32, s: f32, color: Color) {
        let c = [color.r, color.g, color.b, color.a].map(|v| (v * 255.).round() as u8);
        self.sprite(Key::Glyph(g, phys(s), c), Rect::new(x, y, s, s));
    }
    fn tile(&self, key: fn(u16) -> Key, x: f32, y: f32, s: f32) {
        self.sprite(key(phys(s)), Rect::new(x, y, s, s));
    }
    /// The Cities Search mark: a magnifier on a dark rimmed disc, centred on
    /// (x, y) like the badge on the printed card.
    fn search_badge(&self, x: f32, y: f32, radius: f32) {
        let e = &pal().enamel;
        draw_circle(x, y, radius + 1., rgba(e.disc_rim));
        draw_circle(x, y, radius, rgba(e.disc));
        let g = radius * 1.5;
        self.glyph(
            Glyph::city(CityIcon::Search),
            x - g / 2.,
            y - g / 2.,
            g,
            rgba(e.disc_ink),
        );
    }
    /// An age numeral on an ivory enamel disc.
    fn age(&self, age: u8, x: f32, y: f32, s: f32, ink: Color) {
        self.tile(Key::Age, x, y, s);
        centered_title(&age.to_string(), x + s / 2., y + s * 0.69, s * 0.52, ink);
    }
    fn draw(&self, id: FaceId, r: Rect) {
        let c = face(id);
        let s = r.w / CARD_W;
        let city = c.set == Set::Cities;
        self.sprite(Key::Frame(c.color, city, phys(r.w), phys(r.h)), r);
        let at = |x: f32, y: f32, w: f32| Rect::new(r.x + x * s, r.y + y * s, w * s, w * s);
        let image = c.image_slot;
        for (slot, &(x, y)) in SLOTS.iter().enumerate().take(c.icons.len()) {
            let tile = at(x, y, SLOT);
            let px = phys(tile.w);
            if c.food_slots.contains(&slot) {
                self.sprite(Key::Food(px), tile);
            } else if let Some(icon) = c.icons[slot] {
                self.sprite(Key::Icon(icon, px), tile);
                // A Search shares its slot with a standard icon: badge its corner.
                if c.city_icons.get(slot).copied().flatten() == Some(CityIcon::Search) {
                    self.search_badge(tile.right() - 3. * s, tile.y + 3. * s, 9.5 * s);
                }
            } else if let Some(city_icon) = c.city_icons.get(slot).copied().flatten() {
                let i = CityIcon::ALL
                    .iter()
                    .position(|&x| x == city_icon)
                    .unwrap_or(0);
                self.sprite(Key::City(i as u8, px), tile);
            } else if let Some(bonus) = c.bonuses.get(slot).copied().flatten() {
                self.sprite(Key::Bonus(px), tile);
                centered_title(
                    &bonus.to_string(),
                    tile.center().x,
                    tile.center().y + 5.4 * s,
                    15. * s,
                    rgba(pal().enamel.disc_ink),
                );
            } else if c.echo_slots.contains(&slot) {
                self.sprite(Key::Echo(px), tile);
            } else if slot == image {
                self.sprite(Key::Hex(px), tile);
            }
        }
        let name = title_case(&c.title);
        let dark = shade(c.color);
        if city {
            let mut size = 16. * s;
            while measure_face(&name, size, Face::Title) > 190. * s && size > 6. * s {
                size *= 0.95;
            }
            centered_title(&name, r.x + 105. * s, r.y + 68. * s, size, band(c.color));
            self.age(c.value, r.x + 92.5 * s, r.y + 79. * s, 25. * s, dark);
            return;
        }
        let mut titles = self.titles.borrow_mut();
        let layout = titles.entry((id, r.w.to_bits())).or_insert_with(|| {
            let avail = if c.set == Set::Base { 122. } else { 104. };
            let reference = TITLE_SIZE * s;
            let (lines, units) = title_lines(&name, avail, |t| {
                measure_face(t, reference, Face::Title) / reference
            });
            // Font sizes snap to whole pixels; shrink until every line fits.
            let mut size = units * s;
            while lines
                .iter()
                .any(|l| measure_face(l, size, Face::Title) > avail * s)
                && size > 5. * s
            {
                size *= 0.95;
            }
            TitleLayout { lines, size }
        });
        let TitleLayout { lines, size } = layout;
        let size = *size;
        // One line sits on the band's usual baseline; two are centred in it.
        let pitch = 0.98 * size;
        let cap = 0.7 * size;
        let first = if lines.len() == 1 {
            r.y + 24. * s
        } else {
            r.y + 19.5 * s - (cap + pitch) / 2. + cap
        };
        for (i, line) in lines.iter().enumerate() {
            let y = first + i as f32 * pitch;
            title(line, r.x + 49.7 * s, y + 0.9 * s, size, pal().title_shadow);
            title(line, r.x + 49. * s, y, size, pal().title_ink);
        }
        self.age(c.value, r.x + 179. * s, r.y + 7. * s, 25. * s, dark);
        if let Some(g) = set_glyph(c.set) {
            self.glyph(g, r.x + 158. * s, r.y + 11. * s, 16. * s, pal().title_ink);
        }
        self.effects(id, r, s);
    }
    fn draw_full(&self, id: FaceId, r: Rect) {
        self.draw(id, r);
    }
    fn effects(&self, id: FaceId, r: Rect, s: f32) {
        let mut layouts = self.layouts.borrow_mut();
        let layout = layouts
            .entry((id, r.w.to_bits()))
            .or_insert_with(|| layout_effects(id, s, self.martian_internet_printed));
        let (bx, by) = (r.x + FX_BOX.x * s, r.y + FX_BOX.y * s);
        let size = layout.size;
        let line = size * 1.16;
        let inline = size * 1.22;
        for (x, y, fx) in &layout.items {
            let (x, y) = (bx + x, by + y);
            let top = y + (line - inline) / 2. - size * 0.04;
            match fx {
                Fx::Word(text) => {
                    draw_face(text, x, y + size * 0.86, size, pal().card_ink, Face::Body);
                }
                Fx::Demand(text) => {
                    draw_face(text, x, y + size * 0.86, size, pal().demand, Face::Title);
                }
                Fx::Icon(icon) => self.icon(*icon, x, top, inline),
                Fx::Age(age) => self.age(*age, x, top, inline, pal().card_ink),
                Fx::Hex => self.tile(Key::Hex, x, top, inline),
                Fx::Echo => self.glyph(Glyph::Echo, x, top, inline, pal().card_ink),
                Fx::Food => self.tile(Key::Food, x, top, inline),
                Fx::Box => self.tile(Key::Age, x, top, inline),
                Fx::Mark(g) => self.glyph(*g, x, top, inline, pal().card_ink),
            }
        }
    }
}

/// Echo effects first (marked with the ripple), then the dogma effects, each
/// led by its featured icon, or a Figure's karmas. Text tokens like `[5]`
/// and `{[CONCEPT]}` become age discs and icon tiles.
fn effect_runs(id: FaceId, martian_internet_printed: bool) -> Vec<Vec<(Fx, bool)>> {
    paragraphs(id, martian_internet_printed)
        .iter()
        .map(|p| {
            let mut runs = match p.lead {
                Lead::Echo => vec![(Fx::Echo, false)],
                Lead::Icon(icon) => vec![(Fx::Icon(icon), false)],
                Lead::Food => vec![(Fx::Food, false)],
                Lead::Karma => vec![(Fx::Mark(Glyph::Karma), false)],
                Lead::Decree => vec![(Fx::Mark(Glyph::Decree), false)],
            };
            text_runs(&p.text, &mut runs);
            runs
        })
        .collect()
}

/// Appends words and inline symbols; `true` glues a run to the previous one.
fn text_runs(text: &str, out: &mut Vec<(Fx, bool)>) {
    // An empty box would otherwise split into two words.
    let text = text.replace("[ ]", "[BOX]");
    let mut text = text.trim();
    for lead in ["I DEMAND", "I COMPEL"] {
        if let Some(rest) = text.strip_prefix(lead) {
            out.push((Fx::Demand(lead.into()), false));
            text = rest;
        }
    }
    let word = |t: &str| Fx::Word(t.into());
    for chunk in text.split_whitespace() {
        let mut rest = chunk;
        let mut glue = matches!(chunk, "." | "!" | "," | ";");
        while !rest.is_empty() {
            let Some(open) = rest.find('[') else {
                out.push((word(rest.trim_matches(['{', '}'])), glue));
                break;
            };
            let before = rest[..open].trim_end_matches('{');
            if !before.is_empty() {
                out.push((word(before), glue));
                glue = true;
            }
            let Some(close) = rest[open..].find(']') else {
                out.push((word(&rest[open..]), glue));
                break;
            };
            let token = &rest[open + 1..open + close];
            let fx = if token == "IMAGE" || token == "HEX" {
                Fx::Hex
            } else if token == "FOOD" {
                Fx::Food
            } else if token == "BOX" {
                Fx::Box
            } else if let Ok(age) = token.parse() {
                Fx::Age(age)
            } else if let Some(icon) = icon_named(token) {
                Fx::Icon(icon)
            } else {
                word(token)
            };
            out.push((fx, glue));
            glue = true;
            rest = rest[open + close + 1..].trim_start_matches('}');
        }
    }
}

/// Lays out a card title in a band `avail` card units wide; `width` gives a
/// text's width at size 1. A title that would shrink below
/// TITLE_SPLIT_BELOW on one line breaks at the space that allows the largest
/// size, the most balanced break among equals, as on the printed cards.
/// Returns the lines and their size in card units.
fn title_lines(name: &str, avail: f32, width: impl Fn(&str) -> f32) -> (Vec<String>, f32) {
    let fit = |w: f32, max: f32| if w > 0. { (avail / w).min(max) } else { max };
    let one = fit(width(name), TITLE_SIZE).max(5.);
    if one >= TITLE_SPLIT_BELOW {
        return (vec![name.to_owned()], one);
    }
    let best = name
        .match_indices(' ')
        .map(|(i, _)| {
            let (a, b) = (&name[..i], &name[i + 1..]);
            let (wa, wb) = (width(a), width(b));
            (fit(wa.max(wb), TITLE_TWO_LINE_MAX), (wa - wb).abs(), a, b)
        })
        .max_by(|x, y| x.0.total_cmp(&y.0).then(y.1.total_cmp(&x.1)));
    match best {
        Some((size, _, a, b)) if size > one => (vec![a.to_owned(), b.to_owned()], size),
        _ => (vec![name.to_owned()], one),
    }
}

/// Wraps a card's effects into the effect box at the largest size that fits.
fn layout_effects(id: FaceId, s: f32, martian_internet_printed: bool) -> FxLayout {
    let paragraphs = effect_runs(id, martian_internet_printed);
    let (box_w, box_h) = (FX_BOX.w * s, FX_BOX.h * s);
    let mut best = FxLayout {
        size: 5. * s,
        items: Vec::new(),
    };
    for units in [
        8.8, 8.5, 8.2, 7.9, 7.6, 7.3, 7., 6.8, 6.6, 6.4, 6.2, 6., 5.8, 5.6, 5.4,
    ] {
        let size = units * s;
        let line = size * 1.16;
        let space = size * 0.26;
        let mut items = Vec::new();
        let mut y = 0.;
        for runs in &paragraphs {
            let mut x = 0.;
            for (fx, glue) in runs {
                let w = match fx {
                    Fx::Word(t) => measure_face(t, size, Face::Body),
                    Fx::Demand(t) => measure_face(t, size, Face::Title),
                    Fx::Icon(_)
                    | Fx::Age(_)
                    | Fx::Hex
                    | Fx::Echo
                    | Fx::Food
                    | Fx::Box
                    | Fx::Mark(_) => size * 1.22,
                };
                let gap = if *glue || x == 0. { 0. } else { space };
                if x > 0. && x + gap + w > box_w && !glue {
                    x = 0.;
                    y += line;
                } else {
                    x += gap;
                }
                items.push((x, y, fx.clone()));
                x += w;
            }
            y += line + 2.4 * s;
        }
        best = FxLayout { size, items };
        if y - 2.4 * s <= box_h {
            break;
        }
    }
    best
}

// --------------------------------------------------------------- view state

#[derive(Default)]
struct View {
    /// Card shown enlarged this frame (a hovered card name, here or in the log).
    zoom: Option<FaceId>,
    tooltip: Option<String>,
    /// Bonus icons drawn under the tooltip's text (the score pile's hover).
    tooltip_bonuses: Vec<u8>,
    /// Card named by the hovered choice button.
    highlight: Option<FaceId>,
    clicked_card: Option<FaceId>,
    /// A clicked card with several options: the choice bar shows only those.
    card_options: Option<FaceId>,
    /// Card opened full size by a right click; the next click closes it.
    pinned: Option<FaceId>,
    clicked_achievement: Option<(u8, Set)>,
    /// Stays open while the pointer moves from a Safe stat into its card backs.
    safe: Option<(usize, Rect)>,
    popup: Option<Popup>,
    /// The decision whose pile popup was already opened automatically.
    auto_popup: Option<usize>,
    /// First visible hand row.
    hand_scroll: usize,
    copied_at: Option<f64>,
    copying: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    copy_error: Option<String>,
    choices_page: usize,
    tab: usize,
    /// None follows the newest log entry.
    log_scroll: Option<f32>,
    /// While the log's scrollbar is dragged: where on the thumb it was grabbed.
    log_drag: Option<f32>,
    help_scroll: f32,
    log: Option<LogCache>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pile {
    Hand,
    Forecast,
    Score,
}

/// Windows over the table; the choice panel stays usable beneath them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Popup {
    Stack(usize, CardColor),
    Museums(usize),
    Pile(usize, Pile),
    /// A player's achievements: standard cards face down, then the tokens
    /// and issued Decrees.
    Achievements(usize),
    /// The junk: fronts for a seat the engine lets look (von Braun), else backs.
    Junk,
    /// Every card of one set and age: a reference, not the deck's contents.
    Deck(Set, u8),
}

fn legal_card(game: &Game, id: FaceId) -> bool {
    game.human_turn() && game.view.option_cards().any(|c| c == id)
}

// -------------------------------------------------------------- rich text

/// A text colour role, looked up when drawn so cached log rows follow the
/// theme.
#[derive(Clone, Copy)]
enum Tone {
    Text,
    You,
    Rival,
    Hue(CardColor),
}
impl Tone {
    fn color(self) -> Color {
        match self {
            Tone::Text => pal().ink,
            Tone::You => pal().you,
            Tone::Rival => pal().rival,
            Tone::Hue(c) => hue(c),
        }
    }
}

#[derive(Clone)]
enum Piece {
    Word(String, bool, Tone),
    Card(FaceId, String),
    Back(Set, u8),
    Icon(Icon),
}

fn pieces(segs: &[Seg], human: usize, two_player: bool) -> Vec<Piece> {
    fn words(out: &mut Vec<Piece>, text: &str, bold: bool, tone: Tone) {
        for word in text.split_inclusive(' ') {
            out.push(Piece::Word(word.into(), bold, tone));
        }
    }
    let mut out = Vec::new();
    for seg in segs {
        match seg {
            Seg::Text(t) => words(&mut out, t, false, Tone::Text),
            Seg::Strong(t) => words(&mut out, t, true, Tone::Text),
            Seg::Name(p) => words(
                &mut out,
                name(*p, human, two_player),
                true,
                if *p == human { Tone::You } else { Tone::Rival },
            ),
            Seg::Hue(c) => words(&mut out, &c.name().to_lowercase(), true, Tone::Hue(*c)),
            Seg::Card(id) => out.push(Piece::Card(*id, title_case(&face(*id).title))),
            Seg::Back(set, age) => out.push(Piece::Back(*set, *age)),
            Seg::Icon(icon) => out.push(Piece::Icon(*icon)),
        }
    }
    out
}

fn piece_width(p: &Piece, size: f32) -> f32 {
    match p {
        Piece::Word(t, bold, _) => measure(t, size, *bold),
        Piece::Card(_, title) => measure(title, size * 0.93, false) + size + 12.,
        Piece::Back(..) | Piece::Icon(_) => size + 6.,
    }
}

/// Wraps pieces into rows of (x, piece) no wider than `width`.
fn flow(pieces: Vec<Piece>, width: f32, size: f32) -> Vec<Vec<(f32, Piece)>> {
    let mut rows = vec![Vec::new()];
    let mut x = 0.;
    for p in pieces {
        let w = piece_width(&p, size);
        if x + w > width && x > 0. {
            rows.push(Vec::new());
            x = 0.;
        }
        rows.last_mut().unwrap().push((x, p));
        x += w;
    }
    rows
}

/// Draws one piece whose row starts at `top`; hovering a card chip zooms it.
fn draw_piece(p: &Piece, x: f32, top: f32, size: f32, input: &Input, art: &Art, view: &mut View) {
    let h = size + 5.;
    let base = top + size * 1.02;
    match p {
        Piece::Word(t, bold, tone) => draw_str(t, x, base, size, tone.color(), *bold),
        Piece::Card(id, title) => {
            let c = face(*id);
            let tw = measure(title, size * 0.93, false);
            let r = Rect::new(x + 1., top, tw + size + 8., h);
            rrect(r, 3., band(c.color));
            draw_str(title, r.x + 4., base, size * 0.93, pal().title_ink, true);
            let age = Rect::new(r.right() - size - 1., r.y + 1., size, h - 2.);
            rrect(age, 2., shade(c.color));
            centered_title(
                &c.age.to_string(),
                age.center().x,
                base - 0.5,
                size * 0.8,
                pal().title_ink,
            );
            if r.contains(input.mouse) {
                view.zoom = Some(*id);
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, pal().glow);
                if input.right_click {
                    view.pinned = Some(*id);
                }
            }
        }
        Piece::Back(set, age) => {
            let r = Rect::new(x + 2., top, size + 2., h);
            back_chip(*set, *age, r);
            if r.contains(input.mouse) {
                view.tooltip = Some(format!("A hidden {} {age} card", set.name()));
            }
        }
        Piece::Icon(icon) => art.icon(*icon, x + 2., top + 1., size + 2.),
    }
}

// --------------------------------------------------------------------- log

/// The viewer's log as plain text for the clipboard.
fn log_text(game: &Game) -> String {
    let mut out = String::new();
    for entry in &game.view.log {
        match entry {
            Entry::Header(text, _) => out.push_str(&format!("\n{text}\n")),
            Entry::Gap => {}
            Entry::Line(level, segs, count) => {
                out.push_str(&"  ".repeat(usize::from(*level) + 1));
                out.push_str(&plain(segs, game.human, game.two_player));
                if *count > 1 {
                    out.push_str(&format!(" ×{count}"));
                }
                out.push('\n');
            }
        }
    }
    out.trim_start().to_owned()
}

/// The quick reference the help screen prints.
const RULES: &str = "Two actions per turn; the starting player takes one action on the first turn.\n\
Draw from your highest top-card value. With Echoes, a unique highest top card draws Echoes.\n\
Empty base ages are skipped; an empty expansion deck falls back to Base.\n\
With Echoes, a Meld action promotes one eligible Forecast card for a separate free Dogma.\n\
Choose Dogma on a top card to execute its visible echoes and dogma effects.\n\
Achievements need a top card of at least that value and 5 × value points.\n\
Each achievement already owned of that value adds another 5 × value to the cost.\n\
Six achievements plus one per expansion win at the end of an action. A draw above 11 ends by score.\n\
Self-executing or super-executing a card inside a card effect first draws and achieves an 11 (the Chain rule).\n\
With Figures, Figure cards carry karmas (Would, When and Each clauses) and Decree permissions instead of dogmas.\n\
With Cities, a new-color Meld action or a splay draws a City unless you have one in hand.\n\
City icons trigger only on Meld actions; Cities cannot be chosen for Dogma.\n\
Once per turn, a qualifying top City lets you junk a hand card to endorse a Dogma (including a promoted Dogma).\n\
Endorse doubles your non-demand effects and demands, including echoes; sharing still executes once.\n\
Visible fountains count as achievements; flags count if no opponent has more visible cards of that color.\n\
With Artifacts, a Meld action digs when the covered card has at least the new card's value or the image positions match.\n\
Dig at the covered value; you may instead seize an opponent's Museum holding that value.\n\
At turn start, optionally take your display's free Dogma, then store that Artifact in a Museum (or hand if none remain).\n\
When the last Museum fills, return all stored Artifacts; the sole player with most Museums claims one as an achievement.\n\
Martian Internet starts its printed subgame.\n\
Special achievements (or claim via the named card):\n\
  Monument: 4 top demands / Masonry. Empire: 3 of each non-Avatar standard icon / Construction.\n\
  World: 12 Efficiency / Translation. Wonder: 5 colors splayed right or better / Invention.\n\
  Universe: 5 top cards of value 8+ / Astronomy.\n\
  Supremacy: 3 of one icon type in each of 4 colors / Novel. Destiny: 5 Forecast cards / Barometer.\n\
  Wealth: 8 bonuses / Palampore. Heritage: 8 visible images in one color / Loom.\n\
  History: 4 visible echo effects in one color / Photography.\n";

const LOG_SIZE: f32 = 14.;
const LOG_ROW: f32 = 20.;

enum Row {
    Header(String, bool),
    Items(f32, Vec<(f32, Piece)>),
}
struct LogCache {
    /// The view version and width it was laid out for.
    key: (u64, u32),
    rows: Vec<(f32, Row)>,
    height: f32,
}

fn build_log(game: &Game, width: f32) -> LogCache {
    let mut rows = Vec::new();
    let mut y = 4.;
    for entry in &game.view.log {
        match entry {
            Entry::Header(text, current) => {
                y += 6.;
                rows.push((y, Row::Header(text.clone(), *current)));
                y += 26.;
            }
            Entry::Gap => y += 6.,
            Entry::Line(level, segs, count) => {
                let mut segs = segs.clone();
                if *count > 1 {
                    segs.push(Seg::Strong(format!("  ×{count}")));
                }
                let indent = 8. + f32::from(*level) * 16.;
                for (i, row) in flow(
                    pieces(&segs, game.human, game.two_player),
                    width - indent - 12.,
                    LOG_SIZE,
                )
                .into_iter()
                .enumerate()
                {
                    let x = indent + if i > 0 { 12. } else { 0. };
                    rows.push((y, Row::Items(x, row)));
                    y += LOG_ROW;
                }
            }
        }
    }
    LogCache {
        key: (game.version, width.to_bits()),
        rows,
        height: y + 6.,
    }
}

fn log_panel(game: &Game, r: Rect, input: &Input, art: &Art, view: &mut View) {
    let key_matches = view
        .log
        .as_ref()
        .is_some_and(|log| log.key == (game.version, r.w.to_bits()));
    if !key_matches {
        view.log = Some(build_log(game, r.w));
    }
    let log = view.log.take().unwrap();
    panel(r);
    let max = (log.height - r.h).max(0.);
    if r.contains(input.mouse) && input.scroll != 0. {
        let current = view.log_scroll.unwrap_or(max);
        let next = (current - input.scroll.signum() * 60.).clamp(0., max);
        view.log_scroll = (next < max - 1.).then_some(next);
    }
    // The scrollbar: drag the thumb, or click the track to jump there.
    let bar = (r.h * r.h / log.height).max(24.);
    let travel = r.h - bar - 6.;
    let track = Rect::new(r.right() - 14., r.y + 3., 14., r.h - 6.);
    if max > 0. && input.click && track.contains(input.mouse) {
        let top = track.y + travel * view.log_scroll.unwrap_or(max).min(max) / max;
        let grab = input.mouse.y - top;
        view.log_drag = Some(if (0. ..=bar).contains(&grab) {
            grab
        } else {
            bar / 2.
        });
    }
    if let Some(grab) = view.log_drag {
        if input.held && max > 0. {
            let t = ((input.mouse.y - grab - track.y) / travel).clamp(0., 1.);
            let next = t * max;
            view.log_scroll = (next < max - 1.).then_some(next);
        } else {
            view.log_drag = None;
        }
    }
    let offset = view.log_scroll.unwrap_or(max).min(max);
    for (y, row) in &log.rows {
        let top = r.y + y - offset;
        let h = if matches!(row, Row::Header(..)) {
            22.
        } else {
            LOG_ROW
        };
        if top < r.y + 2. || top + h > r.bottom() - 2. {
            continue;
        }
        match row {
            Row::Header(text, current) => {
                let band = Rect::new(r.x + 6., top, r.w - 12., 22.);
                // A wash fading to the right, stronger for the current turn.
                let wash = pal().log_wash;
                let strength = wash.a * if *current { 1. } else { 0.52 };
                for i in 0..8 {
                    let t = i as f32 / 8.;
                    draw_rectangle(
                        band.x + band.w * t,
                        band.y,
                        band.w / 8. + 0.5,
                        band.h,
                        Color {
                            a: strength * (1. - t),
                            ..wash
                        },
                    );
                }
                draw_rectangle(band.x, band.y, 2., band.h, pal().accent);
                title(
                    text,
                    band.x + 9.,
                    band.y + 16.,
                    13.,
                    if *current {
                        pal().log_head_now
                    } else {
                        pal().log_head
                    },
                );
            }
            Row::Items(x, items) => {
                for (px, piece) in items {
                    draw_piece(piece, r.x + x + px, top, LOG_SIZE, input, art, view);
                }
            }
        }
    }
    if max > 0. {
        let active = view.log_drag.is_some() || track.contains(input.mouse);
        let (x, w) = if active {
            (r.right() - 9., 6.)
        } else {
            (r.right() - 7., 4.)
        };
        rrect(
            Rect::new(x, track.y + travel * offset / max, w, bar),
            w / 2.,
            if active { pal().accent } else { pal().line },
        );
    }
    if view.log_scroll.is_some()
        && input.button(
            "Jump to latest",
            Rect::new(r.center().x - 70., r.bottom() - 40., 140., 30.),
            true,
            Style::Primary,
        )
    {
        view.log_scroll = None;
    }
    view.log = Some(log);
}

fn help_panel(r: Rect, input: &Input, view: &mut View) {
    panel(r);
    let text = format!(
        "HOW TO PLAY\n\nWhen it is your move, legal cards glow. Click one to choose it, or click a button in the choice panel below your hand (keys 1-9 work too).\n\nClick a glowing supply achievement to claim it. Hover a player's Safe to see its ordered secrets; click a glowing secret to choose it. Safe numbers show count/capacity. Unknown fronts stay hidden, even in your own Safe.\n\nRight-click a card, or a card name in the log, to see it full size; click again to close it. Hovering a name in the log also previews it. Copy log puts the whole log on the clipboard.\n\nScroll over your hand to see more rows. Each player's summary counts every zone: achievements, score, hand, forecast, Safe and Museums, with the limit after a slash. Issued Decrees sit with the achievements. Hover a count for details and click it to see the cards: fronts for your own hand and forecast, backs for the opponent's unless a rule lets you look (Emperor Meiji's borrowed hand, a Hitchhiking controller's inspection). Click the junk count to see the junk; with Wernher von Braun it shows its fronts. The forecast opens by itself when you promote.\n\nUnder each stack: the splay direction, the icons, bonuses and echo effects that buried cards add, and the stack size. Click the size to see every card in the stack. With Artifacts, the sixth column is the artifact on display.\n\nIn 2-player mode, Switch seats changes your view. Autoswitch follows the next actual choice, including shared effects and demands; forced choices and shortcuts resolve first.\n\nUndo (Ctrl+Z) restores your previous choice (either player's in 2-player mode). Esc opens setup. F11 toggles fullscreen. F12 saves a screenshot.\n\nQUICK REFERENCE\n\n{RULES}\n\nInari plays Innovation, a game by Carl Chudyk. Fonts: Cinzel and Alegreya Sans (SIL Open Font License)."
    );
    let size = 15.;
    let all = lines(&text, r.w - 28., size);
    let row = size * 1.35;
    let visible = ((r.h - 24.) / row).floor() as usize;
    let max = all.len().saturating_sub(visible) as f32;
    if r.contains(input.mouse) && input.scroll != 0. {
        view.help_scroll -= input.scroll.signum() * 3.;
    }
    view.help_scroll = view.help_scroll.clamp(0., max);
    for (i, line) in all
        .iter()
        .skip(view.help_scroll as usize)
        .take(visible)
        .enumerate()
    {
        let caps = !line.is_empty() && line.chars().all(|c| !c.is_lowercase());
        draw_str(
            line,
            r.x + 14.,
            r.y + 14. + size + i as f32 * row,
            size,
            if caps { pal().accent } else { pal().ink },
            caps,
        );
    }
}

// ------------------------------------------------------------------- table

struct Stats {
    achievements: usize,
    score: i64,
    icons: [usize; 7],
    food: usize,
}
fn stats(game: &Game, player: usize) -> Stats {
    let p = &game.view.players[player];
    Stats {
        achievements: p.achievement_count,
        score: p.score_total,
        icons: p.icons,
        food: p.food,
    }
}
/// A card's code, title, face-up value and color on one line.
fn face_line(id: FaceId) -> String {
    let f = face(id);
    format!("{} {} [{} {}]", f.code, f.title, f.value, f.color.name())
}
/// A card's public back, such as "Echoes 3".
fn back(id: FaceId) -> String {
    let f = face(id);
    format!("{} {}", f.set.name(), f.age)
}

/// A zone counted in a player's summary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Zone {
    Achievements,
    Score,
    Hand,
    Forecast,
    Safe,
    Museums,
}

impl Zone {
    fn glyph(self) -> Glyph {
        match self {
            Zone::Achievements => Glyph::Achievement,
            Zone::Score => Glyph::Score,
            Zone::Hand => Glyph::Hand,
            Zone::Forecast => Glyph::Forecast,
            Zone::Safe => Glyph::Safe,
            Zone::Museums => Glyph::Museum,
        }
    }

    /// The zones a summary shows: achievements, score and hand always, the
    /// others when their expansion is in play or the zone holds something.
    fn shown(game: &Game, player: usize) -> Vec<Zone> {
        let v = &game.view;
        let p = &v.players[player];
        let mut zones = vec![Zone::Achievements, Zone::Score, Zone::Hand];
        if v.echoes || !p.forecast.is_empty() {
            zones.push(Zone::Forecast);
        }
        if v.unseen || !p.safe.is_empty() {
            zones.push(Zone::Safe);
        }
        if v.artifacts || !p.museums.is_empty() {
            zones.push(Zone::Museums);
        }
        zones
    }

    /// The count shown and, after it in small type, a zone's limit
    /// ("/9") or the score pile's card count ("(17)").
    fn numbers(self, game: &Game, player: usize) -> (String, Option<String>) {
        let v = &game.view;
        let p = &v.players[player];
        match self {
            Zone::Achievements => (
                p.achievement_count.to_string(),
                Some(format!("/{}", v.achievements_needed)),
            ),
            Zone::Score => (
                p.score_total.to_string(),
                Some(format!("({})", p.score.len())),
            ),
            Zone::Hand => (p.hand.len().to_string(), None),
            Zone::Forecast => (
                p.forecast.len().to_string(),
                Some(format!("/{}", p.forecast_limit)),
            ),
            Zone::Safe => (p.safe.len().to_string(), Some(format!("/{}", p.safe_limit))),
            Zone::Museums => (p.museums.len().to_string(), None),
        }
    }

    /// Whether a legal choice names a card in this zone. Hand cards glow in
    /// the hand itself.
    fn has_legal(self, game: &Game, player: usize) -> bool {
        let p = &game.view.players[player];
        let cards: &[FaceId] = match self {
            Zone::Score => &p.score,
            Zone::Forecast => &p.forecast,
            Zone::Safe => return game.human_turn() && p.safe.iter().any(|c| c.legal),
            _ => return false,
        };
        cards.iter().any(|&id| legal_card(game, id))
    }

    /// The window a click opens; the Safe opens on hover instead.
    fn popup(self, player: usize) -> Option<Popup> {
        match self {
            Zone::Achievements => Some(Popup::Achievements(player)),
            Zone::Score => Some(Popup::Pile(player, Pile::Score)),
            Zone::Hand => Some(Popup::Pile(player, Pile::Hand)),
            Zone::Forecast => Some(Popup::Pile(player, Pile::Forecast)),
            Zone::Museums => Some(Popup::Museums(player)),
            Zone::Safe => None,
        }
    }
}

fn zone_tip(zone: Zone, game: &Game, player: usize) -> String {
    let v = &game.view;
    let p = &v.players[player];
    match zone {
        Zone::Achievements => {
            let held = p
                .achievements
                .iter()
                .map(|a| match a {
                    Held::Card(c) => back(*c),
                    Held::Special(s) => title_case(s.name()),
                    Held::Museum(id) => format!("Museum {}", id + 1),
                })
                .chain(p.decrees.iter().map(|(d, _)| format!("{d} Decree")))
                .collect::<Vec<_>>();
            format!(
                "Achievements: {} of {} needed to win{}{}",
                p.achievement_count,
                v.achievements_needed,
                if v.cities {
                    " (includes active flags and fountains)"
                } else {
                    ""
                },
                if held.is_empty() {
                    String::new()
                } else {
                    format!("\n{}", held.join(", "))
                }
            )
        }
        Zone::Score => {
            let mut values: Vec<u8> = p.score.iter().map(|&id| face(id).value).collect();
            values.sort_unstable_by(|a, b| b.cmp(a));
            let cards: i64 = values.iter().map(|&v| i64::from(v)).sum();
            format!(
                "Score {} ({} cards in the score pile, {} from bonuses){}{}",
                p.score_total,
                p.score.len(),
                p.bonus_score,
                if p.score_visible && !values.is_empty() {
                    format!(
                        "\nCard values: {} (total {cards})",
                        values
                            .iter()
                            .map(u8::to_string)
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                } else if !p.score_visible {
                    format!("\nBacks: {}", backs(&p.score))
                } else {
                    String::new()
                },
                if p.bonuses.is_empty() {
                    String::new()
                } else {
                    "\nBonus icons: the highest scores its value, each other one point:".to_string()
                },
            )
        }
        Zone::Hand => format!(
            "Hand: {} cards{}",
            p.hand.len(),
            if !p.hand_visible {
                format!("\nBacks: {}", backs(&p.hand))
            } else {
                String::new()
            }
        ),
        Zone::Forecast => format!(
            "Forecast: {} cards, limit {}{}",
            p.forecast.len(),
            p.forecast_limit,
            if !p.forecast_visible {
                format!("\nBacks: {}", backs(&p.forecast))
            } else {
                String::new()
            }
        ),
        Zone::Safe => format!("Safe: {} secrets / {} capacity", p.safe.len(), p.safe_limit),
        Zone::Museums => format!(
            "{} Museum{}. {} available; {} junked.",
            p.museums.len(),
            if p.museums.len() == 1 { "" } else { "s" },
            v.supply.museums_available,
            v.supply.museums_junked
        ),
    }
}

fn safe_popup_rect(anchor: Rect, count: usize) -> Rect {
    let width = (count.max(1) as f32 * 118. + 24.).max(330.);
    Rect::new(
        anchor.x.clamp(8., W - width - 8.),
        anchor.bottom(),
        width,
        152.,
    )
}

/// The pulsing glow around a legal choice.
fn glow(r: Rect) {
    let pulse = (get_time() * 3.).sin() as f32 * 0.5 + 0.5;
    for i in 1..=6 {
        let d = i as f32 * 1.6 + 1.;
        let a = (0.3 - i as f32 * 0.045).max(0.) * (0.65 + 0.35 * pulse);
        draw_rectangle_lines(
            r.x - d,
            r.y - d,
            r.w + 2. * d,
            r.h + 2. * d,
            1.8,
            Color {
                a,
                ..pal().glow_halo
            },
        );
    }
    draw_rectangle_lines(r.x - 2., r.y - 2., r.w + 4., r.h + 4., 2., pal().glow);
}

/// Back-only rendering cannot accidentally reveal a hidden card's face.
/// Backs show what is public: the set (its emblem and name) and the age.
fn card_back(art: &Art, set: Set, age: u8, r: Rect, legal: bool) {
    if legal {
        glow(r);
    }
    let ink = set_on_back(set);
    boxed(r, 5., set_color(set), rgba(pal().enamel.rim));
    let wire = Color {
        a: 0.5,
        ..rgba(pal().enamel.rim)
    };
    draw_rectangle_lines(r.x + 4., r.y + 4., r.w - 8., r.h - 8., 1., wire);
    // Laid out for the 77-unit Safe tile; larger backs scale up.
    let s = r.h / 77.;
    let band = Rect::new(r.x + 4., r.y + 46. * s, r.w - 8., 1.);
    draw_rectangle(band.x, band.y, band.w, 1., wire);
    let emblem = 24. * s;
    let (ex, ey) = (r.x + 9. * s, r.y + 10. * s);
    match set_glyph(set) {
        Some(g) => art.glyph(g, ex, ey, emblem, ink),
        None => brand(ex, ey, emblem),
    }
    title(
        set.name(),
        ex + emblem + 6. * s,
        ey + emblem * 0.68,
        12. * s,
        ink,
    );
    centered_title(&age.to_string(), r.center().x, r.y + 70. * s, 22. * s, ink);
}

/// A small face-down card: its set colour and age are public.
fn back_chip(set: Set, age: u8, r: Rect) {
    rrect(r, 2., set_color(set));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1., pal().back_edge);
    centered_title(
        &age.to_string(),
        r.center().x,
        r.y + r.h * 0.73,
        r.h * 0.6,
        set_on_back(set),
    );
}

fn safe_popup(game: &mut Game, input: &Input, art: &Art, view: &mut View) {
    let Some((player, anchor)) = view.safe else {
        return;
    };
    let cards = game.view.players[player].safe.clone();
    let r = safe_popup_rect(anchor, cards.len());
    if !anchor.contains(input.mouse) && !r.contains(input.mouse) {
        view.safe = None;
        return;
    }
    view.tooltip = None;
    view.tooltip_bonuses.clear();
    view.zoom = None;
    panel(r);
    title(
        &format!(
            "{} Safe · {}/{} · oldest to newest",
            if player == game.human {
                "Your"
            } else {
                "Opponent's"
            },
            cards.len(),
            game.view.players[player].safe_limit
        ),
        r.x + 12.,
        r.y + 23.,
        13.,
        pal().accent,
    );
    if cards.is_empty() {
        label("No secrets", r.x + 12., r.y + 66., 16., pal().muted);
    }
    let mut picked = None;
    for (i, card) in cards.iter().enumerate() {
        let tile = Rect::new(r.x + 12. + i as f32 * 118., r.y + 34., 108., 77.);
        let legal = card.legal && game.human_turn();
        card_back(art, card.set, card.age, tile, legal);
        centered(
            &format!(
                "#{}{}",
                i + 1,
                if card.known.is_some() {
                    " · known"
                } else {
                    ""
                }
            ),
            tile.center().x,
            tile.bottom() + 17.,
            12.,
            pal().muted,
            false,
        );
        if tile.contains(input.mouse) {
            view.tooltip = Some(
                card.known
                    .map(face_line)
                    .unwrap_or_else(|| "Unknown secret; its front cannot be inspected.".into()),
            );
            if legal && input.click {
                picked = Some(i);
            }
        }
    }
    label(
        "Click a glowing back to select that secret.",
        r.x + 12.,
        r.bottom() - 8.,
        11.,
        pal().muted,
    );
    if let Some(position) = picked {
        game.choose_secret(player, position);
        view.safe = None;
        view.choices_page = 0;
    }
}

/// Card-back colours by set, as printed: Base tan, Echoes blue, Figures
/// green, Cities red, Artifacts purple, Unseen gray.
fn set_color(set: Set) -> Color {
    pal().set(set).back
}

/// The age, name and emblem printed on a back of the set.
fn set_on_back(set: Set) -> Color {
    pal().set(set).on_back
}

/// The same hues, readable for numbers on the panels.
fn set_ink(set: Set) -> Color {
    pal().set(set).ink
}

/// The set whose supply a special achievement comes from.
fn special_set(sp: Special) -> Set {
    if Special::ECHOES.contains(&sp) {
        Set::Echoes
    } else if Special::CITIES.contains(&sp) {
        Set::Cities
    } else if Special::UNSEEN.contains(&sp) {
        Set::Unseen
    } else {
        Set::Base
    }
}

/// Each expansion's emblem; Base cards carry the Inari mark instead.
fn set_glyph(set: Set) -> Option<Glyph> {
    match set {
        Set::Base => None,
        Set::Echoes => Some(Glyph::Echoes),
        Set::Cities => Some(Glyph::Cities),
        Set::Unseen => Some(Glyph::Unseen),
        Set::Artifacts => Some(Glyph::Artifacts),
        Set::Figures => Some(Glyph::Figures),
    }
}

/// How a special achievement is won, in the words of its card.
fn special_summary(sp: Special) -> String {
    const END: &str = "Claim at the end of any action if you have";
    const NOW: &str = "Claim immediately when you";
    let (when, how, via): (&str, &str, Option<(&str, u8)>) = match sp {
        Special::Monument => (
            END,
            "at least four top cards with a demand effect",
            Some(("Masonry", 1)),
        ),
        Special::Empire => (
            END,
            "at least three of each of the six standard icons (Authority, Concept, Health, Prosperity, Industry, Efficiency) on your board",
            Some(("Construction", 2)),
        ),
        Special::World => (
            END,
            "at least twelve Efficiency icons on your board",
            Some(("Translation", 3)),
        ),
        Special::Wonder => (
            END,
            "five colors splayed, each right, up or aslant",
            Some(("Invention", 4)),
        ),
        Special::Universe => (
            END,
            "five top cards, each of value 8 or higher",
            Some(("Astronomy", 5)),
        ),
        Special::Supremacy => (
            END,
            "at least three icons of one type in each of four different colors (the same type in every color)",
            Some(("Novel", 3)),
        ),
        Special::Destiny => (END, "five cards in your forecast", Some(("Barometer", 4))),
        Special::Wealth => (
            END,
            "at least eight bonuses on your board",
            Some(("Palampore", 5)),
        ),
        Special::Heritage => (
            END,
            "at least eight visible hexes in one color",
            Some(("Loom", 6)),
        ),
        Special::History => (
            END,
            "a color with at least four visible echo effects",
            Some(("Photography", 7)),
        ),
        Special::Tradition => (
            NOW,
            "meld a City with a left arrow onto a color already splayed left",
            None,
        ),
        Special::Repute => (
            NOW,
            "meld a City with a right arrow onto a color already splayed right",
            None,
        ),
        Special::Fame => (
            NOW,
            "meld a City with an up arrow onto a color already splayed up",
            None,
        ),
        Special::Glory => (NOW, "junk a City with a flag from your hand", None),
        Special::Victory => (NOW, "junk a City with a fountain from your hand", None),
        Special::Confidence => (
            END,
            "a top card of value 5 or higher and four or more secrets",
            Some(("Assassination", 1)),
        ),
        Special::Zen => (
            END,
            "a top card of value 6 or higher and no top card of odd value",
            Some(("Meteorology", 2)),
        ),
        Special::Anonymity => (
            END,
            "a top card of value 7 or higher and no standard achievements",
            Some(("Masquerade", 3)),
        ),
        Special::Folklore => (
            END,
            "a top card of value 8 or higher and no Industry icons on your board",
            Some(("April Fool's Day", 4)),
        ),
        Special::Mystery => (
            END,
            "a top card of value 9 or higher and fewer than five colors on your board",
            Some(("Secret History", 5)),
        ),
    };
    let mut text = format!("{}\n{when} {how}.", title_case(sp.name()));
    if let Some((card, age)) = via {
        text.push_str(&format!("\nAlso claimable via {card} (age {age})."));
    }
    text
}

/// A card on the table: the face and the ivory glow when it is a legal
/// choice. A right click opens it full size.
fn table_card(input: &Input, art: &Art, view: &mut View, id: FaceId, r: Rect, legal: bool) {
    let hover = r.contains(input.mouse);
    let s = r.w / CARD_W;
    rrect(
        Rect::new(r.x + 2., r.y + 4., r.w, r.h),
        7. * s,
        pal().card_shadow,
    );
    if legal {
        glow(r);
    }
    if view.highlight == Some(id) {
        draw_rectangle_lines(r.x - 5., r.y - 5., r.w + 10., r.h + 10., 3., pal().you);
    }
    art.draw(id, r);
    if hover && input.right_click {
        view.pinned = Some(id);
    }
    if hover && legal {
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1., 1., 1., 0.12));
        if input.click {
            view.clicked_card = Some(id);
        }
    }
}

/// Buried cards peek out on the side their splay reveals.
fn peeks(st: &StackView, r: Rect, color: CardColor) {
    let (dx, dy, most) = match st.splay {
        Splay::Right => (-5., 0., 3),
        Splay::Left => (5., 0., 3),
        Splay::Up => (0., 4., 3),
        Splay::Aslant => (-4., 3., 3),
        Splay::None => (2., 2., 2),
    };
    let layers = st.cards.len().saturating_sub(1).min(most);
    for k in (1..=layers).rev() {
        let peek = Rect::new(r.x + dx * k as f32, r.y + dy * k as f32, r.w, r.h);
        boxed(
            peek,
            7. * r.w / CARD_W,
            mix(tint(color), BLACK, 0.12 + 0.07 * k as f32),
            pal().peek_edge,
        );
    }
}

/// A rimmed dark tile holding a pale glyph.
fn glyph_tile(art: &Art, g: Glyph, x: f32, y: f32, s: f32) {
    let e = &pal().enamel;
    boxed(
        Rect::new(x, y, s, s),
        s * 0.2,
        rgba(e.disc),
        rgba(e.disc_rim),
    );
    art.glyph(g, x + s * 0.16, y + s * 0.16, s * 0.68, rgba(e.disc_ink));
}

/// Splay direction, what the buried cards add, and the stack size, which
/// opens the whole stack. Returns true when the size is clicked.
fn buried_strip(art: &Art, st: &StackView, card: Rect, input: &Input, view: &mut View) -> bool {
    let (x, y) = (card.x + 2., card.bottom() + STRIP_GAP);
    if st.cards.len() < 2 && st.splay == Splay::None {
        return false;
    }
    let mut counts = [0usize; 7];
    let mut food = 0;
    let mut bonuses = Vec::new();
    let mut echoes = 0;
    let mut images = 0;
    let mut city_icons = Vec::new();
    for (depth, &cid) in st.cards[..st.cards.len().saturating_sub(1)]
        .iter()
        .enumerate()
    {
        let c = face(cid);
        for slot in (0..c.icons.len()).filter(|&slot| st.slot_visible(depth, slot)) {
            if let Some(city_icon) = c.city_icons[slot] {
                city_icons.push(city_icon);
            }
            if c.food_slots.contains(&slot) {
                food += 1;
            } else if let Some(icon) = c.icons[slot] {
                counts[icon as usize] += 1;
            } else if let Some(bonus) = c.bonuses[slot] {
                bonuses.push(bonus);
            } else if c.echo_slots.contains(&slot) {
                echoes += 1;
            } else if slot == c.image_slot {
                images += 1;
            }
        }
    }
    let mut cx = x;
    if let Some(g) = splay_glyph(st.splay) {
        glyph_tile(art, g, cx, y, 18.);
        cx += 24.;
    }
    let plus = |n: usize, cx: f32| {
        let text = format!("+{n}");
        draw_str(&text, cx, y + 14., 13., pal().ink, true);
        measure(&text, 13., true)
    };
    let mut tip = Vec::new();
    for icon in ICONS {
        let n = counts[icon as usize];
        if n == 0 {
            continue;
        }
        art.icon(icon, cx, y + 1., 16.);
        cx += 18. + plus(n, cx + 18.) + 8.;
        tip.push(format!("{n} {}", title_case(icon.name())));
    }
    if food > 0 {
        art.tile(Key::Food, cx, y + 1., 16.);
        cx += 18. + plus(food, cx + 18.) + 8.;
        tip.push(format!("{food} Food"));
    }
    for b in &bonuses {
        art.tile(Key::Bonus, cx, y + 1., 16.);
        centered_title(
            &b.to_string(),
            cx + 8.,
            y + 12.5,
            7.5,
            rgba(pal().enamel.disc_ink),
        );
        cx += 20.;
    }
    if !bonuses.is_empty() {
        tip.push(format!(
            "bonus {}",
            bonuses
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if echoes > 0 {
        art.tile(Key::Echo, cx, y + 1., 16.);
        cx += 18. + plus(echoes, cx + 18.) + 8.;
        tip.push(format!(
            "{echoes} echo effect{}",
            if echoes == 1 { "" } else { "s" }
        ));
    }
    for &c in city_icons.iter().take(4) {
        let i = CityIcon::ALL.iter().position(|&x| x == c).unwrap_or(0) as u8;
        art.sprite(Key::City(i, phys(16.)), Rect::new(cx, y + 1., 16., 16.));
        cx += 19.;
    }
    if !city_icons.is_empty() {
        tip.push(format!(
            "City icons: {}",
            city_icons
                .iter()
                .map(|c| format!("{c:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if images > 0 {
        art.tile(Key::Hex, cx, y + 1., 16.);
        cx += 18. + plus(images, cx + 18.) + 8.;
        tip.push(format!(
            "{images} image hex{}",
            if images == 1 { "" } else { "es" }
        ));
    }
    if Rect::new(x, y, cx - x, 18.).contains(input.mouse) {
        view.tooltip = Some(if st.splay == Splay::None {
            "Not splayed: buried cards show nothing".into()
        } else if tip.is_empty() {
            format!(
                "Splayed {}: buried cards add nothing",
                st.splay.name().to_lowercase()
            )
        } else {
            format!(
                "Splayed {}. Buried cards add: {}",
                st.splay.name().to_lowercase(),
                tip.join(", ")
            )
        });
    }
    let n = st.cards.len();
    let text = format!("{n} card{}", if n == 1 { "" } else { "s" });
    let w = measure(&text, 12.5, true) + 16.;
    let pill = Rect::new(card.right() - w, y, w, 18.);
    let hover = pill.contains(input.mouse);
    boxed(
        pill,
        9.,
        if hover { pal().hover } else { pal().strip },
        if hover { pal().accent } else { pal().line },
    );
    centered(&text, pill.center().x, pill.y + 13.5, 12.5, pal().ink, true);
    if hover {
        view.tooltip = Some(format!("Show all {n} cards in this stack"));
    }
    hover && input.click
}

/// Board columns: the five colors, plus the artifact on display.
fn board_rect(artifacts: bool, column: usize, y: f32) -> Rect {
    let (columns, h) = if artifacts {
        (6., BOARD_H6)
    } else {
        (5., BOARD_H5)
    };
    let w = h * CARD_W / CARD_H;
    let step = (MAIN_W - w) / (columns - 1.);
    Rect::new(MAIN_X + column as f32 * step, y, w, h)
}

fn board(game: &Game, player: usize, y: f32, input: &Input, art: &Art, view: &mut View) {
    let p = &game.view.players[player];
    for (i, color) in COLORS.into_iter().enumerate() {
        let r = board_rect(game.view.artifacts, i, y);
        let st = &p.stacks[i];
        if let Some(top) = st.top() {
            peeks(st, r, color);
            table_card(input, art, view, top, r, legal_card(game, top));
        } else {
            boxed(
                r,
                7.,
                mix(band(color), pal().table, 0.85),
                mix(band(color), pal().table, 0.45),
            );
        }
        if buried_strip(art, st, r, input, view) {
            view.popup = Some(Popup::Stack(player, color));
        }
    }
    if game.view.artifacts {
        display_slot(game, player, board_rect(true, 5, y), input, art, view);
    }
}

/// The artifact on display. Museums live in the player summaries.
fn display_slot(game: &Game, player: usize, r: Rect, input: &Input, art: &Art, view: &mut View) {
    let p = &game.view.players[player];
    if let Some(id) = p.display {
        table_card(input, art, view, id, r, legal_card(game, id));
        art.glyph(
            Glyph::Artifacts,
            r.x + 2.,
            r.bottom() + STRIP_GAP + 1.,
            16.,
            pal().ink,
        );
        label(
            "On display",
            r.x + 22.,
            r.bottom() + STRIP_GAP + 14.,
            12.5,
            pal().ink,
        );
        return;
    }
    let edge = Color {
        a: 0.45,
        ..pal().accent
    };
    let dash = |a: Vec2, b: Vec2| {
        let len = a.distance(b);
        let steps = (len / 10.) as usize;
        for k in 0..steps {
            let t0 = k as f32 / steps as f32;
            let t1 = t0 + 0.55 / steps as f32;
            let (p, q) = (a.lerp(b, t0), a.lerp(b, t1));
            draw_line(p.x, p.y, q.x, q.y, 1.5, edge);
        }
    };
    let (tl, tr) = (vec2(r.x, r.y), vec2(r.right(), r.y));
    let (bl, br) = (vec2(r.x, r.bottom()), vec2(r.right(), r.bottom()));
    dash(tl, tr);
    dash(tr, br);
    dash(br, bl);
    dash(bl, tl);
    art.glyph(
        Glyph::Artifacts,
        r.center().x - 13.,
        r.center().y - 28.,
        26.,
        Color {
            a: 0.4,
            ..pal().ink
        },
    );
    centered_title(
        "Display",
        r.center().x,
        r.center().y + 18.,
        14.,
        pal().label,
    );
    if r.contains(input.mouse) {
        view.tooltip = Some("No artifact on display".into());
    }
}

/// Name, then one cell per zone: achievements, score, hand, forecast, Safe,
/// Museums, as counts with their limits. Hovering a cell names
/// the contents; clicking opens them.
fn player_box(game: &Game, player: usize, r: Rect, input: &Input, art: &Art, view: &mut View) {
    let v = &game.view;
    let active = v.over.is_none() && v.current_player == player;
    panel(r);
    if active {
        draw_rectangle_lines(r.x - 1., r.y - 1., r.w + 2., r.h + 2., 2., pal().glow);
    }
    let color = if player == game.human {
        pal().you
    } else {
        pal().rival
    };
    let who = name(player, game.human, game.two_player);
    title(who, r.x + 12., r.y + 25., 18., color);
    label(
        &format!("Player {}", player + 1),
        r.x + 20. + measure_face(who, 18., Face::Title),
        r.y + 24.,
        13.,
        pal().muted,
    );
    if active {
        let status = if player == game.human {
            "your move"
        } else if game.thinking() {
            "thinking..."
        } else {
            "to move"
        };
        let w = measure_face(status, 10.5, Face::Title) + 18.;
        let pill = Rect::new(r.right() - w - 10., r.y + 9., w, 19.);
        rrect(pill, 9., pal().primary.fill);
        centered_title(
            status,
            pill.center().x,
            pill.y + 13.5,
            10.5,
            pal().primary.ink,
        );
    }
    let zones = Zone::shown(game, player);
    let cell_w = (r.w - 16.) / zones.len() as f32;
    for (i, &zone) in zones.iter().enumerate() {
        let cell = Rect::new(r.x + 8. + i as f32 * cell_w, r.y + 36., cell_w - 4., 54.);
        let hover = cell.contains(input.mouse);
        if hover {
            boxed(cell, 4., pal().hover, pal().line);
        }
        if zone.has_legal(game, player) {
            draw_rectangle_lines(cell.x, cell.y, cell.w, cell.h, 2., pal().glow);
        }
        art.glyph(
            zone.glyph(),
            cell.center().x - 9.,
            cell.y + 5.,
            18.,
            pal().accent,
        );
        let (count, limit) = zone.numbers(game, player);
        let ink = if count == "0" { pal().dim } else { pal().ink };
        let baseline = cell.y + 45.;
        match limit {
            None => centered_title(&count, cell.center().x, baseline, 18., ink),
            Some(small) => {
                let cw = measure_face(&count, 18., Face::Title);
                let sw = measure_face(&small, 12., Face::Title);
                let x = cell.center().x - (cw + sw) / 2.;
                title(&count, x, baseline, 18., ink);
                title(&small, x + cw, baseline, 12., pal().muted);
            }
        }
        if !hover {
            continue;
        }
        if zone == Zone::Safe {
            view.safe = Some((player, cell));
            continue;
        }
        let popup = zone.popup(player);
        view.tooltip = Some(format!(
            "{}{}",
            zone_tip(zone, game, player),
            if popup.is_some() {
                "\nClick to view"
            } else {
                ""
            }
        ));
        if zone == Zone::Score {
            view.tooltip_bonuses = game.view.players[player].bonuses.clone();
        }
        if input.click && popup.is_some() {
            view.popup = popup;
        }
    }
}

/// Decks, achievements, Museums, junk, Decrees and specials between the
/// summaries. With every expansion the rows are full: labels give way to set
/// emblems, then the achievements close up, before anything leaves the panel.
fn supply(game: &Game, r: Rect, input: &Input, art: &Art, view: &mut View) {
    /// Expansion counts per row under each Base count, and in the legend.
    const GRID: usize = 3;
    /// Room for a set emblem standing in for a label.
    const EMBLEM: f32 = 15.;
    let supply = &game.view.supply;
    panel(r);
    let x0 = r.x + 78.;
    let right = r.right() - 8.;
    // Base first; a view without decks draws empty counts rather than failing.
    let empty = DeckRow {
        set: Set::Base,
        counts: [0; 12],
    };
    let (base_row, rest) = supply.decks.split_first().unwrap_or((&empty, &[]));
    // Expansion order: Echoes, Figures, Cities, Artifacts, Unseen.
    let mut expansions: Vec<(Set, &[usize; 12])> =
        rest.iter().map(|row| (row.set, &row.counts)).collect();
    expansions.sort_by_key(|&(set, _)| {
        [
            Set::Echoes,
            Set::Figures,
            Set::Cities,
            Set::Artifacts,
            Set::Unseen,
        ]
        .iter()
        .position(|&s| s == set)
    });
    let rows = expansions.len().div_ceil(GRID);
    // Decks by value: the Base back and count, then the enabled expansions'
    // counts in their set inks, in the grid the legend under the label
    // shows. Clicking a count lists every card of that deck for reference.
    caps("Decks", r.x + 12., r.y + 24., 10.5);
    for (k, (set, _)) in expansions.iter().enumerate() {
        if let Some(g) = set_glyph(*set) {
            art.glyph(
                g,
                r.x + 12. + (k % GRID) as f32 * 14.,
                r.y + 29. + (k / GRID) as f32 * 10.,
                10.,
                set_ink(*set),
            );
        }
    }
    // The Age 0 deck appears only once Age 0 is in play.
    let first = usize::from(!game.view.age_zero);
    let step = (right - x0) / (12 - first) as f32;
    let cell = ((step - 2.) / GRID as f32).min(20.);
    for age in first..=11usize {
        let x = x0 + (age - first) as f32 * step;
        let (chip, size, baseline) = if expansions.is_empty() {
            (Rect::new(x, r.y + 9., 19., 26.), 17., r.y + 28.)
        } else {
            (Rect::new(x, r.y + 6., 16., 21.), 14., r.y + 22.)
        };
        back_chip(Set::Base, age as u8, chip);
        let base = base_row.counts[age];
        let base_text = base.to_string();
        title(
            &base_text,
            chip.right() + 4.,
            baseline,
            size,
            if base == 0 { pal().muted } else { pal().ink },
        );
        let mut targets = vec![(
            Set::Base,
            base,
            Rect::new(
                x - 1.,
                chip.y - 1.,
                chip.w + 6. + measure_face(&base_text, size, Face::Title),
                chip.h + 2.,
            ),
        )];
        for (k, (set, counts)) in expansions.iter().enumerate() {
            let n = counts[age];
            let (nx, ny) = (
                x + (k % GRID) as f32 * cell,
                r.y + 37. + (k / GRID) as f32 * 10.,
            );
            draw_str(
                &n.to_string(),
                nx,
                ny,
                10.5,
                if n == 0 { pal().muted } else { set_ink(*set) },
                true,
            );
            targets.push((*set, n, Rect::new(nx - 2., ny - 9., cell, 10.)));
        }
        for (set, n, hit) in targets {
            if hit.contains(input.mouse) {
                draw_rectangle_lines(hit.x, hit.y, hit.w, hit.h, 1., pal().glow);
                view.tooltip = Some(format!(
                    "{} {age} deck: {n} card{} left\nClick to see every {} age-{age} card",
                    set.name(),
                    if n == 1 { "" } else { "s" },
                    set.name(),
                ));
                if input.click {
                    view.popup = Some(Popup::Deck(set, age as u8));
                }
            }
        }
    }

    // Available standard achievements (including any that effects added),
    // Museums, the junk and the Decrees share one row.
    let y = r.y + if rows < 2 { 44. } else { 51. };
    caps("Achieve", r.x + 12., y + 15., 10.5);
    let available = &supply.achievements;
    let junked = &supply.junk;
    let junk_text = junked.len().to_string();
    let none = "none available";
    let none_w = measure(none, 13., false);
    let label_w = |text: &str| measure_face(text, 10.5, Face::Title) + 8.;
    let decree_chips: Vec<(String, f32)> = supply
        .decrees
        .iter()
        .map(|(decree, _)| {
            let short: String = decree.chars().take(3).collect();
            let w = measure_face(&short, 10., Face::Title) + 12.;
            (short, w)
        })
        .collect();
    let decrees_w = if decree_chips.is_empty() {
        none_w
    } else {
        decree_chips.iter().map(|(_, w)| w + 3.).sum()
    };
    let after = |labels: bool| {
        let head = |text: &str| if labels { label_w(text) } else { EMBLEM };
        let museums = if game.view.artifacts {
            12. + head("Museums") + supply.museums_available as f32 * 23.
        } else {
            0.
        };
        let decrees = if game.view.figures {
            12. + head("Decrees") + decrees_w
        } else {
            0.
        };
        let junk = 12. + label_w("Junk") - 2. + 19. + measure_face(&junk_text, 14., Face::Title);
        museums + junk + decrees
    };
    let room = right - x0;
    let wanted = available.len() as f32 * 20.;
    let labels = available.is_empty() || wanted + after(true) <= room;
    let pitch = if available.is_empty() {
        20.
    } else {
        ((room - after(labels)) / available.len() as f32).clamp(10., 20.)
    };
    let mut x = x0;
    for claimable in available {
        let (age, set) = (claimable.age, claimable.set);
        let tile = Rect::new(x, y, 16., 21.);
        let legal = game.human_turn() && claimable.legal;
        if legal {
            glow(tile);
        }
        back_chip(set, age, tile);
        if Rect::new(tile.x, tile.y, pitch, tile.h).contains(input.mouse) {
            view.tooltip = Some(format!(
                "{} {age} achievement{}",
                set.name(),
                if legal {
                    " · click to achieve"
                } else {
                    " available"
                }
            ));
            if legal && input.click {
                view.clicked_achievement = Some((age, set));
            }
        }
        x += pitch;
    }
    if available.is_empty() {
        label(none, x, y + 15., 13., pal().muted);
        x += none_w;
    } else {
        // The last chip is drawn whole.
        x += 16. - pitch.min(16.) + 4.;
    }
    // A row label, or the emblem of the set that brings the group.
    let head = |text: &str, set: Set, x: &mut f32| {
        if labels {
            caps(text, *x, y + 15., 10.5);
            *x += label_w(text);
        } else {
            if let Some(g) = set_glyph(set) {
                art.glyph(g, *x, y + 5., 11., set_ink(set));
            }
            *x += EMBLEM;
        }
    };
    if game.view.artifacts {
        x += 8.;
        head("Museums", Set::Artifacts, &mut x);
        let start = x;
        for _ in 0..supply.museums_available {
            glyph_tile(art, Glyph::Museum, x, y, 20.);
            x += 23.;
        }
        if Rect::new(start, y, (x - start).max(20.), 21.).contains(input.mouse) {
            view.tooltip = Some(format!(
                "{} Museum{} available",
                supply.museums_available,
                if supply.museums_available == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }
    }
    x += 12.;
    caps("Junk", x, y + 15., 10.5);
    x += label_w("Junk") - 2.;
    art.glyph(Glyph::Junk, x, y + 2., 16., pal().accent);
    title(&junk_text, x + 19., y + 15., 14., pal().ink);
    let hit = Rect::new(x - 40., y, 80., 21.);
    if hit.contains(input.mouse) {
        draw_rectangle_lines(hit.x, hit.y, hit.w, hit.h, 1., pal().glow);
        let specials = supply
            .junk_specials
            .iter()
            .map(|&sp| title_case(sp.name()))
            .collect::<Vec<_>>();
        let plural = if junked.len() == 1 { "" } else { "s" };
        view.tooltip = Some(format!(
            "Junk: {} card{plural}\n{}: {}\nSpecial achievements: {}\nClick to see the junk",
            junked.len(),
            if supply.junk_visible {
                "Cards"
            } else {
                "Backs"
            },
            if supply.junk_visible {
                names(junked)
            } else {
                backs(junked)
            },
            if specials.is_empty() {
                "none".into()
            } else {
                specials.join(", ")
            }
        ));
        if input.click {
            view.popup = Some(Popup::Junk);
        }
    }
    x += 19. + measure_face(&junk_text, 14., Face::Title);
    // Decrees are achievements too: the ones still available to issue.
    if game.view.figures {
        x += 12.;
        head("Decrees", Set::Figures, &mut x);
        for ((decree, color), (short, w)) in supply.decrees.iter().zip(&decree_chips) {
            // A Decree's box carries the tint of the Figures that issue it.
            let chip = Rect::new(x, y + 1., *w, 19.);
            let [_, band, _, tint] = pal().card(*color);
            boxed(chip, 3., tint, band);
            centered_title(short, chip.center().x, y + 15., 10., pal().card_ink);
            if chip.contains(input.mouse) {
                view.tooltip = Some(format!(
                    "{decree} Decree, available\n{}",
                    decree_text(decree)
                ));
            }
            x += w + 3.;
        }
        if supply.decrees.is_empty() {
            label(none, x, y + 15., 13., pal().muted);
        }
    }

    // Available special achievements, one group per set; claimed ones leave
    // the supply. Several groups abbreviate, and each expansion's group
    // carries its emblem and set colour.
    let y = y + 26.;
    caps("Specials", r.x + 12., y + 13., 10.5);
    let groups = &supply.specials;
    let compact = groups.len() > 1;
    let mut x = x0;
    for group in groups {
        let set = group.first().map_or(Set::Base, |&sp| special_set(sp));
        let edge = if set == Set::Base {
            pal().line
        } else {
            set_ink(set)
        };
        if compact && let Some(g) = set_glyph(set) {
            art.glyph(g, x, y + 3., 12., set_ink(set));
            x += EMBLEM;
        }
        for &sp in group {
            let name = title_case(sp.name());
            let short: String = if compact {
                name.chars().take(3).collect()
            } else {
                name.clone()
            };
            let w = if compact {
                27.
            } else {
                measure_face(&short, 10.5, Face::Title) + 14.
            };
            let chip = Rect::new(x, y, w, 18.);
            boxed(chip, 3., pal().strip, edge);
            centered_title(&short, chip.center().x, y + 13., 10., pal().chip_ink);
            if chip.contains(input.mouse) {
                view.tooltip = Some(format!("{}\nAvailable.", special_summary(sp)));
            }
            x += w + if compact { 1. } else { 5. };
        }
        x += 8.;
    }
    if groups.is_empty() {
        label(none, x0, y + 13., 13., pal().muted);
    }
}

fn middle(game: &Game, input: &Input, art: &Art, view: &mut View) {
    let box_w = 320.;
    let rival = 1 - game.human;
    player_box(
        game,
        game.human,
        Rect::new(MAIN_X, MID_Y, box_w, MID_H),
        input,
        art,
        view,
    );
    player_box(
        game,
        rival,
        Rect::new(MAIN_X + MAIN_W - box_w, MID_Y, box_w, MID_H),
        input,
        art,
        view,
    );
    supply(
        game,
        Rect::new(MAIN_X + box_w + 8., MID_Y, MAIN_W - 2. * box_w - 16., MID_H),
        input,
        art,
        view,
    );
}

/// The player whose private piles the human sees: themselves, or the
/// opponent whose execution they control.
fn private_owner(game: &Game) -> usize {
    game.view.private_owner
}

/// A player's hand, forecast or score pile, sorted by the backend.
fn pile_cards(game: &Game, player: usize, pile: Pile) -> Vec<FaceId> {
    let p = &game.view.players[player];
    match pile {
        Pile::Hand => p.hand.clone(),
        Pile::Forecast => p.forecast.clone(),
        Pile::Score => p.score.clone(),
    }
}

fn hand_per_row() -> usize {
    ((MAIN_W + HAND_GAP) / (HAND_W + HAND_GAP)).floor() as usize
}

/// Two full-size rows of hand cards, scrolled a row at a time. The score
/// pile and forecast open from the player summaries.
fn hand(game: &Game, input: &Input, art: &Art, view: &mut View) {
    let player = private_owner(game);
    let area = Rect::new(MAIN_X, HAND_Y, MAIN_W, CHOICE_Y - 6. - HAND_Y);
    let cards = pile_cards(game, player, Pile::Hand);
    let per_row = hand_per_row();
    let rows = cards.len().div_ceil(per_row);
    let max_scroll = rows.saturating_sub(HAND_ROWS);
    if area.contains(input.mouse) && input.scroll != 0. {
        view.hand_scroll = if input.scroll > 0. {
            view.hand_scroll.saturating_sub(1)
        } else {
            view.hand_scroll + 1
        };
    }
    view.hand_scroll = view.hand_scroll.min(max_scroll);
    let top = area.y;
    if cards.is_empty() {
        let slot = Rect::new(area.x, top, HAND_W, HAND_H);
        draw_rectangle_lines(slot.x, slot.y, slot.w, slot.h, 1.5, pal().line);
        centered_title(
            "Empty hand",
            slot.center().x,
            slot.center().y + 5.,
            12.,
            pal().label,
        );
        return;
    }
    let first = view.hand_scroll * per_row;
    for (k, &id) in cards
        .iter()
        .skip(first)
        .take(HAND_ROWS * per_row)
        .enumerate()
    {
        let r = Rect::new(
            area.x + (k % per_row) as f32 * (HAND_W + HAND_GAP),
            top + (k / per_row) as f32 * (HAND_H + HAND_GAP),
            HAND_W,
            HAND_H,
        );
        table_card(input, art, view, id, r, legal_card(game, id));
    }
    let mut notes = Vec::new();
    if player != game.human {
        notes.push("Opponent's hand · you control their turn".to_string());
    }
    if max_scroll > 0 {
        let track = Rect::new(
            area.right() - 6.,
            top,
            4.,
            HAND_ROWS as f32 * (HAND_H + HAND_GAP) - HAND_GAP,
        );
        rrect(track, 2., pal().strip);
        let thumb = track.h * HAND_ROWS as f32 / rows as f32;
        rrect(
            Rect::new(
                track.x,
                track.y + (track.h - thumb) * view.hand_scroll as f32 / max_scroll as f32,
                track.w,
                thumb,
            ),
            2.,
            pal().accent,
        );
        notes.push(format!(
            "Rows {}–{} of {rows} · scroll",
            view.hand_scroll + 1,
            view.hand_scroll + HAND_ROWS
        ));
        // Legal cards scrolled out of view still need to be discoverable.
        let hidden = cards
            .iter()
            .enumerate()
            .filter(|&(i, _)| i < first || i >= first + HAND_ROWS * per_row)
            .any(|(_, &id)| legal_card(game, id));
        if hidden {
            notes.push("More choosable cards when scrolled".into());
        }
    }
    let mut x = area.right() - 14.;
    for note in notes.iter().rev() {
        let w = measure(note, 12.5, true) + 18.;
        x -= w;
        let pill = Rect::new(x, top + 2., w, 20.);
        boxed(pill, 10., pal().primary.fill, pal().primary.edge);
        centered(
            note,
            pill.center().x,
            pill.y + 14.5,
            12.5,
            pal().primary.ink,
            true,
        );
        x -= 6.;
    }
}

const CHOICE_HEADER_H: f32 = 34.;
const OPTION_H: f32 = 30.;

fn choice_option_rect(panel: Rect, row: usize, x: f32, width: f32) -> Rect {
    Rect::new(
        x,
        panel.y + 1. + CHOICE_HEADER_H + 10. + row as f32 * (OPTION_H + 8.),
        width,
        OPTION_H,
    )
}

/// Pagination owns the right end of the header, above every action row.
fn choice_pager_rects(panel: Rect) -> [Rect; 2] {
    [
        Rect::new(panel.right() - 150., panel.y + 3., 36., 30.),
        Rect::new(panel.right() - 68., panel.y + 3., 36., 30.),
    ]
}

/// "All options", left of the pager, while one card's options are shown.
fn all_options_rect(panel: Rect) -> Rect {
    Rect::new(panel.right() - 290., panel.y + 3., 128., 30.)
}

fn choices(game: &mut Game, input: &Input, art: &Art, view: &mut View) {
    let r = Rect::new(MAIN_X, CHOICE_Y, MAIN_W, CHOICE_H);
    panel(r);
    let band = Rect::new(r.x + 4., r.y + 4., r.w - 8., CHOICE_HEADER_H - 3.);
    draw_rectangle(band.x, band.y, band.w, band.h, pal().strip);
    draw_line(
        band.x,
        band.bottom(),
        band.right(),
        band.bottom(),
        1.,
        pal().line,
    );
    if let Some(error) = game.error.clone() {
        heading(
            "Something went wrong",
            r.x + 16.,
            band.y + 23.,
            16.,
            pal().error,
        );
        wrapped(
            &error,
            Rect::new(r.x + 16., band.bottom() + 12., r.w - 200., 120.),
            16.,
            pal().error,
        );
        if input.button(
            "Retry",
            Rect::new(r.right() - 150., r.bottom() - 54., 130., 38.),
            true,
            Style::Primary,
        ) {
            game.save();
        }
        return;
    }
    if let Some((outcome, reason)) = game.view.over.clone() {
        let (headline, color) = match outcome {
            Outcome::Draw => ("Draw".to_owned(), pal().ink),
            Outcome::Winner(winner) if game.two_player => (
                format!("Player {} wins!", winner + 1),
                if winner == game.human {
                    pal().you
                } else {
                    pal().rival
                },
            ),
            Outcome::Winner(winner) if winner == game.human => ("You win!".to_owned(), pal().you),
            Outcome::Winner(_) => ("The AI wins".to_owned(), pal().rival),
        };
        heading("Game over", r.x + 16., band.y + 23., 16., pal().ink);
        heading(&headline, r.x + 16., band.bottom() + 52., 34., color);
        label(&reason, r.x + 18., band.bottom() + 84., 17., pal().muted);
        label(
            "The game is saved. Open Setup (Esc) to start a new one; the table stays available to inspect.",
            r.x + 18.,
            band.bottom() + 124.,
            15.,
            pal().muted,
        );
        return;
    }
    let Some(choice) = game.view.decision.clone() else {
        return;
    };
    if game.two_player && choice.player != game.human {
        heading(
            &format!("Player {} has a choice", choice.player + 1),
            r.x + 16.,
            band.y + 23.,
            16.,
            pal().rival,
        );
        label(
            &format!(
                "You are viewing Player {}. Switch seats to continue.",
                game.human + 1
            ),
            r.x + 16.,
            band.bottom() + 34.,
            16.,
            pal().ink,
        );
        if input.button(
            "Switch seats",
            Rect::new(r.x + 16., band.bottom() + 54., 170., 38.),
            true,
            Style::Primary,
        ) {
            game.switch_seats();
        }
        return;
    }
    if choice.player != game.human || game.thinking() {
        let dots = ".".repeat((get_time() * 2.) as usize % 4);
        let text = if choice.player == game.human {
            "Stopping the previous search".to_string()
        } else {
            "The AI is thinking".to_string()
        };
        heading(
            &format!("{text}{dots}"),
            r.x + 16.,
            band.y + 23.,
            16.,
            pal().rival,
        );
        label(
            game.ai_activity(),
            r.x + 16.,
            band.bottom() + 34.,
            16.,
            pal().ink,
        );
        label(
            "Right-click any card to study it full size while you wait.",
            r.x + 16.,
            band.bottom() + 62.,
            15.,
            pal().muted,
        );
        return;
    }
    // Prompt: effect card chip, then the question.
    let prompt = choice.prompt.clone();
    let pill = Rect::new(band.x + 10., band.y + 5., 96., 21.);
    rrect(pill, 10., pal().primary.fill);
    centered_title(
        "Your move",
        pill.center().x,
        pill.y + 15.,
        11.,
        pal().primary.ink,
    );
    let [previous_page, next_page] = choice_pager_rects(r);
    // A clicked card with one option takes it; with several (a Dogma with
    // or without Auspice) the bar narrows to them.
    let mut picked = None;
    if let Some(id) = view.clicked_card.take() {
        let matching: Vec<usize> = (0..choice.options.len())
            .filter(|&i| choice.options[i].card == Some(id))
            .collect();
        match matching[..] {
            [only] => picked = Some(only),
            [] => {}
            _ => {
                view.card_options = Some(id);
                view.choices_page = 0;
            }
        }
    }
    let narrowed = view
        .card_options
        .filter(|&id| choice.options.iter().any(|o| o.card == Some(id)));
    let all_options = all_options_rect(r);
    let prompt_x = pill.right() + 12.;
    let prompt_right = if narrowed.is_some() {
        all_options.x
    } else {
        previous_page.x
    };
    if let Some(row) = flow(
        pieces(&prompt, game.human, game.two_player),
        prompt_right - prompt_x - 12.,
        15.,
    )
    .into_iter()
    .next()
    {
        for (x, piece) in &row {
            draw_piece(piece, prompt_x + x, band.y + 8., 15., input, art, view);
        }
    }
    // Option buttons flow left to right, three rows per page.
    let size = 14.;
    let max_w = r.w - 32.;
    let mut rows: Vec<Vec<(usize, f32, Vec<Piece>)>> = vec![Vec::new()];
    let mut x = 0.;
    for (i, option) in choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| narrowed.is_none_or(|id| o.card == Some(id)))
    {
        let items = pieces(&option.segs, game.human, game.two_player);
        let w = (items.iter().map(|p| piece_width(p, size)).sum::<f32>() + 48.).min(max_w);
        if x + w > max_w && x > 0. {
            rows.push(Vec::new());
            x = 0.;
        }
        rows.last_mut().unwrap().push((i, w, items));
        x += w + 10.;
    }
    let pages = rows.len().div_ceil(3).max(1);
    view.choices_page = view.choices_page.min(pages - 1);
    const KEYS: [KeyCode; 9] = [
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
        KeyCode::Key6,
        KeyCode::Key7,
        KeyCode::Key8,
        KeyCode::Key9,
    ];
    let mut shown = 0;
    for (row_i, row) in rows.iter().skip(view.choices_page * 3).take(3).enumerate() {
        let mut x = r.x + 16.;
        for (i, w, items) in row {
            let b = choice_option_rect(r, row_i, x, *w);
            let hover = b.contains(input.mouse);
            boxed(
                b,
                5.,
                if hover { pal().hover } else { pal().strip },
                if hover { pal().glow } else { pal().line },
            );
            let badge = vec2(b.x + 16., b.center().y);
            draw_circle(badge.x, badge.y, 10., pal().primary.fill);
            if shown < KEYS.len() {
                centered_title(
                    &(shown + 1).to_string(),
                    badge.x,
                    badge.y + 4.5,
                    11.,
                    pal().primary.ink,
                );
            }
            let mut px = b.x + 32.;
            for p in items {
                let pw = piece_width(p, size);
                if px + pw > b.right() - 8. {
                    break;
                }
                draw_piece(p, px, b.y + 6., size, input, art, view);
                px += pw;
            }
            if hover {
                view.highlight = choice.options[*i].card;
                if input.click {
                    picked = Some(*i);
                }
            }
            if input.keys && shown < KEYS.len() && is_key_pressed(KEYS[shown]) {
                picked = Some(*i);
            }
            shown += 1;
            x += w + 10.;
        }
    }
    if pages > 1 {
        if input.button("<", previous_page, view.choices_page > 0, Style::Secondary) {
            view.choices_page -= 1;
        }
        centered(
            &format!("{}/{}", view.choices_page + 1, pages),
            r.right() - 91.,
            previous_page.y + 20.,
            14.,
            pal().muted,
            false,
        );
        if input.button(
            ">",
            next_page,
            view.choices_page + 1 < pages,
            Style::Secondary,
        ) {
            view.choices_page += 1;
        }
    }
    if narrowed.is_some() && input.button("All options", all_options, true, Style::Secondary) {
        view.card_options = None;
        view.choices_page = 0;
    }
    if let Some(index) = picked {
        game.choose_option(index);
        view.choices_page = 0;
        view.card_options = None;
        view.popup = None;
    }
}

/// Icon comparison: one column per icon, the leader's count highlighted.
/// Side-by-side comparison: achievements and score, then the seven icons,
/// each column's leader highlighted.
fn player_table(game: &Game, input: &Input, art: &Art, view: &mut View) {
    let r = Rect::new(SIDE_X, OPP_Y, SIDE_W, ICONS_H);
    panel(r);
    let players = [game.human, 1 - game.human];
    let stats = players.map(|p| stats(game, p));
    // Signed: an engine-next Figures score can be negative.
    let value = |column: usize, row: usize| match column {
        0 => stats[row].achievements as i64,
        1 => stats[row].score,
        9 => stats[row].food as i64,
        i => stats[row].icons[i - 2] as i64,
    };
    // Food is an icon only once Age 0 is in play.
    let columns = if game.view.age_zero { 10 } else { 9 };
    let x0 = r.x + 58.;
    let col = (r.right() - 8. - x0) / columns as f32;
    let divider = x0 + 2. * col;
    draw_line(
        divider,
        r.y + 10.,
        divider,
        r.bottom() - 10.,
        1.,
        pal().line,
    );
    for column in 0..columns {
        let cx = x0 + column as f32 * col;
        let header = match column {
            0 => {
                art.glyph(
                    Glyph::Achievement,
                    cx + (col - 22.) / 2.,
                    r.y + 13.,
                    22.,
                    pal().accent,
                );
                format!("Achievements ({} needed)", game.view.achievements_needed)
            }
            1 => {
                art.glyph(
                    Glyph::Score,
                    cx + (col - 22.) / 2.,
                    r.y + 13.,
                    22.,
                    pal().accent,
                );
                "Score".to_string()
            }
            9 => {
                art.tile(Key::Food, cx + (col - 27.) / 2., r.y + 11., 27.);
                "Food".to_string()
            }
            i => {
                let icon = ICONS[i - 2];
                art.icon(icon, cx + (col - 27.) / 2., r.y + 11., 27.);
                title_case(icon.name())
            }
        };
        let mut opened = None;
        for (row, &player) in players.iter().enumerate() {
            let n = value(column, row);
            let cell = Rect::new(cx + 2., r.y + 50. + row as f32 * 36., col - 4., 30.);
            // Achievement and score cells open that player's cards.
            if column < 2 && cell.contains(input.mouse) {
                draw_rectangle_lines(cell.x, cell.y, cell.w, cell.h, 1.5, pal().glow);
                if input.click {
                    opened = Some(if column == 0 {
                        Popup::Achievements(player)
                    } else {
                        Popup::Pile(player, Pile::Score)
                    });
                }
            }
            let lead = n > value(column, 1 - row);
            if lead {
                boxed(cell, 4., pal().lead.fill, pal().lead.edge);
            }
            let text = n.to_string();
            let size =
                (19f32).min(19. * (col - 6.) / measure_face(&text, 19., Face::Title).max(1.));
            centered_title(
                &text,
                cell.center().x,
                cell.y + 22.,
                size,
                if n == 0 {
                    pal().dim
                } else if lead {
                    pal().lead.ink
                } else {
                    pal().ink
                },
            );
        }
        if Rect::new(cx, r.y + 8., col, 114.).contains(input.mouse) {
            view.tooltip = Some(format!(
                "{header}: {} {}, {} {}{}",
                name(players[0], game.human, game.two_player),
                value(column, 0),
                name(players[1], game.human, game.two_player),
                value(column, 1),
                if column < 2 {
                    "\nClick a number to see those cards"
                } else {
                    ""
                },
            ));
        }
        if opened.is_some() {
            view.popup = opened;
        }
    }
    for (row, &player) in players.iter().enumerate() {
        title(
            name(player, game.human, game.two_player),
            r.x + 14.,
            r.y + 72. + row as f32 * 36.,
            16.,
            if player == game.human {
                pal().you
            } else {
                pal().rival
            },
        );
        if game.view.over.is_none() && game.view.current_player == player {
            draw_circle(r.x + 8., r.y + 66. + row as f32 * 36., 2.5, pal().glow);
        }
    }
}

fn side(game: &Game, input: &Input, art: &Art, view: &mut View) {
    player_table(game, input, art, view);
    for (i, tab) in ["Game log", "Help"].into_iter().enumerate() {
        let r = Rect::new(SIDE_X + i as f32 * 104., TABS_Y, 100., 28.);
        let current = view.tab == i;
        if input.button(tab, r, !current, Style::Secondary) {
            view.tab = i;
        }
        if current {
            boxed(r, 5., pal().primary.fill, pal().primary.edge);
            centered(tab, r.center().x, r.y + 19., 14., pal().primary.ink, true);
        }
    }
    if view.tab == 0 {
        if let Some(pending) = &view.copying {
            let result = match pending.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Clipboard worker stopped. Try copying again.".into()))
                }
            };
            if let Some(result) = result {
                view.copying = None;
                match result {
                    Ok(()) => view.copied_at = Some(get_time()),
                    Err(error) => view.copy_error = Some(error),
                }
            }
        }
        let copied = view.copied_at.is_some_and(|t| get_time() - t < 1.5);
        let copy_rect = Rect::new(SIDE_X + SIDE_W - 100., TABS_Y, 100., 28.);
        if input.button(
            if view.copying.is_some() {
                "Copying..."
            } else if view.copy_error.is_some() {
                "Copy failed"
            } else if copied {
                "Copied"
            } else {
                "Copy log"
            },
            copy_rect,
            view.copying.is_none(),
            Style::Secondary,
        ) {
            view.copied_at = None;
            view.copy_error = None;
            let text = log_text(game);
            if super::clipboard::is_wsl() {
                let (sender, receiver) = std::sync::mpsc::channel();
                view.copying = Some(receiver);
                std::thread::spawn(move || {
                    let _ = sender.send(super::clipboard::copy_windows(&text));
                });
            } else {
                miniquad::window::clipboard_set(&text);
                view.copied_at = Some(get_time());
            }
        }
        if copy_rect.contains(input.mouse)
            && let Some(error) = &view.copy_error
        {
            view.tooltip = Some(format!("Could not copy log: {error}"));
        }
    }
    let r = Rect::new(SIDE_X, LOG_Y, SIDE_W, 952. - LOG_Y);
    if view.tab == 0 {
        log_panel(game, r, input, art, view);
    } else {
        help_panel(r, input, view);
    }
}

enum Tile {
    Face(FaceId),
    Back(Set, u8),
    /// A special achievement or Museum: its glyph, name and, when hovered,
    /// a summary.
    Token(Glyph, String, Option<String>),
    /// An issued Decree: its name in a box tinted with its issuing color.
    Decree(String, CardColor),
}

/// Popups fill the table above the choice panel, which stays usable.
const POPUP_AREA: Rect = Rect {
    x: 8.,
    y: 46.,
    w: W - 16.,
    h: CHOICE_Y - 52.,
};

fn popup_window(game: &Game, input: &Input, art: &Art, view: &mut View) {
    let Some(popup) = view.popup else {
        return;
    };
    let owner = if popup_player(popup) == Some(game.human) {
        "Your"
    } else {
        "Opponent's"
    };
    let (tiles, captions, title): (Vec<Tile>, Vec<String>, String) = match popup {
        Popup::Deck(set, age) => {
            let mut cards: Vec<_> = faces()
                .iter()
                .enumerate()
                .filter(|(_, c)| c.set == set && c.age == age)
                .collect();
            cards.sort_by_key(|(_, c)| (stack_index(c.color), c.title.clone()));
            (
                cards
                    .iter()
                    .map(|&(id, _)| Tile::Face(id as FaceId))
                    .collect(),
                Vec::new(),
                format!(
                    "All {} {} age-{age} cards · reference",
                    cards.len(),
                    set.name()
                ),
            )
        }
        Popup::Stack(player, color) => {
            let st = &game.view.players[player].stacks[stack_index(color)];
            let n = st.cards.len();
            let splay = match st.splay {
                Splay::None => "not splayed".to_string(),
                s => format!("splayed {}", s.name().to_lowercase()),
            };
            (
                st.cards.iter().rev().map(|&id| Tile::Face(id)).collect(),
                (0..n)
                    .map(|i| match i {
                        0 => "Top".to_string(),
                        _ if i + 1 == n => format!("{} · bottom", i + 1),
                        _ => (i + 1).to_string(),
                    })
                    .collect(),
                format!("{owner} {} stack · {splay}", color.name().to_lowercase()),
            )
        }
        Popup::Museums(player) => {
            let p = &game.view.players[player];
            (
                p.museums.iter().map(|&(_, id)| Tile::Face(id)).collect(),
                p.museums
                    .iter()
                    .map(|&(museum, _)| format!("Museum {}", museum + 1))
                    .collect(),
                format!("{owner} Museums"),
            )
        }
        Popup::Achievements(player) => {
            let p = &game.view.players[player];
            let mut tiles: Vec<Tile> = p
                .achievements
                .iter()
                .map(|held| match *held {
                    Held::Card(id) => Tile::Back(face(id).set, face(id).age),
                    Held::Special(sp) => Tile::Token(
                        Glyph::Achievement,
                        title_case(sp.name()),
                        Some(special_summary(sp)),
                    ),
                    Held::Museum(n) => {
                        Tile::Token(Glyph::Museum, format!("Museum {}", n + 1), None)
                    }
                })
                .collect();
            tiles.extend(
                p.decrees
                    .iter()
                    .map(|(d, color)| Tile::Decree(format!("{d} Decree"), *color)),
            );
            (
                tiles,
                Vec::new(),
                format!(
                    "{owner} achievements · {} of {} needed",
                    p.achievement_count, game.view.achievements_needed
                ),
            )
        }
        Popup::Junk => {
            let junk = &game.view.supply.junk;
            (
                junk.iter()
                    .map(|&id| {
                        if game.view.supply.junk_visible {
                            Tile::Face(id)
                        } else {
                            Tile::Back(face(id).set, face(id).age)
                        }
                    })
                    .collect(),
                Vec::new(),
                format!(
                    "Junk · {} card{}",
                    junk.len(),
                    if junk.len() == 1 { "" } else { "s" }
                ),
            )
        }
        Popup::Pile(player, pile) => {
            let cards = pile_cards(game, player, pile);
            let p = &game.view.players[player];
            // The engine says whose fronts this seat may see: its own, a hand
            // Meiji lends, or an executor's piles a Hitchhiking controller
            // inspects.
            let faces = match pile {
                Pile::Hand => p.hand_visible,
                Pile::Score => p.score_visible,
                Pile::Forecast => p.forecast_visible,
            };
            let title = match pile {
                Pile::Hand => format!("{owner} hand · {} cards", cards.len()),
                Pile::Forecast => {
                    format!("{owner} forecast · {}/{}", cards.len(), p.forecast_limit)
                }
                Pile::Score => format!(
                    "{owner} score pile · {} card{} · {} points",
                    cards.len(),
                    if cards.len() == 1 { "" } else { "s" },
                    p.score_total
                ),
            };
            (
                cards
                    .iter()
                    .map(|&id| {
                        if faces {
                            Tile::Face(id)
                        } else {
                            let c = face(id);
                            Tile::Back(c.set, c.age)
                        }
                    })
                    .collect(),
                Vec::new(),
                title,
            )
        }
    };
    if tiles.is_empty() && matches!(popup, Popup::Deck(..) | Popup::Stack(..)) {
        view.popup = None;
        return;
    }
    let n = tiles.len().max(1);
    let caption_h = if captions.is_empty() { 12. } else { 34. };
    let (room_w, room_h) = (POPUP_AREA.w - 48., POPUP_AREA.h - 80.);
    // The column count that gives the largest cards.
    let (per_row, cw) = (1..=n)
        .map(|per_row| {
            let rows = n.div_ceil(per_row) as f32;
            let cw = 250f32
                .min((room_h / rows - caption_h) * CARD_W / CARD_H)
                .min(room_w / per_row as f32 - 14.);
            (per_row, cw)
        })
        // Ties favor more columns: a wide window rather than a tall one.
        .fold(
            (1, 0.),
            |best, next| if next.1 >= best.1 { next } else { best },
        );
    let rows = n.div_ceil(per_row);
    let ch = cw * CARD_H / CARD_W;
    let pw = (per_row as f32 * (cw + 14.) + 34.).max(measure(&title, 17., true) + 90.);
    let ph = 60. + rows as f32 * (ch + caption_h) + 10.;
    let r = Rect::new(
        POPUP_AREA.x + (POPUP_AREA.w - pw) / 2.,
        POPUP_AREA.y + (POPUP_AREA.h - ph) / 2.,
        pw,
        ph,
    );
    draw_rectangle(r.x + 4., r.y + 6., r.w, r.h, pal().shadow);
    panel(r);
    draw_rectangle(r.x + 4., r.y + 4., r.w - 8., 36., pal().strip);
    draw_line(
        r.x + 4.,
        r.y + 40.,
        r.right() - 4.,
        r.y + 40.,
        1.,
        pal().line,
    );
    draw_face(&title, r.x + 16., r.y + 28., 16., pal().accent, Face::Title);
    let close = Rect::new(r.right() - 38., r.y + 8., 28., 28.);
    let close_hover = close.contains(input.mouse);
    boxed(
        close,
        5.,
        if close_hover {
            pal().hover
        } else {
            pal().paper
        },
        pal().line,
    );
    centered("×", close.center().x, close.y + 21., 20., pal().ink, true);
    if tiles.is_empty() {
        label("Empty", r.x + 24., r.y + 58. + ch / 2., 16., pal().muted);
    }
    for (i, tile) in tiles.iter().enumerate() {
        let (col, row) = (i % per_row, i / per_row);
        let cr = Rect::new(
            r.x + 24. + col as f32 * (cw + 14.),
            r.y + 58. + row as f32 * (ch + caption_h),
            cw,
            ch,
        );
        match *tile {
            Tile::Face(id) => {
                table_card(input, art, view, id, cr, legal_card(game, id));
            }
            Tile::Back(set, age) => card_back(art, set, age, cr, false),
            Tile::Token(glyph, ref text, ref tip) => {
                boxed(cr, 7., pal().strip, pal().accent);
                art.glyph(
                    glyph,
                    cr.center().x - 13.,
                    cr.y + cr.h * 0.22,
                    26.,
                    pal().accent,
                );
                centered_title(text, cr.center().x, cr.y + cr.h * 0.68, 16., pal().ink);
                if let Some(tip) = tip
                    && cr.contains(input.mouse)
                {
                    view.tooltip = Some(tip.clone());
                }
            }
            Tile::Decree(ref text, color) => {
                let [_, band, _, tint] = pal().card(color);
                boxed(cr, 7., tint, band);
                centered_title(text, cr.center().x, cr.center().y + 6., 16., pal().card_ink);
                if cr.contains(input.mouse) {
                    let name = text.strip_suffix(" Decree").unwrap_or(text);
                    view.tooltip = Some(format!("{text}, issued\n{}", decree_text(name)));
                }
            }
        }
        if let Some(caption) = captions.get(i) {
            label(caption, cr.x + 2., cr.bottom() + 18., 13., pal().muted);
        }
    }
    let choice_panel = Rect::new(MAIN_X, CHOICE_Y, MAIN_W, CHOICE_H);
    if input.click
        && (close_hover || (!r.contains(input.mouse) && !choice_panel.contains(input.mouse)))
    {
        view.popup = None;
    }
}

fn popup_player(popup: Popup) -> Option<usize> {
    match popup {
        Popup::Stack(player, _)
        | Popup::Museums(player)
        | Popup::Pile(player, _)
        | Popup::Achievements(player) => Some(player),
        Popup::Junk => None,
        Popup::Deck(..) => None,
    }
}

/// Opens the forecast or score pile once per decision whose card choices all
/// come from it, such as promoting from the forecast.
fn auto_popup(game: &Game, view: &mut View) {
    let decision = game.steps();
    if view.auto_popup == Some(decision) || !game.human_turn() {
        return;
    }
    view.auto_popup = Some(decision);
    let cards: Vec<FaceId> = game.view.option_cards().collect();
    let player = private_owner(game);
    let p = &game.view.players[player];
    for (pile, held) in [(Pile::Forecast, &p.forecast), (Pile::Score, &p.score)] {
        if !cards.is_empty() && cards.iter().all(|c| held.contains(c)) {
            if view.popup.is_none() {
                view.popup = Some(Popup::Pile(player, pile));
            }
            return;
        }
    }
}

fn zoom_overlay(art: &Art, view: &View, input: &Input) {
    let Some(id) = view.zoom else {
        return;
    };
    let (w, h) = (525., 375.);
    let x = if input.mouse.x < W / 2. {
        input.mouse.x + 30.
    } else {
        input.mouse.x - 30. - w
    }
    .clamp(8., W - w - 8.);
    let y = (input.mouse.y - h / 2.).clamp(8., H - h - 8.);
    rrect(Rect::new(x + 5., y + 8., w, h), 17., pal().card_shadow);
    art.draw_full(id, Rect::new(x, y, w, h));
}

fn tooltip(art: &Art, view: &View, input: &Input) {
    let Some(text) = &view.tooltip else {
        return;
    };
    let rows = lines(text, 340., 14.);
    let bonuses = &view.tooltip_bonuses;
    let bonus_row = if bonuses.is_empty() { 0. } else { 26. };
    let w = rows
        .iter()
        .map(|l| measure(l, 14., false))
        .fold(bonuses.len() as f32 * 24., f32::max)
        + 20.;
    let h = rows.len() as f32 * 19. + 12. + bonus_row;
    let x = (input.mouse.x + 16.).min(W - w - 6.);
    let y = if input.mouse.y + 24. + h > H {
        input.mouse.y - h - 8.
    } else {
        input.mouse.y + 24.
    };
    let r = Rect::new(x, y, w, h);
    draw_rectangle(r.x + 2., r.y + 3., r.w, r.h, pal().shadow);
    boxed(r, 4., pal().strip, pal().accent);
    for (i, line) in rows.iter().enumerate() {
        label(line, r.x + 10., r.y + 20. + i as f32 * 19., 14., pal().ink);
    }
    // The laurels, as the stack footers draw them: the highest counts in
    // full, the rest in muted ink.
    let y = r.y + rows.len() as f32 * 19. + 10.;
    for (i, bonus) in bonuses.iter().enumerate() {
        let x = r.x + 10. + i as f32 * 24.;
        art.tile(Key::Bonus, x, y, 20.);
        centered_title(
            &bonus.to_string(),
            x + 10.,
            y + 14.5,
            9.,
            if i == 0 {
                rgba(pal().enamel.disc_ink)
            } else {
                pal().muted
            },
        );
    }
}

/// The Inari mark: one dot per icon colour around a pale centre.
fn brand(x: f32, y: f32, size: f32) {
    for ((dx, dy), r, color) in sprites::brand_dots(pal().enamel.age[1]) {
        draw_circle(x + dx * size, y + dy * size, r * size, rgba(color));
    }
}

fn top_bar(game: &mut Game, input: &Input, menu: &mut bool) -> bool {
    let p = pal();
    draw_rectangle(0., 0., W, 40., p.bar);
    draw_rectangle(0., 39.5, W, p.bar_rule, p.bar_line);
    let v = &game.view;
    let status = if v.over.is_some() {
        "Game over".to_string()
    } else if game.two_player {
        format!(
            "Turn {} · P{} · action {}/{}",
            v.turn,
            v.current_player + 1,
            v.action,
            v.actions
        )
    } else {
        format!(
            "Turn {}   ·   {} · action {} of {}",
            v.turn,
            if v.current_player == game.human {
                "Your turn"
            } else {
                "AI's turn"
            },
            v.action,
            v.actions
        )
    };
    let status_size = if game.two_player { 14. } else { 15. };
    let engine = if game.two_player {
        format!(
            "2-player · viewing P{} · seed {} · {}",
            game.human + 1,
            game.seed,
            if game.saved { "saved" } else { "NOT SAVED" }
        )
    } else {
        format!(
            "{}  ·  {}  ·  seed {}  ·  {}",
            shorten(&game.model_name, 200., 13.),
            game.ai_label(),
            game.seed,
            if game.saved { "saved" } else { "NOT SAVED" }
        )
    };
    let w = measure(&engine, 13., false);
    let ex = W - if game.two_player { 704. } else { 330. } - w;
    label(
        &engine,
        ex,
        25.,
        13.,
        if game.saved { p.bar_muted } else { p.bar_error },
    );
    // The mode (every enabled expansion) comes first and the status follows
    // it; a long mode shortens rather than running under the status.
    let status_w = measure(&status, status_size, true);
    let mode_x = 14.;
    let mode_room = ex - 24. - status_w - 28. - mode_x;
    let mode = shorten(&v.mode, mode_room, 15.);
    draw_str(&mode, mode_x, 26., 15., p.bar_ink, false);
    let status_x = (mode_x + measure(&mode, 15., false) + 28.)
        .max(if game.two_player { 460. } else { 550. })
        .min(ex - 24. - status_w);
    heading(&status, status_x, 26., status_size, p.bar_accent);
    let ctrl = is_key_down(KeyCode::LeftControl)
        || is_key_down(KeyCode::RightControl)
        || is_key_down(KeyCode::LeftSuper)
        || is_key_down(KeyCode::RightSuper);
    if game.two_player {
        if input.button(
            "Switch seats",
            Rect::new(W - 690., 5., 144., 30.),
            true,
            Style::Bar,
        ) {
            game.switch_seats();
        }
        if input.button(
            if game.auto_switch {
                "Autoswitch: on"
            } else {
                "Autoswitch: off"
            },
            Rect::new(W - 536., 5., 210., 30.),
            true,
            Style::Bar,
        ) {
            game.set_auto_switch(!game.auto_switch);
        }
    }
    // Two-player bars have no room for the button; the key still works.
    if game.view.subgame > 0
        && ((!game.two_player
            && input.button(
                if game.view.peeking {
                    "Subgame  (P)"
                } else {
                    "Parent  (P)"
                },
                Rect::new(W - 476., 5., 150., 30.),
                true,
                Style::Bar,
            ))
            || (input.keys && !ctrl && is_key_pressed(KeyCode::P)))
    {
        game.toggle_peek();
    }
    let undo = input.button(
        "Undo  (Ctrl+Z)",
        Rect::new(W - 316., 5., 150., 30.),
        game.can_undo(),
        Style::Bar,
    ) || (input.keys && ctrl && is_key_pressed(KeyCode::Z) && game.can_undo());
    if input.button(
        "Setup  (Esc)",
        Rect::new(W - 156., 5., 140., 30.),
        true,
        Style::Bar,
    ) {
        *menu = true;
    }
    if undo {
        game.undo();
    }
    undo
}

fn table(game: &mut Game, real: &Input, art: &Art, view: &mut View, menu: &mut bool) {
    let viewer = game.human;
    draw_table(game, real, art, view, menu);
    if game.human != viewer {
        // Discard private hover/zoom state and redraw the entire frame before
        // presenting a new seat. No second action can fire during that redraw.
        *view = View {
            tab: view.tab,
            ..View::default()
        };
        draw_table(game, &Input::blocked(), art, view, menu);
    }
}

fn draw_table(game: &mut Game, real: &Input, art: &Art, view: &mut View, menu: &mut bool) {
    view.zoom = None;
    view.tooltip = None;
    view.tooltip_bonuses.clear();
    auto_popup(game, view);
    let blocked = Input::blocked();
    // A card opened full size takes the next click, which only closes it.
    let pinned = view.pinned.is_some();
    if pinned && (real.click || real.right_click) {
        view.pinned = None;
    }
    let real = if pinned { &blocked } else { real };
    let popup = view.popup.is_some();
    let safe_hover = view.safe.is_some_and(|(player, anchor)| {
        anchor.contains(real.mouse)
            || safe_popup_rect(anchor, game.view.players[player].safe.len()).contains(real.mouse)
    });
    // A popup covers the table but leaves the choice panel and top bar live.
    let live = if safe_hover { &blocked } else { real };
    let input = if popup { &blocked } else { live };
    table_background(art);
    let rival = 1 - game.human;
    board(game, rival, OPP_Y, input, art, view);
    middle(game, input, art, view);
    board(game, game.human, OWN_Y, input, art, view);
    hand(game, input, art, view);
    view.highlight = None;
    choices(game, live, art, view);
    side(game, input, art, view);
    if top_bar(game, live, menu) {
        let log = view.log.take();
        *view = View {
            log,
            tab: view.tab,
            ..View::default()
        };
    }
    if popup {
        popup_window(game, real, art, view);
    } else {
        safe_popup(game, real, art, view);
    }
    if let Some((age, set)) = view.clicked_achievement.take() {
        game.achieve_standard(age, set);
        view.choices_page = 0;
    }
    if view.zoom.is_some() {
        zoom_overlay(art, view, real);
    } else {
        tooltip(art, view, real);
    }
    pinned_card(art, view);
}

/// The right-clicked card, full size over a dimmed table.
fn pinned_card(art: &Art, view: &View) {
    let Some(id) = view.pinned else {
        return;
    };
    draw_rectangle(0., 0., W, H, pal().scrim);
    let (w, h) = (700., 500.);
    let r = Rect::new((W - w) / 2., (H - h) / 2. - 14., w, h);
    rrect(Rect::new(r.x + 6., r.y + 9., w, h), 23., pal().card_shadow);
    art.draw_full(id, r);
    centered(
        "Click anywhere to close",
        W / 2.,
        r.bottom() + 32.,
        15.,
        pal().muted,
        false,
    );
}

// ------------------------------------------------------------------- setup

#[derive(Default)]
struct Editor {
    focus: Option<usize>,
    select_all: bool,
}
impl Editor {
    fn field(&mut self, id: usize, text: &mut String, r: Rect, input: &Input) {
        if input.click && r.contains(input.mouse) {
            self.focus = Some(id);
            self.select_all = false;
        }
        if self.focus == Some(id) {
            let ctrl = is_key_down(KeyCode::LeftControl)
                || is_key_down(KeyCode::RightControl)
                || is_key_down(KeyCode::LeftSuper)
                || is_key_down(KeyCode::RightSuper);
            if ctrl && is_key_pressed(KeyCode::A) {
                self.select_all = true;
            }
            if is_key_pressed(KeyCode::Backspace) || is_key_pressed(KeyCode::Delete) {
                if self.select_all {
                    text.clear();
                    self.select_all = false;
                } else {
                    text.pop();
                }
            }
            if ctrl && is_key_pressed(KeyCode::V) {
                if self.select_all {
                    text.clear();
                    self.select_all = false;
                }
                if let Some(paste) = miniquad::window::clipboard_get() {
                    text.push_str(paste.trim());
                }
            }
            while let Some(c) = get_char_pressed() {
                if !ctrl && !c.is_control() {
                    if self.select_all {
                        text.clear();
                        self.select_all = false;
                    }
                    text.push(c);
                }
            }
        }
        let focused = self.focus == Some(id);
        boxed(
            r,
            5.,
            pal().table,
            if focused { pal().accent } else { pal().line },
        );
        if focused {
            boxed(inset(r, 1.), 4., pal().table, pal().accent);
        }
        let mut shown = text.clone();
        while !shown.is_empty() && measure(&shown, 17., false) > r.w - 25. {
            shown.remove(0);
        }
        if self.select_all && focused {
            let w = measure(&shown, 17., false);
            draw_rectangle(r.x + 10., r.y + 10., w + 4., r.h - 20., pal().selection);
        }
        label(&shown, r.x + 12., r.y + r.h / 2. + 6., 17., pal().ink);
    }
}

struct SetupFields {
    elo: String,
    seed: String,
}

enum SetupAction {
    Start { resume: bool },
    Return,
}

fn setup(
    settings: &mut Settings,
    input: &Input,
    editor: &mut Editor,
    fields: &mut SetupFields,
    has_game: bool,
    busy: bool,
    error: &str,
) -> Option<SetupAction> {
    const ROW: f32 = 66.;
    // Theme, mode, expansions, seat, seed and save, plus AI strength or
    // seat switching in 2-player mode.
    let rows = 7.;
    let height = 66. + rows * ROW + 30. + 48. + 24. + if has_game { 52. } else { 0. };
    let panel = Rect::new((W - 620.) / 2., (H - height) / 2., 620., height);
    draw_rectangle(panel.x + 4., panel.y + 6., panel.w, panel.h, pal().shadow);
    boxed(panel, 10., pal().paper, pal().line);
    draw_rectangle(panel.x + 4., panel.y + 4., panel.w - 8., 52., pal().bar);
    draw_line(
        panel.x + 4.,
        panel.y + 56.,
        panel.right() - 4.,
        panel.y + 56.,
        1.,
        pal().line,
    );
    brand(panel.x + 28., panel.y + 14., 28.);
    title("INARI", panel.x + 66., panel.y + 38., 24., pal().bar_accent);
    draw_str(
        "Game setup",
        panel.x + 160.,
        panel.y + 37.,
        18.,
        pal().bar_ink,
        false,
    );
    let x = panel.x + 32.;
    let full = panel.w - 64.;
    let mut y = panel.y + 66.;
    // Each row: a caption, then a 40-unit control.
    let mut row = |caption: &str| {
        label(caption, x, y + 16., 15., pal().muted);
        let top = y + 24.;
        y += ROW;
        top
    };
    let style = |on: bool| if on { Style::Primary } else { Style::Secondary };
    let top = row("Theme");
    for (i, theme) in Theme::ALL.into_iter().enumerate() {
        if input.button(
            theme.name(),
            Rect::new(x + i as f32 * 142., top, 134., 38.),
            true,
            style(settings.theme == theme),
        ) {
            settings.theme = theme;
        }
    }
    let top = row("Mode");
    for (i, (name, two_player)) in [("vs AI", false), ("2-player", true)]
        .into_iter()
        .enumerate()
    {
        if input.button(
            name,
            Rect::new(x + i as f32 * 284., top, 272., 38.),
            true,
            style(settings.two_player == two_player),
        ) {
            settings.two_player = two_player;
            editor.focus = None;
        }
    }
    let top = row("Expansions");
    let pitch = (full + 8.) / 5.;
    for (i, (name, enabled)) in [
        ("Echoes", &mut settings.echoes),
        ("Figures", &mut settings.figures),
        ("Cities", &mut settings.cities),
        ("Unseen", &mut settings.unseen),
        ("Artifacts", &mut settings.artifacts),
    ]
    .into_iter()
    .enumerate()
    {
        if input.button(
            name,
            Rect::new(x + i as f32 * pitch, top, pitch - 8., 38.),
            true,
            style(*enabled),
        ) {
            *enabled = !*enabled;
        }
    }
    let top = row(if settings.two_player {
        "Starting view"
    } else {
        "Your seat"
    });
    for i in 0..2 {
        if input.button(
            &format!("Player {}", i + 1),
            Rect::new(x + i as f32 * 284., top, 272., 38.),
            true,
            style(settings.human == i),
        ) {
            settings.human = i;
        }
    }
    if settings.two_player {
        let top = row("Seat switching");
        if input.button(
            if settings.auto_switch {
                "Autoswitch: on"
            } else {
                "Autoswitch: off"
            },
            Rect::new(x, top, 272., 38.),
            true,
            Style::Primary,
        ) {
            settings.auto_switch = !settings.auto_switch;
        }
    } else {
        // One strength for the network AI: a policy temperature up to its
        // greedy move, simulations above. An empty field keeps a hand-set one.
        let top = row("AI strength (Elo)");
        let note = strength::describe(settings.sims, settings.temperature);
        let note = if settings.elo.is_some() {
            note
        } else {
            format!("set by hand: {note}")
        };
        label(
            &note,
            x + full - measure(&note, 14., false),
            top - 8.,
            14.,
            pal().muted,
        );
        for (i, (name, elo)) in [
            ("400", 400),
            ("860", strength::GREEDY_ELO),
            ("1,250", 1250),
            ("1,410", strength::MAX_ELO),
        ]
        .iter()
        .enumerate()
        {
            if input.button(
                name,
                Rect::new(x + i as f32 * 106., top, 98., 38.),
                true,
                style(settings.elo == Some(*elo)),
            ) {
                fields.elo = elo.to_string();
            }
        }
        editor.field(
            2,
            &mut fields.elo,
            Rect::new(x + 4. * 106., top, full - 4. * 106., 40.),
            input,
        );
    }
    let parsed = if settings.two_player {
        Ok(None)
    } else {
        strength::parse_elo(&fields.elo)
    };
    if let Ok(Some(elo)) = parsed {
        settings.elo = Some(elo);
        (settings.sims, settings.temperature) = strength::setting(elo);
    }
    let top = row("Seed");
    editor.field(3, &mut fields.seed, Rect::new(x, top, full, 40.), input);
    let seed = parse_seed(&fields.seed);
    let top = row("Saved game (.json / .txt)");
    editor.field(1, &mut settings.resume, Rect::new(x, top, full, 40.), input);
    let message = parsed.as_ref().err().or(seed.as_ref().err());
    if let Some(message) = message {
        wrapped(message, Rect::new(x, y, full, 30.), 15., pal().error);
    } else if !error.is_empty() {
        wrapped(error, Rect::new(x, y, full, 30.), 15., pal().error);
    }
    y += 30.;
    if input.button(
        "New game",
        Rect::new(x, y, 272., 48.),
        !busy && parsed.is_ok() && seed.is_ok(),
        Style::Primary,
    ) {
        return Some(SetupAction::Start { resume: false });
    }
    if input.button(
        "Resume save",
        Rect::new(x + 284., y, 272., 48.),
        !busy && parsed.is_ok() && !settings.resume.trim().is_empty(),
        Style::Secondary,
    ) {
        return Some(SetupAction::Start { resume: true });
    }
    if has_game
        && input.button(
            "Back to game  (Esc)",
            Rect::new(x, y + 60., full, 40.),
            parsed.is_ok(),
            Style::Secondary,
        )
    {
        return Some(SetupAction::Return);
    }
    None
}

pub async fn run(options: Options) {
    prevent_quit();
    load_fonts().await;
    let mut settings = options.settings;
    let mut fields = SetupFields {
        elo: settings.elo.map(|elo| elo.to_string()).unwrap_or_default(),
        seed: options.seed.unwrap_or_else(|| fresh_seed(None)).to_string(),
    };
    let mut art = Art::default();
    let mut game: Option<Game> = None;
    let mut opening = None;
    let mut menu = true;
    let mut error = String::new();
    let mut editor = Editor::default();
    let mut view = View::default();
    let mut fullscreen = false;
    let mut smoke_frames = 0;
    let mut smoke_chose = options.resume;
    let smoke_started = Instant::now();
    let mut last_choice_count = 0;
    let mut request = options.start.then_some(options.resume);
    loop {
        if is_quit_requested() {
            break;
        }
        if options.screenshot.is_some() && smoke_started.elapsed().as_secs() >= 30 {
            eprintln!(
                "Desktop smoke timed out: {}",
                game.as_ref()
                    .and_then(|g| g.error.as_deref())
                    .unwrap_or(&error)
            );
            std::process::exit(1);
        }
        let frame_start = Instant::now();
        let input = Input::read();
        let scale = (screen_width() / W).min(screen_height() / H);
        THEME.set(settings.theme);
        // A match shows the rule it plays; new matches play the printed one.
        let martian_internet_printed = game
            .as_ref()
            .is_none_or(|g| g.view.martian_internet_printed);
        art.begin_frame(scale * screen_dpi_scale(), martian_internet_printed);
        set_camera(&Camera2D {
            // Camera2D flips screen Y internally; positive zoom keeps UI Y down.
            zoom: vec2(2. * scale / screen_width(), 2. * scale / screen_height()),
            target: vec2(W / 2., H / 2.),
            ..Default::default()
        });
        clear_background(pal().backdrop);
        if is_key_pressed(KeyCode::F11) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }
        if is_key_pressed(KeyCode::Escape) {
            if !menu && view.pinned.is_some() {
                view.pinned = None;
            } else if !menu && view.safe.is_some() {
                view.safe = None;
            } else if !menu && view.popup.is_some() {
                view.popup = None;
            } else if game.is_some()
                && (!menu || settings.two_player || strength::parse_elo(&fields.elo).is_ok())
            {
                menu = !menu;
                editor.focus = None;
            }
        }
        if let Some(g) = &mut game {
            if !menu && (g.sims != settings.sims || g.temperature != settings.temperature) {
                g.set_sims(settings.sims);
                g.set_temperature(settings.temperature);
                if let Err(message) = settings.save() {
                    error = message;
                }
            }
            g.tick(!menu);
            if g.steps() != last_choice_count {
                last_choice_count = g.steps();
                view.choices_page = 0;
                view.card_options = None;
            }
        }
        // Load/replay on a worker too: long saves or slow disks never freeze the window.
        if let Some(resume) = request.take() {
            let s = settings.clone();
            let seed = if resume {
                None
            } else {
                match parse_seed(&fields.seed) {
                    Ok(seed) => Some(seed),
                    Err(message) => {
                        error = message;
                        continue;
                    }
                }
            };
            let (tx, rx) = std::sync::mpsc::channel();
            match std::thread::Builder::new()
                .name("play-app-load".into())
                .spawn(move || {
                    let _ = tx.send(Game::open(&s, resume, seed));
                }) {
                Ok(_) => {
                    opening = Some(rx);
                    error = "Loading game and opponent...".into();
                }
                Err(e) => error = format!("Could not load game: {e}"),
            }
        }
        if let Some(rx) = &opening {
            match rx.try_recv() {
                Ok(result) => {
                    opening = None;
                    match result {
                        Ok(g) => {
                            fields.seed = fresh_seed(Some(g.seed)).to_string();
                            settings.echoes = g.view.echoes;
                            settings.figures = g.view.figures;
                            settings.cities = g.view.cities;
                            settings.unseen = g.view.unseen;
                            settings.artifacts = g.view.artifacts;
                            settings.resume = g.path.to_string_lossy().into();
                            error = settings.save().err().unwrap_or_default();
                            game = Some(g);
                            menu = false;
                            view = View::default();
                        }
                        Err(e) => {
                            error = e;
                            menu = true;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    opening = None;
                    error = "Game loader stopped unexpectedly.".into();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if menu {
            for file in get_dropped_files() {
                if let Some(path) = file.path
                    && path.extension().is_some_and(|e| e == "json" || e == "txt")
                {
                    settings.resume = path.to_string_lossy().into();
                }
            }
            let theme = settings.theme;
            table_background(&art);
            let action = setup(
                &mut settings,
                &input,
                &mut editor,
                &mut fields,
                game.is_some(),
                opening.is_some() || game.as_ref().is_some_and(Game::thinking),
                &error,
            );
            // A theme applies at once and is remembered without starting a game.
            if settings.theme != theme
                && let Err(message) = settings.save()
            {
                error = message;
            }
            match action {
                Some(SetupAction::Start { resume }) => request = Some(resume),
                Some(SetupAction::Return) => {
                    menu = false;
                    editor.focus = None;
                }
                None => {}
            }
        } else if let Some(g) = &mut game {
            table(g, &input, &art, &mut view, &mut menu);
            if g.two_player && (settings.human != g.human || settings.auto_switch != g.auto_switch)
            {
                settings.human = g.human;
                settings.auto_switch = g.auto_switch;
                if let Err(message) = settings.save() {
                    error = message;
                }
            }
            if let Some(screenshot) = &options.screenshot {
                smoke_frames += 1;
                if g.human_turn() && (!smoke_chose || (g.two_player && g.steps() < 2)) {
                    g.choose_option(0);
                    smoke_chose = true;
                    // This frame shows the state before the choice: capture
                    // a later one.
                    smoke_frames = 0;
                }
                if smoke_frames > 5 && g.human_turn() && g.steps() >= 2 {
                    set_default_camera();
                    get_screen_data().export_png(screenshot);
                    println!(
                        "Desktop smoke passed: {} saved choices, human decision ready, screenshot written.",
                        g.steps()
                    );
                    break;
                }
            }
        }
        if options.screenshot.is_some() && menu && opening.is_none() && request.is_none() {
            eprintln!("Desktop smoke failed: {error}");
            std::process::exit(1);
        }
        set_default_camera();
        if is_key_pressed(KeyCode::F12) {
            let _ = std::fs::create_dir_all(DATA_DIR);
            let path = std::path::Path::new(DATA_DIR).join("screenshot.png");
            get_screen_data().export_png(&path.display().to_string());
        }
        // A static card table needs only 30 fps; cap even on uncapped WSL/X11.
        let remaining = Duration::from_millis(33).saturating_sub(frame_start.elapsed());
        std::thread::sleep(remaining);
        next_frame().await;
    }
    // Font atlases own textures. Drop them while macroquad's context is alive,
    // instead of during thread-local destruction after the window has closed.
    FONTS.with(|slot| *slot.borrow_mut() = [None, None, None]);
}

#[cfg(test)]
mod tests {
    use super::super::faces::by_code;
    use super::*;

    #[test]
    fn resizing_invalidates_a_pinned_layout_before_a_matching_pixel_width_preview() {
        let original_density = PX.get();
        let mut art = Art::default();
        let id = by_code("B097").unwrap();
        art.begin_frame(0.75, false);
        let pinned_pixels = phys(700.);
        art.layouts.get_mut().insert(
            (id, 700f32.to_bits()),
            FxLayout {
                size: 8.8 * 700. / CARD_W,
                items: Vec::new(),
            },
        );

        // Resizing from 1380x720 to 1840x960 makes the hover preview's
        // physical width equal to the old pinned card's physical width.
        art.begin_frame(1., false);
        assert_eq!(phys(525.), pinned_pixels);
        assert!(art.layouts.get_mut().is_empty());
        let preview = art
            .layouts
            .get_mut()
            .entry((id, 525f32.to_bits()))
            .or_insert_with(|| FxLayout {
                size: 8.8 * 525. / CARD_W,
                items: Vec::new(),
            });
        assert_eq!(preview.size, 22.);

        // A static frame must keep its cached layout.
        art.begin_frame(1., false);
        assert_eq!(art.layouts.get_mut().len(), 1);
        PX.set(original_density);
    }

    #[test]
    fn repeated_resizing_keeps_only_current_density_sprites() {
        let original_density = PX.get();
        let mut art = Art::default();
        // Unmanaged placeholder handles exercise cache eviction without
        // opening a window or allocating real GPU resources in unit tests.
        let placeholder = Texture2D::from_miniquad_texture(
            macroquad::miniquad::TextureId::from_raw_id(macroquad::miniquad::RawId::OpenGl(0)),
        );
        for i in 0..=150 {
            let density = 0.5 + i as f32 / 300.;
            art.begin_frame(density, false);
            assert!(art.sprites.get_mut().is_empty());
            for color in CardColor::ALL {
                for (w, h) in [(BOARD_H5 * CARD_W / CARD_H, BOARD_H5), (HAND_W, HAND_H)] {
                    art.sprites.get_mut().insert(
                        Key::Frame(color, false, phys(w), phys(h)),
                        placeholder.clone(),
                    );
                }
            }
            art.sprites.get_mut().insert(
                Key::Frame(CardColor::Blue, false, phys(700.), phys(500.)),
                placeholder.clone(),
            );
            art.begin_frame(density, false);
            assert_eq!(art.sprites.get_mut().len(), 11);
        }
        PX.set(original_density);
    }

    #[test]
    fn pagination_clicks_never_hit_even_full_width_choice_rows() {
        let panel = Rect::new(MAIN_X, CHOICE_Y, MAIN_W, CHOICE_H);
        let all_options = all_options_rect(panel);
        let pagers = choice_pager_rects(panel);
        assert!(!pagers.iter().any(|pager| pager.overlaps(&all_options)));
        for pager in pagers.into_iter().chain([all_options]) {
            assert!(panel.contains(pager.center()));
            for row in 0..3 {
                let option = choice_option_rect(panel, row, panel.x + 16., panel.w - 32.);
                assert!(option.bottom() < panel.bottom());
                assert!(!option.overlaps(&pager));
                assert!(!option.contains(pager.center()));
            }
        }
    }

    /// Hovering any special achievement explains how it is claimed.
    #[test]
    fn every_special_achievement_has_a_summary() {
        let all = Special::ALL
            .into_iter()
            .chain(Special::ECHOES)
            .chain(Special::CITIES)
            .chain(Special::UNSEEN);
        let mut seen = 0;
        for sp in all {
            let text = special_summary(sp);
            assert!(text.starts_with(&title_case(sp.name())), "{text}");
            assert!(text.contains("Claim"), "{text}");
            seen += 1;
        }
        assert_eq!(seen, 20);
    }

    /// Card faces are drawn from data: every token in every effect becomes
    /// a word, an icon, an age disc or a hex, never raw brackets.
    #[test]
    fn effect_text_tokens_become_symbols() {
        for (id, card) in faces().iter().enumerate() {
            for runs in effect_runs(id as FaceId, false) {
                for (fx, _) in runs {
                    if let Fx::Word(word) | Fx::Demand(word) = fx {
                        assert!(
                            !word.contains('[') && !word.contains(']') && !word.is_empty(),
                            "{}: {word:?}",
                            card.code
                        );
                    }
                }
            }
        }
        let bangle = effect_runs(by_code("E001").unwrap(), false);
        // The echo leads, then two Authority effects.
        assert!(matches!(bangle[0][0].0, Fx::Echo));
        assert!(bangle[0].iter().any(|(fx, _)| matches!(fx, Fx::Age(1))));
        assert!(matches!(bangle[1][0].0, Fx::Icon(Icon::Authority)));
        let archery = effect_runs(by_code("B001").unwrap(), false);
        assert!(
            matches!(&archery[0][1].0, Fx::Demand(w) if w == "I DEMAND"),
            "demands lead with I DEMAND"
        );
    }

    #[test]
    fn title_case_capitalises_each_part() {
        for (data, shown) in [
            (
                "ALONG THE RIVER DURING THE QINGMING FESTIVAL",
                "Along the River During the Qingming Festival",
            ),
            ("THE WHEEL", "The Wheel"),
            (
                "U.S. DECLARATION OF INDEPENDENCE",
                "U.S. Declaration of Independence",
            ),
            (
                "JEDLIK'S ELECTROMAGNETIC SELF-ROTOR",
                "Jedlik's Electromagnetic Self-Rotor",
            ),
            (
                "MEIJI-MURA STAMP VENDING MACHINE",
                "Meiji-Mura Stamp Vending Machine",
            ),
            ("SUBMARINE H. L. HUNLEY", "Submarine H. L. Hunley"),
            (
                "EARHART'S LOCKHEED ELECTRA 10E",
                "Earhart's Lockheed Electra 10E",
            ),
            ("YEAGER'S BELL X-1A", "Yeager's Bell X-1A"),
            ("DELOREAN DMC-12", "DeLorean DMC-12"),
            ("MAGIC 8-BALL", "Magic 8-Ball"),
            ("3D PRINTING", "3D Printing"),
            ("MP3", "MP3"),
            ("GPS", "GPS"),
            ("A.I.", "A.I."),
            ("16 PSYCHE", "16 Psyche"),
            ("draw past top age", "Draw Past Top Age"),
        ] {
            assert_eq!(title_case(data), shown);
        }
    }

    /// Long titles break onto two lines rather than shrinking until they
    /// are unreadable; short ones keep one line at full size.
    #[test]
    fn long_titles_break_onto_two_lines() {
        // A proportional stand-in for the title font.
        let width = |t: &str| t.chars().count() as f32 * 0.62;
        let layout = |title: &str| title_lines(&title_case(title), 104., width);
        assert_eq!(layout("ARCHERY"), (vec!["Archery".to_owned()], TITLE_SIZE));
        let (lines, size) = layout("SOCIAL NETWORKING");
        assert_eq!(lines.len(), 1);
        assert!(size >= TITLE_SPLIT_BELOW);
        let (lines, size) = layout("ALONG THE RIVER DURING THE QINGMING FESTIVAL");
        assert_eq!(lines, ["Along the River During", "the Qingming Festival"]);
        assert!(size > 7.5 && size <= TITLE_TWO_LINE_MAX);
        // Among breaks that reach the two-line limit, the most balanced wins.
        let (lines, size) = layout("ELECTRUM STATER OF EPHESUS");
        assert_eq!(lines, ["Electrum Stater", "of Ephesus"]);
        assert_eq!(size, TITLE_TWO_LINE_MAX);
        // One long word has nowhere to break, so it shrinks instead.
        let (lines, size) = layout("COUNTERINTELLIGENCEAGENCY");
        assert_eq!(lines.len(), 1);
        assert!(size < TITLE_SPLIT_BELOW);
    }

    #[test]
    fn boards_and_two_hand_rows_fit_between_the_panels() {
        // Each board keeps room for its splay peeks and buried-card strip.
        const {
            assert!(OPP_Y + BOARD_H5 + STRIP_GAP + 18. <= MID_Y);
            assert!(BOARD_H6 <= BOARD_H5);
            assert!(MID_Y + MID_H <= OWN_Y);
            assert!(OWN_Y + BOARD_H5 + STRIP_GAP + 18. <= HAND_Y);
            let rows = HAND_ROWS as f32 * (HAND_H + HAND_GAP) - HAND_GAP;
            assert!(HAND_Y + rows <= CHOICE_Y - 6.);
            assert!(CHOICE_Y + CHOICE_H <= H - 6.);
            assert!(LOG_Y > TABS_Y + 28.);
        }
        // The hand scrollbar sits right of the last column.
        let per_row = hand_per_row();
        assert!(per_row >= 8);
        assert!(per_row as f32 * (HAND_W + HAND_GAP) - HAND_GAP <= MAIN_W - 8.);
        for artifacts in [false, true] {
            let columns = if artifacts { 6 } else { 5 };
            let rects: Vec<_> = (0..columns)
                .map(|i| board_rect(artifacts, i, OWN_Y))
                .collect();
            assert!((rects[columns - 1].right() - (MAIN_X + MAIN_W)).abs() < 0.01);
            assert!(rects.windows(2).all(|w| w[0].right() + 8. < w[1].x));
        }
        assert!(POPUP_AREA.bottom() < CHOICE_Y);
    }
}
