# Design: viewport regions, zoom, scroll/pan

Source: [`docs/spec.md`](../spec.md)'s Success Criteria #4-6, and the Open
Questions this resolves (viewport/zoom/scroll API shape). Referenced by
[`tasks/plan.md`](../../tasks/plan.md)'s header, which deliberately deferred
these three features pending this design pass.

## Objective

Resolve one concrete API shape covering all three remaining v0 success
criteria:

4. Multiple independent viewport regions coexisting within a single terminal.
5. A zoom level scaling a canvas's rendered output.
6. Scroll/pan for a canvas larger than the terminal.

## Grounding: what exists today

`guiltty-core`'s `Canvas` is a fixed-size RGBA8 pixel buffer
(`crates/guiltty-core/src/lib.rs`). Drawing (`draw_text`, `draw_shape`,
`draw_sprite`) all operate on one `Canvas`. `Backend::present(&mut self,
canvas: &Canvas)` transmits exactly one `Canvas` per call — there is no
`Frame`/`Terminal` wrapper, no multi-image placement, and no concept of a
sub-region today. `docs/spec.md`'s illustrative `frame.viewport(Rect::new(...))
.render_shape(...)` example presumes a `Terminal`/`Frame` abstraction that
was never built — T1-T4 shipped a smaller, more direct API (`Canvas` +
`Backend::present(canvas)` called straight from application code). This
design intentionally builds on what actually exists rather than first
building the aspirational `Frame`/`Terminal` wrapper, which is out of scope
here.

`crates/guiltty-kitty/src/lib.rs`'s `present()` transmits the canvas's pixel
buffer as a single kitty image at whatever the cursor's current position is
(`CursorMovementPolicy::DontMove`) — it has no concept of "where on screen"
beyond that. `tasks/plan.md`'s T1 task explicitly deferred multi-image
placement to this design pass (see that task's "Note on scope").

## Decision: two new `Canvas` primitives, no `Backend` changes

Everything below is built from **two new methods on `Canvas`**, both pure
`guiltty-core` logic with no backend-specific code (per `docs/spec.md`'s
Boundaries: "keep `guiltty-core` free of any kitty-specific code"). The
`Backend` trait is **not changed** — every feature below still ends in a
plain `Backend::present(&Canvas)` call, exactly as today.

```rust
impl Canvas {
    /// Extracts the sub-rectangle `rect` as a new, owned `Canvas` of exactly
    /// `rect.width x rect.height` pixels. Any part of `rect` outside `self`'s
    /// bounds is padded with fully-transparent pixels (`Color::default()`)
    /// rather than shrinking the result or erroring — callers always get a
    /// predictably-sized result, matching the project's existing "clip,
    /// don't panic or error, on out-of-bounds" style (`set_pixel`,
    /// `draw_sprite`).
    pub fn crop(&self, rect: Rect) -> Canvas;

    /// Composites `source`'s pixels into `self` at `at` (top-left), clipped
    /// to `self`'s bounds — any part of `source` that would land outside
    /// `self` is simply not drawn, the same clip-intersection approach
    /// `draw_sprite` already uses for a `Bitmap` source. Fully transparent
    /// source pixels (`alpha == 0`) are skipped rather than overwriting,
    /// matching `draw_sprite`'s existing behavior. Unlike `draw_sprite`,
    /// `blit` does **not** save/restore an under-footprint — see "Why no
    /// save/restore-under for blit" below.
    pub fn blit(&mut self, source: &Canvas, at: Point);

    /// Nearest-neighbor resamples `self` to `(width as f32 * factor, height
    /// as f32 * factor).round()`. `factor` must be finite and `> 0.0`.
    ///
    /// # Panics
    /// Panics if `factor` is not finite or is `<= 0.0` — a nonsensical zoom
    /// factor is a programmer error, not a recoverable runtime condition,
    /// consistent with `Canvas::new`'s existing panic-on-nonsensical-size
    /// precedent.
    pub fn scaled(&self, factor: f32) -> Canvas;
}
```

### Why no save/restore-under for `blit`

`draw_sprite` saves the canvas content a sprite's *previous* footprint
covered and restores it before drawing the sprite at its new position — this
is what lets a single persistent background scene host a moving sprite
without a full-frame redraw. `blit` deliberately does **not** do this: the
features built on it (viewport regions, scroll/pan) all naturally redraw
their whole source canvas fresh every frame (ratatui's own `draw(|frame|
{...})` per-frame-redraw model, which `docs/spec.md`'s Code Style section
already draws on for inspiration) — there's no "previous background" to
restore, since the *destination* canvas is expected to be rebuilt from
scratch each time `blit` is used to compose it. Adding save/restore-under to
`blit` would be unused complexity for how these features actually get used;
sprites keep their own mechanism unchanged.

## Viewport regions (Success Criterion #4)

A viewport region is: draw into your own region-sized `Canvas`, `blit` it
into one shared terminal-sized `Canvas`, present that **one** shared canvas.
Regions are drawn independently (nothing about drawing into region A's
canvas touches region B's), and they visually coexist in a single terminal
because they're composited into one image before presentation — satisfying
Success Criterion #4 without any multi-image kitty placement, cursor
positioning, or `Backend` changes.

```rust
let mut screen = Canvas::new(800, 600);

