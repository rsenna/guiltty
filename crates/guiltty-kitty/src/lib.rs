//! Kitty graphics protocol backend: encodes and transmits canvas state as kitty
//! escape sequences, built on the [`kittage`] crate for protocol encoding rather than
//! hand-rolling escape-sequence construction (see the module's git history for the
//! original hand-rolled implementation and the reasoning behind switching).
//!
//! Requires a kitty-compatible terminal supporting protocol version 0.20.0 or later
//! (needed for the `C` "don't move cursor" control key used by [`KittyBackend::present`]).

use guiltty_core::{Backend, Canvas, Error};
use kittage::action::Action;
use kittage::display::{CursorMovementPolicy, DisplayConfig};
use kittage::image::Image;
use kittage::medium::{ChunkSize, Medium};
use kittage::{ImageDimensions, NumberOrId, PixelFormat, Verbosity};
use std::io::{self, Write};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};

/// Hands out a fresh, terminal-global-namespace-unique image id to every `KittyBackend`
/// instance, so independent backends don't collide by all claiming the same id (which
/// would let one backend's `present()` replace or delete another's image/placement).
static NEXT_IMAGE_ID: AtomicU32 = AtomicU32::new(1);

/// The kitty graphics protocol backend. Encodes a [`Canvas`]'s pixel buffer as a kitty
/// graphics protocol APC escape sequence (RGBA8, base64-encoded, chunked per the
/// protocol's 4096-byte-per-chunk limit) and writes it to `W`. Defaults to writing to
/// stdout; use [`KittyBackend::with_writer`] to inject any other `io::Write` (e.g. a
/// `Vec<u8>` in tests, so protocol tests don't need a real terminal).
///
/// Each instance owns a unique image id (see [`NEXT_IMAGE_ID`]) and reuses it, alongside
/// a fixed placement id, across every `present()` call, so repeated calls update the same
/// on-screen placement instead of accumulating new anonymous images.
pub struct KittyBackend<W: Write = io::Stdout> {
    writer: W,
    image_id: u32,
}

/// Manual, opaque `Debug` impl (rather than `#[derive(Debug)]`) so `KittyBackend<W>` stays
/// `Debug` regardless of whether `W` itself implements `Debug` -- callers relying on
/// `#[derive(Debug)]` elsewhere shouldn't be forced into a `W: Debug` bound just to log a
/// struct that never exposes `W`'s contents anyway.
impl<W: Write> std::fmt::Debug for KittyBackend<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KittyBackend").finish_non_exhaustive()
    }
}

