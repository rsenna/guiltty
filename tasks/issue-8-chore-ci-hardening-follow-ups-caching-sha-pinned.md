# Tasks: Issue #8 — CI hardening follow-ups (caching, SHA-pinned actions)

Source: https://github.com/rsenna/guiltty/issues/8
Enrichment: issue comment on #8.

Quality gate (every task): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo llvm-cov --workspace --summary-only --fail-under-lines 90` (CI enforces this floor as of #15; run locally with the same flag to match).

---

- [x] **T1 — Pin existing actions to reviewed commit SHAs** 🔒 — ✅ merged (#9)

Acceptance:
- `.github/workflows/ci.yml`'s `actions/checkout@v4` step references SHA `11d5960a326750d5838078e36cf38b85af677262` with trailing comment `# v4.4.0`.
- The `jdx/mise-action@v4` step references SHA `9e7f7633ff6f6d6048a9418a68d48f288f50eb14` with trailing comment `# v4.2.3`.
- No other workflow behavior changes beyond the SHA pins themselves and the runner-OS pin (`ubuntu-latest` → `ubuntu-24.04`) added during review, in the same reproducibility spirit.

Verify:
- Push the branch and confirm the `quality` job on GitHub Actions completes successfully (fmt/clippy/llvm-cov all green) — a bad SHA fails the checkout/tool-install step immediately.
- Local: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo llvm-cov --workspace --summary-only --fail-under-lines 90`.

Files: `.github/workflows/ci.yml`

Dependencies: none.

---

- [x] **T2 — Add Cargo/build caching to the quality job** 🔒 — ✅ merged (#11)

Acceptance:
- `Swatinem/rust-cache` (pinned to SHA `23869a5bd66c73db3c0ac40331f3206eb23791dc` with trailing comment `# v2.9.1`) is added to `.github/workflows/ci.yml`, placed after `cargo fmt --check` and before `cargo clippy` (moved here per #11 review feedback: fmt is cheap and needs no cache, so it should fail fast without waiting on cache restore).
- A second CI run on the same branch with no `Cargo.lock` change restores the cache (visible in the run's "Cache restored" log line) and the registry-download/full-recompile portion of the job is visibly shorter than the cold-cache baseline.
- `quality` job still runs `fmt`, `clippy`, and `llvm-cov` exactly as before — caching must not change what runs, only how long it takes.

Verify:
- Push twice (or push then re-trigger) and compare job duration/logs between the cold run and the cache-hit run.
- Local: same quality-gate commands as T1.

Files: `.github/workflows/ci.yml`

Dependencies: T1 (do the SHA-pinning pass once, covering the new action too, rather than pinning in two separate commits).

---

- [ ] **T3 — Configure Dependabot for GitHub Actions** 🔒

Acceptance:
- `.github/dependabot.yml` exists with a `github-actions` ecosystem entry pointed at `/` (or wherever `.github/workflows/` lives relative to repo root), on a reasonable schedule (e.g. weekly).
- Dependabot is confirmed enabled for the repo (either via this config alone, since GitHub auto-detects `dependabot.yml`, or by checking repo Settings > Code security if the maintainer wants to verify manually).

Verify:
- `cat .github/dependabot.yml` matches GitHub's documented schema (no CI check exists for this file; validate by eye / `gh api repos/rsenna/guiltty/dependabot/alerts` reachability if desired).
- Local: same quality-gate commands as T1 (this task doesn't touch Rust code, but the standing rule is to run the gate before considering any task done).

Files: `.github/dependabot.yml` (new)

Dependencies: T1 (Dependabot needs pinned SHAs to have something to update; T2's caching is orthogonal and doesn't block this).
