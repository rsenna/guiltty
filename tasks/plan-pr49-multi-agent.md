# Delivery plan: PR #49 multi-agent Spec Kit integration

**Outcome:** Keep PR #49 scoped to safe, consistent multi-agent Spec Kit integration, address valid review findings, verify the revision, and leave the pull request ready for maintainer merge after prerequisite PR #44 lands.

**Applications:** None. This change modifies repository instruction/configuration files; guiltty is a library, not a deployable application.

## Acceptance checks

- [x] **unit** — `cargo test --workspace`: 55 tests pass at the PR branch revision.
- [x] **unit** — `cargo llvm-cov --workspace --summary-only --fail-under-lines 90`: 92.16% line coverage at the PR branch revision.
- [x] **manual** — validate all 50 integration manifest hashes against their referenced files; audit copied agent instructions for automatic repository-hook execution (none remain).
- [x] **manual** — independent review found no actionable findings; GitHub CI and Codacy checks passed on PR #49.
- [x] **manual** — verified OMP taskstoissues frontmatter includes `bash`, `read`, and both GitHub tools required by its prerequisite, remote-check, document-reading, and issue-creation steps.

There is no browser/application acceptance suite: this PR changes multi-agent documentation and integration metadata only; it ships no application.

**Code revision tested:** `0a5a27b21ff34e690817daa92ed17dbf65a75f8e` on branch `speckit-multi-agent`; the OMP frontmatter/docs change is validated separately and does not change Rust source.

## Plan

- [x] Define PR scope as multi-agent integration only; leave core `.specify/` bootstrap to issue #39 / PR #44.
- [x] Inventory current PR findings, separating stale core-bootstrap findings from valid multi-agent findings.

## Build

- [x] Remove duplicated core `.specify/` bootstrap content from PR #49.
- [x] Make per-agent skills/commands require explicit user approval before repository-defined hooks execute; preserve optional-hook opt-in.
- [x] Scope feature selection and handoffs to each invocation; reserve `.specify/feature.json` for single-agent legacy fallback; serialize shared feature/task writes.
- [x] Correct task-to-issue paths, canonical feature-scoped deduplication, ambiguous legacy handling, checked-task skipping, and repository pagination expectations.
- [x] Align task generation and implementation guidance: behavioral work is test-first, convergence tasks have explicit severity and test/implementation order, phase names match generated task files, and ignore files are task-scoped.
- [x] Add the required `bash` and `read` tools to the OMP taskstoissues command allowlist while preserving GitHub issue tools.
- [x] Correct checklist template fallback/resolution, analysis finding IDs, handoff prompts, and per-agent consistency.
- [x] Refresh all integration manifest hashes after content changes.

## Test

- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Run `cargo test --workspace` (55 passed).
- [x] Run `cargo llvm-cov --workspace --summary-only --fail-under-lines 90` (92.16%).
- [x] Validate JSON and all 50 manifest file hashes; audit that no automatic hook-execution directives remain.
- [x] Complete independent agent review, address findings, push via `pr.sh`, update PR description, and reply to applicable findings.

## Review

- [ ] Person reviews and accepts PR #49. Agent must not self-approve or mark it ready.

## Merge

- [ ] PR #44 (issue #39 core Spec Kit bootstrap) merges first; PR #49 depends on its shared `.specify/` scripts, templates, and core integration metadata.
- [ ] Maintainer reviews and merges PR #49 only after PR #44 is merged; agent must not merge or mark the PR ready.