impl KittyBackend<io::Stdout> {
    /// Creates a backend that writes to stdout.
    pub fn new() -> Self {
        Self {
            writer: io::stdout(),
            image_id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl Default for KittyBackend<io::Stdout> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Write> KittyBackend<W> {
    /// Creates a backend that writes escape sequences to `writer` instead of stdout.
    pub fn with_writer(writer: W) -> Self {
        Self {
            writer,
            image_id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl<W: Write> Backend for KittyBackend<W> {
    type Error = Error;

    fn present(&mut self, canvas: &Canvas) -> Result<(), Error> {
        let to_backend_err = |e: std::io::Error| Error::Backend(e.to_string());

        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            // A zero-width/height canvas has no valid kitty raw-image representation --
            // there's nothing to present, so no-op.
            return Ok(());
        }

        let rgba = canvas.rgba8_bytes();
        // NOTE: kittage's own ImageDimensions doc comment says width/height are "divided
        // by 4 (for some reason)" -- verified empirically (see PR discussion) that for
        // this Direct-medium transmit path, raw pixel width/height produce the correct
        // s=/v= control-key values (no /4 scaling applied) -- confirmed by inspecting
        // actual encoder output, not by real-terminal rendering (still unverified end to
        // end). If real-terminal testing ever shows a 4x size/position mismatch, this is
        // the first place to look.
        let dims = ImageDimensions {
            width: w,
            height: h,
        };

        let id = NonZeroU32::new(self.image_id).expect("image_id is never zero");

        let image = Image {
            num_or_id: NumberOrId::Id(id),
            format: PixelFormat::Rgba32(dims, None),
            medium: Medium::Direct {
                // Max allowed chunk size (1024 * 4-byte units = 4096 base64 bytes per
                // escape code, the protocol's limit) -- kittage does NOT chunk
                // automatically when this is None; it silently emits one oversized
                // escape sequence instead (verified: a 100x100 canvas produced one
                // ~53KB escape code with no chunk_size set). This must be set
                // explicitly.
                chunk_size: ChunkSize::new(std::num::NonZeroU16::new(1024).unwrap()),
                data: std::borrow::Cow::Borrowed(&rgba),
            },
        };

        let config = DisplayConfig {
            cursor_movement: CursorMovementPolicy::DontMove,
            ..Default::default()
        };

        let action = Action::TransmitAndDisplay {
            image,
            config,
            // Explicit placement id (same numeric value as the image id -- placement
            // ids are namespaced per-image, so no cross-instance collision risk) so
            // repeated present() calls target the same on-screen placement instead of
            // relying on TransmitAndDisplay's implicit default placement.
            placement_id: Some(id),
        };

        action
            .write_transmit_to(&mut self.writer, Verbosity::Silent)
            .map_err(to_backend_err)?;
        self.writer
            .flush()
            .map_err(|e| Error::Backend(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // kittage's own base64 encoding (base64-simd) emits unpadded output (no trailing
    // "=" characters) -- NO_PAD, not STANDARD, or decoding fails with "Invalid padding".
    use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64;
    use base64::Engine as _;

    fn present_to_buf(canvas: &Canvas) -> String {
        let mut buf = Vec::new();
        let mut backend = KittyBackend::with_writer(&mut buf);
        backend.present(canvas).expect("present should succeed");
        let out = String::from_utf8(buf).expect("output should be valid UTF-8");
        // Known kittage 0.4.0 quirk (confirmed via byte-level inspection, reported
        // upstream): write_transmit_to writes the ESC-backslash string terminator
        // TWICE at the very end of the final (m=0) chunk -- single-chunk output and
        // the last chunk of multi-chunk output alike. Believed harmless in practice
        // (an extra empty escape sequence with nothing preceding it), but
        // real-terminal behavior is unverified. Strip the duplicate here so tests
        // assert against the single well-formed terminator our test parsing logic
        // expects; NOT stripped in the actual present() implementation, since
        // "harmless duplicate" is an assumption, not a verified fact -- silently
        // rewriting kittage's output ourselves would just trade one unverified
        // assumption for another.
        assert!(
            out.ends_with("\x1b\\\x1b\\"),
            "expected the known kittage double-terminator quirk; if this fails, \
             kittage may have fixed it upstream -- remove this workaround and \
             re-check present()'s own behavior is still correct"
        );
        out[..out.len() - "\x1b\\".len()].to_string()
    }

    #[test]
    fn present_encodes_control_keys_and_payload() {
        let canvas = Canvas::new(2, 1);
        let out = present_to_buf(&canvas);

        assert!(out.starts_with("\x1b_Ga=T,i="));
        // Control keys kittage includes that our own hand-rolled version didn't:
        // p= (explicit placement id, alongside i=) and t=d (transmission medium,
        // "direct" -- kitty defaults to this when omitted, but kittage is explicit).
        assert!(out.contains(",q=2,C=1,f=32,s=2,v=1,t=d,m=0;"));
        assert!(out.ends_with("\x1b\\"));

        let payload_start = out.find(';').unwrap() + 1;
        let payload_end = out.len() - "\x1b\\".len();
        let payload = &out[payload_start..payload_end];
        let decoded = BASE64
            .decode(payload)
            .expect("payload should be valid base64");
        assert_eq!(decoded, canvas.rgba8_bytes());
    }

    #[test]
    fn present_includes_matching_image_and_placement_ids() {
        let canvas = Canvas::new(1, 1);
        let out = present_to_buf(&canvas);
        let i_pos = out.find("i=").expect("i= present") + 2;
        let i_end = out[i_pos..].find(',').unwrap() + i_pos;
        let image_id = &out[i_pos..i_end];

        assert!(
            out.contains(&format!("p={image_id},")),
            "expected placement id to match image id {image_id} in {out}"
        );
    }

    #[test]
    fn present_single_chunk_has_m0() {
        let canvas = Canvas::new(1, 1);
        let out = present_to_buf(&canvas);
        assert!(out.contains(",m=0;"));
        // Exactly one APC sequence: two escape terminators total (one per "\x1b\\").
        assert_eq!(out.matches("\x1b_G").count(), 1);
    }

    #[test]
    fn present_chunks_large_payload_with_m1_then_m0() {
        // Big enough that base64(RGBA8 bytes) exceeds MAX_CHUNK_LEN, forcing >1 chunk.
        // 4 bytes/pixel * 4/3 base64 expansion > 4096 needs width*height > ~2350 px.
        let canvas = Canvas::new(100, 100);
        let out = present_to_buf(&canvas);

        let chunk_count = out.matches("\x1b_G").count();
        assert!(
            chunk_count > 1,
            "expected multiple chunks, got {chunk_count}"
        );

        // All chunks but the last carry m=1; the last carries m=0.
        let m1_count = out.matches("m=1;").count();
        let m0_count = out.matches("m=0;").count();
        assert_eq!(m1_count, chunk_count - 1);
        assert_eq!(m0_count, 1);

        // Reassemble the base64 payload across all chunks and confirm it round-trips.
        let mut payload = String::new();
        for part in out.split("\x1b_G").skip(1) {
            let body = part.strip_suffix("\x1b\\").expect("chunk ends with ST");
            let semi = body.find(';').unwrap();
            payload.push_str(&body[semi + 1..]);
        }
        let decoded = BASE64
            .decode(&payload)
            .expect("reassembled payload should be valid base64");
        assert_eq!(decoded, canvas.rgba8_bytes());
    }

    #[test]
    fn present_zero_size_canvas_is_a_noop() {
        // Not routed through present_to_buf(), which asserts the known
        // double-terminator quirk -- a zero-size canvas short-circuits before ever
        // calling kittage, so there's no output (and no terminator) at all.
        let canvas = Canvas::new(0, 0);
        let mut buf = Vec::new();
        let mut backend = KittyBackend::with_writer(&mut buf);
        backend.present(&canvas).expect("present should succeed");
        assert!(
            buf.is_empty(),
            "a zero-size canvas has no valid kitty image representation"
        );
    }

    #[test]
    fn present_reuses_the_same_image_id_across_calls() {
        let canvas = Canvas::new(1, 1);
        let mut buf = Vec::new();
        let mut backend = KittyBackend::with_writer(&mut buf);
        backend
            .present(&canvas)
            .expect("first present should succeed");
        backend
            .present(&canvas)
            .expect("second present should succeed");
        let out = String::from_utf8(buf).expect("output should be valid UTF-8");

        let i_pos = out.find("i=").expect("i= present") + 2;
        let i_end = out[i_pos..].find(',').unwrap() + i_pos;
        let image_id = &out[i_pos..i_end];

        // Both frames use the same image id so the terminal replaces the prior frame
        // instead of accumulating a new anonymous image each call.
        assert_eq!(out.matches(&format!("i={image_id},")).count(), 2);
    }

    #[test]
    fn distinct_backend_instances_get_distinct_image_ids() {
        let canvas = Canvas::new(1, 1);
        let mut buf_a = Vec::new();
        let mut backend_a = KittyBackend::with_writer(&mut buf_a);
        backend_a.present(&canvas).expect("present should succeed");
        let out_a = String::from_utf8(buf_a).unwrap();

        let mut buf_b = Vec::new();
        let mut backend_b = KittyBackend::with_writer(&mut buf_b);
        backend_b.present(&canvas).expect("present should succeed");
        let out_b = String::from_utf8(buf_b).unwrap();

        let extract_id = |out: &str| {
            let i_pos = out.find("i=").unwrap() + 2;
            let i_end = out[i_pos..].find(',').unwrap() + i_pos;
            out[i_pos..i_end].to_string()
        };
        assert_ne!(extract_id(&out_a), extract_id(&out_b));
    }
}