let mut region_a = Canvas::new(400, 300);
region_a.draw_shape(&Shape::rect(Point::new(0, 0), 100, 50), Fill::solid(Color::rgb(0, 255, 0)));
screen.blit(&region_a, Point::new(0, 0));

let mut region_b = Canvas::new(400, 300);
region_b.draw_text("HELLO", Point::new(10, 10), &TextStyle::default());
screen.blit(&region_b, Point::new(400, 0));

backend.present(&screen)?;
```

This diverges from `docs/spec.md`'s illustrative `frame.viewport(Rect)
.render_shape(...)` example (which presumes a not-yet-built `Frame`
wrapper) but keeps the same conceptual surface — "get a sub-area, draw into
it, it shows up in the right place." If a `Frame`/`Terminal` wrapper is ever
built on top of this (out of scope here), it would implement `viewport()` by
allocating a region `Canvas` and calling `blit` on drop/commit — this design
doesn't block that, it just doesn't require it first.

**Scope decision — screen positioning is the caller's job.** Where each
viewport lands *on the terminal* (which terminal cell the composited screen
canvas starts at) is governed by the same "wherever the cursor is when
`present()` sends the escape sequence" rule that already applies today —
`guiltty` does not add a layout engine or manage cursor position itself.
This matches the project's existing minimal-scope precedent (no
mouse/keyboard handling, no interactive widgets, per `docs/spec.md`'s
Boundaries) and ratatui's own convention of the host application owning
terminal layout.

## Zoom (Success Criterion #5)

`Canvas::scaled(factor)` is applied to whatever canvas is about to be
presented — the whole canvas, a `crop`ped scroll/pan window, or a
`blit`-composited multi-region screen — right before the `present()` call.
This keeps zoom orthogonal to both other features: it's a final resampling
step, not a property baked into any other primitive.

```rust
let zoomed = screen.scaled(1.5);
backend.present(&zoomed)?;
```

Nearest-neighbor was chosen over bilinear for v0: it's simpler, matches the
project's existing "no anti-aliasing" precedent (`draw_shape`'s doc
comments already note shapes are drawn without anti-aliasing), and avoids
introducing blur/blending logic before there's a concrete need for it.
Bilinear (or other resampling) can be added later as a different method or
an option, without breaking `scaled`'s existing callers.

## Scroll/pan (Success Criterion #6)

A canvas larger than the terminal is just a big `Canvas`; the "viewport"
onto it is a `crop`ped window, and panning is changing that window's offset
between frames.

```rust
let mut world = Canvas::new(4000, 3000); // larger than any real terminal
// ...draw the whole scene into `world` once, or incrementally...

let mut viewport_offset = Point::new(0, 0);
loop {
    let visible = world.crop(Rect::new(viewport_offset.x, viewport_offset.y, 800, 600));
    backend.present(&visible)?;
    viewport_offset.x += 10; // pan right 10px/frame, for example
}
```

Composes with zoom by calling `.scaled(factor)` on the cropped result before
presenting, same as the viewport-regions case above.

## Non-goals

- **No `Frame`/`Terminal`/`draw()` abstraction.** `docs/spec.md`'s
  illustrative code implies one; building it is a separate, larger
  architectural question this design doesn't take on. Everything above
  works directly against `Canvas` + `Backend::present`, matching what T1-T4
  actually shipped.
- **No independent multi-image kitty placement.** Viewport regions composite
  into one canvas and present as one image; `guiltty-kitty` gains no new
  placement/positioning logic from this design.
- **No layout engine / terminal cursor management.** Screen positioning of
  composited output is the calling application's responsibility, same as
  today.
- **No bilinear/other resampling filters.** Nearest-neighbor only for v0.

## Open questions

- Whether `crop`/`blit`/`scaled` should also gain in-place (`&mut self`,
  no new allocation) variants for hot per-frame paths, once there's a
  concrete performance reason to — deferred until profiling says so.
- Whether a future `Frame`/`Terminal` convenience wrapper (out of scope
  here) is worth building once real applications start using this API and
  find the manual `Canvas::new` + `blit`/`crop` + `present` sequence
  repetitive.

## Follow-up

Once this design is accepted, break Success Criteria #4-6 into
`tasks/plan.md`-style tasks (new task IDs, e.g. `V1`-`V3`, to avoid
colliding with `plan.md`'s own `T1`-`T4` and `plan-kitty-e2e.md`'s `K1`-`K4`)
covering: `Canvas::crop`/`blit`/`scaled` implementation + unit tests, and a
runnable example demonstrating all three (viewport regions, zoom, scroll/pan
composed together) per Success Criterion #7's intent.
