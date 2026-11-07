# Delivery plan: PR #44 Spec Kit bootstrap review follow-up

**Outcome:** Resolve valid PR #44 review notes, keep the bootstrap scripts/templates and tests consistent, verify the branch, and leave the pull request ready for maintainer merge.

**Applications:** None. This change is repository tooling and documentation for a Rust library; no deployable user application changes.

## Acceptance checks

- [x] **unit** — `cargo test --workspace`: 55 tests pass.
- [x] **unit** — `cargo llvm-cov --workspace --summary-only --fail-under-lines 90`: 92.16% line coverage.
- [x] **unit** — `bash tests/shell/specify-scripts-test.sh`: 40 shell assertions pass; suite uses Bash and standard utilities without Python.
- [x] **manual** — validate all 10 Speckit manifest hashes, shell syntax, JSON validity, and `git diff --check`.
- [x] **manual** — independent reviewer found no actionable issue in the Bash-only JSON assertions; CI/Codacy passed before this final test-only update.

There is no browser/application acceptance suite because this repository is a library with no deployable application. The Blume project also has zero browser scenarios stored for this delivery.

## Plan

- [x] Triage new PR #44 notes and distinguish current findings from previously resolved threads.

## Build

- [x] Harden `create-new-feature.sh`: empty and option-like slugs, missing/empty template, generated feature state ignore, safe user-controlled output.
- [x] Fix prerequisite order and task-template test guidance; refresh Speckit manifest hashes and workflow timestamps.
- [x] Wire bootstrap shell tests into CI and correct JSON/exit-status assertions.
- [x] Replace Python JSON parsing in the regression suite with exact Bash assertions for deterministic output contracts.

## Test

- [x] Run project formatter, clippy, workspace tests, and 90% coverage gate.
- [x] Run the shell regression suite; 40 passed.
- [x] Independently review the final test-only change; no findings.

## Review

- [ ] Obtain Blume person approval before pushing the latest committed review response.
- [ ] Push the reviewed branch update through `pr.sh` and reply to CodeRabbit thread 4242391817.
- [ ] Person/maintainer reviews PR #44 and confirms it is ready to merge.

## Merge

- [ ] Maintainer merges PR #44; agent must not merge or mark it ready.
