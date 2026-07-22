//! Backend-agnostic core primitives (`Color`, `Point`, `Rect`) and the [`Backend`] trait that
//! rendering backends (e.g. `guiltty-kitty`) implement. Canvas, shape, sprite, region, and
//! zoom/scroll logic described in the spec is not implemented yet — this is the scaffold only.

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

    /// Presents the current frame to the terminal. Backends define their own
    /// frame/state representation in later iterations of this trait.
    fn present(&mut self) -> Result<(), Self::Error>;
}

/// A pixel buffer that can be drawn into. RGBA8, origin top-left, row-major.
#[derive(Debug, Clone)]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

impl Canvas {
    /// Creates a canvas of the given size, every pixel starting fully transparent
    /// (`Color::default()`, alpha = 0).
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![Color::default(); (width * height) as usize],
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

    /// Returns the color at `(x, y)`, or `None` if out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.pixels.get((y * self.width + x) as usize).copied()
    }

    /// Sets the color at `(x, y)`. Silently ignores out-of-bounds coordinates — there's
    /// nothing a caller needs recover from, so this isn't a `Result`.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.pixels[(y * self.width + x) as usize] = color;
    }

    /// Draws `text` starting at `origin` (top-left of the first glyph) using `style`.
    ///
    /// v0's built-in font covers only space, digits, and uppercase `A`-`Z` (see the
    /// `font` module) — unsupported characters (lowercase, punctuation, non-ASCII) are
    /// skipped, leaving a blank glyph-width gap so surrounding text stays aligned.
    pub fn draw_text(&mut self, text: &str, origin: Point, style: &TextStyle) {
        let scale = style.scale.max(1);
        let advance = ((font::GLYPH_WIDTH + 1) * scale) as i32;
        let mut cursor_x = origin.x;
        for ch in text.chars() {
            if let Some(rows) = font::glyph(ch) {
                for (row_idx, row) in rows.iter().enumerate() {
                    for (col_idx, pixel) in row.chars().enumerate() {
                        if pixel != '#' {
                            continue;
                        }
                        let px0 = cursor_x + (col_idx as u32 * scale) as i32;
                        let py0 = origin.y + (row_idx as u32 * scale) as i32;
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let px = px0 + dx as i32;
                                let py = py0 + dy as i32;
                                if px >= 0 && py >= 0 {
                                    self.set_pixel(px as u32, py as u32, style.color);
                                }
                            }
                        }
                    }
                }
            }
            cursor_x += advance;
        }
    }
}

/// Styling for [`Canvas::draw_text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    pub color: Color,
    /// Integer pixel-scale factor for each font pixel (1 = native 3x5 glyph size).
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
}
