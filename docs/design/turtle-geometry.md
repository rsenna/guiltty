# Design: turtle geometry (`guiltty-turtle`)

Source: not a `docs/spec.md` v0 success criterion — forward-looking work for
`iklo`, which will need turtle-style drawing on top of `guiltty` later.
Depends on [`docs/design/sprite-crate-extraction.md`](sprite-crate-extraction.md),
which must land first: this design assumes `guiltty-sprite`'s `Sprite`
already has absolute (`move_to`) and relative (`heading`/`forward`/`turn`)
movement — turtle graphics adds nothing to *movement* itself, only pen
state and drawing.

## Objective

Give callers a Logo-style turtle: a `guiltty-sprite`-backed actor that can
trace a line as it moves, with pen up/down toggling whether movement draws.
Multiple turtles (a feature of some Logo dialects) then falls out for free
from having multiple `Sprite`s, exactly as multiple sprites already do.

## Decision: `Turtle` wraps a `Sprite`, adds only pen state

Movement (absolute and relative) is entirely `guiltty-sprite`'s concern
already — see that doc's rationale for why. What's specifically "turtle
graphics" and not more generally useful is narrow: **does moving leave a
visible trail**. That's the only thing this crate adds:

```rust
pub struct Turtle {
    sprite: Sprite,
    pen_down: bool,
    pen_color: Color,
}

impl Turtle {
    pub fn new(bitmap: Bitmap, position: Point) -> Self; // pen down, black, wraps Sprite::new

    // --- pen state (new — this crate's entire reason to exist) ---
    pub fn pen_up(&mut self) -> &mut Self;
    pub fn pen_down(&mut self) -> &mut Self;
    pub fn set_pen_color(&mut self, color: Color) -> &mut Self;

    // --- movement (delegates straight to the wrapped Sprite, then draws if pen is down) ---
    // Drawing moves return Result, not `&mut Self`: `clear_footprint` can now
    // fail (see companion doc's "Footprint staleness"), and a failed move must
    // not silently continue as if it had drawn. `?`-chaining replaces
    // method-chaining for these three.
    pub fn forward(&mut self, canvas: &mut Canvas, distance: f32) -> Result<&mut Self, StaleFootprint>;
    pub fn backward(&mut self, canvas: &mut Canvas, distance: f32) -> Result<&mut Self, StaleFootprint>;
    pub fn turn(&mut self, degrees: f32) -> &mut Self;   // no line to draw — heading-only, no canvas needed
    pub fn left(&mut self, degrees: f32) -> &mut Self;
    pub fn right(&mut self, degrees: f32) -> &mut Self;
    pub fn goto(&mut self, canvas: &mut Canvas, position: Point) -> Result<&mut Self, StaleFootprint>; // absolute move + draw

    pub fn sprite(&self) -> &Sprite;       // escape hatch to the underlying Sprite
    pub fn sprite_mut(&mut self) -> &mut Sprite;

    // Recovery from Err(StaleFootprint) (see below): discards the sprite's
    // stale footprint and re-places it at its current position -- no trail
    // segment is drawn (there's no known-good `from` pixel state to draw
    // over). After `resync`, `forward`/`backward`/`goto` work normally again.
    pub fn resync(&mut self, canvas: &mut Canvas) -> &mut Self;
}
```

Each drawing move (`forward`/`backward`/`goto`) uses `guiltty-sprite`'s
`clear_footprint`/`place` split (not the bundled `draw_on`) specifically to
get a trail line drawn *between* the two — using `draw_on` here would
restore the sprite's *old* footprint **after** the trail line already
drew into it, erasing exactly the pixels where the line started, on every
single move. In order: (1) `sprite.clear_footprint(canvas)` — reveals
whatever the canvas actually showed before the icon was last placed there
(which includes any trail segment drawn on a prior move, since that segment
was drawn *before* that prior move's own `place` call captured it), (2)
record the current position as `from`, (3) delegate to the wrapped
`Sprite`'s own `forward`/`backward`/`move_to` to update its position, (4) if
`pen_down`, draw `canvas.draw_shape(&Shape::line(from, sprite.position()),
Fill::solid(pen_color))` onto the now-cleared canvas, (5)
`sprite.place(canvas)` — captures the footprint *after* the trail segment is
already there, then blits the icon on top. The icon ends up sitting over the
trail's endpoint each move (cosmetic — trail-under-icon, not the other way
around — acceptable and easy to flip later if wanted).

