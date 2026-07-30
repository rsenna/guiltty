# Plan: kitty-based E2E verification

Source: [`docs/spec-kitty-e2e.md`](../docs/spec-kitty-e2e.md)'s Success Criteria.

This tracks the E2E-verification work described in `docs/spec-kitty-e2e.md` —
an Xvfb-backed, real-kitty-binary test harness that automates the
"protocol-acceptance" tier of verification `tasks/plan.md`'s T1 and T4 have
been missing since their first PRs (no kitty-compatible terminal has been
available in this environment). Task IDs here are prefixed `K` (not `T`) to
avoid collision with `tasks/plan.md`'s own T1–T4 when the two files are
referenced together.

Quality gate (code-affecting tasks): same commands as [`tasks/plan.md`](plan.md)'s Quality gate line — not restated here to avoid the two files drifting out of sync. Per the spec's Boundaries, this E2E suite itself must stay **separate from and non-blocking to** that gate — it is `#[ignore]`'d and/or run from a dedicated CI job, never folded into the default `cargo test --workspace` run.

Sequencing: **K1 first** — nothing else can run without a provisioned kitty binary. **K2 depends on K1** only in the sense that it needs a kitty binary to test against locally while developing it, not a hard code dependency. **K3 depends on K1 and K2** — it orchestrates both. **K4 (the documented manual procedure) is independent** of K1–K3 and can be written any time; it's pure documentation of a manual, non-virtualized workflow.

---

- [ ] **K1 — Provision a pinned, signature-verified kitty binary (Linux + macOS)** 🔒

Acceptance:
- A documented, scripted way (e.g. a `scripts/fetch-kitty.sh` or similar) fetches a specific, pinned kitty release version for Linux (x86_64/arm64) and macOS, verifying the download against kitty's published signature before use — no building from source.
- BSD is explicitly documented as out of scope for this automated path (use the platform's package manager instead, e.g. FreeBSD's `pkg install kitty`), matching the same gap already accepted for WezTerm in an earlier draft.
- kitty is wired in as dev-only tooling: no `Cargo.toml` entry in any guiltty crate, consumed purely as an external subprocess binary.

Verify:
- Run the fetch script on a clean checkout (or CI) and confirm it produces a working `kitty` binary whose version matches the pinned value.
- **Negative case, required, not optional:** run the script against a deliberately tampered/corrupted download (e.g. flip a byte in a local copy before signature verification, or point it at a mismatched signature file) and confirm it fails loudly and does **not** leave a "verified" binary in place — a script that only ever exercises the happy path can't actually prove the signature check does anything.
- Local: quality gate above is unaffected (this task touches no Rust code).

Files: `scripts/fetch-kitty.sh` (new); possibly `mise.toml` instead, if `mise`'s registry turns out to have a kitty entry (check before writing the custom script — if one exists, use it and drop the script from this task's scope).

Dependencies: none.

🔒 Ask first: exact kitty version to pin, and the provisioning mechanism itself (pinned download script vs. a `mise` entry, if one exists) — both explicitly called out as ask-first in the spec's Boundaries.

---

- [ ] **K2 — Test harness binary: present a Canvas, capture kitty's accept/reject response**

