//! The table's drawing and input layer on egui. Layout is in canvas units:
//! the 1840 x 960 design, letterboxed into the window. egui's points are
//! canvas units (the app sets pixels-per-point to fit the canvas), so the
//! canvas only adds an origin offset.
//!
//! Drawing goes through egui's painter in call order on one layer, so later
//! calls cover earlier ones, as a painter's canvas should. Real egui
//! widgets (text fields, selectable text, drag handles) are placed on the
//! same layer, in the same order.

use std::cell::RefCell;
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, Context, CornerRadius, CursorIcon, FontData, FontDefinitions,
    FontFamily, FontId, Galley, Id, LayerId, Margin, Painter, Pos2, Response, Sense, Stroke,
    StrokeKind, TextEdit, TextureHandle, TextureId, TextureOptions, Ui, UiBuilder, pos2,
};
pub use eframe::egui::{Key, Vec2, vec2};

use super::theme::Palette;

/// The design canvas the table is laid out on.
pub const W: f32 = 1840.;
pub const H: f32 = 960.;

// ------------------------------------------------------------------ values

/// An sRGB colour with straight alpha, components 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}
impl From<Color> for Color32 {
    fn from(c: Color) -> Self {
        let [r, g, b, a] = [c.r, c.g, c.b, c.a].map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }
}
pub const WHITE: Color = Color::new(1., 1., 1., 1.);
pub const BLACK: Color = Color::new(0., 0., 0., 1.);

/// A rectangle by corner and size, in canvas units. Edges count as inside,
/// as the table's hit tests have always assumed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub const fn right(&self) -> f32 {
        self.x + self.w
    }
    pub const fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> Vec2 {
        vec2(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
    #[cfg(test)]
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x <= other.right()
            && self.right() >= other.x
            && self.y <= other.bottom()
            && self.bottom() >= other.y
    }
}

// ------------------------------------------------------------------- fonts

/// The design's typefaces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    Body,
    Bold,
    /// Cinzel capitals for titles, names and numerals.
    Title,
}
impl Face {
    const fn name(self) -> &'static str {
        match self {
            Face::Body => "body",
            Face::Bold => "bold",
            Face::Title => "title",
        }
    }
    pub fn font(self, size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(self.name().into()))
    }
}

/// The design's fonts ship with the app (SIL Open Font License), so text
/// looks the same on every platform. They cover every character the app
/// writes; the titling face falls back to the body face for anything else.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::empty();
    let files: [(Face, &'static [u8]); 3] = [
        (
            Face::Body,
            include_bytes!("../assets/fonts/AlegreyaSans-Medium.ttf"),
        ),
        (
            Face::Bold,
            include_bytes!("../assets/fonts/AlegreyaSans-Bold.ttf"),
        ),
        (
            Face::Title,
            include_bytes!("../assets/fonts/Cinzel-Bold.ttf"),
        ),
    ];
    let body = Face::Body.name().to_owned();
    for (face, bytes) in files {
        let name = face.name().to_owned();
        fonts
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_static(bytes)));
        let mut family = vec![name.clone()];
        if name != body {
            family.push(body.clone());
        }
        fonts.families.insert(FontFamily::Name(name.into()), family);
    }
    // egui's own widgets (text fields) write in the body face.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.insert(family, vec![body.clone()]);
    }
    ctx.set_fonts(fonts);
}

/// egui's widgets in the theme's colours: text field selection and caret.
pub fn apply_theme(ctx: &Context, pal: &Palette) {
    ctx.all_styles_mut(|s| {
        let v = &mut s.visuals;
        v.dark_mode = pal.dark;
        v.selection.bg_fill = pal.selection.into();
        v.selection.stroke = Stroke::new(1., Color32::from(pal.ink));
        v.text_cursor.stroke = Stroke::new(2., Color32::from(pal.accent));
        v.extreme_bg_color = Color32::TRANSPARENT;
        v.override_text_color = Some(pal.ink.into());
    });
}

// ------------------------------------------------------------------ canvas

struct Canvas {
    painter: Painter,
    ui: Ui,
    /// The canvas's top left corner, in points.
    origin: Vec2,
    time: f64,
    /// Something on screen moves by itself: paint again soon.
    animating: bool,
}
impl Canvas {
    fn pos(&self, x: f32, y: f32) -> Pos2 {
        pos2(x, y) + self.origin
    }
    fn rect(&self, r: Rect) -> egui::Rect {
        egui::Rect::from_min_size(self.pos(r.x, r.y), vec2(r.w, r.h))
    }
}

thread_local! {
    static CANVAS: RefCell<Option<Canvas>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Canvas) -> R) -> R {
    CANVAS.with(|c| f(c.borrow_mut().as_mut().expect("drawing outside a frame")))
}

/// Starts a frame whose canvas sits at `origin` (points) in the window.
pub fn begin(ctx: &Context, origin: Vec2) {
    let screen = ctx.content_rect();
    let ui = Ui::new(
        ctx.clone(),
        Id::unique("inari-canvas"),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(screen),
    );
    let canvas = Canvas {
        painter: ctx.layer_painter(LayerId::background()),
        ui,
        origin,
        time: ctx.input(|i| i.time),
        animating: false,
    };
    CANVAS.with(|c| *c.borrow_mut() = Some(canvas));
}

