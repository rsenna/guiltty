//! E2E harness: presents a `Canvas` to kitty and records whether kitty accepted
//! (`OK`) or rejected (error) the transmission.
//!
//! Usage: `kitty_e2e_harness <result-file>`
//!
//! Writes the kitty graphics protocol escape sequence to stdout (which, when this
//! binary is spawned inside a kitty window, is that window's PTY), reads kitty's
//! response from stdin, and writes `PASS` or `FAIL` to `<result-file>`.
//!
//! The result file path is passed in as an argument (never a fixed path) so concurrent
//! runs can't race each other or read a stale result from a previous run.
//!
//! This uses `build_transmit_action` + `write_transmit_to(Verbosity::All)` rather than
//! `Action::execute`: `execute` parses the response internally and would bypass the
//! unit-tested `interpret_response` classifier, and its blocking-read strategy is
//! exactly what we want to keep out of the harness so the interpretation logic stays a
//! pure, testable function of raw bytes.

use guiltty::{Canvas, Color, Fill, Point, Shape};
use guiltty_kitty::{build_transmit_action, interpret_response, Response};
use kittage::Verbosity;
use std::io::{self, Read, Write};
use std::num::NonZeroU32;

fn main() {
    let result_file = std::env::args()
        .nth(1)
        .expect("usage: kitty_e2e_harness <result-file>");

    // kitty's response is an APC sequence with no trailing newline, so a canonical-mode
    // (line-buffered) PTY would buffer it indefinitely and the read below would hang.
    // Put stdin in raw mode first so the response is delivered byte-by-byte as it arrives.
    // The guard restores the original terminal settings on drop (including on panic).
    let _tty = TtyGuard::enter_raw_mode();

    let result = run();
    std::fs::write(&result_file, result).expect("writing the result file should succeed");
}

/// Saves the original stdin termios, puts stdin into raw mode, and restores it on drop.
///
/// kitty and the PTY this harness runs in are Unix-only, so this is compiled out on
/// non-Unix targets (where the harness never runs against a real terminal anyway).
#[cfg(unix)]
struct TtyGuard {
    original: Option<libc::termios>,
}

#[cfg(unix)]
impl TtyGuard {
    fn enter_raw_mode() -> Self {
        // SAFETY: tcgetattr/tcsetattr operate on the process's own stdin fd; the termios
        // struct is zero-initialized before tcgetattr fills it, and cfmakeraw only clears
        // flags. No pointers escape this struct.
        unsafe {
            let mut original: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut original) != 0 {
                // stdin is not a TTY — nothing to save/raw, nothing to restore.
                return TtyGuard { original: None };
            }
            let mut raw = original;
            libc::cfmakeraw(&mut raw);
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw);
            TtyGuard {
                original: Some(original),
            }
        }
    }
}

#[cfg(unix)]
impl Drop for TtyGuard {
    fn drop(&mut self) {
        // SAFETY: restoring the previously-read termios to the process's own stdin fd.
        if let Some(original) = &self.original {
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, original);
            }
        }
    }
}

#[cfg(not(unix))]
struct TtyGuard;

#[cfg(not(unix))]
impl TtyGuard {
    fn enter_raw_mode() -> Self {
        // Non-Unix targets have no kitty/PTY to speak to, so raw mode is irrelevant.
        TtyGuard
    }
}

fn run() -> &'static str {
    // A small canvas with real pixel data (a filled rect) so the transmission carries a
    // non-trivial payload, not just an empty/transparent buffer.
    let mut canvas = Canvas::new(4, 4);
    canvas.draw_shape(
        &Shape::rect(Point::new(0, 0), 4, 4),
        Fill::solid(Color::rgb(255, 0, 0)),
    );

    // Use the process id as the image/placement id so this run does not overwrite an
    // image a fresh terminal may already hold under a fixed id; the harness runs in a
    // fresh kitty window, but a unique id costs nothing and avoids the collision class.
    let id = NonZeroU32::new(std::process::id()).expect("the process id is non-zero");
    let action = build_transmit_action(&canvas, id);

    // Verbosity::All (q=0) makes kitty respond with OK/error; Silent would suppress the
    // reply entirely and leave nothing to read.
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if action.write_transmit_to(&mut out, Verbosity::All).is_err() {
        return "FAIL";
    }
    if out.flush().is_err() {
        return "FAIL";
    }

    match interpret_response(&read_response()) {
        Response::Pass => "PASS",
        Response::Fail | Response::Ambiguous => "FAIL",
    }
}

/// Reads kitty's response from stdin (the window's PTY) until the APC terminator
/// (ESC backslash) or EOF, whichever comes first.
fn read_response() -> Vec<u8> {
    let mut raw = Vec::new();
    let mut stdin = io::stdin();
    let mut byte = [0u8; 1];
    loop {
        match stdin.read(&mut byte) {
            Ok(0) => break, // EOF
            Ok(_) => {
                raw.push(byte[0]);
                if raw.ends_with(b"\x1b\\") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    raw
}
