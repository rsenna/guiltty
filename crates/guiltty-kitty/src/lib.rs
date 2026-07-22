//! Kitty graphics protocol backend: encodes and transmits canvas state as
//! raw kitty escape sequences. No C FFI, no dependency on `kittage`/`little-kitty`.

use guiltty_core::{Backend, Error};

/// The kitty graphics protocol backend. Encoding/transmission logic lands
/// here in later iterations; this is the scaffold only.
#[derive(Debug, Default)]
pub struct KittyBackend;

impl Backend for KittyBackend {
    fn present(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_succeeds_on_scaffold_backend() {
        let mut backend = KittyBackend;
        assert!(backend.present().is_ok());
    }
}
