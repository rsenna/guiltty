//! Movable sprites: a [`Bitmap`] positioned over a `guiltty-core` [`Canvas`] via
//! [`Sprite`], using save/restore-under so moving and redrawing a sprite doesn't leave a
//! trail of its previous position. Extracted out of `guiltty-core` to keep that crate
//! scoped to the absolute-coordinate drawing surface; see
//! `docs/design/sprite-crate-extraction.md`.

use guiltty_core::{Canvas, Color, Error, Point, Rect};

/// A small RGBA8 image used as sprite content. Structurally similar to `Canvas`'s pixel
/// buffer, but represents drawable material rather than a render target. Can be built
/// in-memory ([`Bitmap::new`]/[`Bitmap::solid`]) or loaded from a file on disk
/// ([`Bitmap::from_file`]).
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

    /// Loads an image file (PNG, JPEG, GIF, or BMP) from `path` and converts it to a
    /// bitmap. Every source pixel format (RGB, grayscale, indexed palette, etc.) is
    /// converted to this crate's RGBA8 color model: sources without an alpha channel get
    /// a fully-opaque (255) default alpha.
    ///
    /// Returns `Err(Error::ImageLoad(_))` rather than panicking for a missing file, an
    /// unsupported format, or malformed/corrupt image data -- per this crate's
    /// recoverable-error convention, a bad file on disk is exactly the kind of
    /// caller-facing condition that shouldn't panic.
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        let img = image::open(path.as_ref())
            .map_err(|e| Error::ImageLoad(format!("{}: {e}", path.as_ref().display())))?;
        let rgba = img.into_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba
            .pixels()
            .map(|p| Color::rgba(p[0], p[1], p[2], p[3]))
            .collect();
        Ok(Self {
            width,
            height,
            pixels,
        })
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

    /// Row-major pixel index for `(x, y)`, or `None` if out of bounds -- mirrors
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
/// [`Sprite::clear_footprint`] can restore them. Tagged with the id of the `Canvas` it
/// was captured from (never restored onto a different canvas instance) and the
/// `region_version` of its own `rect` at capture time (see "Footprint staleness" in
/// `docs/design/sprite-crate-extraction.md`).
#[derive(Debug)]
struct DrawnFootprint {
    canvas_id: u64,
    rect: Rect,
    version: u64,
    pixels: Vec<Color>,
}

/// Returned by [`Sprite::clear_footprint`] (and propagated by [`Sprite::draw_on`]) when
/// the canvas has changed, within this footprint's own region, since it was captured.
/// The canvas is left untouched -- no partial restore happens. Recover via
/// [`Sprite::discard_footprint`], which drops the stale footprint (abandoning the
/// sprite's old on-canvas pixels rather than risking restoring stale ones) so the sprite
/// can be placed again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaleFootprint;

impl std::fmt::Display for StaleFootprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "sprite footprint is stale: canvas changed since it was captured"
        )
    }
}

impl std::error::Error for StaleFootprint {}