Acceptance:
- `present()` hard-codes `Verbosity::Silent` and `write_transmit_to` (fire-and-forget, no response read) — neither is response-capable, so a harness that needs to read kitty's reply can't just call `present()` as-is. Rather than reimplementing the `kittage::Action::TransmitAndDisplay` construction separately in the harness (which would test a parallel reimplementation, not `present()`'s actual production logic — flagged by review), refactor `crates/guiltty-kitty/src/lib.rs` to extract that construction into a shared, verbosity-parameterized helper (e.g. `fn build_transmit_action(canvas: &Canvas, id: NonZeroU32) -> Action`) that both `present()` (called with `Silent`) and the new harness (called with a response-producing verbosity, see next bullet) use — so the harness genuinely exercises production code, not a lookalike.
- The harness calls that shared helper, then executes it with **`Verbosity::All`**, not `ErrorsOnly` — `ErrorsOnly` suppresses kitty's `OK` response entirely, so an accepted (correct) transmission would give `Action::execute` nothing to read, hit the K3 hard deadline, and get misreported as a failure. `All` is required specifically so the *success* case has something to observe, not just the error case (flagged by review).
- The harness reads kitty's response from its own stdin (its window's PTY) via `Action::execute` — not via any remote-control text-capture action, since APC responses aren't on-screen "text" content.
- Writes a plain PASS/FAIL result to a **unique temporary file path** passed in as an argument (never a fixed/well-known path — avoids races between concurrent runs and stale-result false positives).
- If prototyping shows `Action::execute` isn't suitable (e.g. its blocking-read strategy doesn't fit this harness's needs), document the alternative read strategy actually used and why, per the spec's open question on this point.
- The stdin-response-to-PASS/FAIL interpretation logic is factored out into a plain function taking raw bytes (not requiring an actual kitty process or PTY to invoke), specifically so it can be unit-tested in isolation — not just exercised end-to-end.

Verify:
- **Unit tests (required, run in the default `cargo test --workspace`, not gated behind `#[ignore]`):** the response-interpretation function above, fed fixture byte sequences for kitty's documented OK response, an error response, and a truncated/malformed response — asserting PASS/FAIL/ambiguous-as-appropriate for each, without needing a real kitty process.
- Manually launch the full harness inside a real (or Xvfb-backed) kitty window and confirm it writes a correct PASS/FAIL result file for both an accepted and a deliberately malformed transmission (end-to-end confirmation that the unit-tested logic above is actually wired up correctly).
- Local: quality gate above (the harness binary itself should be fmt/clippy-clean; the extracted response-interpretation function should be covered by the standing 90% line-coverage gate like any other unit-testable code, even though the binary's process-orchestration glue around it isn't).

Files: `examples/src/bin/kitty_e2e_harness.rs` (new) — reuses the existing `guiltty-examples` crate's binary-target convention rather than adding a new dedicated crate; `crates/guiltty-kitty/src/lib.rs` (small internal refactor — extract the shared action-construction helper described above; `present()`'s public signature/behavior is unchanged).

Dependencies: none at the code level (can be written before K1 lands), but needs a kitty binary (K1) to actually exercise it against.

---

- [ ] **K3 — Headless orchestration: Xvfb + kitty + `#[ignore]`'d test** 🔒

