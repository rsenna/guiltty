# Iklo Constitution

These are the principles that govern every spec, plan, and task in this repo.
They supersede convenience. Amendments require an ADR under
[`specs/decisions/`](../../specs/decisions/).

## Core Principles

### I. Test-First (Non-Negotiable)

Every behavioral change lands with a failing test first. Red → Green →
regression → commit. No feature is "done" until `make test` is green and the
new behavior is covered by a test that fails against `main`.

### II. One Epic In Flight

At most one feature under `specs/` is active at a time. Concurrent epics
fracture attention, produce merge pain, and let half-formed specs contaminate
each other. When a new epic is activated, the previous one either ships or is
paused explicitly (its `specs/NNN-*/` directory stays put; nothing else moves
forward until it's back on the runway).

### III. Substrate Before Feature

Iklo is a language, a shell, and an in-process live-image runtime that must
share one grammar. When any load-bearing capability grows a second consumer
(a second backend, a second call site, a second scope), the *shape* of the
seam gets extracted to an interface **before** the second implementation is
built. We install boundaries early; we do not retrofit them under pressure.

### IV. Kebab-Case Iklo, Idiomatic Rust

Iklo-level identifiers (primitives, keywords, forms, sigils) are kebab-case
without exception, including subtraction-lookalikes: `x-1` is one identifier;
subtraction requires whitespace on both sides. Rust code follows idiomatic
Rust (`snake_case` items, `PascalCase` types) — the two style systems do not
bleed. See [`.github/instructions/rust.instructions.md`](../../.github/instructions/rust.instructions.md).

### V. Comments Justify Themselves

Code is documentation by default. Comments appear only where the *why* is
non-obvious. Do not narrate what the code does. Doc comments on public APIs;
usually nothing on internal helpers. See
[`.github/instructions/self-explanatory-code-commenting.instructions.md`](../../.github/instructions/self-explanatory-code-commenting.instructions.md).

### VI. Load-Bearing Decisions Become ADRs

Any decision that is expensive to reverse, that determines the shape of code
across multiple crates, or that gets questioned a second time becomes an ADR
under [`specs/decisions/`](../../specs/decisions/). ADRs are sequential
(`ADR-NNNN`), never deleted, and superseded rather than edited. Lightweight
inline `**Decision (date)**` notes are fine until they harden.

### VII. No Workarounds Left Standing

If a bug is directly caused by or tightly coupled to code being changed, it
gets fixed as part of that change. Failing tests are never skipped or removed
to make a build pass. Half-fixes and "for now" scaffolding either graduate
into real solutions in the same PR or become an issue on
[rsenna/iklo](https://github.com/rsenna/iklo/issues) with the code that
requires them.

## Development Constraints

- **Rust**, edition 2021, workspace `resolver = "2"`. Toolchain pinned via
  [`mise.toml`](../../mise.toml).
- **Storage:** the runtime image lives behind a `Substrate` boundary (see
  [ADR-0001](../../specs/decisions/ADR-0001-substrate-boundary.md)). Only
  substrate implementations may hold binding state directly.
- **Dependencies:** adding a workspace dependency or a new crate is an
  "ask first" change. Removing one usually isn't.
- **Grammar changes:** any change to sigils, keywords, or terminator rules
  requires an ADR — the parser has many quiet consumers.

## Workflow

Spec-driven, four gates powered by GitHub Spec Kit v0.12+:

1. **`/speckit.specify`** — author a feature spec in `specs/NNN-<slug>/spec.md`.
   Creates and switches to branch `NNN-<slug>`.
2. **`/speckit.plan`** — produce `specs/NNN-<slug>/plan.md` (technical approach,
   structure decisions) from the spec.
3. **`/speckit.tasks`** — produce `specs/NNN-<slug>/tasks.md` (ordered,
   TDD-shaped, one commit per task).
4. **`/speckit.implement`** — execute the tasks; each task ends with a commit.

Optional gates: `/speckit.clarify` (before `/speckit.plan`),
`/speckit.checklist` (after `/speckit.plan`), `/speckit.analyze` (after
`/speckit.tasks`), `/speckit.converge` (retro-fit remaining work into tasks).

**Bugs** are GitHub Issues on
[rsenna/iklo](https://github.com/rsenna/iklo/issues) — not files in this
repo. Promote an issue into a `specs/NNN-*/spec.md` when it grows into real
design work.

## Governance

- These principles supersede all other practices in the repo.
- Amendments require an ADR (context, alternatives rejected, consequences).
- Every PR/review verifies compliance; complexity must be justified in
  `plan.md`'s Complexity Tracking section.
- Runtime development guidance lives in [`../../AGENTS.md`](../../AGENTS.md).

**Version**: 1.0.0 | **Ratified**: 2026-07-17 | **Last Amended**: 2026-07-17
