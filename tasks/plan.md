# Plan: guiltty v0 — remaining feature work

Source: [`docs/spec.md`](../docs/spec.md)'s Success Criteria.

This tracks the v0 feature work *not* tied to a GitHub issue (unlike
`tasks/issue-<n>-<slug>.md` files, which are issue-sourced). It covers only
the items that are already well-specified by `docs/spec.md` and don't need
further design work — see the roadmap discussion for why viewport regions
(#4), zoom (#5), and scroll/pan (#6) are deliberately **not** in this file yet:
their API shape is an open question in spec.md itself and needs a short
design pass (a separate `docs/design/viewport-regions-zoom-scroll.md`) before
they can be broken into tasks. Kitty-protocol image *placement/positioning*
(i.e. transmitting more than one independently-positioned image) is deferred
to that same design pass too — see the note after T1 below for why.

Quality gate (every task): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo llvm-cov --workspace --summary-only --fail-under-lines 90`.

Sequencing, by task ID (no separate "tier" labels — go straight by dependency
order below): **T1 first**, unconditionally — nothing has ever rendered to a
real terminal (`KittyBackend::present()` is a no-op stub), which blocks
visually verifying literally everything else, including Success Criterion #7.
**T2 and T3 are independent of T1 and of each other** — both can be built in
parallel with T1 or with each other. **T4 (the runnable example) is last**,
depending on T1–T3 (T3 is a soft dependency — see its own Dependencies line):
spec.md's Success Criterion #7 wants the example to demonstrate the full v0
feature set covered so far, and viewport regions/zoom/scroll (tracked
separately, see above) aren't part of this file's scope, so T4 here only
demonstrates what's covered by T1–T3.

---

- [ ] **T1 — Kitty protocol: encode and transmit a static image** 🔒

Acceptance:
- The `Backend` trait (`crates/guiltty-core/src/lib.rs`) changes from
  `fn present(&mut self) -> Result<(), Self::Error>` to a signature that
  actually receives the `Canvas` to render, e.g.
  `fn present(&mut self, canvas: &Canvas) -> Result<(), Self::Error>` — the
  current signature has no way to hand a canvas to the backend at all, and
  `KittyBackend` is a zero-sized struct storing no frame state. Document the
  chosen signature change in this task's PR description since it's a
  breaking change to the trait (acceptable pre-1.0, all crates at `0.0.0`,
  but worth calling out explicitly).
- `KittyBackend::present()` encodes the passed `Canvas`'s pixel buffer as a
  kitty graphics protocol escape sequence (APC `_G...` payload,
  base64-encoded RGBA data) and writes it to the backend's output stream,
  instead of the current `Ok(())` no-op. Transmission target should be
  injectable (e.g. write to a generic `io::Write`) so protocol tests don't
  need a real terminal.
- New unit/protocol tests in `guiltty-kitty` assert the encoded escape
  sequence's structure (control keys, base64 payload, chunking if the
  payload exceeds the protocol's per-chunk size limit) against known-good
  expected byte sequences — per spec.md's Testing Strategy "Protocol tests"
  category, no real terminal required.
- No new external dependency beyond what's needed for base64 encoding;
  adding one is a 🔒 ask-first item per spec.md's Boundaries — confirm with
  the maintainer before adding (e.g. a `base64` crate vs. hand-rolled).
- **Note on scope:** this task transmits a single full-canvas image only —
  it does not add support for placing multiple independently-positioned
  images at arbitrary terminal cell locations. There's no current use case
  for that: sprites are already composited directly into the `Canvas`'s
  pixel buffer (see `Canvas::draw_sprite`), so a single-image transmission
  already renders them correctly. The only real motivation for
  multi-image placement is future independent viewport regions (spec.md
  Success Criterion #4), which is deferred to the
  `docs/design/viewport-regions-zoom-scroll.md` design pass — placement
  will be scoped as part of that design instead of guessed at here.

Verify:
- Local: quality gate above.
- Manual: a small ad hoc smoke-test binary (can be a scratch example, doesn't
  need to be T4's real example yet) run in a real kitty-compatible terminal,
  confirming *something* visibly renders — per spec.md's Testing Strategy,
  this is the manual/visual acceptance check, not automated in v0.

Files: `crates/guiltty-core/src/lib.rs` (`Backend` trait signature), `crates/guiltty-kitty/src/lib.rs`, `crates/guiltty-kitty/Cargo.toml` (if a new dependency is approved)

Dependencies: none.

---

- [ ] **T2 — `Shape::Path` polygon fill**

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

Dependencies: none — independent of T1, doesn't touch the kitty crate.

---

- [ ] **T3 — `Bitmap::from_file` image loading** 🔒

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
- New unit tests also cover the recoverable-error paths promised above: a
  missing file and a malformed/corrupt image both return `Err(...)` rather
  than panicking, protecting the public API's documented no-panic boundary
  (spec.md's Code Style section).

Verify:
- Local: quality gate above.

Files: `crates/guiltty-core/src/lib.rs`, `crates/guiltty-core/Cargo.toml` (new dependency, pending approval), a small fixture image file (new)

Dependencies: none — independent, but the runnable example (T4) benefits from
this landing first so sprites can use a real image instead of `Bitmap::solid`.

---

- [ ] **T4 — Runnable example demonstrating T1–T3**

Acceptance:
- `examples/src/bin/placeholder.rs` is replaced (or a new example binary is
  added and the placeholder removed) with a real demo that: creates a
  `Canvas`, draws text and at least one shape of each kind covered so far
  (including a filled closed `Path` per T2), places and moves a `Sprite`
  (using a `Bitmap::from_file`-loaded image per T3 if that task has landed,
  otherwise `Bitmap::solid`), and calls `present()` (updated signature per
  T1) to actually render via `KittyBackend`.
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

Dependencies: T1, T2. T3 is a soft dependency — deliberately not hard-required,
since it pulls in a new ask-first dependency of uncertain approval timing;
T4 falls back to `Bitmap::solid` if T3 hasn't landed yet, but should use T3's
real image loading if it has.
