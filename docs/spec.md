# Spec: guiltty v0

## Objective

guiltty is a Rust library for drawing real, pixel-level 2D graphics into a terminal, using the kitty graphics protocol as its first backend. It exists because character-cell TUI toolkits (ratatui, cursive) cannot express real images, smooth shapes, or precise positioning — kitty's graphics protocol unlocks that, the way `kui.nvim` demonstrated but locked to Neovim as a host.

**User:** the author, solo + AI agents, consuming this as a dependency from another Rust project. A more broadly usable public crate is the long-term shape but not the immediate driver.

**Success (conceptually):** a Rust developer can create a canvas, draw text and shapes into it, place and move sprites over a preserved background, carve out multiple independent viewport regions within one terminal, zoom a canvas, and work with canvases larger than the visible terminal — all rendered live via the kitty graphics protocol.

See [docs/intent/kitty-graphics-ui-toolkit.md](intent/kitty-graphics-ui-toolkit.md) for the full confirmed intent this spec implements.

## Tech Stack

- **Language:** Rust, latest stable toolchain, 2021 edition, no nightly-only features
- **Structure:** Cargo workspace (not a single crate) — a backend-agnostic core plus swappable rendering backends behind a trait, so future backends (notcurses, sixel, ctx.graphics-style) can be added without touching core drawing logic
- **v0 backend:** kitty graphics protocol only, implemented in pure Rust (no C FFI), on top of the [`kittage`](https://github.com/itsjunetime/kittage) crate for protocol encoding rather than hand-rolling escape-sequence construction. `kittage` was adopted after the initial hand-rolled implementation (spec.md's original stance had excluded it, reading it only as a reference) surfaced enough real protocol subtleties in review (cursor movement, image/placement identity, quiet-mode responses) that the case for a maintained, broader-coverage encoder outweighed the original zero-dependency goal. `little-kitty` remains unused: it pulls in `crossterm` and its full terminal-I/O stack (~20 transitive crates) for response-reading we don't need, versus kittage's leaner default footprint (~16 crates, no crossterm required).
- **Design inspiration, not a dependency:** [notcurses](https://github.com/dankamongmen/notcurses) — its plane/visual model informed the viewport-region and compositing design, evaluated and explicitly rejected as a v0 dependency (C FFI cost, system-install requirement, and object-model mismatch outweigh its multi-protocol benefit at this stage)
- **Dev UX inspiration:** [ratatui](https://github.com/ratatui/ratatui) — its `Terminal<Backend>` + `Frame` + `draw(|frame| { ... })` immediate-mode render loop and composable `Widget` trait inspire the shape of guiltty's top-level API, given how well-known this UX is to Rust TUI developers
- **Color model:** RGBA8 throughout (canvas pixels, shape fills, sprite bitmaps)
- **Coordinate system:** pixel-addressable, origin top-left
- **Rasterization:** CPU-side software rasterizer producing a pixel buffer per canvas, shipped to the terminal as an image via the kitty protocol. No GPU acceleration in v0 (left open for later, not ruled out)
- **License:** MIT

## Commands

```
Build:        cargo build --workspace
Test:         cargo test --workspace
Lint:         cargo clippy --workspace --all-targets -- -D warnings
Format:       cargo fmt --all
Format check: cargo fmt --all -- --check
Run example:  cargo run -p guiltty-examples --bin <example-name>
```

## Project Structure

```
Cargo.toml              → workspace manifest
crates/
  guiltty-core/         → backend-agnostic canvas, shapes, text, sprites, regions, zoom/scroll logic; defines the Backend trait
  guiltty-kitty/        → kitty graphics protocol backend implementing Backend (escape-sequence encoding/transmission)
  guiltty/              → facade crate: re-exports core API + default (kitty) backend for consumers
examples/               → runnable demo binaries exercising the full v0 feature set in a real terminal
tests/                  → cross-crate integration tests
docs/
  intent/               → confirmed-intent documents (interview-me outputs)
  spec.md               → this file
tasks/                  → plan.md and todo.md (populated in the Plan/Tasks phases)
```

## Code Style

Standard Rust idioms: `snake_case` for functions/variables, `CamelCase` for types, `rustfmt` defaults, `clippy` clean with no warnings suppressed without a documented reason. Public API returns `Result<T, guiltty_core::Error>` rather than panicking; panics are reserved for programmer-error invariants (e.g., an out-of-bounds internal index), never for recoverable conditions like a failed terminal write.

Illustrative target shape for the core API (not yet implemented), following ratatui's `Terminal`/`Frame`/`draw()` immediate-mode pattern. The `event::read()`/`Event::Key` calls below are illustrative of an application-level input loop (e.g. via `crossterm` or similar) driving *when* to redraw — guiltty itself does not provide mouse/keyboard event handling in v0, per the Boundaries section below.

```rust
let mut terminal = guiltty::init()?; // Terminal<KittyBackend>
terminal.set_canvas_size(800, 600)?;
terminal.set_zoom(1.5);

let mut sprite = Sprite::new(Bitmap::from_file("ship.png")?, Point::new(50, 50));

loop {
    terminal.draw(|frame| {
        frame.render_text("hello", Point::new(10, 10), &TextStyle::default());
        frame.render_shape(Shape::circle(Point::new(100, 100), 40), Fill::solid(Color::rgb(255, 0, 0)));
        frame.render_sprite(&sprite);

        let region = frame.viewport(Rect::new(0, 0, 400, 300));
        region.render_shape(Shape::rect(Point::new(0, 0), 100, 50), Fill::solid(Color::rgb(0, 255, 0)));
    })?;

    sprite.move_to(Point::new(60, 50));

    if matches!(event::read()?, Event::Key(_)) {
        break;
    }
}
```

## Testing Strategy

- **Unit tests** (in `guiltty-core`): shape rasterization correctness (pixel buffer assertions against expected output), canvas/sprite/region state transitions, zoom/scroll math — these run in CI with no terminal required.
- **Protocol tests** (in `guiltty-kitty`): byte-level assertions that canvas state produces the correct kitty escape-sequence encoding, without requiring an actual terminal.
- **Manual/visual verification**: example binaries in `examples/` are run by hand in a real kitty-protocol-compatible terminal as the acceptance check for actual rendering — this is not automated in v0, and is documented as a manual checklist rather than a CI gate. See [docs/spec-kitty-e2e.md](spec-kitty-e2e.md) for a planned automated tier (protocol-acceptance testing against kitty itself, the protocol's reference implementation, run headlessly under Xvfb as a black-box dev-dependency) that narrows but does not eliminate this manual step.
- **Coverage expectation:** core logic (rasterizer, canvas/sprite/region/zoom state) should be well-covered by unit tests since it's fully testable without a terminal; the kitty transmission layer is covered by protocol/encoding tests; end-to-end visual correctness stays manual for v0.

## Boundaries

- **Always:** run `cargo fmt`, `cargo clippy`, and `cargo test --workspace` before considering a task done; keep `guiltty-core` free of any kitty-specific (or any backend-specific) code — backend concerns live only in backend crates; document public API items with doc comments.
- **Ask first:** adding any new external dependency (especially anything requiring C/FFI); adding a new backend crate; changing the workspace crate boundaries defined above; changing the license.
- **Never:** let backend-specific code leak into `guiltty-core`; introduce panics on recoverable error paths in public API; commit secrets; remove a failing test without explicit approval; build mouse/event handling, interactive widgets (buttons, clickable elements), or a scriptable CLI binary interface — all explicitly out of scope for v0 per [docs/intent/kitty-graphics-ui-toolkit.md](intent/kitty-graphics-ui-toolkit.md).

## Success Criteria

v0 is done when, using only the kitty backend:

1. A `Canvas` can be created and drawn into with text.
2. Basic shapes — lines, rectangles, circles, ellipses, triangles, and arbitrary open/closed paths — can be drawn with solid color fill.
3. Sprites (movable 2D bitmaps) can be placed, moved, and re-rendered without corrupting the preserved background behind them.
4. Multiple independent viewport regions can coexist within a single terminal, each independently drawable with text + shapes.
5. A canvas can be set to a zoom level that scales its rendered output.
6. A canvas larger than the current terminal viewport can be created, drawn into beyond visible bounds, and viewed with some form of scroll/pan/clip so content isn't simply lost when the terminal is smaller than the canvas.
7. All of the above is demonstrated by at least one runnable example in `examples/` that a human can visually verify in a real kitty-compatible terminal.
8. `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all -- --check` all pass.

## Open Questions

- Exact MSRV (minimum supported Rust version) — default to "latest stable" until a reason to pin arises.
- Whether v0's example should be a specific demo app (e.g., a file browser with image thumbnails) or several small feature-focused examples — to be decided in the Plan phase.
- Exact shape of the zoom/scroll API (e.g., viewport offset + zoom factor vs. a camera abstraction) — to be resolved during planning/implementation, not blocking spec approval.
- Whether/when to publish to crates.io — out of scope for v0 planning.
