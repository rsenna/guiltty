//! Facade crate: re-exports the core API and the default (kitty) backend
//! for consumers of guiltty.

pub use guiltty_core::{Backend, Bitmap, Canvas, Color, Error, Fill, Point, Rect, Shape, Sprite, TextStyle};
pub use guiltty_kitty::KittyBackend;
