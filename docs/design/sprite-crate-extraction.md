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
`(x + distance * cos(heading), y + distance * sin(heading))` from its
current position and heading, then is drawn exactly like any other absolute
move. Relative motion is a stateful convenience layer that always resolves
to an absolute position before anything is drawn; `Canvas` never needs to
know a caller was "thinking in relative terms" at all. This is why the
paradigm split maps directly onto the crate split: `guiltty-core` only ever
deals in absolutes, and the relative layer lives entirely in `guiltty-sprite`
on top of it.

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

Resolution, in two additive (non-breaking) parts:

- `Canvas` gains one new public accessor, `pub fn id(&self) -> u64`
  (or an opaque `CanvasId` newtype if we'd rather not expose the raw
  `u64`) — enough for `guiltty-sprite` to replicate the
  wrong-canvas-guard without needing direct field access.
- The draw method itself moves to `guiltty-sprite` as `sprite.draw_on(&mut
  canvas)` (a method on `Sprite`, since `Canvas` can no longer host an
  inherent method for a foreign type), reimplemented entirely against
  `Canvas`'s existing public `pixel`/`set_pixel` — trading direct slice
  indexing for per-pixel bounds-checked accessor calls. This is slightly
  more overhead per pixel, not a behavior change, and consistent with how
  every other cross-boundary drawing operation in this codebase already
  works; revisit only if profiling ever shows it matters.

No other part of `Canvas`'s public API needs to change. `Bitmap` moves
alongside `Sprite` (it's `Sprite`'s only real dependency).

## API sketch (`guiltty-sprite`)

```rust
pub struct Sprite {
    bitmap: Bitmap,
    position: Point,      // absolute — same meaning as today
    heading_deg: f32,     // NEW — relative-movement state; 0.0 = facing +x (east)
    last_draw: Option<DrawnFootprint>,
}

impl Sprite {
    pub fn new(bitmap: Bitmap, position: Point) -> Self; // heading defaults to 0.0

    // --- absolute (unchanged from today) ---
    pub fn position(&self) -> Point;
    pub fn move_to(&mut self, position: Point);

    // --- relative (new) ---
    pub fn heading(&self) -> f32;
    pub fn set_heading(&mut self, degrees: f32);
    pub fn forward(&mut self, distance: f32);   // moves along current heading
    pub fn backward(&mut self, distance: f32);  // forward(-distance)
    pub fn turn(&mut self, degrees: f32);       // positive = clockwise
    pub fn left(&mut self, degrees: f32);       // sugar for turn(-degrees)
    pub fn right(&mut self, degrees: f32);      // sugar for turn(degrees)

    pub fn bitmap(&self) -> &Bitmap;
    pub fn draw_on(&mut self, canvas: &mut Canvas); // was Canvas::draw_sprite
}
```

`forward`/`backward` track position as `f32` internally (same
rounding-drift reasoning as `guiltty-turtle`'s original sketch: many small
moves compounding integer rounding error is a real problem for both games
and turtle patterns) and round to `Point`'s `i32` only when the position is
read or drawn.

## Non-goals

- **No collision detection.** Mentioned as a motivating future use case for
  relative sprite movement (games), but out of scope for this design —
  revisit once there's a concrete need.
- **No change to the existing save/restore-under trail-avoidance
  behavior** — `draw_on` preserves `draw_sprite`'s exact semantics, just
  relocated and reimplemented against public `Canvas` accessors.
- **No pen/drawing behavior on `Sprite` itself** — that's
  `guiltty-turtle`'s job, on top of this crate; see the companion doc.

## Follow-up

Two PRs, in order:

1. **Extract `guiltty-sprite`**: new workspace member, move `Sprite`/`Bitmap`
   verbatim, add `Canvas::id()`, reimplement `draw_on` against `Canvas`'s
   public API, update `guiltty`'s facade re-exports and any existing
   sprite-related tests/examples to the new crate and call-site
   (`sprite.draw_on(&mut canvas)` instead of `canvas.draw_sprite(&mut
   sprite)`). Purely mechanical — no behavior change — should be reviewable
   as such.
2. **Add relative movement**: `heading`/`forward`/`backward`/`turn`/`left`/
   `right` on `Sprite`, with unit tests covering heading after known turn
   sequences, position after known forward/turn sequences (including the
   sub-pixel rounding case), and that `move_to` and `forward` compose
   correctly when mixed.
