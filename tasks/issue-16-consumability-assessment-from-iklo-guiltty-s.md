# Tasks: Issue #16 — Consumability assessment from iklo (guiltty's first/main planned client)

Source: https://github.com/rsenna/guiltty/issues/16
Enrichment: issue comment on #16.

Quality gate (code-affecting tasks): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo llvm-cov --workspace --summary-only --fail-under-lines 90` (CI enforces this floor as of #15; run locally with the same flag to match).

This issue is a consumability self-assessment, not a bug/feature request. Its
own bottom line is "no action needed from iklo's side right now." The tasks
below cover only the cheap, low-risk documentation/metadata fixes the
assessment surfaced — they deliberately exclude the big rock (real kitty
protocol encoding in `KittyBackend::present()`, a richer typed `Error` enum
(beyond the current single `Backend(String)` variant), a working end-to-end
example), which is already `docs/spec.md`'s tracked v0 work and isn't newly
scoped by this issue.

---

- [ ] **T1 — Fix README's stale "Status" section**

Acceptance:
- `README.md`'s status section no longer claims "no Canvas, no shapes, no
  sprites" — it accurately states that `Canvas`, `Shape` (Line/Rect/Circle/
  Ellipse/Triangle/Path), and `Sprite` (with save/restore-under) are
  implemented and unit-tested in `guiltty-core`.
- The "still missing" list is narrowed to what's actually still missing: real
  kitty-protocol wire encoding in `KittyBackend::present()`, independent
  viewport regions, zoom, scroll/pan for oversized canvases, and a working
  end-to-end example.

Verify:
- Read the updated README section against `crates/guiltty-core/src/lib.rs` and
  `crates/guiltty-kitty/src/lib.rs` to confirm every claim matches the code.
- Local: same quality-gate commands as above (docs-only change, so `fmt`/
  `clippy` are the relevant ones; no need to re-run `llvm-cov`).

Files: `README.md`

Dependencies: none.

---

- [ ] **T2 — Declare an MSRV**

Acceptance:
- `rust-version` is set (workspace-level in the root `Cargo.toml`'s
  `[workspace.package]`, inherited by each crate via `rust-version.workspace =
  true`) to a value consistent with `mise.toml`'s pinned dev toolchain
  (`1.96.0`), unless the maintainer decides during review that a lower floor
  is intentional.
- `cargo build --workspace` and `cargo test --workspace` still pass with the
  declared MSRV as the active toolchain.

Verify:
- `cargo metadata --format-version 1 | jq '.packages[] | select(.name | startswith("guiltty")) | .rust_version'` shows the new value for every workspace crate.
- Local: same quality-gate commands as above.

Files: `Cargo.toml`, `crates/guiltty-core/Cargo.toml`, `crates/guiltty-kitty/Cargo.toml`, `crates/guiltty/Cargo.toml`, `examples/Cargo.toml`

Dependencies: none.