/// Ends the frame; true when something animates and wants another soon.
pub fn end() -> bool {
    CANVAS
        .with(|c| c.borrow_mut().take())
        .is_some_and(|c| c.animating)
}

pub fn ctx() -> Context {
    with(|c| c.ui.ctx().clone())
}

/// Seconds since the app started, for animations; marks the frame animated.
pub fn time() -> f64 {
    with(|c| {
        c.animating = true;
        c.time
    })
}

/// Seconds since the app started, without asking for more frames.
pub fn clock() -> f64 {
    with(|c| c.time)
}

pub fn cursor(icon: CursorIcon) {
    with(|c| c.ui.ctx().set_cursor_icon(icon));
}

// ----------------------------------------------------------------- shapes

pub fn draw_rectangle(x: f32, y: f32, w: f32, h: f32, color: Color) {
    with(|c| {
        let r = c.rect(Rect::new(x, y, w, h));
        c.painter.rect_filled(r, 0., color);
    });
}

/// An outline inside the rectangle, half `thickness` wide: the weight every
/// outline on the table was designed with.
pub fn draw_rectangle_lines(x: f32, y: f32, w: f32, h: f32, thickness: f32, color: Color) {
    with(|c| {
        let r = c.rect(Rect::new(x, y, w, h));
        let stroke = Stroke::new(thickness / 2., color);
        c.painter.rect_stroke(r, 0., stroke, StrokeKind::Inside);
    });
}

pub fn draw_line(x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
    with(|c| {
        let points = [c.pos(x1, y1), c.pos(x2, y2)];
        c.painter
            .line_segment(points, Stroke::new(thickness, color));
    });
}

pub fn draw_circle(x: f32, y: f32, r: f32, color: Color) {
    with(|c| {
        c.painter.circle_filled(c.pos(x, y), r, color);
    });
}

/// A rectangle shading evenly from `left` to `right`.
pub fn draw_gradient(r: Rect, left: Color, right: Color) {
    with(|c| {
        let rect = c.rect(r);
        let (left, right) = (Color32::from(left), Color32::from(right));
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(rect.left_top(), left);
        mesh.colored_vertex(rect.right_top(), right);
        mesh.colored_vertex(rect.right_bottom(), right);
        mesh.colored_vertex(rect.left_bottom(), left);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        c.painter.add(mesh);
    });
}

/// A filled rounded rectangle.
pub fn rrect(r: Rect, radius: f32, color: Color) {
    with(|c| {
        let radius = radius.min(r.w / 2.).min(r.h / 2.).max(0.);
        c.painter
            .rect_filled(c.rect(r), CornerRadius::from(radius), color);
    });
}

pub fn load_texture(name: &str, w: usize, h: usize, rgba: &[u8]) -> TextureHandle {
    let image = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba);
    ctx().load_texture(name, image, TextureOptions::LINEAR)
}

pub fn draw_texture(texture: TextureId, r: Rect) {
    with(|c| {
        let uv = egui::Rect::from_min_max(pos2(0., 0.), pos2(1., 1.));
        c.painter.image(texture, c.rect(r), uv, Color32::WHITE);
    });
}

// ------------------------------------------------------------------- text

fn layout(c: &Canvas, text: &str, size: f32, face: Face, color: Color) -> Arc<Galley> {
    c.painter
        .layout_no_wrap(text.to_owned(), face.font(size), color.into())
}

/// The distance from a line's top to its baseline.
fn ascent(galley: &Galley, size: f32) -> f32 {
    galley
        .rows
        .first()
        .and_then(|row| row.glyphs.first())
        .map_or(size * 0.8, |g| g.pos.y)
}

pub fn measure(text: &str, size: f32, face: Face) -> f32 {
    if text.is_empty() {
        return 0.;
    }
    with(|c| layout(c, text, size, face, WHITE).size().x)
}

/// Text whose baseline starts at (x, y).
pub fn draw_text(text: &str, x: f32, y: f32, size: f32, color: Color, face: Face) {
    if text.is_empty() {
        return;
    }
    with(|c| {
        let galley = layout(c, text, size, face, color);
        let top = c.pos(x, y - ascent(&galley, size));
        c.painter.galley(top, galley, color.into());
    });
}

/// Text that can be selected and copied with the mouse, along with every
/// other selectable text on the canvas. Placed as `draw_text` places it.
pub fn selectable_text(text: &str, x: f32, y: f32, size: f32, color: Color, face: Face) {
    if text.is_empty() {
        return;
    }
    with(|c| {
        let galley = layout(c, text, size, face, color);
        let top = c.pos(x, y - ascent(&galley, size));
        let rect = egui::Rect::from_min_size(top, galley.size());
        let label = egui::Label::new(galley)
            .selectable(true)
            .wrap_mode(egui::TextWrapMode::Extend);
        c.ui.put(rect, label);
    });
}

