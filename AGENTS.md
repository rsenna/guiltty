# AGENTS.md — guiltty agent operating guide

This file tells any AI agent (or new human contributor) how to work on
`guiltty`. It is intentionally short and factual. For everything else:

- **The full v0 spec** (objective, tech stack, success criteria, boundaries)
  → [`docs/spec.md`](docs/spec.md).
- **CI/coverage gate details** → [`docs/spec-ci.md`](docs/spec-ci.md).
- **Kitty-protocol E2E verification plan** →
  [`docs/spec-kitty-e2e.md`](docs/spec-kitty-e2e.md).
- **Forward-looking designs not yet implemented** →
  [`docs/design/sprite-crate-extraction.md`](docs/design/sprite-crate-extraction.md),
  [`docs/design/turtle-geometry.md`](docs/design/turtle-geometry.md),
  [`docs/design/viewport-regions-zoom-scroll.md`](docs/design/viewport-regions-zoom-scroll.md).
- **Confirmed project intent** →
  [`docs/intent/kitty-graphics-ui-toolkit.md`](docs/intent/kitty-graphics-ui-toolkit.md).
- **Planned/in-flight work, issue-by-issue** → [`tasks/`](tasks/).

## What guiltty is, in one paragraph

`guiltty` is a Rust library for drawing real, pixel-level 2D graphics into a
terminal, using the [Kitty Graphics Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
as its first backend -- unlocking real images, smooth shapes, and precise
positioning that character-cell TUI toolkits (ratatui, cursive) can't
express. Solo project (author + AI coding agents), no fixed deadline,
consumed as a dependency from another Rust project (`iklo`) rather than
published to crates.io for now.

## What is actually implemented today

Anything not on this list is aspirational -- don't assume `docs/spec.md`'s
illustrative `Terminal`/`Frame` API sketch or its forward-looking Success
Criteria describe working code.

- **`guiltty-core`** (`crates/guiltty-core`) -- `Color` (RGBA8), `Point`,
  `Rect`; the backend-agnostic `Backend` trait; a `Canvas` supporting text
  (`draw_text`) and shape drawing (`draw_shape`: lines, rects, circles,
  ellipses, triangles, and arbitrary open/closed paths, closed ones filled
  via even-odd scanline fill). Also tracks a per-tile "region version" grid
  (`Canvas::id`/`Canvas::region_version`) so a sprite's saved footprint can
  detect staleness -- see `guiltty-sprite` below.
- **`guiltty-sprite`** (`crates/guiltty-sprite`) -- `Bitmap` (in-memory or
  loaded via `Bitmap::from_file`, PNG/JPEG/GIF/BMP) and `Sprite`: a movable
  bitmap over a `Canvas` using save/restore-under (`draw_on`, or the
  `clear_footprint`/`place`/`discard_footprint` split) so moving and
  redrawing doesn't leave a trail. Extracted out of `guiltty-core` (see
  `docs/design/sprite-crate-extraction.md`); relative movement
  (`heading`/`forward`/`turn`) and the `guiltty-turtle` crate built on top
  of it are designed but **not yet implemented**.
- **`guiltty-kitty`** (`crates/guiltty-kitty`) -- `KittyBackend`
  implementing `Backend`: encodes and transmits a `Canvas`'s pixel buffer as
  a real kitty graphics protocol escape sequence (via the
  [`kittage`](https://github.com/itsjunetime/kittage) crate), covered by
  protocol-level tests. **Not yet confirmed against a real terminal** -- no
  kitty-compatible terminal has been available in this environment; see
  `docs/spec-kitty-e2e.md`.
- **`guiltty`** (`crates/guiltty`) -- facade crate re-exporting the above
  for consumers.
- **`examples/src/bin/demo.rs`** -- exercises canvas/text/shapes/sprites
  across two rendered frames, for manual visual verification once a
  kitty-compatible terminal is available.

**Not started:** independent viewport regions, zoom, scroll/pan for
canvases larger than the terminal -- designed
(`docs/design/viewport-regions-zoom-scroll.md`) but not yet broken into
tasks or implemented. See `README.md`'s status checklist for the same
information in outward-facing form.

## Decided rules

From `docs/spec.md`'s Boundaries section (the source of truth -- restated
here only for quick reference):

- **Always:** run `cargo fmt`, `cargo clippy`, and `cargo test --workspace`
  before considering a task done; keep `guiltty-core` free of any
  backend-specific code; document public API items with doc comments.
- **Ask first:** adding any new external dependency (especially anything
  requiring C/FFI); adding a new backend crate; changing the workspace
  crate boundaries; changing the license.
- **Never:** let backend-specific code leak into `guiltty-core`; introduce
  panics on recoverable error paths in public API; commit secrets; remove a
  failing test without explicit approval; build mouse/event handling,
  interactive widgets, or a scriptable CLI binary interface (out of scope
  for v0 per `docs/intent/kitty-graphics-ui-toolkit.md`).

This repo also follows the `pull-request-process`/`map-issue-to-tasks`/
`fix-mapped-issue` skills for shipping work: worktrees (never the shared
checkout), a dedicated git/PR identity, `tasks/issue-N-*.md` task
breakdowns, and PRs that bots/the maintainer review and merge -- never
self-merged.

## Dev commands

```
Build:        cargo build --workspace
Test:         cargo test --workspace
Lint:         cargo clippy --workspace --all-targets -- -D warnings
Format:       cargo fmt --all
Format check: cargo fmt --all -- --check
Coverage:     cargo llvm-cov --workspace --summary-only --fail-under-lines 90
Run example:  cargo run -p guiltty-examples --bin <example-name>
```

Rust toolchain is pinned via [mise](https://mise.jdx.dev/) (`mise.toml`).
CI (`.github/workflows/ci.yml`) runs fmt/clippy/coverage on every PR and
push to `main`; the 90% line-coverage floor is enforced there too (see
`docs/spec-ci.md`).

## Where things live

```
Cargo.toml              -> workspace manifest
repo.toml               -> repo metadata (stage, description) for the cross-repo repo-standard convention
crates/
  guiltty-core/         -> Canvas, Color, Point, Rect, Shape, Backend trait, region-version tile grid
  guiltty-sprite/       -> Bitmap, Sprite (draw_on/clear_footprint/place/discard_footprint)
  guiltty-kitty/        -> KittyBackend (kitty graphics protocol encoding/transmission)
  guiltty/              -> facade crate re-exporting the above for consumers
examples/               -> runnable demo binaries
docs/
  intent/               -> confirmed-intent documents
  spec.md, spec-ci.md, spec-kitty-e2e.md -> specs (see links above)
  design/               -> forward-looking designs, not yet implemented
tasks/                  -> issue-<n>-<slug>.md task breakdowns, plan.md/plan-kitty-e2e.md for non-issue-sourced v0 work
```

`specs/`/`specs/decisions/` (new spec-kit-format work and ADRs) and
`.specify/` (spec-kit tooling) are part of the repo-standard `in-progress`
tier this repo is adopting (issue #39) but may not exist yet depending on
which of that issue's tasks have landed by the time you're reading this --
check for them directly rather than assuming this map is exhaustive.
