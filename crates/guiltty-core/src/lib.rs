//! Backend-agnostic core primitives (`Color`, `Point`, `Rect`) and the [`Backend`] trait that
//! rendering backends (e.g. `guiltty-kitty`) implement. `Canvas` supports shape drawing, text,
//! and movable sprites (see [`Canvas::draw_sprite`]); region/zoom/scroll logic described in the
//! spec is not implemented yet.

/// RGBA8 color, used throughout for canvas pixels, shape fills, and sprite bitmaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Creates a fully opaque color (alpha = 255) from the given RGB components.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a color from explicit RGBA components.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// A pixel-addressable point, origin top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// Creates a point at the given pixel coordinates (origin top-left).
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// A pixel-addressable rectangle, origin top-left, width/height in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Creates a rectangle at pixel coordinates `(x, y)` (origin top-left) with the given
    /// `width`/`height` in pixels.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Errors surfaced by the public API. Panics are reserved for programmer-error
/// invariants only; recoverable conditions (e.g. a failed terminal write) go through here.
#[derive(Debug)]
pub enum Error {
    /// Placeholder variant until backend implementations define real error cases.
    Backend(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Backend(msg) => write!(f, "backend error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// A rendering backend capable of presenting drawn output to a real terminal.
/// Implemented by backend crates (e.g. `guiltty-kitty`); `guiltty-core` never
/// depends on a specific backend.
pub trait Backend {
    /// The error type this backend surfaces from its operations. Lets backends expose
    /// richer, backend-specific error information while still fitting this trait; backends
    /// with no need for that can simply use [`Error`].
    type Error: std::error::Error;

    /// Presents `canvas`'s current pixel buffer to the terminal.
    fn present(&mut self, canvas: &Canvas) -> Result<(), Self::Error>;
}

/// Monotonically increasing counter handing out a fresh, unique id to every `Canvas`
/// instance (including ones produced by `Canvas::clone()`) — see `Canvas::id`.
static NEXT_CANVAS_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A pixel buffer that can be drawn into. RGBA8, origin top-left, row-major.
#[derive(Debug)]
pub struct Canvas {
    /// Distinguishes this canvas from every other one, including clones of itself, so a
    /// `Sprite`'s saved-under footprint (see [`DrawnFootprint`]) is never restored onto
    /// the wrong canvas.
    id: u64,
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

/// Manually implemented (rather than `#[derive(Clone)]`) so a cloned canvas gets its own
/// fresh `id` instead of inheriting the original's — see the `id` field's doc comment.
impl Clone for Canvas {
    fn clone(&self) -> Self {
        Self {
            id: NEXT_CANVAS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            width: self.width,
            height: self.height,
            pixels: self.pixels.clone(),
        }
    }
}

impl Canvas {
    /// Creates a canvas of the given size, every pixel starting fully transparent
    /// (`Color::default()`, alpha = 0).
    ///
    /// # Panics
    /// Panics if `width * height` overflows `usize` — on 32-bit targets (where `usize`
    /// is only 32 bits wide) this can happen well below `u32::MAX` on either dimension.
    ///
    /// Also panics if the resulting `Vec<Color>` allocation exceeds the platform's
    /// maximum capacity — dimensions whose pixel count fits `usize` may still overflow
    /// available memory when multiplied by `size_of::<Color>()`.
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize)
            .checked_mul(height as usize)
            .expect("Canvas dimensions too large: width * height overflows usize");
        Self {
            id: NEXT_CANVAS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            width,
            height,
            pixels: vec![Color::default(); len],
        }
    }

    /// Canvas width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Canvas height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Row-major pixel index for `(x, y)`, or `None` if out of bounds. Centralizes the
    /// bounds check and index arithmetic — done in `usize` so it can't overflow the way
    /// a `u32` `y * width + x` computation could for very large canvases.
    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Returns the color at `(x, y)`, or `None` if out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    /// Sets the color at `(x, y)`. Silently ignores out-of-bounds coordinates — there's
    /// nothing a caller needs recover from, so this isn't a `Result`.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = color;
        }
    }

    /// Flattens this canvas's pixel buffer into row-major RGBA8 bytes (4 bytes per pixel:
    /// R, G, B, A in that order), the format backends transmit to a terminal.
    pub fn rgba8_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.pixels.len() * 4);
        for color in &self.pixels {
            bytes.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
        bytes
    }

    /// Draws `text` starting at `origin` (top-left of the first glyph) using `style`.
    ///
    /// v0's built-in font covers only space, digits, and uppercase `A`-`Z` (see the
    /// `font` module) — unsupported characters (lowercase, punctuation, non-ASCII) are
    /// skipped, leaving a blank glyph-width gap so surrounding text stays aligned.
    ///
    /// All coordinate/scale arithmetic is done in `i64` — wide enough that no
    /// `u32`/`i32`-representable `origin`, canvas size, or `TextStyle::scale` can
    /// overflow it — and each glyph's scaled block is intersected with the canvas
    /// bounds before iterating its destination pixels, rather than iterating every
    /// `scale²` pixel and discarding out-of-bounds ones one at a time.
    pub fn draw_text(&mut self, text: &str, origin: Point, style: &TextStyle) {
        let scale = style.scale.max(1) as i64;
        let advance = (font::GLYPH_WIDTH as i64 + 1) * scale;
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let origin_y = origin.y as i64;
        let mut cursor_x = origin.x as i64;

        for ch in text.chars() {
            if cursor_x >= canvas_w {
                break; // everything further right is off-canvas; nothing more to draw
            }
            if let Some(rows) = font::glyph(ch) {
                self.draw_glyph(
                    &rows,
                    cursor_x,
                    origin_y,
                    scale,
                    canvas_w,
                    canvas_h,
                    style.color,
                );
            }
            cursor_x += advance;
        }
    }

    /// Renders one glyph's `'#'` pixels (each scaled to a `scale`x`scale` block) at
    /// `(cursor_x, origin_y)`. Split out of `draw_text` purely to keep nesting shallow
    /// there — iterating a glyph's rows/columns is naturally two loops deep on its own.
    #[allow(clippy::too_many_arguments)]
    fn draw_glyph(
        &mut self,
        rows: &[&str],
        cursor_x: i64,
        origin_y: i64,
        scale: i64,
        canvas_w: i64,
        canvas_h: i64,
        color: Color,
    ) {
        for (row_idx, row) in rows.iter().enumerate() {
            let py0 = origin_y + row_idx as i64 * scale;
            if py0 + scale <= 0 || py0 >= canvas_h {
                continue; // this glyph row is entirely above/below the canvas
            }
            for (col_idx, pixel) in row.chars().enumerate() {
                if pixel != '#' {
                    continue;
                }
                let px0 = cursor_x + col_idx as i64 * scale;
                if px0 + scale <= 0 || px0 >= canvas_w {
                    continue; // this glyph column is entirely off the left/right edge
                }
                let (y_lo, y_hi) = (py0.max(0), (py0 + scale).min(canvas_h));
                let (x_lo, x_hi) = (px0.max(0), (px0 + scale).min(canvas_w));
                self.fill_clipped_rect(x_lo, x_hi, y_lo, y_hi, color);
            }
        }
    }
}