Acceptance:
- A `#[test]`, `#[ignore]`'d by default (needs the kitty binary and Xvfb present — not part of the fast unit-test loop), that: starts Xvfb on a **dynamically allocated display number** (not a fixed `:99`-style number — pick/probe a free display per invocation so concurrent runs or a dev's own already-running X server don't collide), starts kitty configured for software rendering (set `LIBGL_ALWAYS_SOFTWARE=1` in the child process's environment so kitty's GPU-rendering path initializes against Xvfb's virtual framebuffer without real hardware acceleration) with `allow_remote_control=socket-only --listen-on unix:<unique-per-run-path>` (a fresh socket path per invocation, e.g. including the test's own process id, to avoid colliding with an unrelated kitty instance or a concurrent test run), issues the `launch` remote-control action to spawn K2's harness inside that kitty window, polls the harness's result file against a **hard deadline** (fails rather than hanging if no response ever arrives), and tears down Xvfb/kitty/temp files **unconditionally** — an RAII-style guard (kill on `Drop`), not only on the success path, so a panic or early-return can't leak processes or temp state.
- Confirmed runnable on demand (`cargo test --workspace -- --ignored` or similar), separate from and non-blocking to the existing fmt/clippy/90%-coverage gate; not wired into any required CI check by this task (a follow-up CI-integration task, if wanted, is separate and would itself be a 🔒 CI-change gate).
- Documented as **Linux-only** (Xvfb is X11-specific, no macOS equivalent) — macOS coverage stays manual, tracked by K4.
- The RAII guard's cleanup behavior is implemented so it can be exercised **without** actually spawning Xvfb/kitty (e.g. the guard owns generic `Child` handles/kill-on-`Drop` logic that can be pointed at any child process), specifically so its unconditional-cleanup claim is independently testable rather than only ever observed as a side effect of the full integration run.
- `#[ignore]` only skips *execution* in the default `cargo test --workspace` run — it does **not** exclude the ignored test's compiled source from `cargo llvm-cov`'s line-coverage accounting, so an untested orchestration test can silently drag the workspace below the standing 90% floor (flagged by review). This task must either (a) add an explicit coverage exclusion for `crates/guiltty-kitty/tests/e2e_kitty.rs` to the `cargo llvm-cov` invocation used for the gate (e.g. `--ignore-filename-regex`), documented in this repo's CI/quality-gate docs, or (b) keep the orchestration file itself thin enough that its only non-trivial logic is the RAII guard already covered by the unit tests above, with no additional uncovered branches of consequence. Pick one and document the choice in this task's PR.

Verify:
- **Unit test (required, run in the default `cargo test --workspace`, not gated behind `#[ignore]`):** construct the RAII guard around one or more short-lived dummy child processes (not real Xvfb/kitty), then drop it via an early return **and** via a deliberate panic (`std::panic::catch_unwind` around the panicking path) and assert in both cases that the dummy processes are no longer running afterward — this is what actually proves "unconditional," not just the happy-path integration test below.
- Run the ignored integration test locally (Linux, with Xvfb + K1's kitty binary available) and confirm it passes against a known-good transmission and fails against a deliberately broken one (e.g. a truncated payload) — and run it twice concurrently to confirm the dynamic display/socket allocation actually prevents the two runs from colliding.
- Local: quality gate above (the orchestrating test code itself must be fmt/clippy-clean; the RAII guard's own unit tests above count toward the standing 90% line-coverage gate; the `#[ignore]`'d integration test itself is exempt from that gate by nature of not running in the default suite, but don't let that become an excuse to skip the unit-testable cleanup-on-panic case above).

Files: `crates/guiltty-kitty/tests/e2e_kitty.rs` (new); any new dev-dependency needed for process orchestration (subject to the ask-first gate below if one is added).

Dependencies: K1, K2.

🔒 Ask first: any new dev-dependency needed for process/Xvfb orchestration beyond `std::process::Command`.

---

- [ ] **K4 — Document the manual pixel-level visual verification procedure**

Acceptance:
- A concrete, run-it-yourself set of steps (in `docs/` or this repo's README) for verifying `present()`'s actual rendered output by eye in a real, non-virtualized kitty instance — replacing the "no terminal available in this environment" caveat that's shipped with every kitty-facing PR so far (T1, T4) with something a human with a real terminal can actually follow.
- Explicitly covers macOS and BSD as the only verification path available to them (no Xvfb equivalent on either platform).
- Cross-references `tasks/plan.md`'s T1/T4 so a future reader knows this is the promised follow-up to their still-open manual-verification caveat.
- `examples/src/bin/demo.rs` (T4) currently sends both `present()` calls back-to-back and exits immediately, so a human running it may only ever see the final frame and can't reliably confirm the initial frame rendered or that the sprite moved without corrupting the background (flagged by review). This task's scope includes a small patch to that demo adding an explicit pause between the two `present()` calls (e.g. block on a keypress, or a short `std::thread::sleep`) so both frames are separately observable — this is the one piece of this otherwise docs-only task that touches Rust code.

Verify:
- A human (not this agent — no kitty-compatible terminal available here) follows the documented steps against the (now pausing) T4 example and confirms canvas/shapes/sprite render correctly and sprite movement doesn't corrupt the background, having actually seen both frames rather than only the final state.
- Local: quality gate above (the small `demo.rs` pause edit should be fmt/clippy-clean like the rest of the example).

Files: `docs/manual-kitty-verification.md` (new); `examples/src/bin/demo.rs` (small edit — add an inter-frame pause); a cross-reference edit to `tasks/plan.md` and/or `README.md`.

Dependencies: none — independent of K1–K3, can be written any time.
