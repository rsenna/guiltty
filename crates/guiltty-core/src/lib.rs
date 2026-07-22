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
}