/// Styling for [`Canvas::draw_text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    pub color: Color,
    /// Integer pixel-scale factor for each font pixel (1 = native 3x5 glyph size).
    /// `draw_text` clamps `0` up to `1` — there's no such thing as zero-size text.
    pub scale: u32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::rgb(255, 255, 255),
            scale: 1,
        }
    }
}

/// Minimal built-in bitmap font for v0: 3x5 pixel glyphs covering space, digits, and
/// uppercase `A`-`Z` only. Deliberately incomplete (no lowercase, punctuation, or
/// non-ASCII) — full font coverage is a follow-up task once real text needs demand it.
mod font {
    pub const GLYPH_WIDTH: u32 = 3;

    /// Returns the glyph for `ch` as 5 rows of `GLYPH_WIDTH` characters (`'#'` = lit
    /// pixel, anything else = unlit), or `None` for unsupported characters.
    pub fn glyph(ch: char) -> Option<[&'static str; 5]> {
        Some(match ch {
            ' ' => ["...", "...", "...", "...", "..."],
            '0' => ["###", "#.#", "#.#", "#.#", "###"],
            '1' => [".#.", "##.", ".#.", ".#.", "###"],
            '2' => ["###", "..#", "###", "#..", "###"],
            '3' => ["###", "..#", "###", "..#", "###"],
            '4' => ["#.#", "#.#", "###", "..#", "..#"],
            '5' => ["###", "#..", "###", "..#", "###"],
            '6' => ["###", "#..", "###", "#.#", "###"],
            '7' => ["###", "..#", "..#", "..#", "..#"],
            '8' => ["###", "#.#", "###", "#.#", "###"],
            '9' => ["###", "#.#", "###", "..#", "###"],
            'A' => [".#.", "#.#", "###", "#.#", "#.#"],
            'B' => ["##.", "#.#", "##.", "#.#", "##."],
            'C' => ["###", "#..", "#..", "#..", "###"],
            'D' => ["##.", "#.#", "#.#", "#.#", "##."],
            'E' => ["###", "#..", "##.", "#..", "###"],
            'F' => ["###", "#..", "##.", "#..", "#.."],
            'G' => ["###", "#..", "#.#", "#.#", "###"],
            'H' => ["#.#", "#.#", "###", "#.#", "#.#"],
            'I' => ["###", ".#.", ".#.", ".#.", "###"],
            'J' => ["..#", "..#", "..#", "#.#", "###"],
            'K' => ["#.#", "#.#", "##.", "#.#", "#.#"],
            'L' => ["#..", "#..", "#..", "#..", "###"],
            'M' => ["#.#", "###", "###", "#.#", "#.#"],
            'N' => ["#.#", "###", "###", "###", "#.#"],
            'O' => ["###", "#.#", "#.#", "#.#", "###"],
            'P' => ["###", "#.#", "###", "#..", "#.."],
            'Q' => ["###", "#.#", "#.#", "###", "..#"],
            'R' => ["###", "#.#", "##.", "#.#", "#.#"],
            'S' => ["###", "#..", "###", "..#", "###"],
            'T' => ["###", ".#.", ".#.", ".#.", ".#."],
            'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
            'V' => ["#.#", "#.#", "#.#", "#.#", ".#."],
            'W' => ["#.#", "#.#", "###", "###", "#.#"],
            'X' => ["#.#", "#.#", ".#.", "#.#", "#.#"],
            'Y' => ["#.#", "#.#", ".#.", ".#.", ".#."],
            'Z' => ["###", "..#", ".#.", "#..", "###"],
            _ => return None,
        })
    }
}

/// How a [`Shape`] is painted. v0 supports solid color only; more fill kinds
/// (gradients, patterns) can be added as new variants without breaking callers
/// that already match on `Fill::Solid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Solid(Color),
}

impl Fill {
    /// A solid, uniform color fill.
    pub fn solid(color: Color) -> Self {
        Self::Solid(color)
    }

    fn color(&self) -> Color {
        match self {
            Self::Solid(c) => *c,
        }
    }
}

/// A drawable shape. Rects/circles/ellipses/triangles/closed paths are filled solid;
/// `Line` and open `Path`s have no interior to fill, so their `Fill`'s color is used as
/// the stroke color instead.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line {
        from: Point,
        to: Point,
    },
    Rect {
        origin: Point,
        width: u32,
        height: u32,
    },
    Circle {
        center: Point,
        radius: u32,
    },
    Ellipse {
        center: Point,
        rx: u32,
        ry: u32,
    },
    Triangle {
        a: Point,
        b: Point,
        c: Point,
    },
    /// An arbitrary open or closed path connecting `points` in order. An **open** path
    /// (`closed: false`) is drawn as connected line segments (stroke only). A **closed**
    /// path (`closed: true`) is filled solid instead, using a scanline polygon fill with
    /// the **even-odd rule** (simpler than nonzero-winding, standard for a scanline fill,
    /// and sufficient since v0 doesn't need winding-direction semantics) — this also
    /// correctly handles concave and self-intersecting closed paths, not just convex
    /// ones. `closed` no longer means "also stroke the closing edge": it selects fill
    /// instead of stroke entirely, consistent with how `Rect`/`Circle`/`Ellipse`/
    /// `Triangle` are filled solid with no separate outline stroke.
    Path {
        points: Vec<Point>,
        closed: bool,
    },
}

impl Shape {
    pub fn line(from: Point, to: Point) -> Self {
        Self::Line { from, to }
    }

    pub fn rect(origin: Point, width: u32, height: u32) -> Self {
        Self::Rect {
            origin,
            width,
            height,
        }
    }

    pub fn circle(center: Point, radius: u32) -> Self {
        Self::Circle { center, radius }
    }

