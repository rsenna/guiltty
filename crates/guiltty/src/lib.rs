//! Facade crate: re-exports the core API and the default (kitty) backend
//! for consumers of guiltty.

pub use guiltty_core::{Backend, Canvas, Color, Error, Point, Rect, TextStyle};
pub use guiltty_kitty::KittyBackend;
