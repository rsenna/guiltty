//! Backend-agnostic core primitives (`Color`, `Point`, `Rect`) and the [`Backend`] trait that
//! rendering backends (e.g. `guiltty-kitty`) implement. `Canvas` supports shape drawing and
//! text; movable sprites live in the separate `guiltty-sprite` crate, built entirely on this
//! one's public API. Region/zoom/scroll logic described in the spec is not implemented yet.

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
    /// Failed to load an image file (missing file, unsupported/malformed format, etc.)
    /// via [`Bitmap::from_file`].
    ImageLoad(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Backend(msg) => write!(f, "backend error: {msg}"),
            Error::ImageLoad(msg) => write!(f, "image load error: {msg}"),
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

/// Side length, in pixels, of one tile in `Canvas`'s region-version grid (see
/// `Canvas::region_version`). An implementation detail, not public API.
const TILE_SIZE: u32 = 32;

/// A pixel buffer that can be drawn into. RGBA8, origin top-left, row-major.
#[derive(Debug)]
pub struct Canvas {
    /// Distinguishes this canvas from every other one, including clones of itself, so a
    /// sprite's saved-under footprint (see the `guiltty-sprite` crate) is never restored
    /// onto the wrong canvas.
    id: u64,
    width: u32,
    height: u32,
    pixels: Vec<Color>,
    /// Tile grid backing `region_version`: one counter per `TILE_SIZE`x`TILE_SIZE` tile,
    /// stamped with `next_version` whenever a pixel-mutating call touches that tile.
    tiles_x: u32,
    tiles_y: u32,
    tile_versions: Vec<u64>,
    /// Bumped once per pixel-mutating call, then stamped onto every tile that call's
    /// bounding region overlaps. Never reset except by `Canvas::new`/`Canvas::clone`, so
    /// `region_version` results are only ever comparable within one `Canvas` instance's
    /// lifetime (matching `id`'s per-instance scoping).
    next_version: u64,
}

/// Manually implemented (rather than `#[derive(Clone)]`) so a cloned canvas gets its own
/// fresh `id` instead of inheriting the original's — see the `id` field's doc comment. The
/// version-tracking fields reset fresh too: a clone's `id` won't match any footprint
/// captured from the original, so the original's version history has nothing left to
/// stay comparable with.
impl Clone for Canvas {
    fn clone(&self) -> Self {
        Self {
            id: NEXT_CANVAS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            width: self.width,
            height: self.height,
            pixels: self.pixels.clone(),
            tiles_x: self.tiles_x,
            tiles_y: self.tiles_y,
            tile_versions: vec![0; self.tile_versions.len()],
            next_version: 0,
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
        let tiles_x = width.div_ceil(TILE_SIZE).max(1);
        let tiles_y = height.div_ceil(TILE_SIZE).max(1);
        Self {
            id: NEXT_CANVAS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            width,
            height,
            pixels: vec![Color::default(); len],
            tiles_x,
            tiles_y,
            tile_versions: vec![0; (tiles_x as usize) * (tiles_y as usize)],
            next_version: 0,
        }
    }

    /// Uniquely identifies this `Canvas` instance (including across clones -- see
    /// [`Canvas::clone`]). Used by `guiltty-sprite` to guard against restoring a
    /// sprite's saved footprint onto the wrong canvas.
    pub fn id(&self) -> u64 {
        self.id
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
            self.touch_region(Rect::new(x as i32, y as i32, 1, 1));
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

    /// Returns the most recent write-version among the tiles `region` overlaps -- i.e.
    /// "the most recent pixel-mutating call that could have touched any pixel in here."
    /// `0` if `region` touches no tile that's ever been written to (including a region
    /// entirely off-canvas). Used by `guiltty-sprite` to detect a stale footprint: a
    /// captured `region_version` that no longer matches a fresh call means something
    /// wrote into that region since capture. See `docs/design/sprite-crate-extraction.md`'s
    /// "Footprint staleness".
    pub fn region_version(&self, region: Rect) -> u64 {
        let Some((tx_lo, tx_hi, ty_lo, ty_hi)) = self.tile_range(region) else {
            return 0;
        };
        let mut max_version = 0u64;
        for ty in ty_lo..=ty_hi {
            for tx in tx_lo..=tx_hi {
                max_version =
                    max_version.max(self.tile_versions[(ty * self.tiles_x + tx) as usize]);
            }
        }
        max_version
    }

    /// Bumps every tile `region` overlaps to a fresh version. Called from every
    /// pixel-mutating method (`set_pixel`, `draw_shape`, `draw_text`) with that call's own
    /// bounding region, so `region_version` can later tell whether anything wrote into a
    /// given area since some earlier point in time. A no-op if `region` is entirely
    /// off-canvas.
    fn touch_region(&mut self, region: Rect) {
        let Some((tx_lo, tx_hi, ty_lo, ty_hi)) = self.tile_range(region) else {
            return;
        };
        self.next_version += 1;
        let version = self.next_version;
        for ty in ty_lo..=ty_hi {
            for tx in tx_lo..=tx_hi {
                self.tile_versions[(ty * self.tiles_x + tx) as usize] = version;
            }
        }
    }

    /// Clips `region` to this canvas's bounds (in `i64`, mirroring the rest of this
    /// file's overflow-safe clipping idiom) and converts the result to an inclusive tile
    /// index range `(tx_lo, tx_hi, ty_lo, ty_hi)`. `None` if the clipped region is empty
    /// (entirely off-canvas, or zero-sized).
    fn tile_range(&self, region: Rect) -> Option<(u32, u32, u32, u32)> {
        let canvas_w = self.width as i64;
        let canvas_h = self.height as i64;
        let x_lo = (region.x as i64).max(0);
        let x_hi = (region.x as i64 + region.width as i64).min(canvas_w);
        let y_lo = (region.y as i64).max(0);
        let y_hi = (region.y as i64 + region.height as i64).min(canvas_h);
        if x_hi <= x_lo || y_hi <= y_lo {
            return None;
        }
        Some((
            (x_lo as u32) / TILE_SIZE,
            ((x_hi - 1) as u32) / TILE_SIZE,
            (y_lo as u32) / TILE_SIZE,
            ((y_hi - 1) as u32) / TILE_SIZE,
        ))
    }

    /// Bounding region of `shape` in canvas coordinate space, for `touch_region` only --
    /// not used for rendering, so it doesn't need pixel-perfect precision, just to
    /// contain every pixel the shape's own drawing logic below could touch. Computed in
    /// `i64` so the extreme coordinates/radii this crate already guards against
    /// elsewhere (see the `draw_shape_*_does_not_overflow_or_panic` tests) can't overflow
    /// here either.
    fn shape_bbox(shape: &Shape) -> Rect {
        let (x_lo, y_lo, x_hi, y_hi) = match shape {
            Shape::Line { from, to } => (
                (from.x as i64).min(to.x as i64),
                (from.y as i64).min(to.y as i64),
                (from.x as i64).max(to.x as i64) + 1,
                (from.y as i64).max(to.y as i64) + 1,
            ),
            Shape::Rect {
                origin,
                width,
                height,
            } => (
                origin.x as i64,
                origin.y as i64,
                origin.x as i64 + *width as i64,
                origin.y as i64 + *height as i64,
            ),
            Shape::Circle { center, radius } => Self::radial_bbox(*center, *radius, *radius),
            Shape::Ellipse { center, rx, ry } => Self::radial_bbox(*center, *rx, *ry),
            Shape::Triangle { a, b, c } => {
                let xs = [a.x as i64, b.x as i64, c.x as i64];
                let ys = [a.y as i64, b.y as i64, c.y as i64];
                (
                    xs.into_iter().min().unwrap(),
                    ys.into_iter().min().unwrap(),
                    xs.into_iter().max().unwrap() + 1,
                    ys.into_iter().max().unwrap() + 1,
                )
            }
            Shape::Path { points, .. } => {
                if points.is_empty() {
                    (0, 0, 0, 0)
                } else {
                    (
                        points.iter().map(|p| p.x as i64).min().unwrap(),
                        points.iter().map(|p| p.y as i64).min().unwrap(),
                        points.iter().map(|p| p.x as i64).max().unwrap() + 1,
                        points.iter().map(|p| p.y as i64).max().unwrap() + 1,
                    )
                }
            }
        };
        Self::rect_from_i64_bounds(x_lo, y_lo, x_hi, y_hi)
    }

    /// Bounding region of a circle/ellipse (`center` +/- `rx`/`ry`), for `shape_bbox`.
    fn radial_bbox(center: Point, rx: u32, ry: u32) -> (i64, i64, i64, i64) {
        (
            center.x as i64 - rx as i64,
            center.y as i64 - ry as i64,
            center.x as i64 + rx as i64 + 1,
            center.y as i64 + ry as i64 + 1,
        )
    }

    /// Builds a `Rect` from `i64` bounds, clamping to `i32`/`u32`-representable ranges
    /// rather than propagating extreme values into `Rect`'s fields. Only used as an
    /// approximate `touch_region` input, never for rendering, so this clamping can't
    /// affect what's actually drawn -- only (conservatively) which tiles get marked
    /// touched.
    fn rect_from_i64_bounds(x_lo: i64, y_lo: i64, x_hi: i64, y_hi: i64) -> Rect {
        let x = x_lo.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let y = y_lo.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let width = (x_hi - x_lo).clamp(0, u32::MAX as i64) as u32;
        let height = (y_hi - y_lo).clamp(0, u32::MAX as i64) as u32;
        Rect::new(x, y, width, height)
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

        let text_width = advance * text.chars().count() as i64;
        let text_height = font::GLYPH_HEIGHT as i64 * scale;
        self.touch_region(Self::rect_from_i64_bounds(
            origin.x as i64,
            origin_y,
            origin.x as i64 + text_width,
            origin_y + text_height,
        ));

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
    /// Every glyph is 5 rows tall (see the glyph table below).
    pub const GLYPH_HEIGHT: u32 = 5;

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
        self.touch_region(Self::shape_bbox(shape));
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

        let mut crossings: Vec<f64> = Vec::new();
        for y in min_y..=max_y {
            let yf = y as f64 + 0.5;
            crossings.clear();
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
            // A closed polygon always crosses any scanline through its interior an even
            // number of times; an odd count would mean degenerate input (e.g. a
            // duplicated point) produced a malformed edge list. `chunks_exact(2)` below
            // would silently drop a trailing unpaired crossing, so assert the invariant
            // explicitly in debug builds rather than let that happen unnoticed.
            debug_assert!(
                crossings.len().is_multiple_of(2),
                "polygon scanline crossing count must be even, got {}",
                crossings.len()
            );
            crossings.sort_by(|a, b| a.total_cmp(b));

            for pair in crossings.chunks_exact(2) {
                // Consistent pixel-center sampling with `yf` above: column x is "inside"
                // iff its center (x + 0.5) falls within [pair[0], pair[1]) -- i.e.
                // x_lo/x_hi are the smallest integers satisfying x + 0.5 >= pair[0] and
                // x + 0.5 >= pair[1], respectively. Using raw ceil/floor on the crossing
                // coordinates themselves (without this offset) would fill an extra
                // column whenever a crossing lands on an exact integer, asymmetric with
                // how `yf`'s +0.5 offset already treats row boundaries.
                let x_lo = ((pair[0] - 0.5).ceil() as i64).max(0);
                let x_hi = ((pair[1] - 0.5).ceil() as i64).min(canvas_w);
                for x in x_lo..x_hi {
                    self.set_pixel(x as u32, y as u32, color);
                }
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
        assert_eq!(c.pixel(1, 2), Some(color)); // left lobe
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
}