    pub fn ellipse(center: Point, rx: u32, ry: u32) -> Self {
        Self::Ellipse { center, rx, ry }
    }

    pub fn triangle(a: Point, b: Point, c: Point) -> Self {
        Self::Triangle { a, b, c }
    }

    pub fn path(points: Vec<Point>, closed: bool) -> Self {
        Self::Path { points, closed }
    }
}

impl Canvas {
    /// Draws `shape` into this canvas using `fill`. See [`Shape`] for which variants are
    /// filled solid vs. stroked.
    pub fn draw_shape(&mut self, shape: &Shape, fill: Fill) {
        let color = fill.color();
        match shape {
            Shape::Line { from, to } => self.stroke_line(*from, *to, color),
            Shape::Rect {
                origin,
                width,
                height,
            } => self.fill_rect(*origin, *width, *height, color),
            Shape::Circle { center, radius } => self.fill_ellipse(*center, *radius, *radius, color),
            Shape::Ellipse { center, rx, ry } => self.fill_ellipse(*center, *rx, *ry, color),
            Shape::Triangle { a, b, c } => self.fill_triangle(*a, *b, *c, color),
            Shape::Path { points, closed } => {
                if *closed {
                    self.fill_polygon_even_odd(points, color);
                } else {
                    self.stroke_path(points, color);
                }
            }
        }
    }