/// A movable 2D bitmap positioned over a canvas. See [`Sprite::draw_on`] for how moving
/// and redrawing a sprite avoids leaving a trail of its previous position.
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

    /// Moves the sprite to a new position. Takes effect the next time [`Sprite::draw_on`]
    /// (or [`Sprite::clear_footprint`]/[`Sprite::place`]) is called -- that restores
    /// whatever the sprite covered at its previous position before drawing it at the new
    /// one.
    pub fn move_to(&mut self, position: Point) {
        self.position = position;
    }

    /// The sprite's bitmap content.
    pub fn bitmap(&self) -> &Bitmap {
        &self.bitmap
    }

    /// Draws this sprite's bitmap onto `canvas` at its current position, clipped to
    /// canvas bounds: `clear_footprint` followed by `place` (see both for what each
    /// step does). This is what most callers want -- anything that doesn't need to draw
    /// something else in between clearing the old footprint and placing the new one
    /// (`guiltty-turtle`'s trail-drawing is the exception; see that crate).
    ///
    /// Propagates [`StaleFootprint`] from `clear_footprint` without calling `place` --
    /// i.e. on error, the canvas is left completely untouched (no restore, no new draw).
    pub fn draw_on(&mut self, canvas: &mut Canvas) -> Result<(), StaleFootprint> {
        self.clear_footprint(canvas)?;
        self.place(canvas);
        Ok(())
    }

    /// Restores the canvas pixels this sprite's last [`Sprite::place`] call covered,
    /// clearing `last_draw` on success.
    ///
    /// - `Ok(())`, a no-op, if the sprite was never drawn, or if its last footprint was
    ///   captured from a *different* `Canvas` (irrelevant here -- dropped, not restored).
    /// - `Err(StaleFootprint)`, canvas left untouched, if this footprint's region has
    ///   changed (per `canvas.region_version`) since it was captured -- something else
    ///   wrote into it in the meantime, so restoring would overwrite that write with
    ///   stale pixels. See "Footprint staleness" in
    ///   `docs/design/sprite-crate-extraction.md`.
    /// - `Ok(())`, restored, otherwise.
    pub fn clear_footprint(&mut self, canvas: &mut Canvas) -> Result<(), StaleFootprint> {
        let Some(footprint) = self.last_draw.as_ref() else {
            return Ok(());
        };
        if footprint.canvas_id != canvas.id() {
            self.last_draw = None;
            return Ok(());
        }
        if canvas.region_version(footprint.rect) != footprint.version {
            return Err(StaleFootprint);
        }
        let footprint = self
            .last_draw
            .take()
            .expect("checked Some above, and canvas_id/region_version both matched");
        Self::restore_footprint(canvas, &footprint);
        Ok(())
    }

    /// Captures the canvas pixels currently under this sprite's position, then blits the
    /// sprite's bitmap over them (fully transparent bitmap pixels, `alpha == 0`, are
    /// skipped rather than overwriting the canvas; non-transparent pixels replace
    /// outright -- no alpha blending, no anti-aliasing). Does **not** restore any
    /// previous footprint first -- see [`Sprite::clear_footprint`] for that, or
    /// [`Sprite::draw_on`] for both together.
    ///
    /// The captured footprint's `region_version` is read *after* this blit completes,
    /// not before -- the blit is itself a canvas write to that same region, so capturing
    /// beforehand would make every sprite self-invalidate on its very next
    /// `clear_footprint` call.
    pub fn place(&mut self, canvas: &mut Canvas) {
        let (px, py) = (self.position.x as i64, self.position.y as i64);
        let bmp_w = self.bitmap.width as i64;
        let bmp_h = self.bitmap.height as i64;
        let canvas_w = canvas.width() as i64;
        let canvas_h = canvas.height() as i64;
        let x_lo = px.max(0);
        let x_hi = (px + bmp_w).min(canvas_w);
        let y_lo = py.max(0);
        let y_hi = (py + bmp_h).min(canvas_h);
        let cap_w = (x_hi - x_lo).max(0) as u32;
        let cap_h = (y_hi - y_lo).max(0) as u32;
        let mut saved = Vec::with_capacity(cap_w as usize * cap_h as usize);

        for y in y_lo..y_hi {
            for x in x_lo..x_hi {
                let (cx, cy) = (x as u32, y as u32);
                saved.push(canvas.pixel(cx, cy).unwrap_or_default());
                let (bx, by) = ((x - px) as u32, (y - py) as u32);
                if let Some(color) = self.bitmap.pixel(bx, by) {
                    if color.a != 0 {
                        canvas.set_pixel(cx, cy, color);
                    }
                }
            }
        }

        let rect = Rect::new(x_lo as i32, y_lo as i32, cap_w, cap_h);
        let version = canvas.region_version(rect); // after the blit above -- see doc comment
        self.last_draw = Some(DrawnFootprint {
            canvas_id: canvas.id(),
            rect,
            version,
            pixels: saved,
        });
    }

    /// Drops this sprite's saved footprint, if any, without attempting to restore it --
    /// the escape hatch out of a permanently-stale footprint (`Canvas`'s underlying
    /// version only increases, so a `clear_footprint` that once returned
    /// `Err(StaleFootprint)` never stops doing so on retry). The sprite's previous
    /// on-canvas pixels are abandoned as-is -- a visible artifact, not cleaned up -- but
    /// the sprite becomes drawable again via [`Sprite::place`]/[`Sprite::draw_on`].
    pub fn discard_footprint(&mut self) {
        self.last_draw = None;
    }

    /// Restores the canvas pixels a footprint covered, clipped to whatever part of it
    /// still falls within current canvas bounds. Goes through `Canvas`'s public
    /// `set_pixel` (no direct pixel-buffer access available from this crate).
    fn restore_footprint(canvas: &mut Canvas, footprint: &DrawnFootprint) {
        let canvas_w = canvas.width() as i64;
        let canvas_h = canvas.height() as i64;
        let row_len = footprint.rect.width as usize;
        for dy in 0..footprint.rect.height as i64 {
            let y = footprint.rect.y as i64 + dy;
            if y < 0 || y >= canvas_h {
                continue;
            }
            let row = dy as usize * row_len;
            for dx in 0..footprint.rect.width as i64 {
                let x = footprint.rect.x as i64 + dx;
                if x < 0 || x >= canvas_w {
                    continue;
                }
                canvas.set_pixel(x as u32, y as u32, footprint.pixels[row + dx as usize]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use guiltty_core::{Fill, Shape};

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
    fn bitmap_from_file_loads_rgba_png() {
        let b = Bitmap::from_file("tests/fixtures/rgba_2x2.png").expect("fixture should load");
        assert_eq!((b.width(), b.height()), (2, 2));
        assert_eq!(b.pixel(0, 0), Some(Color::rgba(255, 0, 0, 255)));
        assert_eq!(b.pixel(1, 0), Some(Color::rgba(0, 255, 0, 128)));
        assert_eq!(b.pixel(0, 1), Some(Color::rgba(0, 0, 255, 255)));
        assert_eq!(b.pixel(1, 1), Some(Color::rgba(255, 255, 0, 0)));
    }

    #[test]
    fn bitmap_from_file_converts_rgb_to_rgba8_with_opaque_default_alpha() {
        let b = Bitmap::from_file("tests/fixtures/rgb_2x2.png").expect("fixture should load");
        assert_eq!((b.width(), b.height()), (2, 2));
        assert_eq!(b.pixel(0, 0), Some(Color::rgba(10, 20, 30, 255)));
        assert_eq!(b.pixel(1, 0), Some(Color::rgba(40, 50, 60, 255)));
        assert_eq!(b.pixel(0, 1), Some(Color::rgba(70, 80, 90, 255)));
        assert_eq!(b.pixel(1, 1), Some(Color::rgba(100, 110, 120, 255)));
    }

    #[test]
    fn bitmap_from_file_converts_grayscale_to_rgba8() {
        let b = Bitmap::from_file("tests/fixtures/grayscale_2x2.png").expect("fixture should load");
        assert_eq!((b.width(), b.height()), (2, 2));
        assert_eq!(b.pixel(0, 0), Some(Color::rgba(0, 0, 0, 255)));
        assert_eq!(b.pixel(1, 0), Some(Color::rgba(85, 85, 85, 255)));
        assert_eq!(b.pixel(0, 1), Some(Color::rgba(170, 170, 170, 255)));
        assert_eq!(b.pixel(1, 1), Some(Color::rgba(255, 255, 255, 255)));
    }

    #[test]
    fn bitmap_from_file_missing_file_returns_err_not_panic() {
        let result = Bitmap::from_file("tests/fixtures/does_not_exist.png");
        assert!(matches!(result, Err(Error::ImageLoad(_))));
    }

    #[test]
    fn bitmap_from_file_malformed_image_returns_err_not_panic() {
        let result = Bitmap::from_file("tests/fixtures/malformed.png");
        assert!(matches!(result, Err(Error::ImageLoad(_))));
    }

    #[test]
    fn sprite_move_to_updates_position() {
        let mut s = Sprite::new(Bitmap::solid(1, 1, Color::rgb(1, 1, 1)), Point::new(0, 0));
        assert_eq!(s.position(), Point::new(0, 0));
        s.move_to(Point::new(5, 7));
        assert_eq!(s.position(), Point::new(5, 7));
    }

    #[test]
    fn draw_on_opaque_pixels_overwrite_background() {
        let mut c = Canvas::new(4, 4);
        c.set_pixel(1, 1, Color::rgb(50, 50, 50)); // pre-existing background content
        let mut sprite = Sprite::new(Bitmap::solid(2, 2, Color::rgb(9, 9, 9)), Point::new(1, 1));
        sprite.draw_on(&mut c).expect("first draw always succeeds");
        for y in 1..3 {
            for x in 1..3 {
                assert_eq!(c.pixel(x, y), Some(Color::rgb(9, 9, 9)), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn draw_on_transparent_pixels_preserve_background() {
        let mut c = Canvas::new(3, 3);
        c.set_pixel(1, 1, Color::rgb(50, 50, 50)); // background under the transparent sprite pixel
        let bitmap = Bitmap::new(
            1,
            1,
            vec![Color::rgba(9, 9, 9, 0)], // fully transparent
        );
        let mut sprite = Sprite::new(bitmap, Point::new(1, 1));
        sprite.draw_on(&mut c).expect("first draw always succeeds");
        // The transparent sprite pixel must not have overwritten the background beneath it.
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(50, 50, 50)));
    }

    #[test]
    fn draw_on_clips_to_canvas_bounds_without_panic() {
        let mut c = Canvas::new(2, 2);
        // Sprite mostly off-canvas to the bottom-right; only its top-left pixel is visible.
        let mut sprite = Sprite::new(Bitmap::solid(4, 4, Color::rgb(1, 2, 3)), Point::new(1, 1));
        sprite.draw_on(&mut c).expect("first draw always succeeds");
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(1, 2, 3)));
        assert_eq!(c.pixel(0, 0), Some(Color::default()));
    }

    #[test]
    fn draw_on_negative_position_does_not_panic() {
        let mut c = Canvas::new(2, 2);
        // Sprite anchored off-canvas to the top-left; only its bottom-right pixel is visible.
        let mut sprite = Sprite::new(Bitmap::solid(2, 2, Color::rgb(4, 5, 6)), Point::new(-1, -1));
        sprite.draw_on(&mut c).expect("first draw always succeeds");
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(4, 5, 6)));
        assert_eq!(c.pixel(1, 1), Some(Color::default()));
    }

    #[test]
    fn draw_on_move_and_redraw_restores_old_footprint() {
        let mut c = Canvas::new(5, 1);
        c.set_pixel(0, 0, Color::rgb(50, 50, 50)); // pre-existing background at the sprite's start
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(0, 0));
        sprite.draw_on(&mut c).expect("first draw always succeeds");
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(9, 9, 9)));

        sprite.move_to(Point::new(4, 0));
        sprite
            .draw_on(&mut c)
            .expect("nothing else wrote into the footprint in between");
        // Old position must be restored to what it was before the sprite was ever drawn
        // there -- not left painted with the sprite's color.
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(50, 50, 50)));
        // New position now shows the sprite.
        assert_eq!(c.pixel(4, 0), Some(Color::rgb(9, 9, 9)));
    }

    #[test]
    fn draw_on_redraw_at_same_position_is_a_noop_change() {
        let mut c = Canvas::new(3, 1);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(7, 7, 7)), Point::new(1, 0));
        sprite.draw_on(&mut c).expect("first draw always succeeds");
        sprite.draw_on(&mut c).expect("redraw without moving"); // redraw without moving
        assert_eq!(c.pixel(1, 0), Some(Color::rgb(7, 7, 7)));
    }

    #[test]
    fn draw_on_clone_has_no_drawing_history() {
        let mut c = Canvas::new(3, 1);
        let mut original = Sprite::new(Bitmap::solid(1, 1, Color::rgb(1, 1, 1)), Point::new(0, 0));
        original
            .draw_on(&mut c)
            .expect("first draw always succeeds");

        // Cloning after drawing must not carry over last_draw -- otherwise drawing the
        // clone elsewhere would "restore" the original's footprint out from under it.
        let mut clone = original.clone();
        clone.move_to(Point::new(2, 0));
        clone
            .draw_on(&mut c)
            .expect("clone starts with no footprint to restore");

        // The original sprite's pixel must be untouched by the clone's draw.
        assert_eq!(c.pixel(0, 0), Some(Color::rgb(1, 1, 1)));
        assert_eq!(c.pixel(2, 0), Some(Color::rgb(1, 1, 1)));
    }

    #[test]
    fn draw_on_wrong_canvas_is_a_noop_not_a_panic() {
        let mut canvas_a = Canvas::new(2, 1);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(0, 0));
        sprite
            .draw_on(&mut canvas_a)
            .expect("first draw always succeeds"); // captures canvas_a's background into last_draw

        let mut canvas_b = Canvas::new(2, 1);
        canvas_b.set_pixel(0, 0, Color::rgb(2, 2, 2)); // canvas_b's own distinct background
        sprite.move_to(Point::new(1, 0));
        sprite
            .draw_on(&mut canvas_b)
            .expect("a footprint from a different canvas is dropped, not an error");

        // The stale footprint captured from canvas_a must not have been restored onto
        // canvas_b's position (0,0); canvas_b's own background must be untouched.
        assert_eq!(canvas_b.pixel(0, 0), Some(Color::rgb(2, 2, 2)));
        assert_eq!(canvas_b.pixel(1, 0), Some(Color::rgb(9, 9, 9)));
    }

    #[test]
    fn clear_footprint_immediately_after_place_succeeds() {
        // Guards against stamping the footprint's version before place's own blit --
        // that would make the blit self-invalidate the footprint it just created.
        let mut c = Canvas::new(3, 3);
        c.set_pixel(1, 1, Color::rgb(50, 50, 50));
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(1, 1));
        sprite.place(&mut c);
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(9, 9, 9)));

        sprite
            .clear_footprint(&mut c)
            .expect("no intervening write since place");
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(50, 50, 50)));
    }

    #[test]
    fn clear_footprint_after_successful_restore_is_a_safe_noop() {
        let mut c = Canvas::new(3, 1);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(7, 7, 7)), Point::new(1, 0));
        sprite.place(&mut c);
        sprite
            .clear_footprint(&mut c)
            .expect("first clear restores successfully");
        // last_draw is now None: a second call has nothing to restore, so it's a safe
        // no-op -- not an error, and it must not touch the canvas again.
        sprite
            .clear_footprint(&mut c)
            .expect("clearing an already-cleared sprite is a no-op, not an error");
    }

    #[test]
    fn clear_footprint_returns_stale_after_intervening_write() {
        let mut c = Canvas::new(4, 4);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(1, 1));
        sprite.place(&mut c);

        // Something else writes into the same region before this sprite clears --
        // standing in for another sprite's trail crossing this one's footprint.
        c.set_pixel(1, 1, Color::rgb(3, 3, 3));

        let result = sprite.clear_footprint(&mut c);
        assert_eq!(result, Err(StaleFootprint));
        // The canvas must be left exactly as the intervening write left it -- no partial
        // restore of the stale background.
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(3, 3, 3)));
    }

    #[test]
    fn draw_on_propagates_stale_error_without_drawing() {
        let mut c = Canvas::new(4, 4);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(1, 1));
        sprite.place(&mut c);
        c.set_pixel(1, 1, Color::rgb(3, 3, 3)); // intervening write

        sprite.move_to(Point::new(2, 2));
        let result = sprite.draw_on(&mut c);
        assert_eq!(result, Err(StaleFootprint));
        // draw_on must not have called place() after clear_footprint failed: the new
        // position must show no sprite pixels, and the old position's intervening write
        // must be untouched.
        assert_eq!(c.pixel(1, 1), Some(Color::rgb(3, 3, 3)));
        assert_eq!(c.pixel(2, 2), Some(Color::default()));
    }

    #[test]
    fn disjoint_write_does_not_invalidate_footprint() {
        let mut c = Canvas::new(200, 200);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(1, 1));
        sprite.place(&mut c);

        // Write far away -- a different tile in the region-version grid -- standing in
        // for a second, independent sprite/turtle moving elsewhere on a shared canvas.
        c.set_pixel(190, 190, Color::rgb(3, 3, 3));

        sprite
            .clear_footprint(&mut c)
            .expect("a disjoint write must not invalidate this footprint");
    }

    #[test]
    fn discard_footprint_recovers_after_stale() {
        let mut c = Canvas::new(4, 4);
        let mut sprite = Sprite::new(Bitmap::solid(1, 1, Color::rgb(9, 9, 9)), Point::new(1, 1));
        sprite.place(&mut c);
        c.set_pixel(1, 1, Color::rgb(3, 3, 3)); // makes the footprint permanently stale

        assert_eq!(sprite.clear_footprint(&mut c), Err(StaleFootprint));
        sprite.discard_footprint();
        // The sprite is drawable again -- place() no longer has a footprint to consult.
        sprite.move_to(Point::new(2, 2));
        sprite.place(&mut c);
        assert_eq!(c.pixel(2, 2), Some(Color::rgb(9, 9, 9)));
    }

    #[test]
    fn draw_shape_touching_a_sprites_footprint_makes_it_stale() {
        // Not just set_pixel: any pixel-mutating Canvas call (draw_shape here) must
        // participate in the same region-version tracking.
        let mut c = Canvas::new(4, 4);
        let mut sprite = Sprite::new(Bitmap::solid(2, 2, Color::rgb(9, 9, 9)), Point::new(0, 0));
        sprite.place(&mut c);

        c.draw_shape(
            &Shape::line(Point::new(0, 0), Point::new(3, 0)),
            Fill::solid(Color::rgb(3, 3, 3)),
        );

        assert_eq!(sprite.clear_footprint(&mut c), Err(StaleFootprint));
    }
}
