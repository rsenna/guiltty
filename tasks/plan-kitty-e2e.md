# Plan: kitty-based E2E verification

Source: [`docs/spec-kitty-e2e.md`](../docs/spec-kitty-e2e.md)'s Success Criteria.

This tracks the E2E-verification work described in `docs/spec-kitty-e2e.md` —
an Xvfb-backed, real-kitty-binary test harness that automates the
"protocol-acceptance" tier of verification `tasks/plan.md`'s T1 and T4 have
been missing since their first PRs (no kitty-compatible terminal has been
available in this environment). Task IDs here are prefixed `K` (not `T`) to
avoid collision with `tasks/plan.md`'s own T1–T4 when the two files are
referenced together.

Quality gate (code-affecting tasks): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo llvm-cov --workspace --summary-only --fail-under-lines 90`. Per the spec's Boundaries, this E2E suite itself must stay **separate from and non-blocking to** that gate — it is `#[ignore]`'d and/or run from a dedicated CI job, never folded into the default `cargo test --workspace` run.

Sequencing: **K1 first** — nothing else can run without a provisioned kitty binary. **K2 depends on K1** only in the sense that it needs a kitty binary to test against locally while developing it, not a hard code dependency. **K3 depends on K1 and K2** — it orchestrates both. **K4 (the documented manual procedure) is independent** of K1–K3 and can be written any time; it's pure documentation of a manual, non-virtualized workflow.

---

- [ ] **K1 — Provision a pinned, signature-verified kitty binary (Linux + macOS)** 🔒

Acceptance:
- A documented, scripted way (e.g. a `scripts/fetch-kitty.sh` or similar) fetches a specific, pinned kitty release version for Linux (x86_64/arm64) and macOS, verifying the download against kitty's published signature before use — no building from source.
- BSD is explicitly documented as out of scope for this automated path (use the platform's package manager instead, e.g. FreeBSD's `pkg install kitty`), matching the same gap already accepted for WezTerm in an earlier draft.
- kitty is wired in as dev-only tooling: no `Cargo.toml` entry in any guiltty crate, consumed purely as an external subprocess binary.

Verify:
- Run the fetch script on a clean checkout (or CI) and confirm it produces a working `kitty` binary whose version matches the pinned value.
- Local: quality gate above is unaffected (this task touches no Rust code).

Files: new provisioning script (location TBD — e.g. `scripts/fetch-kitty.sh`), possibly `mise.toml` if `mise`'s registry turns out to have a kitty entry (check before writing a custom script).

Dependencies: none.

🔒 Ask first: exact kitty version to pin, and the provisioning mechanism itself (pinned download script vs. a `mise` entry, if one exists) — both explicitly called out as ask-first in the spec's Boundaries.

---

- [ ] **K2 — Test harness binary: present a Canvas, capture kitty's accept/reject response**

Acceptance:
- A small, dedicated binary (not a `#[test]` — it must run as a standalone process inside a real kitty window, spawned via kitty's `launch` remote-control action) builds a `Canvas` exercising `present()`, transmits it using `Verbosity::ErrorsOnly` (or `All`) instead of `Silent`, and reads kitty's response from its own stdin (its window's PTY) — not via any remote-control text-capture action, since APC responses aren't on-screen "text" content.
- Writes a plain PASS/FAIL result to a **unique temporary file path** passed in as an argument (never a fixed/well-known path — avoids races between concurrent runs and stale-result false positives).
- Reuses kittage's `Action::execute` (or documents why a different read strategy was chosen instead) per the spec's open question on this point.

Verify:
- Manually launch the harness inside a real (or Xvfb-backed) kitty window and confirm it writes a correct PASS/FAIL result file for both an accepted and a deliberately malformed transmission.
- Local: quality gate above (the harness binary itself should be fmt/clippy-clean; full coverage isn't expected for a manual-launch-only binary, but keep it simple enough not to need much).

Files: new binary target — exact crate/location TBD during implementation (e.g. a `[[bin]]` in `guiltty-kitty` gated as dev-only, or a small dedicated harness crate under `tests/`).

Dependencies: none at the code level (can be written before K1 lands), but needs a kitty binary (K1) to actually exercise it against.

---

- [ ] **K3 — Headless orchestration: Xvfb + kitty + `#[ignore]`'d test** 🔒

Acceptance:
- A `#[test]`, `#[ignore]`'d by default (needs the kitty binary and Xvfb present — not part of the fast unit-test loop), that: starts Xvfb, starts kitty configured for software rendering with `allow_remote_control=socket-only --listen-on unix:<unique-per-run-path>` (a fresh path per invocation, e.g. including the test's own process id, to avoid colliding with an unrelated kitty instance or a concurrent test run), issues the `launch` remote-control action to spawn K2's harness inside that kitty window, polls the harness's result file against a **hard deadline** (fails rather than hanging if no response ever arrives), and tears down Xvfb/kitty/temp files **unconditionally** — an RAII-style guard (kill on `Drop`), not only on the success path, so a panic or early-return can't leak processes or temp state.
- Confirmed runnable on demand (`cargo test --workspace -- --ignored` or similar), separate from and non-blocking to the existing fmt/clippy/90%-coverage gate; not wired into any required CI check by this task (a follow-up CI-integration task, if wanted, is separate and would itself be a 🔒 CI-change gate).
- Documented as **Linux-only** (Xvfb is X11-specific, no macOS equivalent) — macOS coverage stays manual, tracked by K4.

Verify:
- Run the ignored test locally (Linux, with Xvfb + K1's kitty binary available) and confirm it passes against a known-good `present()` call and fails against a deliberately broken one (e.g. a truncated payload).
- Local: quality gate above (the orchestrating test code itself must be fmt/clippy-clean; the `#[ignore]`'d test itself is exempt from the 90% coverage requirement by nature of not running in the default suite, but don't let that become an excuse to leave it untested against both pass and fail cases as noted above).

Files: new test file (location TBD — e.g. `crates/guiltty-kitty/tests/e2e_kitty.rs` or a workspace-level `tests/` integration test), any new dev-dependency needed for process orchestration (subject to the ask-first gate below if one is added).

Dependencies: K1, K2.

🔒 Ask first: any new dev-dependency needed for process/Xvfb orchestration beyond `std::process::Command`.

---

- [ ] **K4 — Document the manual pixel-level visual verification procedure**

Acceptance:
- A concrete, run-it-yourself set of steps (in `docs/` or this repo's README) for verifying `present()`'s actual rendered output by eye in a real, non-virtualized kitty instance — replacing the "no terminal available in this environment" caveat that's shipped with every kitty-facing PR so far (T1, T4) with something a human with a real terminal can actually follow.
- Explicitly covers macOS and BSD as the only verification path available to them (no Xvfb equivalent on either platform).
- Cross-references `tasks/plan.md`'s T1/T4 so a future reader knows this is the promised follow-up to their still-open manual-verification caveat.

Verify:
- A human (not this agent — no kitty-compatible terminal available here) follows the documented steps against the T4 example and confirms canvas/shapes/sprite render correctly and sprite movement doesn't corrupt the background.
- Local: quality gate above is unaffected (docs-only change).

Files: new doc (e.g. `docs/manual-kitty-verification.md`), possibly a cross-reference edit to `tasks/plan.md` and/or `README.md`.

Dependencies: none — independent of K1–K3, can be written any time.