    /// Bresenham's line algorithm — no anti-aliasing, one pixel wide. The segment is
    /// clipped to canvas bounds first (via [`liang_barsky_clip`]), so an extremely long
    /// or mostly-offscreen line only ever walks pixels that could actually land on the
    /// canvas, rather than potentially billions of invisible steps.
    fn stroke_line(&mut self, from: Point, to: Point, color: Color) {
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let Some((mut x0, mut y0, x1, y1)) = liang_barsky_clip(
            from.x as i64,
            from.y as i64,
            to.x as i64,
            to.y as i64,
            canvas_w,
            canvas_h,
        ) else {
            return;
        };

        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx: i64 = if x0 < x1 { 1 } else { -1 };
        let sy: i64 = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x0 >= 0 && y0 >= 0 {
                self.set_pixel(x0 as u32, y0 as u32, color);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Strokes an open path as connected line segments. Closed paths are filled instead
    /// (see [`Shape::Path`]), so this never needs to draw a closing segment.
    fn stroke_path(&mut self, points: &[Point], color: Color) {
        for pair in points.windows(2) {
            self.stroke_line(pair[0], pair[1], color);
        }
    }

    /// Fills a rectangle, clipped to canvas bounds up front (so a huge or far-offscreen
    /// rectangle doesn't still iterate its full, mostly-invisible extent) and computed in
    /// `i64` to avoid overflow for extreme `origin`/`width`/`height` combinations.
    fn fill_rect(&mut self, origin: Point, width: u32, height: u32, color: Color) {
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let x0 = origin.x as i64;
        let y0 = origin.y as i64;
        let x_hi = (x0 + width as i64).min(canvas_w);
        let y_hi = (y0 + height as i64).min(canvas_h);
        self.fill_clipped_rect(x0.max(0), x_hi, y0.max(0), y_hi, color);
    }

    /// Fills the rectangle `[x_lo, x_hi) x [y_lo, y_hi)`, in canvas pixel space, with
    /// `color`. Callers are responsible for having already clipped this range to canvas
    /// bounds (an empty range, `x_hi <= x_lo` or `y_hi <= y_lo`, is a harmless no-op).
    /// Shared by `fill_rect` and `draw_text`'s per-glyph scaled pixel block, so neither
    /// needs its own nested fill loop.
    fn fill_clipped_rect(&mut self, x_lo: i64, x_hi: i64, y_lo: i64, y_hi: i64, color: Color) {
        for y in y_lo..y_hi {
            for x in x_lo..x_hi {
                self.set_pixel(x as u32, y as u32, color);
            }
        }
    }

    /// Filled ellipse; a circle is the `rx == ry` case (see [`Shape::Circle`]). The scan
    /// range is clipped to canvas bounds up front (same reasoning as `fill_rect`). The
    /// inclusion test is done in `f64` as `(dx/rx)^2 + (dy/ry)^2 <= 1` rather than
    /// cross-multiplied integers — `u32::MAX`-sized radii overflow even `i128` in the
    /// cross-multiplied form (`rx^2*ry^2 + ry^2*rx^2` can exceed `u128::MAX` at the
    /// bounding-box corners), while every value here is an exact `f64` integer (well
    /// under 2^53), so the division introduces no meaningful rasterization error.
    fn fill_ellipse(&mut self, center: Point, rx: u32, ry: u32, color: Color) {
        if rx == 0 || ry == 0 {
            return;
        }
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let (cx, cy) = (center.x as i64, center.y as i64);
        let (rx64, ry64) = (rx as i64, ry as i64);
        let (rxf, ryf) = (rx as f64, ry as f64);

        let y_lo = (cy - ry64).max(0);
        let y_hi = (cy + ry64).min(canvas_h - 1);
        let x_lo = (cx - rx64).max(0);
        let x_hi = (cx + rx64).min(canvas_w - 1);

        for y in y_lo..=y_hi {
            let dyr = (y - cy) as f64 / ryf;
            for x in x_lo..=x_hi {
                let dxr = (x - cx) as f64 / rxf;
                if dxr * dxr + dyr * dyr <= 1.0 {
                    self.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }

    /// Fills a triangle via a barycentric sign test, scanning only the triangle's
    /// bounding box intersected with canvas bounds (so an offscreen or oversized triangle
    /// doesn't scan its full, mostly-invisible extent). Coordinates stay in `i64` all the
    /// way to `set_pixel` — never downcast to `i32` — so no precision is lost even for a
    /// canvas wider than `i32::MAX`.
    fn fill_triangle(&mut self, a: Point, b: Point, c: Point, color: Color) {
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let min_x = (a.x.min(b.x).min(c.x) as i64).max(0);
        let max_x = (a.x.max(b.x).max(c.x) as i64).min(canvas_w - 1);
        let min_y = (a.y.min(b.y).min(c.y) as i64).max(0);
        let max_y = (a.y.max(b.y).max(c.y) as i64).min(canvas_h - 1);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if point_in_triangle(x, y, a, b, c) {
                    self.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }

    /// Fills a closed polygon (`points`, implicitly closed back to the first point) using
    /// a scanline fill with the **even-odd rule**: for each scanline, find every edge
    /// crossing, sort the crossing x-coordinates, and fill between each successive pair.
    /// This correctly handles concave polygons and self-intersecting ones too (each
    /// crossing still flips inside/outside, regardless of winding direction) -- unlike
    /// `fill_triangle`'s barycentric test, which only applies to (non-self-intersecting)
    /// triangles.
    ///
    /// Scans at `y + 0.5` (not integer `y`) so a polygon vertex never sits exactly on a
    /// scanline -- that would otherwise make the standard `(y1 <= yf) != (y2 <= yf)`
    /// crossing test count a shared vertex between two edges inconsistently.
    fn fill_polygon_even_odd(&mut self, points: &[Point], color: Color) {
        let n = points.len();
        if n < 3 {
            return; // Fewer than 3 points has no interior to fill.
        }

        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let min_y = points.iter().map(|p| p.y as i64).min().unwrap().max(0);
        let max_y = points
            .iter()
            .map(|p| p.y as i64)
            .max()
            .unwrap()
            .min(canvas_h - 1);

        for y in min_y..=max_y {
            let yf = y as f64 + 0.5;
            let mut crossings: Vec<f64> = Vec::new();
            for i in 0..n {
                let p1 = points[i];
                let p2 = points[(i + 1) % n];
                let (y1, y2) = (p1.y as f64, p2.y as f64);
                if (y1 <= yf) != (y2 <= yf) {
                    let x1 = p1.x as f64;
                    let x2 = p2.x as f64;
                    let t = (yf - y1) / (y2 - y1);
                    crossings.push(x1 + t * (x2 - x1));
                }
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap());

            for pair in crossings.chunks_exact(2) {
                let x_lo = (pair[0].ceil() as i64).max(0);
                let x_hi = (pair[1].floor() as i64 + 1).min(canvas_w);
                for x in x_lo..x_hi {
                    self.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

/// A small RGBA8 image used as sprite content. Structurally similar to `Canvas`'s pixel
/// buffer, but represents drawable material rather than a render target. v0 only
/// supports building bitmaps in-memory — there's no file/PNG loading yet.
#[derive(Debug, Clone)]
pub struct Bitmap {
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

impl Bitmap {
    /// Creates a bitmap from an explicit pixel buffer (row-major, RGBA8).
    ///
    /// # Panics
    /// Panics if `pixels.len() != width * height`, or via [`Bitmap::checked_len`] if
    /// `width`/`height` don't fit in `usize` on the current target, or if
    /// `width * height` overflows `usize`.
    pub fn new(width: u32, height: u32, pixels: Vec<Color>) -> Self {
        let expected = Self::checked_len(width, height);
        assert_eq!(
            pixels.len(),
            expected,
            "Bitmap::new: pixels.len() ({}) must equal width*height ({})",
            pixels.len(),
            expected
        );
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Creates a bitmap of the given size, every pixel set to `color`.
    pub fn solid(width: u32, height: u32, color: Color) -> Self {
        let len = Self::checked_len(width, height);
        Self {
            width,
            height,
            pixels: vec![color; len],
        }
    }

    /// `width * height` as a `usize`, converting each dimension with a checked cast first
    /// (rather than a truncating `as usize`) so this can't silently disagree with the
    /// `u32` dimensions on a target where `usize` is narrower than `u32`.
    fn checked_len(width: u32, height: u32) -> usize {
        let w: usize = width.try_into().expect("Bitmap width too large for usize");
        let h: usize = height
            .try_into()
            .expect("Bitmap height too large for usize");
        w.checked_mul(h)
            .expect("Bitmap dimensions too large: width * height overflows usize")
    }

    /// Bitmap width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Bitmap height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Row-major pixel index for `(x, y)`, or `None` if out of bounds — mirrors
    /// `Canvas::index`.
    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Returns the color at `(x, y)`, or `None` if out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        self.index(x, y).map(|i| self.pixels[i])
    }
}

/// The rectangle of canvas pixels a [`Sprite`] last drew over, saved so
/// [`Canvas::draw_sprite`] can restore them before drawing the sprite again elsewhere.
/// Tagged with the id of the `Canvas` it was captured from, so it's never mistakenly
/// restored onto a different canvas instance (see `Canvas::draw_sprite`).
#[derive(Debug)]
struct DrawnFootprint {
    canvas_id: u64,
    x: i64,
    y: i64,
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

/// A movable 2D bitmap positioned over a canvas. See [`Canvas::draw_sprite`] for how
/// moving and redrawing a sprite avoids leaving a trail of its previous position.
#[derive(Debug)]
pub struct Sprite {
    bitmap: Bitmap,
    position: Point,
    last_draw: Option<DrawnFootprint>,
}

/// Manually implemented (rather than `#[derive(Clone)]`) so a cloned sprite starts with
/// no drawing history of its own: it copies the bitmap and position, but not
/// `last_draw`, since the clone has never actually been drawn anywhere. Without this, a
/// clone of an already-drawn sprite would restore the *original* sprite's footprint the
/// first time it's drawn, corrupting whatever the original still shows on the canvas.
impl Clone for Sprite {
    fn clone(&self) -> Self {
        Self {
            bitmap: self.bitmap.clone(),
            position: self.position,
            last_draw: None,
        }
    }
}

impl Sprite {
    /// Creates a sprite from `bitmap`, placed at `position` (top-left of the bitmap).
    pub fn new(bitmap: Bitmap, position: Point) -> Self {
        Self {
            bitmap,
            position,
            last_draw: None,
        }
    }

    /// The sprite's current position.
    pub fn position(&self) -> Point {
        self.position
    }

    /// Moves the sprite to a new position. Takes effect the next time
    /// [`Canvas::draw_sprite`] is called with this sprite — that call restores whatever
    /// the sprite covered at its previous position before drawing it at the new one.
    pub fn move_to(&mut self, position: Point) {
        self.position = position;
    }

    /// The sprite's bitmap content.
    pub fn bitmap(&self) -> &Bitmap {
        &self.bitmap
    }
}

impl Canvas {
    /// Draws `sprite`'s bitmap onto this canvas at its current position, clipped to
    /// canvas bounds.
    ///
    /// Uses save-under/restore-under: if `sprite` was drawn by an earlier call to this
    /// method, whatever the canvas showed at that previous footprint is restored first,
    /// then the canvas content at the new footprint is captured before the sprite is
    /// drawn over it. This is what lets a sprite move and be redrawn repeatedly without
    /// leaving a trail of its old position — **as long as nothing else draws into its
    /// footprint in between two `draw_sprite` calls for it**; if something does, this
    /// restore overwrites that intervening content. A real per-frame redraw loop (a
    /// future `Terminal`/`Frame` abstraction redrawing the whole scene from scratch each
    /// frame) wouldn't have that limitation; this is the simpler, self-contained
    /// mechanism available today. Each `Sprite` only remembers its own last footprint —
    /// drawing a different sprite, or drawing this one onto a different `Canvas`, doesn't
    /// know about or restore anything another sprite painted over the same area.
    ///
    /// Within the new footprint, fully transparent bitmap pixels (`alpha == 0`) are
    /// skipped rather than overwriting the canvas; non-transparent pixels replace
    /// outright (no alpha blending, matching the no-anti-aliasing precedent set by
    /// `draw_shape`).
    pub fn draw_sprite(&mut self, sprite: &mut Sprite) {
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;

        if let Some(prev) = sprite.last_draw.take() {
            // A footprint captured from a *different* canvas has nothing to do with this
            // one's current pixels; restoring it here would corrupt this canvas with
            // stale content from elsewhere, so just drop it instead.
            if prev.canvas_id == self.id {
                self.restore_footprint(&prev);
            }
        }

        let (px, py) = (sprite.position.x as i64, sprite.position.y as i64);
        let bmp_w = sprite.bitmap.width as i64;
        let bmp_h = sprite.bitmap.height as i64;
        let x_lo = px.max(0);
        let x_hi = (px + bmp_w).min(canvas_w);
        let y_lo = py.max(0);
        let y_hi = (py + bmp_h).min(canvas_h);
        let cap_w = (x_hi - x_lo).max(0) as usize;
        let cap_h = (y_hi - y_lo).max(0) as usize;
        let mut saved = Vec::with_capacity(cap_w * cap_h);

        let canvas_row_len = self.width as usize;
        let bmp_row_len = sprite.bitmap.width as usize;
        for y in y_lo..y_hi {
            let canvas_row = y as usize * canvas_row_len;
            let bmp_row = (y - py) as usize * bmp_row_len;
            for x in x_lo..x_hi {
                // In-bounds by construction: x_lo/x_hi/y_lo/y_hi are already clipped to
                // both canvas and bitmap dimensions, so no bounds check is needed here.
                let idx = canvas_row + x as usize;
                saved.push(self.pixels[idx]);
                let color = sprite.bitmap.pixels[bmp_row + (x - px) as usize];
                if color.a != 0 {
                    self.pixels[idx] = color;
                }
            }
        }

        sprite.last_draw = Some(DrawnFootprint {
            canvas_id: self.id,
            x: x_lo,
            y: y_lo,
            width: cap_w as u32,
            height: cap_h as u32,
            pixels: saved,
        });
    }

    /// Restores the canvas pixels a sprite's previous footprint covered, clipped to
    /// whatever part of that footprint still falls within current canvas bounds.
    fn restore_footprint(&mut self, prev: &DrawnFootprint) {
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let row_len = prev.width as usize;
        for dy in 0..prev.height as i64 {
            let y = prev.y + dy;
            if y < 0 || y >= canvas_h {
                continue;
            }
            let row = dy as usize * row_len;
            for dx in 0..prev.width as i64 {
                let x = prev.x + dx;
                if x < 0 || x >= canvas_w {
                    continue;
                }
                self.set_pixel(x as u32, y as u32, prev.pixels[row + dx as usize]);
            }
        }
    }
}

/// Clips the segment `(x0,y0)-(x1,y1)` to `[0,w) x [0,h)` via Liang-Barsky, returning the
/// clipped integer endpoints, or `None` if the segment doesn't intersect the canvas at
/// all. Used by `stroke_line` so an extreme-but-valid `Point` pair (e.g. one endpoint at
/// `i32::MAX`) can't force a Bresenham walk of billions of steps — the walk only ever
/// covers the (canvas-bounded) visible portion of the segment.
fn liang_barsky_clip(
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    w: i64,
    h: i64,
) -> Option<(i64, i64, i64, i64)> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let dx = (x1 - x0) as f64;
    let dy = (y1 - y0) as f64;
    let mut t0 = 0.0f64;
    let mut t1 = 1.0f64;
    // (p, q) pairs for the left/right/top/bottom boundaries, in Liang-Barsky's form.
    let checks = [
        (-dx, x0 as f64),
        (dx, (w - 1 - x0) as f64),
        (-dy, y0 as f64),
        (dy, (h - 1 - y0) as f64),
    ];
    for (p, q) in checks {
        if p == 0.0 {
            if q < 0.0 {
                return None; // parallel to this boundary and outside it
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return None;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }
    if t0 > t1 {
        return None;
    }
    let clamp = |t: f64| -> (i64, i64) {
        (
            (x0 as f64 + t * dx).round() as i64,
            (y0 as f64 + t * dy).round() as i64,
        )
    };
    let (cx0, cy0) = clamp(t0);
    let (cx1, cy1) = clamp(t1);
    Some((cx0, cy0, cx1, cy1))
}

/// Sign of the cross product `(p2 - p1) x (p - p1)`; used by [`point_in_triangle`] to
/// tell which side of edge `p1`-`p2` the point `(px, py)` is on. Widened to `i128` so a
/// triangle spanning the full `i32` coordinate range can't overflow the cross-product
/// terms (an `i64` version can, for points near the domain's extremes).
fn edge_sign(px: i64, py: i64, p1: Point, p2: Point) -> i128 {
    let dx_edge = i128::from(p2.x) - i128::from(p1.x);
    let dy_edge = i128::from(p2.y) - i128::from(p1.y);
    let dx_point = i128::from(px) - i128::from(p1.x);
    let dy_point = i128::from(py) - i128::from(p1.y);
    dx_edge * dy_point - dy_edge * dx_point
}

fn point_in_triangle(px: i64, py: i64, a: Point, b: Point, c: Point) -> bool {
    let d1 = edge_sign(px, py, a, b);
    let d2 = edge_sign(px, py, b, c);
    let d3 = edge_sign(px, py, c, a);
    let has_neg = d1 < 0 || d2 < 0 || d3 < 0;
    let has_pos = d1 > 0 || d2 > 0 || d3 > 0;
    !(has_neg && has_pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_rgb_defaults_to_opaque() {
        let c = Color::rgb(10, 20, 30);
        assert_eq!(c, Color::rgba(10, 20, 30, 255));
    }

    #[test]
    fn point_new_sets_coordinates() {
        let p = Point::new(3, 4);
        assert_eq!(p, Point { x: 3, y: 4 });
    }

    #[test]
    fn rect_new_sets_fields() {
        let r = Rect::new(1, 2, 100, 50);
        assert_eq!(
            r,
            Rect {
                x: 1,
                y: 2,
                width: 100,
                height: 50
            }
        );
    }

    #[test]
    fn canvas_new_is_fully_transparent() {
        let c = Canvas::new(4, 3);
        assert_eq!(c.width(), 4);
        assert_eq!(c.height(), 3);
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(c.pixel(x, y), Some(Color::default()));
            }
        }
    }

    #[test]
    fn canvas_pixel_out_of_bounds_is_none() {
        let c = Canvas::new(2, 2);
        assert_eq!(c.pixel(2, 0), None);
        assert_eq!(c.pixel(0, 2), None);
    }

    #[test]
    fn canvas_set_pixel_roundtrips() {
        let mut c = Canvas::new(2, 2);
        c.set_pixel(1, 1, Color::rgb(1, 2, 3));
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(1, 2, 3)));
        assert_eq!(c.pixel(0, 0), Some(Color::default()));
    }

    #[test]
    fn canvas_set_pixel_out_of_bounds_is_noop() {
        let mut c = Canvas::new(2, 2);
        c.set_pixel(5, 5, Color::rgb(1, 2, 3)); // must not panic
        assert_eq!(c.pixel(0, 0), Some(Color::default()));
    }

    #[test]
    fn draw_text_renders_known_glyph() {
        let mut c = Canvas::new(3, 5);
        let style = TextStyle {
            color: Color::rgb(255, 0, 0),
            scale: 1,
        };
        c.draw_text("I", Point::new(0, 0), &style);
        // 'I' is ["###", ".#.", ".#.", ".#.", "###"]
        assert_eq!(c.pixel(0, 0), Some(style.color));
        assert_eq!(c.pixel(1, 0), Some(style.color));
        assert_eq!(c.pixel(2, 0), Some(style.color));
        assert_eq!(c.pixel(0, 1), Some(Color::default()));
        assert_eq!(c.pixel(1, 1), Some(style.color));
        assert_eq!(c.pixel(2, 1), Some(Color::default()));
        assert_eq!(c.pixel(1, 4), Some(style.color));
    }

    #[test]
    fn draw_text_skips_unsupported_chars_but_still_advances() {
        let mut c = Canvas::new(8, 5);
        let style = TextStyle::default();
        c.draw_text("aI", Point::new(0, 0), &style); // 'a' unsupported, skipped
        for y in 0..5 {
            for x in 0..4 {
                assert_eq!(c.pixel(x, y), Some(Color::default()), "at ({x},{y})");
            }
        }
        // 'I' should render shifted by one glyph advance (GLYPH_WIDTH + 1 = 4)
        assert_eq!(c.pixel(4, 0), Some(style.color));
    }

    #[test]
    fn draw_shape_line_is_bresenham_horizontal() {
        let mut c = Canvas::new(4, 5);
        let color = Color::rgb(1, 2, 3);
        c.draw_shape(
            &Shape::line(Point::new(0, 2), Point::new(3, 2)),
            Fill::solid(color),
        );
        for x in 0..4 {
            assert_eq!(c.pixel(x, 2), Some(color), "at x={x}");
        }
        assert_eq!(c.pixel(0, 0), Some(Color::default()));
        assert_eq!(c.pixel(0, 4), Some(Color::default()));
    }

    #[test]
    fn draw_shape_rect_fills_solid() {
        let mut c = Canvas::new(4, 4);
        let color = Color::rgb(9, 9, 9);
        c.draw_shape(&Shape::rect(Point::new(1, 1), 2, 2), Fill::solid(color));
        for y in 1..3 {
            for x in 1..3 {
                assert_eq!(c.pixel(x, y), Some(color), "at ({x},{y})");
            }
        }
        assert_eq!(c.pixel(0, 0), Some(Color::default()));
        assert_eq!(c.pixel(3, 3), Some(Color::default()));
    }

    #[test]
    fn draw_shape_circle_fills_center_not_corners() {
        let mut c = Canvas::new(5, 5);
        let color = Color::rgb(4, 5, 6);
        c.draw_shape(&Shape::circle(Point::new(2, 2), 2), Fill::solid(color));
        assert_eq!(c.pixel(2, 2), Some(color)); // center always inside
        assert_eq!(c.pixel(0, 0), Some(Color::default())); // corner outside radius 2
    }

    #[test]
    fn draw_shape_ellipse_respects_independent_radii() {
        let mut c = Canvas::new(9, 5);
        let color = Color::rgb(7, 8, 9);
        // wide, short ellipse: rx=4, ry=1 centered at (4,2)
        c.draw_shape(&Shape::ellipse(Point::new(4, 2), 4, 1), Fill::solid(color));
        assert_eq!(c.pixel(4, 2), Some(color)); // center
        assert_eq!(c.pixel(0, 2), Some(color)); // far left edge, within rx
        assert_eq!(c.pixel(4, 0), Some(Color::default())); // above ry, outside
    }

    #[test]
    fn draw_shape_triangle_fills_interior_not_exterior() {
        let mut c = Canvas::new(6, 6);
        let color = Color::rgb(1, 1, 1);
        c.draw_shape(
            &Shape::triangle(Point::new(0, 0), Point::new(5, 0), Point::new(0, 5)),
            Fill::solid(color),
        );
        assert_eq!(c.pixel(1, 1), Some(color)); // well inside the right triangle
        assert_eq!(c.pixel(5, 5), Some(Color::default())); // outside (opposite corner)
    }

    #[test]
    fn draw_shape_path_strokes_open_segments_only() {
        let mut c = Canvas::new(4, 4);
        let color = Color::rgb(2, 2, 2);
        c.draw_shape(
            &Shape::path(
                vec![Point::new(0, 0), Point::new(3, 0), Point::new(3, 3)],
                false,
            ),
            Fill::solid(color),
        );
        assert_eq!(c.pixel(0, 0), Some(color));
        assert_eq!(c.pixel(3, 0), Some(color));
        assert_eq!(c.pixel(3, 3), Some(color));
        // open path: no segment connects (3,3) back to (0,0); (1,1) sits on where the
        // closing segment would pass, so it's the meaningful pixel to check stays blank
        assert_eq!(c.pixel(1, 1), Some(Color::default()));
    }

    #[test]
    fn draw_shape_path_closed_convex_fills_interior_not_exterior() {
        // A 6x6 square, convex.
        let mut c = Canvas::new(8, 8);
        let color = Color::rgb(3, 3, 3);
        c.draw_shape(
            &Shape::path(
                vec![
                    Point::new(1, 1),
                    Point::new(6, 1),
                    Point::new(6, 6),
                    Point::new(1, 6),
                ],
                true,
            ),
            Fill::solid(color),
        );
        assert_eq!(c.pixel(3, 3), Some(color)); // well inside the square
        assert_eq!(c.pixel(0, 0), Some(Color::default())); // outside
        assert_eq!(c.pixel(7, 7), Some(Color::default())); // outside
    }

    #[test]
    fn draw_shape_path_closed_concave_fills_interior_not_the_notch() {
        // An "L"/notch shape: concave at (3,3), a triangular notch cut into the
        // top-left corner. `closed` connects the last point (3,3) back to the first
        // (1,1) automatically -- no need to repeat (1,1) at the end of `points`.
        let mut c = Canvas::new(8, 8);
        let color = Color::rgb(4, 4, 4);
        c.draw_shape(
            &Shape::path(
                vec![
                    Point::new(1, 1),
                    Point::new(6, 1),
                    Point::new(6, 6),
                    Point::new(1, 6),
                    Point::new(1, 3),
                    Point::new(3, 3), // concave vertex, notch apex
                ],
                true,
            ),
            Fill::solid(color),
        );
        assert_eq!(c.pixel(4, 4), Some(color)); // inside the main body
        assert_eq!(c.pixel(0, 0), Some(Color::default())); // outside entirely
                                                           // Inside the carved-out notch near its corner: still outside the polygon.
        assert_eq!(c.pixel(1, 2), Some(Color::default()));
    }

    #[test]
    fn draw_shape_path_closed_self_intersecting_uses_even_odd_rule() {
        // A "bowtie": edges (1,1)-(6,6) and (6,1)-(1,6) cross near the shape's center,
        // forming a self-intersecting quadrilateral (two triangular lobes pinched at the
        // crossing point). Even-odd rule fills each lobe, with the two lobes merging
        // into one solid span only on the scanline through the pinch point.
        let mut c = Canvas::new(8, 8);
        let color = Color::rgb(5, 5, 5);
        c.draw_shape(
            &Shape::path(
                vec![
                    Point::new(1, 1),
                    Point::new(6, 6),
                    Point::new(6, 1),
                    Point::new(1, 6),
                ],
                true,
            ),
            Fill::solid(color),
        );
        assert_eq!(c.pixel(3, 3), Some(color)); // the pinch row: filled edge-to-edge
        assert_eq!(c.pixel(2, 2), Some(color)); // left lobe
        assert_eq!(c.pixel(5, 2), Some(color)); // right lobe
        assert_eq!(c.pixel(3, 1), Some(Color::default())); // gap between the two lobes
        assert_eq!(c.pixel(0, 0), Some(Color::default())); // outside entirely
    }

    #[test]
    fn draw_text_huge_scale_does_not_overflow_or_panic() {
        let mut c = Canvas::new(3, 3);
        let style = TextStyle {
            color: Color::rgb(1, 1, 1),
            scale: u32::MAX,
        };
        // Previously, computing `advance`/pixel coordinates in u32/i32 could overflow
        // and panic for a scale this large. This must complete without panicking; the
        // resulting glyph block is enormous and clipped to the canvas, so it fills it.
        c.draw_text("I", Point::new(0, 0), &style);
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(c.pixel(x, y), Some(style.color), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn draw_shape_large_ellipse_radius_does_not_overflow_or_panic() {
        let mut c = Canvas::new(4, 4);
        let color = Color::rgb(1, 1, 1);
        // rx * rx * ry * ry alone (60_000^4) already exceeds i64::MAX; previously this
        // overflowed (and panicked in debug) inside the cross-multiplied inclusion test.
        c.draw_shape(&Shape::circle(Point::new(2, 2), 60_000), Fill::solid(color));
        // The canvas is tiny relative to the radius, so it's entirely inside the circle.
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(c.pixel(x, y), Some(color), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn draw_shape_max_radius_ellipse_does_not_overflow_or_panic() {
        let mut c = Canvas::new(4, 4);
        let color = Color::rgb(5, 5, 5);
        // rx^2*ry^2 + ry^2*rx^2 at the bounding-box corners exceeds even u128::MAX when
        // rx == ry == u32::MAX; the f64-based inclusion test has no such limit.
        c.draw_shape(
            &Shape::circle(Point::new(2, 2), u32::MAX),
            Fill::solid(color),
        );
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(c.pixel(x, y), Some(color), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn draw_shape_line_extreme_endpoint_does_not_hang() {
        let mut c = Canvas::new(4, 1);
        let color = Color::rgb(6, 6, 6);
        // Without clipping to canvas bounds first, Bresenham would walk roughly
        // i32::MAX steps to reach the (invisible) far endpoint.
        c.draw_shape(
            &Shape::line(Point::new(0, 0), Point::new(i32::MAX, 0)),
            Fill::solid(color),
        );
        for x in 0..4 {
            assert_eq!(c.pixel(x, 0), Some(color), "at x={x}");
        }
    }

    #[test]
    fn draw_shape_triangle_extreme_coordinates_does_not_overflow_or_panic() {
        let mut c = Canvas::new(4, 4);
        let color = Color::rgb(2, 2, 2);
        // Cross-product terms in the old i64 edge_sign overflow for points this far apart;
        // this must complete without panicking.
        c.draw_shape(
            &Shape::triangle(
                Point::new(i32::MAX, i32::MAX),
                Point::new(i32::MIN, i32::MAX),
                Point::new(i32::MAX, i32::MIN),
            ),
            Fill::solid(color),
        );
        // All three vertices sit at the extremes of the i32 domain (not just one, as in an
        // earlier version of this test), so edge_sign's cross-product terms are evaluated
        // near their largest possible magnitude. The hypotenuse is the line x + y = -1
        // (i32::MIN + i32::MAX); this canvas's corner sits on the same side as (MAX, MAX),
        // comfortably inside.
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(c.pixel(x, y), Some(color), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn bitmap_new_and_pixel_roundtrip() {
        let b = Bitmap::new(
            2,
            2,
            vec![
                Color::rgb(1, 0, 0),
                Color::rgb(2, 0, 0),
                Color::rgb(3, 0, 0),
                Color::rgb(4, 0, 0),
            ],
        );
        assert_eq!(b.width(), 2);
        assert_eq!(b.height(), 2);
        assert_eq!(b.pixel(0, 0), Some(Color::rgb(1, 0, 0)));
        assert_eq!(b.pixel(1, 1), Some(Color::rgb(4, 0, 0)));
        assert_eq!(b.pixel(2, 0), None);
    }

    #[test]
    #[should_panic(expected = "must equal width*height")]
    fn bitmap_new_panics_on_mismatched_pixel_count() {
        Bitmap::new(2, 2, vec![Color::default(); 3]);
    }

    #[test]
    fn bitmap_solid_fills_every_pixel() {
        let b = Bitmap::solid(3, 2, Color::rgb(9, 9, 9));
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(b.pixel(x, y), Some(Color::rgb(9, 9, 9)));
            }
        }
    }

    #[test]
    fn sprite_move_to_updates_position() {
        let mut s = Sprite::new(Bitmap::solid(1, 1, Color::rgb(1, 1, 1)), Point::new(0, 0));
        assert_eq!(s.position(), Point::new(0, 0));
        s.move_to(Point::new(5, 7));
        assert_eq!(s.position(), Point::new(5, 7));
    }

    #[test]
    fn draw_sprite_opaque_pixels_overwrite_background() {
        let mut c = Canvas::new(4, 4);
        c.set_pixel(1, 1, Color::rgb(50, 50, 50)); // pre-existing background content
        let mut sprite = Sprite::new(Bitmap::solid(2, 2, Color::rgb(9, 9, 9)), Point::new(1, 1));
        c.draw_sprite(&mut sprite);
        for y in 1..3 {
            for x in 1..3 {
                assert_eq!(c.pixel(x, y), Some(Color::rgb(9, 9, 9)), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn draw_sprite_transparent_pixels_preserve_background() {
        let mut c = Canvas::new(3, 3);
        c.set_pixel(1, 1, Color::rgb(50, 50, 50)); // background under the transparent sprite pixel
        let bitmap = Bitmap::new(
            1,
            1,
            vec![Color::rgba(9, 9, 9, 0)], // fully transparent
        );
        let mut sprite = Sprite::new(bitmap, Point::new(1, 1));
        c.draw_sprite(&mut sprite);
        // The transparent sprite pixel must not have overwritten the background beneath it.
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(50, 50, 50)));
    }

    #[test]
    fn draw_sprite_clips_to_canvas_bounds_without_panic() {
        let mut c = Canvas::new(2, 2);
        // Sprite mostly off-canvas to the bottom-right; only its top-left pixel is visible.
        let mut sprite = Sprite::new(Bitmap::solid(4, 4, Color::rgb(1, 2, 3)), Point::new(1, 1));
        c.draw_sprite(&mut sprite);
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(1, 2, 3)));
        assert_eq!(c.pixel(0, 0), Some(Color::default()));
    }

    #[test]
    fn draw_sprite_negative_position_does_not_panic() {
        let mut c = Canvas::new(2, 2);
        // Sprite anchored off-canvas to the top-left; only its bottom-right pixel is visible.
        let mut sprite = Sprite::new(Bitmap::solid(2, 2, Color::rgb(4, 5, 6)), Point::new(-1, -1));
        c.draw_sprite(&mut sprite);
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(4, 5, 6)));
        assert_eq!(c.pixel(1, 1), Some(Color::default()));
    }

    #[test]
    fn draw_sprite_move_and_redraw_restores_old_footprint() {
        let mut c = Canvas::new(5, 1);
        c.set_pixel(0, 0, Color::rgb(50, 50, 50)); // pre-existing background at the sprite's start
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(0, 0));
        c.draw_sprite(&mut sprite);
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(9, 9, 9)));

        sprite.move_to(Point::new(4, 0));
        c.draw_sprite(&mut sprite);
        // Old position must be restored to what it was before the sprite was ever drawn
        // there — not left painted with the sprite's color.
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(50, 50, 50)));
        // New position now shows the sprite.
        assert_eq!(c.pixel(4, 0), Some(Color::rgb(9, 9, 9)));
    }

    #[test]
    fn draw_sprite_redraw_at_same_position_is_a_noop_change() {
        let mut c = Canvas::new(3, 1);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(7, 7, 7)), Point::new(1, 0));
        c.draw_sprite(&mut sprite);
        c.draw_sprite(&mut sprite); // redraw without moving
        assert_eq!(c.pixel(1, 0), Some(Color::rgb(7, 7, 7)));
    }

    #[test]
    fn draw_sprite_clone_has_no_drawing_history() {
        let mut c = Canvas::new(3, 1);
        let mut original = Sprite::new(Bitmap::solid(1, 1, Color::rgb(1, 1, 1)), Point::new(0, 0));
        c.draw_sprite(&mut original);

        // Cloning after drawing must not carry over last_draw -- otherwise drawing the
        // clone elsewhere would "restore" the original's footprint out from under it.
        let mut clone = original.clone();
        clone.move_to(Point::new(2, 0));
        c.draw_sprite(&mut clone);

        // The original sprite's pixel must be untouched by the clone's draw.
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(1, 1, 1)));
        assert_eq!(c.pixel(2, 0), Some(Color::rgb(1, 1, 1)));
    }

    #[test]
    fn draw_sprite_on_a_different_canvas_does_not_leak_the_first_canvas_pixels() {
        let mut canvas_a = Canvas::new(2, 1);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(0, 0));
        canvas_a.draw_sprite(&mut sprite); // captures canvas_a's background into last_draw

        let mut canvas_b = Canvas::new(2, 1);
        canvas_b.set_pixel(0, 0, Color::rgb(2, 2, 2)); // canvas_b's own distinct background
        sprite.move_to(Point::new(1, 0));
        canvas_b.draw_sprite(&mut sprite);

        // The stale footprint captured from canvas_a must not have been restored onto
        // canvas_b's position (0,0); canvas_b's own background must be untouched.
        assert_eq!(canvas_b.pixel(0, 0), Some(Color::rgb(2, 2, 2)));
        assert_eq!(canvas_b.pixel(1, 0), Some(Color::rgb(9, 9, 9)));
    }
}
