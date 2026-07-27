# Plan: guiltty v0 — remaining feature work

Source: [`docs/spec.md`](../docs/spec.md)'s Success Criteria.

This tracks the v0 feature work *not* tied to a GitHub issue (unlike
`tasks/issue-<n>-<slug>.md` files, which are issue-sourced). It covers only
the items that are already well-specified by `docs/spec.md` and don't need
further design work — see the roadmap discussion for why viewport regions
(#4), zoom (#5), and scroll/pan (#6) are deliberately **not** in this file yet:
their API shape is an open question in spec.md itself and needs a short
design pass (a separate `docs/design/viewport-regions-zoom-scroll.md`) before
they can be broken into tasks.

Quality gate (every task): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo llvm-cov --workspace --summary-only --fail-under-lines 90`.

Sequencing: Tier 0 first (nothing has ever rendered to a real terminal —
`KittyBackend::present()` is a no-op stub, which blocks visually verifying
literally everything else, including Success Criterion #7). Tier 1 tasks are
independent of Tier 0 and can be built in parallel. The runnable example
(Tier 4, T5 below) is deliberately last — spec.md's Success Criterion #7 wants
it to demonstrate the full v0 feature set, and viewport regions/zoom/scroll
(tracked separately, see above) aren't part of this file's scope, so T5 here
only demonstrates what's covered by T1–T4.

---

- [ ] **T1 — Kitty protocol: encode and transmit a static image** 🔒

Acceptance:
- `KittyBackend::present()` (`crates/guiltty-kitty/src/lib.rs`) encodes a
  `Canvas`'s current pixel buffer as a kitty graphics protocol escape
  sequence (APC `_G...` payload, base64-encoded RGBA data) and writes it to
  the backend's output stream, instead of the current `Ok(())` no-op.
  Transmission target should be injectable (e.g. write to a generic
  `io::Write`) so protocol tests don't need a real terminal.
- New unit/protocol tests in `guiltty-kitty` assert the encoded escape
  sequence's structure (control keys, base64 payload, chunking if the
  payload exceeds the protocol's per-chunk size limit) against known-good
  expected byte sequences — per spec.md's Testing Strategy "Protocol tests"
  category, no real terminal required.
- No new external dependency beyond what's needed for base64 encoding;
  adding one is a 🔒 ask-first item per spec.md's Boundaries — confirm with
  the maintainer before adding (e.g. a `base64` crate vs. hand-rolled).

Verify:
- Local: quality gate above.
- Manual: a small ad hoc smoke-test binary (can be a scratch example, doesn't
  need to be T5's real example yet) run in a real kitty-compatible terminal,
  confirming *something* visibly renders — per spec.md's Testing Strategy,
  this is the manual/visual acceptance check, not automated in v0.

Files: `crates/guiltty-kitty/src/lib.rs`, `crates/guiltty-kitty/Cargo.toml` (if a new dependency is approved)

Dependencies: none.

---

- [ ] **T2 — Kitty protocol: image placement/positioning**

Acceptance:
- Building on T1, `present()` (or a related method) supports placing the
  transmitted image at a specific terminal cell position, not just always at
  the cursor's current position — needed for anything beyond a single
  full-screen image (sprites, future regions).
- Protocol tests assert the placement-related escape-sequence fields
  (e.g. cursor positioning before transmission, or the protocol's placement
  keys) are correct.

Verify:
- Local: quality gate above.
- Manual: extend the T1 smoke test to draw at two different positions in one
  run, confirming both appear in the expected locations.

Files: `crates/guiltty-kitty/src/lib.rs`

Dependencies: T1.

---

- [ ] **T3 — `Shape::Path` polygon fill**

Acceptance:
- `Canvas::draw_shape` fills closed `Shape::Path` variants with `Fill::Solid`
  (currently stroke-only per the type's own doc comment in
  `crates/guiltty-core/src/lib.rs`) using a standard scanline/even-odd or
  nonzero-winding polygon fill algorithm, consistent with the existing
  `fill_triangle`/`fill_ellipse` style already in the file.
- Open (non-closed) paths remain stroke-only — filling an open path isn't
  geometrically well-defined without deciding an implicit closing edge,
  which spec.md doesn't call for.
- New unit tests assert pixel-buffer fill correctness for at least one
  convex and one concave closed path, following the existing test style
  (`draw_shape`/`fill_*` tests already in the file).

Verify:
- Local: quality gate above (llvm-cov must stay ≥90%; new fill code needs
  covering tests, not just the acceptance-criteria happy path).

Files: `crates/guiltty-core/src/lib.rs`

Dependencies: none — independent of T1/T2, doesn't touch the kitty crate.

---

- [ ] **T4 — `Bitmap::from_file` image loading** 🔒

Acceptance:
- `Bitmap` gains a constructor that loads pixel data from an image file
  (e.g. PNG) on disk, matching the illustrative
  `Bitmap::from_file("ship.png")?` call in spec.md's Code Style section.
  Returns `Result<Self, guiltty_core::Error>` per spec.md's Code Style
  (no panics on a bad/missing file).
- Requires a new external image-decoding dependency (e.g. the `image` or
  `png` crate) — this is an explicit 🔒 ask-first item per spec.md's
  Boundaries ("adding any new external dependency"). **Pause and get
  approval on the specific crate before implementing.**
- New unit test loads a small fixture image (checked into the repo, e.g.
  under a `crates/guiltty-core/tests/fixtures/` or similar) and asserts the
  resulting `Bitmap`'s dimensions/pixel data match expectations.

Verify:
- Local: quality gate above.

Files: `crates/guiltty-core/src/lib.rs`, `crates/guiltty-core/Cargo.toml` (new dependency, pending approval), a small fixture image file (new)

Dependencies: none — independent, but the runnable example (T5) benefits from
this landing first so sprites can use a real image instead of `Bitmap::solid`.

---

- [ ] **T5 — Runnable example demonstrating T1–T4**

Acceptance:
- `examples/src/bin/placeholder.rs` is replaced (or a new example binary is
  added and the placeholder removed) with a real demo that: creates a
  `Canvas`, draws text and at least one shape of each kind covered so far
  (including a filled closed `Path` per T3), places and moves a `Sprite`
  (using a `Bitmap::from_file`-loaded image per T4 if that task has landed,
  otherwise `Bitmap::solid`), and calls `present()` to actually render via
  `KittyBackend` (T1/T2).
- A human running the example in a real kitty-compatible terminal can
  visually verify canvas/text/shapes/sprite rendering, per spec.md's
  Testing Strategy manual-verification approach.
- Note: this example does **not** need to demonstrate viewport regions,
  zoom, or scroll/pan — those are out of this file's scope (see header) and
  will need their own follow-up example update once their design/tasks land.

Verify:
- Local: quality gate above.
- Manual: `cargo run -p guiltty-examples --bin <example-name>` in a real
  kitty-compatible terminal; visually confirm canvas/shapes/sprite render
  and sprite movement doesn't corrupt the background.

Files: `examples/src/bin/placeholder.rs` (replace or remove), new example binary under `examples/src/bin/`

Dependencies: T1, T2, T3; T4 is not a hard dependency (falls back to `Bitmap::solid`) but should land first if feasible.
