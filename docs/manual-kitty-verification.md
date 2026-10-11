# Manual kitty verification

How to verify, by eye, that `KittyBackend::present()` actually renders in a real
kitty terminal. This is the manual, non-virtualized counterpart to the automated
Xvfb-backed E2E test (`crates/guiltty-kitty/tests/e2e_kitty.rs`, Linux-only) and
the promised follow-up to the "no kitty-compatible terminal available" caveat that
shipped with `tasks/plan.md`'s T1 and T4.

## Prerequisites

- A kitty-compatible terminal. kitty itself is the reference implementation and
  the most reliable oracle; Ghostty and iTerm2 also support the kitty graphics
  protocol (protocol version 0.20.0+ is required for the `C` "don't move cursor"
  control key `present()` uses).
- A built demo binary:

  ```sh
  cargo build -p guiltty-examples --bin demo
  ```

## Steps

1. Open a kitty-compatible terminal and run:

   ```sh
   cargo run -p guiltty-examples --bin demo
   ```

2. **First frame.** The demo draws text, one shape of each kind (line, rect,
   circle, ellipse, triangle, and a filled closed path), and a sprite, then calls
   `present()` once. You should see all of these rendered, and the demo pauses
   with:

   ```
   First frame rendered. Press Enter to render the second frame...
   ```

   Confirm the first frame shows the full scene before continuing.

3. **Second frame.** Press Enter. The demo moves the sprite to the far right and
   calls `present()` a second time. Confirm that:

   - the sprite is now at the new position (right side), and
   - the background where the sprite *was* is restored intact — no trail, no
     smearing, no leftover pixels from the first frame.

   This second frame is what proves the sprite actually moved and that
   save/restore-under preserved the background; a single `present()` after an
   in-memory move would not.

## What to look for

| Element | Expected |
|---|---|
| Text | "GUILTTY V0 DEMO" near the top-left |
| Line | a horizontal gray line |
| Rect | a red rectangle |
| Circle | a blue circle |
| Ellipse | a green ellipse |
| Triangle | a yellow triangle |
| Filled path | a purple star-ish polygon (even-odd fill) |
| Sprite | a small yellow/black image, moving left → right between frames |

If any shape is missing, mis-sized (e.g. 4× too large/small — see the
`ImageDimensions` note in `crates/guiltty-kitty/src/lib.rs`), or the background is
corrupted after the sprite moves, that is a real rendering bug to report.

## Platform notes

- **macOS** and **BSD** have no Xvfb equivalent (Quartz / no X11), so this manual
  procedure is the *only* verification path on those platforms — the automated
  E2E test is Linux-only.
- **BSD**: kitty publishes only the companion `kitten` CLI for BSDs, not the full
  terminal, so install the terminal via the platform package manager (e.g.
  FreeBSD `pkg install kitty` / ports `x11/kitty`).

## Relationship to the automated test

This manual check and the automated `e2e_kitty.rs` test cover the same
"protocol-acceptance" question from opposite directions: the automated test proves
kitty *accepts* the escape sequence (an `OK` response), while this manual check
proves the pixels *render correctly* — something the protocol's `OK`/error reply
cannot tell you (kitty accepts a transmission it then renders wrong, and vice
versa). See `docs/spec-kitty-e2e.md` for the full rationale.
