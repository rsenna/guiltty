# Design: extract `guiltty-sprite`, add relative movement to `Sprite`

Source: not a `docs/spec.md` v0 success criterion — forward-looking
groundwork for `iklo` (games, turtle graphics), prompted by wanting
`guiltty-core` to stay scoped to exactly "draw into a kitty-like terminal"
and nothing else. Companion doc:
[`docs/design/turtle-geometry.md`](turtle-geometry.md), which builds on top
of what this doc adds.

## Objective

Two changes, sequenced as one design since the second only makes sense once
the first has landed:

1. Move `Sprite`/`Bitmap` out of `guiltty-core` into a new `guiltty-sprite`
   crate — `guiltty-core` keeps only the absolute-coordinate drawing surface
   (`Canvas`, `Shape`, text, the `Backend` trait).
2. Give `Sprite` a **relative, actor-centric** movement API (`heading`,
   `forward`/`backward`, `turn`) alongside its **existing absolute** one
   (`move_to`) — both stay first-class, neither replaces the other.

## Two coordinate paradigms, both grounded in one absolute truth

Two ways of thinking about movement are both genuinely useful, and this
design keeps both available rather than picking one:

- **Absolute (canvas-coordinate).** The canvas is the source of truth: a
  `(0, 0)` origin, everything else addressed as absolute coordinates —
  `Shape::line(Point::new(x0, y0), Point::new(x1, y1))`, `Sprite::move_to(Point)`.
  This is what `guiltty-core` already is today and stays exactly that.
- **Relative (actor-centric).** Movement described from the mover's own
  point of view — "go forward 10, turn right 90" — with no absolute
  coordinate mentioned at all. This is turtle geometry's defining trait, but
  it's useful independent of turtle *graphics* (drawing a trail): a
  game sprite that has a heading and moves forward along it needs the same
  primitive, with no pen involved.

The relative paradigm is not a competing coordinate system requiring its own
storage — a `Sprite`'s `forward(distance)` computes one absolute
`(x + distance * heading_deg.to_radians().cos(), y + distance *
heading_deg.to_radians().sin())` from its current position and heading
(heading is stored and specified in **degrees**; radians only exist inside
the formula itself), then is drawn exactly like any other absolute move. On
this canvas's top-left origin (positive Y downward), 0° faces +x (east) and
positive degrees turn **clockwise** — e.g. 90° faces +y (south), not north.
Relative motion is a stateful convenience layer that always resolves to an
absolute position before anything is drawn; `Canvas` never needs to know a
caller was "thinking in relative terms" at all. This is why the paradigm
split maps directly onto the crate split: `guiltty-core` only ever deals in
absolutes, and the relative layer lives entirely in `guiltty-sprite` on top
of it.

**Both movement APIs stay on `Sprite` itself** — this isn't relative-only:
`move_to(Point)` (already implemented today) remains for absolute
placement, and `forward`/`turn` are additive. A caller can freely mix both
on the same sprite (e.g. `sprite.move_to(spawn_point); sprite.forward(5.0);`).

## The extraction's one real wrinkle: `Canvas::draw_sprite` touches private fields