Step (1) can now fail: if `clear_footprint` returns `Err(StaleFootprint)`
— *another* sprite's write actually overlapped this sprite's footprint
since its last `place`, e.g. a *different* `Turtle`'s pen-down move drew
through this one's current footprint (disjoint turtles never trigger this
— see the companion doc's region-scoping) — `forward`/`backward`/`goto`
propagate the error instead of drawing, leaving this turtle's position, the
sprite's `last_draw`, and the canvas all untouched (see
[`sprite-crate-extraction.md`](sprite-crate-extraction.md)'s "Footprint
staleness"). This is what makes it safe for two turtles' trails to cross:
whichever one next tries to redraw over the intersection gets a caught
error on that one move instead of silently erasing the other turtle's
trail. It does not automatically preserve both trails through the overlap;
it only guarantees the conflict can't pass silently.

Retrying the same move after `Err(StaleFootprint)` cannot succeed on its
own — the underlying version only increases, so the mismatch is permanent
for that footprint. The caller must call `resync(canvas)` first (discards
the stale footprint, re-places the icon with no trail segment for that
recovery step), then retry the move normally. A turtle that never calls
`resync` after a stale error stays stuck: every subsequent move on it
repeats the same failed `clear_footprint` check.

`turn`/`left`/`right`
only change heading — no line to draw, so unlike the movement methods they
don't need a `&mut Canvas` argument at all.

This is a strict subset of what the original standalone sketch proposed:
no separate position/heading fields (the wrapped `Sprite` already has them),
no separate rounding-drift handling (already solved in `guiltty-sprite`).
It does, however, depend on `guiltty-sprite` exposing the `clear_footprint`/
`place` split alongside `draw_on` — see that doc's API sketch.

## Non-goals

(Unchanged from the original sketch.)

- **No automatic closed-shape fill.** A turtle traces one `Shape::Line` per
  move; it does not detect when a path closes and switch to `Shape::Path`'s
  fill behavior. Out of scope for this first version.
- **No angle/arc/circle turtle commands** — only straight segments and
  in-place turns. Can be added later without breaking this API.
- **No serialization/replay of a turtle's move history.**
- **No collision/game-oriented features** — those belong to plain
  `guiltty-sprite` usage (relative movement without a pen), not this crate.

## Open questions

- Whether `Turtle::new` should take an already-constructed `Sprite` instead
  of a `Bitmap`+`Point` (so a caller who already built one via
  `guiltty-sprite` doesn't have to unpack/repack it) — leaning toward
  accepting `Sprite` directly once `guiltty-sprite` lands, revisit at
  implementation time.
- Default heading-0° convention (east vs. Logo's traditional north) —
  inherited from `guiltty-sprite`, not re-decided here.

## Follow-up

Once [`sprite-crate-extraction.md`](sprite-crate-extraction.md)'s two PRs
land: scaffold `crates/guiltty-turtle` (new workspace member, depending on
both `guiltty-sprite`, for `Sprite`/`Bitmap`, **and directly on
`guiltty-core`**, for `Canvas`/`Color`/`Point`/`Shape`/`Fill` — the sketch
above uses all of these directly, so this isn't a `guiltty-sprite`-only
dependency), implement `Turtle` per the sketch above with unit tests
(pen-down drawing a line segment on the move that triggered it, pen-up not
drawing, pen-color changes affecting only subsequent segments,
`turn` not requiring a canvas, a regression test asserting a single
turtle's multi-move trail has **no gap** at any previous position — the
exact bug this design's `clear_footprint`/`place` ordering exists to
prevent — a two-turtle test where B's footprint overlaps a segment A
draws afterward: B's next `forward` returns `Err(StaleFootprint)` instead
of erasing A's trail, a test that two turtles moving in genuinely disjoint
areas never see `Err(StaleFootprint)` from each other, and a recovery test
that `resync` followed by a normal move succeeds after a prior
`StaleFootprint`), and a small example under `examples/`
tracing a recognizable shape (e.g. a star or spiral) to double as the
manual visual check.
