//! Facade crate: re-exports the core API and the default (kitty) backend
//! for consumers of guiltty.

pub use guiltty_core::{Backend, Canvas, Color, Error, Fill, Point, Rect, Shape, TextStyle};
pub use guiltty_kitty::KittyBackend;
pub use guiltty_sprite::{Bitmap, Sprite, StaleFootprint};