`Canvas::draw_sprite` (`crates/guiltty-core/src/lib.rs`) isn't a simple
consumer of `Canvas`'s public API today — its save/restore-under logic
reads `self.pixels` directly (not through the public, bounds-checked
`pixel()`) and tags each `DrawnFootprint` with `self.id`, a private field
that exists solely so a sprite's saved footprint is never restored onto the
wrong `Canvas` instance. Both are private to `guiltty-core`; once `Sprite`
lives in a different crate, an inherent `Canvas::draw_sprite` can't exist
there anymore (Rust's orphan rule), and the new crate has no access to
`Canvas`'s private fields either way.

Resolution, in two parts:

- `Canvas` gains two new public accessors: `pub fn id(&self) -> u64`
  (or an opaque `CanvasId` newtype if we'd rather not expose the raw
  `u64`) — enough for `guiltty-sprite` to replicate the
  wrong-canvas-guard without needing direct field access — and `pub fn
  version(&self) -> u64`, a monotonic counter bumped on every
  pixel-mutating call (`set_pixel`, `draw_shape`, a sprite's `place`, …),
  used below to detect a stale footprint. Both are additive and
  non-breaking: `guiltty-core`'s existing public API is unchanged, only
  extended.
- The draw method itself moves to `guiltty-sprite` as `sprite.draw_on(&mut
  canvas)` (a method on `Sprite`, since `Canvas` can no longer host an
  inherent method for a foreign type), reimplemented entirely against
  `Canvas`'s existing public `pixel`/`set_pixel` — trading direct slice
  indexing for per-pixel bounds-checked accessor calls. This is slightly
  more overhead per pixel, not a behavior change, and consistent with how
  every other cross-boundary drawing operation in this codebase already
  works; revisit only if profiling ever shows it matters.

**This second part *is* a breaking change**, and the extraction as a whole
should ship as one: `Sprite`, `Bitmap`, and `Canvas::draw_sprite` disappear
from `guiltty-core`'s public API, and `canvas.draw_sprite(&mut sprite)`
call sites become `sprite.draw_on(&mut canvas)`. `guiltty`'s facade crate
can re-export `Sprite`/`Bitmap`'s new location under the same path (so
`guiltty::Sprite` keeps working), but it **cannot** preserve
`Canvas::draw_sprite` as an inherent method — a re-export doesn't grant a
downstream crate the right to add inherent methods to `Canvas`. Given the
project is pre-1.0 with every crate at `0.0.0` (`docs/spec.md`'s existing
precedent for T1's `Backend::present` signature change), no compatibility
shim is planned: this ships as a documented breaking change in the PR
description, with call sites in this repo's own examples/tests updated in
the same PR, not a deprecation cycle.

No other part of `Canvas`'s public API needs to change. `Bitmap` moves
alongside `Sprite` (it's `Sprite`'s only real dependency) — including its
`from_file` error path: `Bitmap::from_file` keeps returning
`Result<Self, guiltty_core::Error>` (the `Error::ImageLoad` variant already
defined in `guiltty-core`), rather than inventing a new crate-local error
type. `guiltty-sprite` already depends on `guiltty-core` directly (for
`Canvas`, `Color`, `Point`, and now `Canvas::id()`), so depending on its
`Error` type too is not a new coupling — just reusing what's already
required.

## API sketch (`guiltty-sprite`)

```rust
pub struct Sprite {
    bitmap: Bitmap,
    exact_position: (f32, f32), // canonical position — sub-pixel precision
    heading_deg: f32,           // NEW — relative-movement state; 0.0 = facing +x (east)
    last_draw: Option<DrawnFootprint>,
}

impl Sprite {
    pub fn new(bitmap: Bitmap, position: Point) -> Self; // heading defaults to 0.0

    // --- absolute (unchanged from today) ---
    pub fn position(&self) -> Point;           // exact_position, rounded to i32
    pub fn move_to(&mut self, position: Point); // resets exact_position to (x as f32, y as f32) -- no fractional carry-over across an absolute jump

    // --- relative (new) ---
    pub fn heading(&self) -> f32;
    pub fn set_heading(&mut self, degrees: f32);
    pub fn forward(&mut self, distance: f32);   // moves exact_position along current heading
    pub fn backward(&mut self, distance: f32);  // forward(-distance)
    pub fn turn(&mut self, degrees: f32);       // positive = clockwise
    pub fn left(&mut self, degrees: f32);       // sugar for turn(-degrees)
    pub fn right(&mut self, degrees: f32);      // sugar for turn(degrees)

    pub fn bitmap(&self) -> &Bitmap;

    // `draw_on` is `clear_footprint` followed by `place` -- see below. Most callers
    // (anything not interleaving other drawing between a sprite's redraws, e.g.
    // `guiltty-turtle`) just want this one call.
    pub fn draw_on(&mut self, canvas: &mut Canvas); // was Canvas::draw_sprite

    // The two steps `draw_on` composes, exposed separately for callers (like
    // `guiltty-turtle`) that need to draw something else *in between* clearing the
    // sprite's old footprint and placing it at the new one -- seeing/using only
    // `draw_on` can't do this, since it bundles restore+capture+blit as one atomic
    // step with nothing else able to run in the middle.
    pub fn clear_footprint(&mut self, canvas: &mut Canvas) -> Result<(), StaleFootprint>; // restore-only; Ok(()) no-op if never drawn; Err(StaleFootprint) — canvas left untouched — if drawn on a different Canvas or if the canvas has changed since this footprint was captured (see "Footprint staleness" below)
    pub fn place(&mut self, canvas: &mut Canvas);           // capture-new-footprint-then-blit only, no restore
}
```

`exact_position` — not `Point` — is the struct's one canonical position
field; `Point` is only ever a rounded *view* of it, produced by `position()`
and consumed by `draw_on`. This resolves the rounding-drift problem
directly: many small `forward()` calls each accumulate into
`exact_position` at full `f32` precision, and only get rounded to `i32` at
the moment something (`position()`, `draw_on`) actually needs a pixel
coordinate — so fractional displacement from repeated sub-pixel moves is
never silently discarded call-by-call. `move_to` is the one place that
*resets* `exact_position` outright (from the supplied integer `Point`,
losing any prior fractional part) rather than accumulating into it, since an
absolute jump has no meaningful "fractional carry-over" from wherever the
sprite was before.

## Footprint staleness: version-stamped, fail-fast

`clear_footprint` restores a snapshot captured at `place` time. If anything
else draws into that same region between the capture and the restore — a
second `clear_footprint` call replaying an already-consumed snapshot, or
(in `guiltty-turtle`) a *different* sprite's trail drawn through this
sprite's footprint before it's cleared — a naive restore silently blits the
old snapshot back, discarding whatever drew there in the meantime. This is
a pixel-level hazard, not a "whose trail is it" one: the canvas has no
notion of ownership, only of what was written and when.

The fix is version-stamping, checked fail-fast rather than avoided by
restricting when callers are allowed to draw:

```rust
struct DrawnFootprint {
    canvas_id: u64,
    version: u64,   // Canvas::version() at the moment this footprint was captured
    // .. existing footprint pixel data ..
}

pub struct StaleFootprint; // canvas.version() has advanced since capture
```

`Canvas::version()` is a monotonic counter bumped on every pixel-mutating
call (`set_pixel`, `draw_shape`, a sprite's `place`). `clear_footprint`
compares its footprint's stored `version` against the canvas's current
one; on a mismatch it returns `Err(StaleFootprint)` and leaves the canvas
untouched, instead of restoring pixels that no longer reflect what's
actually been drawn. `last_draw` is left in place on error (the caller
decides whether to retry, skip the clear, or propagate the error) and is
only cleared on a successful restore.

The counter is per-`Canvas`, not per-region: any write anywhere on the
canvas invalidates every open footprint on it, even ones nowhere near what
was drawn. That's a deliberate over-approximation — a false-positive
`StaleFootprint` is a caller-visible error to handle, not silent pixel
corruption, and per-region tracking would need spatial indexing this
design doesn't need yet. Revisit only if this proves too conservative in
practice (e.g. many independent sprites on one large canvas).

## Non-goals

- **No collision detection.** Mentioned as a motivating future use case for
  relative sprite movement (games), but out of scope for this design —
  revisit once there's a concrete need.
- **No change to the existing save/restore-under trail-avoidance
  behavior** — `draw_on` preserves `draw_sprite`'s exact semantics, just
  relocated and reimplemented against public `Canvas` accessors.
  `clear_footprint`/`place` are additive decompositions of that same
  behavior (see companion doc's Turtle for why they're needed), not a
  new drawing model.
- **No pen/drawing behavior on `Sprite` itself** — that's
  `guiltty-turtle`'s job, on top of this crate; see the companion doc.

## Follow-up

Two PRs, in order:

1. **Extract `guiltty-sprite`**: new workspace member, move `Sprite`/`Bitmap`
   verbatim (including `Bitmap::from_file`'s `Result<Self, guiltty_core::Error>`
   signature, unchanged), add `Canvas::id()` and `Canvas::version()`,
   reimplement `draw_on`/`clear_footprint`/`place` against `Canvas`'s
   public API, update `guiltty`'s facade re-exports and any existing
   sprite-related tests/examples to the new crate and call-site
   (`sprite.draw_on(&mut canvas)` instead of `canvas.draw_sprite(&mut
   sprite)`). No drawing-behavior change, but a breaking public-API change
   as described above — call this out explicitly in the PR description,
   don't call it "non-breaking." Unit tests must cover `clear_footprint`'s
   stale-detection: calling it twice in a row returns `Err(StaleFootprint)`
   on the second call, and a write to the canvas between `place` and
   `clear_footprint` (standing in for another sprite's trail crossing this
   one's footprint) does too — both leaving the canvas' pixels unchanged.
2. **Add relative movement**: `heading`/`forward`/`backward`/`turn`/`left`/
   `right` on `Sprite`, with unit tests covering heading after known turn
   sequences, position after known forward/turn sequences (including the
   sub-pixel rounding case), and that `move_to` and `forward` compose
   correctly when mixed.
