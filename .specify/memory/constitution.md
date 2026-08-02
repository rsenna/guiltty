# guiltty Constitution

These are the principles that govern every spec, plan, and task in this
repo. They supersede convenience. Amendments require an ADR under
`specs/decisions/` (added alongside this file by issue #39's task
breakdown -- not yet a live link here since task merge order isn't
guaranteed).

## Core Principles

### I. Backend-Agnostic Core

`guiltty-core` stays scoped to the absolute-coordinate drawing surface
(`Canvas`, `Shape`, text, the `Backend` trait) and never contains
backend-specific code; backend concerns live only in backend crates
(`guiltty-kitty` today). Crates built on `guiltty-core`'s public API
(`guiltty-sprite`, and `guiltty-turtle` once it exists) follow the same
rule one level up: they never reach into another crate's private state,
only its public API. Adding a new backend crate, or changing the workspace
crate boundaries this describes, is an **ask-first** change.

### II. Recoverable Errors Never Panic

Public API returns `Result<T, guiltty_core::Error>` for recoverable
conditions (a missing/malformed image file, a failed terminal write, a
stale sprite footprint) rather than panicking. Panics are reserved for
programmer-error invariants only (e.g. an out-of-bounds internal index) --
never a condition a caller could legitimately hit and need to recover
from.

### III. Ask First On New Dependencies

Adding any new external dependency (especially anything requiring C/FFI)
is an **ask-first** change, same as a new backend crate or workspace
boundary change (Principle I). Removing one usually isn't.

### IV. Test-First For Behavioral Changes

Every behavioral change lands with tests that actually assert the new
behavior -- not coverage-padding. `guiltty-core`/`guiltty-sprite` unit
tests assert pixel-buffer/state correctness with no terminal required;
`guiltty-kitty` protocol tests assert byte-level escape-sequence encoding;
actual rendered output stays a manual/visual check for now (see
`docs/spec-kitty-e2e.md` for the planned automated tier). CI enforces a
90% line-coverage floor (`docs/spec-ci.md`) -- a PR that drops below it
fails, but clearing the floor is a side effect of real tests, never the
goal itself.

### V. Pre-1.0 Breaking Changes Are Cheap, Not Silent

Every crate in this workspace is at `0.0.0`. Breaking a public API
pre-1.0 is acceptable and sometimes the right call (see the
`guiltty-sprite` extraction's precedent) -- but it must be called out
explicitly in the PR description as a breaking change, never shipped as
if it were routine.

### VI. Docs That Contradict Code Are Bugs

A stale "not yet implemented" note, a broken doc link, a crate list
missing a crate that now exists -- these are bugs, not polish, and they
only get more misleading the longer they're left. Fix doc staleness
encountered while touching the affected area in the same PR, not a
follow-up (`docs/spec.md`'s crate list went stale exactly this way after
the `guiltty-sprite` extraction, and was fixed as part of the same repo-
standard bootstrap this constitution belongs to -- see issue #39).

## Development Constraints

- **Rust**, latest stable toolchain, 2021 edition, no nightly-only
  features. Toolchain pinned via [`mise.toml`](../../mise.toml).
- **Structure:** a Cargo workspace, not a single crate -- see Principle I.
- **Color/coordinates:** RGBA8 throughout; pixel-addressable, origin
  top-left.
- Full tech-stack rationale (why `kittage` over hand-rolled encoding, why
  `notcurses`/GPU acceleration are out of scope for now, etc.) lives in
  [`docs/spec.md`](../../docs/spec.md), not restated here.

## Workflow

Unlike iklo's spec-kit-driven `/speckit.*` gates, this repo's day-to-day
shipping process predates `.specify/` and stays as-is: the
`pull-request-process`/`map-issue-to-tasks`/`fix-mapped-issue` skills --
worktrees (never the shared checkout), a dedicated git/PR identity,
issue → `tasks/issue-N-*.md` task breakdown → one task per PR → bots/the
maintainer review and merge, never self-merged. `.specify/`'s templates
and scripts are bootstrapped (issue #39) for future spec-kit-format work
under `specs/NNN-slug/`, alongside this process, not replacing it.

**Bugs and feature work** are GitHub Issues on
[rsenna/guiltty](https://github.com/rsenna/guiltty/issues). Promote one
into a `specs/NNN-slug/spec.md` when it grows into real design work worth
the spec-kit gates; smaller decisions can stay as a `docs/design/*.md`
write-up (this repo's existing convention, e.g.
[`docs/design/viewport-regions-zoom-scroll.md`](../../docs/design/viewport-regions-zoom-scroll.md))
or an ADR under `specs/decisions/`.

## Governance

- These principles supersede all other practices in the repo.
- Amendments require an ADR (context, alternatives rejected,
  consequences) under `specs/decisions/`.
- Every PR/review verifies compliance.
- Day-to-day agent operating guidance lives in `AGENTS.md` (added
  alongside this file by issue #39's task breakdown -- not yet a live
  link here since task merge order isn't guaranteed).

**Version**: 1.0.0 | **Ratified**: 2026-08-02 | **Last Amended**: 2026-08-02
