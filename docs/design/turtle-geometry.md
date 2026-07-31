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
    pub fn forward(&mut self, canvas: &mut Canvas, distance: f32) -> &mut Self;
    pub fn backward(&mut self, canvas: &mut Canvas, distance: f32) -> &mut Self;
    pub fn turn(&mut self, degrees: f32) -> &mut Self;   // no line to draw — heading-only, no canvas needed
    pub fn left(&mut self, degrees: f32) -> &mut Self;
    pub fn right(&mut self, degrees: f32) -> &mut Self;
    pub fn goto(&mut self, canvas: &mut Canvas, position: Point) -> &mut Self; // absolute move + draw

    pub fn sprite(&self) -> &Sprite;       // escape hatch to the underlying Sprite
    pub fn sprite_mut(&mut self) -> &mut Sprite;
}
```

Each drawing move (`forward`/`backward`/`goto`) does, in order: (1) record
the current absolute position as `from`, (2) delegate to the wrapped
`Sprite`'s own `forward`/`backward`/`move_to` to update its position, (3) if
`pen_down`, draw `canvas.draw_shape(&Shape::line(from, sprite.position()),
Fill::solid(pen_color))`, (4) draw the sprite's bitmap on top via
`sprite.draw_on(canvas)` so the turtle's own icon still renders
non-destructively over the trail it just drew, using the same
save/restore-under mechanism `Sprite` already has. `turn`/`left`/`right`
only change heading — no line to draw, so unlike the movement methods they
don't need a `&mut Canvas` argument at all.

This is a strict subset of what the original standalone sketch proposed:
no separate position/heading fields (the wrapped `Sprite` already has them),
no separate rounding-drift handling (already solved in `guiltty-sprite`).

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
`guiltty-sprite`), implement `Turtle` per the sketch above with unit tests
(pen-up not drawing, pen-color changes affecting only subsequent segments,
`turn` not requiring a canvas), and a small example under `examples/`
tracing a recognizable shape (e.g. a star or spiral) to double as the
manual visual check.
