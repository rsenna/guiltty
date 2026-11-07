# Spec: CI quality gates

## Objective

**Status: implemented.** This spec records the rollout of GitHub Actions CI, Rust format/lint/test/coverage gates, and complexity linting. At the time it was drafted, those checks ran only locally; the workflow now runs on pull requests and pushes to `main`, enforcing the 90% line-coverage floor.

**Original motivation:** review threads across PRs #4, #5, and #6 repeatedly caught integer-overflow and bounds-clipping bugs after the fact. The goal was to catch such regressions earlier with automated, non-LLM checks.

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

1. **Stand up CI + report coverage.** ✅ done — GitHub Actions runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, the Speckit bootstrap shell regression suite, and `cargo llvm-cov --workspace --summary-only` (which also runs the full Rust test suite). Coverage is reported in CI.
2. **Enforce the coverage gate.** ✅ done — Once task 1's workflow ran for real and its measured number was confirmed stable, `--fail-under-lines 90` was added to the coverage step so a PR that drops line coverage below 90% fails CI.

## Success Criteria

1. A GitHub Actions workflow runs on every PR and push to `main`, executing fmt/clippy/test/coverage and the Speckit bootstrap shell regression suite.
2. `clippy.toml` exists with complexity-lint thresholds; `cargo clippy` in CI includes the `too_many_lines`/`excessive_nesting` complexity lints and stays green against current code (or current code is adjusted to satisfy them).
3. Task 2: CI fails a PR whose line coverage drops below 90%.
4. Both tasks pass the project's existing quality gate locally before being pushed (this spec doesn't relax that).

## Open Questions

- Whether to also gate on `cargo audit`/dependency-vulnerability scanning — out of scope for this spec (no dependencies exist yet beyond the workspace's own crates), revisit once external dependencies are added.