/// Invisible selectable text squeezed to `width`, over a picture such as an
/// icon: selecting across the picture copies `text`, its name.
pub fn selectable_stand_in(text: &str, x: f32, y: f32, width: f32, size: f32, face: Face) {
    if text.is_empty() {
        return;
    }
    with(|c| {
        let natural = layout(c, text, size, face, WHITE).size().x;
        let gaps = text.chars().count().saturating_sub(1).max(1) as f32;
        let mut job = egui::text::LayoutJob::single_section(
            text.to_owned(),
            egui::TextFormat {
                font_id: face.font(size),
                color: Color32::TRANSPARENT,
                extra_letter_spacing: (width - natural) / gaps,
                ..Default::default()
            },
        );
        job.wrap.max_width = f32::INFINITY;
        let galley = c.painter.layout_job(job);
        let top = c.pos(x, y - ascent(&galley, size));
        let rect = egui::Rect::from_min_size(top, galley.size());
        let label = egui::Label::new(galley)
            .selectable(true)
            .wrap_mode(egui::TextWrapMode::Extend);
        // Selected text is recoloured with the selection stroke: keep it
        // invisible, so only the highlight shows over the picture.
        let mut ui = c.ui.new_child(egui::UiBuilder::new().max_rect(rect));
        ui.visuals_mut().selection.stroke.color = Color32::TRANSPARENT;
        ui.put(rect, label);
    });
}

/// An editable single-line text field filling `r`, with a caret, mouse
/// selection and the clipboard. Returns whether it has keyboard focus.
pub fn text_field(id: &str, text: &mut String, r: Rect, size: f32, ink: Color) -> bool {
    with(|c| {
        // The text keeps 10 units clear of the field's edges.
        let rect = c.rect(r).shrink2(vec2(10., 0.));
        let edit = TextEdit::singleline(text)
            .id(c.ui.make_persistent_id(id))
            .font(Face::Body.font(size))
            .text_color(ink.into())
            .frame(egui::Frame::NONE)
            .margin(Margin::ZERO)
            .align(Align2::LEFT_CENTER)
            .min_size(rect.size())
            .desired_width(rect.width());
        let response = c.ui.put(rect, edit);
        response.has_focus()
    })
}

/// Whether the text field `id` has keyboard focus (as of the last frame).
pub fn has_focus(id: &str) -> bool {
    with(|c| {
        let id = c.ui.make_persistent_id(id);
        c.ui.ctx().memory(|m| m.has_focus(id))
    })
}

/// Takes keyboard focus away from every text field.
pub fn drop_focus() {
    with(|c| c.ui.ctx().memory_mut(|m| m.stop_text_input()));
}

/// Covers `r` so egui widgets drawn earlier (selectable text) take no pointer
/// input through it; it reports drags for moving windows.
pub fn cover(id: &str, r: Rect) -> Response {
    with(|c| {
        let rect = c.rect(r);
        let id = c.ui.scope_id().with(id);
        c.ui.interact(rect, id, Sense::click_and_drag())
    })
}

// ------------------------------------------------------------------ input

/// The pointer this frame, in canvas units.
pub struct Pointer {
    pub pos: Vec2,
    /// The left button went down.
    pub click: bool,
    /// The left button is down (dragging).
    pub held: bool,
    pub right_click: bool,
    /// Whole wheel notches: positive moves the content down (scrolls up).
    pub scroll: f32,
}

thread_local! {
    /// Wheel movement short of a whole notch, carried to the next frame.
    static WHEEL: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

pub fn pointer() -> Pointer {
    with(|c| {
        let origin = c.origin;
        c.ui.ctx().input(|i| {
            // A notch is a line, or 50 points of smooth scrolling.
            let wheel: f32 = i
                .events
                .iter()
                .map(|e| match e {
                    egui::Event::MouseWheel { unit, delta, .. } => match unit {
                        egui::MouseWheelUnit::Point => delta.y / 50.,
                        egui::MouseWheelUnit::Line => delta.y,
                        egui::MouseWheelUnit::Page => delta.y * 10.,
                    },
                    _ => 0.,
                })
                .sum();
            let total = WHEEL.get() + wheel;
            let notches = total.trunc();
            WHEEL.set(total - notches);
            Pointer {
                pos: i
                    .pointer
                    .latest_pos()
                    .map_or(vec2(-1e5, -1e5), |p| p.to_vec2() - origin),
                click: i.pointer.primary_pressed(),
                held: i.pointer.primary_down(),
                right_click: i.pointer.secondary_pressed(),
                scroll: notches,
            }
        })
    })
}

pub fn key_pressed(key: Key) -> bool {
    with(|c| c.ui.ctx().input(|i| i.key_pressed(key)))
}

/// Ctrl, or Cmd on a Mac.
pub fn command_down() -> bool {
    with(|c| {
        c.ui.ctx()
            .input(|i| i.modifiers.command || i.modifiers.ctrl)
    })
}

// -------------------------------------------------------------------- png

/// Writes straight-alpha RGBA rows, top row first, as a PNG file.
pub fn write_png(path: &std::path::Path, w: usize, h: usize, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}
