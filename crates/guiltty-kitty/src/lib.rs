//! Kitty graphics protocol backend: encodes and transmits canvas state as
//! raw kitty escape sequences. No C FFI, no dependency on `kittage`/`little-kitty`.
//!
//! Requires a kitty-compatible terminal supporting protocol version 0.20.0 or later
//! (needed for the `C` "don't move cursor" control key used by [`KittyBackend::present`]).

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use guiltty_core::{Backend, Canvas, Error};
use std::io::{self, Write};
use std::sync::atomic::{AtomicU32, Ordering};

/// Maximum base64-encoded bytes per APC chunk, per the kitty graphics protocol spec.
const MAX_CHUNK_LEN: usize = 4096;

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
        let to_backend_err = |e: io::Error| Error::Backend(e.to_string());

        let rgba = canvas.rgba8_bytes();
        let encoded = BASE64.encode(&rgba);
        let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(MAX_CHUNK_LEN).collect();

        // A zero-width/height canvas has no valid kitty raw-image representation
        // (s=0,v=0 is not a well-formed image) -- there's nothing to present, so no-op.
        if chunks.is_empty() {
            return Ok(());
        }

        let last_index = chunks.len() - 1;
        for (i, chunk) in chunks.iter().enumerate() {
            let more = u8::from(i != last_index);
            if i == 0 {
                // i=<image_id>: this instance's unique image id (see NEXT_IMAGE_ID) --
                // repeated present() calls target the same image instead of colliding
                // with other backend instances' images.
                // p=<image_id>: a fixed placement id (reusing the image id's numeric
                // value; placement ids are namespaced per-image, so no cross-instance
                // collision risk). Pinning an explicit placement id, rather than relying
                // on a=T's implicit default placement, is what keeps repeated present()
                // calls updating the SAME on-screen placement -- NOTE: this has not been
                // verified against a real kitty terminal (none available in this
                // environment); if repeated present() calls are found to blank the
                // image in practice, revisit with an explicit a=t (transmit-only) +
                // a=p (placement) split instead of combined a=T.
                // q=2: suppress the terminal's OK/error APC responses, so callers that
                // read stdin afterward don't see them interleaved with real input.
                // C=1: don't move the cursor after displaying, so repeated present()
                // calls redraw at the same canvas origin instead of drifting down.
                // Requires kitty >= 0.20.0 (see module docs).
                write!(
                    self.writer,
                    "\x1b_Ga=T,f=32,i={},p={},q=2,C=1,s={},v={},m={};",
                    self.image_id,
                    self.image_id,
                    canvas.width(),
                    canvas.height(),
                    more
                )
                .map_err(to_backend_err)?;
            } else {
                write!(self.writer, "\x1b_Gm={};", more).map_err(to_backend_err)?;
            }
            // Written as raw bytes rather than through a `str`/`write!` conversion --
            // base64 output is always ASCII, but this avoids an unnecessary UTF-8
            // validation (and its associated `expect`/panic risk) on the hot path.
            self.writer.write_all(chunk).map_err(to_backend_err)?;
            self.writer.write_all(b"\x1b\\").map_err(to_backend_err)?;
        }
        self.writer.flush().map_err(to_backend_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn present_to_buf(canvas: &Canvas) -> String {
        let mut buf = Vec::new();
        let mut backend = KittyBackend::with_writer(&mut buf);
        backend.present(canvas).expect("present should succeed");
        String::from_utf8(buf).expect("output should be valid UTF-8")
    }

    #[test]
    fn present_encodes_control_keys_and_payload() {
        let canvas = Canvas::new(2, 1);
        let out = present_to_buf(&canvas);

        assert!(out.starts_with("\x1b_Ga=T,f=32,i="));
        assert!(out.contains(",q=2,C=1,s=2,v=1,m=0;"));
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
        let canvas = Canvas::new(0, 0);
        let out = present_to_buf(&canvas);
        assert_eq!(
            out, "",
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
