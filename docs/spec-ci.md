# Spec: CI quality gates

## Objective

This repo has no CI at all yet — `cargo fmt`/`clippy`/`test` only ever run locally, on whoever remembers to run them. This spec stands up CI from scratch and adds two kinds of automated, non-LLM static analysis on top of it: test coverage measurement (with a minimum enforced once Task 2 lands) and code-complexity linting.

**Why now:** review threads across recent PRs (#4, #5, #6) repeatedly caught the same classes of bug (integer overflow, missing bounds-clipping) after the fact, across several review rounds each. None of that would have been prevented by coverage or complexity lints specifically, but the absence of *any* CI means nothing is checked automatically before a human/bot ever looks at a diff — not even `cargo test`. Coverage and complexity are the two cheapest, highest-signal, non-LLM checks to add on top of that baseline.

## Current baseline (measured 2026-07-23, via `cargo llvm-cov --workspace`)

| Metric | Value |
|---|---|
| Lines | 90.96% (542 total, 49 missed) |
| Functions | 98.28% (58 total, 1 missed) |
| Regions | 85.62% (1120 total, 161 missed) |

`guiltty-kitty` is 100% (a stub crate). The gaps in `guiltty-core` are reasonable, not alarming: `Error`'s `Display` impl is never formatted in a test, several `font` module glyph-table branches aren't individually exercised (only a handful of the 37 characters), and a few defensive early-return branches in `liang_barsky_clip`/`fill_ellipse` (e.g. "line parallel to a boundary") aren't hit by any current test.

## Tooling

- **Coverage:** [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) — LLVM source-based coverage, actively maintained, used by rust-lang itself. Pinned in this repo's `mise.toml` (`cargo:cargo-llvm-cov`) so local and CI use the identical version.
- **CI:** GitHub Actions (this repo is hosted on GitHub; no other CI system is in use anywhere in the project).
- **Complexity lints:** two specific `clippy` lints — `too_many_lines` and `excessive_nesting` (stable Rust, no nightly/new dependency needed) — enabled via `[workspace.lints.clippy]`, with thresholds set in a new `clippy.toml`. `cognitive_complexity` is deliberately excluded: it's a `restriction`-group lint whose own docs recommend `too_many_lines`/`excessive_nesting` instead.

## Metric and threshold

- **Metric:** line coverage only (not regions/branches) — simplest to reason about in a CI failure message, the most commonly used metric for this kind of gate.
- **Threshold:** 90% lines — essentially today's measured level (90.96%). Because the baseline sits slightly above this floor, the gate limits aggregate coverage regressions rather than catching every individual undertested addition immediately; tightening it further is left as future tuning once more headroom is available.

## Tasks

1. **Stand up CI + report coverage (no gate yet).** A GitHub Actions workflow running `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo llvm-cov --workspace --summary-only` (which also runs the full test suite; coverage is computed and printed in the CI log, not yet enforced). Also adds `clippy.toml` with the complexity-lint thresholds and enables `too_many_lines`/`excessive_nesting` via `[workspace.lints.clippy]`. Landing the gate before ever seeing it run for real in CI risks having to immediately re-tune it if CI's numbers differ even slightly from local (different LLVM/toolchain build, etc.) — this task lets that be observed first.
2. **Enforce the coverage gate.** ✅ done — Once task 1's workflow ran for real and its measured number was confirmed stable, `--fail-under-lines 90` was added to the coverage step so a PR that drops line coverage below 90% fails CI.

## Success Criteria

1. A GitHub Actions workflow runs on every PR and push to `main`, executing fmt/clippy/test/coverage.
2. `clippy.toml` exists with complexity-lint thresholds; `cargo clippy` in CI includes the `too_many_lines`/`excessive_nesting` complexity lints and stays green against current code (or current code is adjusted to satisfy them).
3. Task 2: CI fails a PR whose line coverage drops below 90%.
4. Both tasks pass the project's existing quality gate locally before being pushed (this spec doesn't relax that).

## Open Questions

- Whether to also gate on `cargo audit`/dependency-vulnerability scanning — out of scope for this spec (no dependencies exist yet beyond the workspace's own crates), revisit once external dependencies are added.
