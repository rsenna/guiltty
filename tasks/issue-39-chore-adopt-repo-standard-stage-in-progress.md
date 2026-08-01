# Tasks: Issue #39 — chore: adopt repo-standard (stage=in-progress)

Source: https://github.com/rsenna/guiltty/issues/39
Enrichment: issue comment on #39.

Quality gate: none of these tasks touch Rust source, so the usual `cargo fmt`/`clippy`/`llvm-cov` gate doesn't apply. "Verify" below is doc/structure inspection instead.

This issue brings `guiltty` up to the `in-progress` tier of the cross-repo
doc/folder standard ([`2026-08-01-repo-standard-design.md`](https://github.com/rsenna/rs-claude-plugins/blob/main/docs/superpowers/specs/2026-08-01-repo-standard-design.md)).
It is pure scaffolding: no existing `docs/*.md` content moves, no Rust code
changes, no CI changes.

Sequencing: **T1 and T3 are independent** and can happen in any order or in
parallel. **T2 benefits from T1** (an `AGENTS.md` written after `repo.toml`
already declares the stage can reference it) but doesn't hard-depend on it.
**T4 is the largest task**; it has no hard dependency on T1–T3 at the file
level, but soft-depends on T3 (its constitution content references
`specs/decisions/`) and should come last anyway since it's the one most
likely to surface an open question (tool dependencies in
`.specify/scripts/`) worth resolving with the smaller tasks already landed.

---

- [ ] **T1 — Declare `stage = "in-progress"` in `repo.toml`**

Acceptance:
- `repo.toml` gains a `stage = "in-progress"` line, matching the design
  doc's four valid values (`prototype | in-progress | released | archived`).
- No other `repo.toml` fields change.

Verify:
- `git diff repo.toml` shows only the added `stage = "in-progress"` line —
  proves nothing else in the file changed, which `cat` alone can't.

Files: `repo.toml`

Dependencies: none.

---

- [ ] **T2 — Create `AGENTS.md`**

Acceptance:
- `AGENTS.md` exists at repo root, in the design doc's "in-progress" style:
  what's actually implemented today (reality-check, not aspirational —
  mirror `README.md`'s existing DONE/IN PROGRESS/NOT STARTED status list,
  don't restate `docs/spec.md`'s forward-looking Success Criteria as if
  already done), decided rules/conventions (the Always/Ask first/Never list
  already in `docs/spec.md`'s Boundaries section, restated or linked), dev
  commands (`docs/spec.md`'s Commands block: build/test/lint/format/run
  example), and a "where things live" map (workspace crate layout — mirror
  `docs/spec.md`'s Project Structure section).
- Links out to `docs/spec.md`, `docs/spec-ci.md`, `docs/spec-kitty-e2e.md`,
  and each current `docs/design/` doc individually --
  `docs/design/sprite-crate-extraction.md`, `docs/design/turtle-geometry.md`,
  `docs/design/viewport-regions-zoom-scroll.md` (not a `docs/design/*.md`
  glob, which doesn't resolve as an actual Markdown link) — rather than
  duplicating their content — follow
  iklo's `AGENTS.md` pattern (a short hub page pointing at the real sources)
  rather than inlining everything.

Verify:
- Read `AGENTS.md` against `docs/spec.md`, the workspace manifests
  (`Cargo.toml` files), and the current crate/example/test tree; every
  "implemented" claim must match what's actually there today — checking
  only `crates/*/src/lib.rs` isn't enough to verify dev-command, workspace-
  layout, or doc-link claims too (same discipline as issue #16/T1's README
  fix).

Files: `AGENTS.md` (new)

Dependencies: none (T1 is a soft dependency only — natural to land first
since `AGENTS.md` can reference the now-declared stage, not a hard block).

---

- [ ] **T3 — Create `specs/` and `specs/decisions/`**

Acceptance:
- `specs/` and `specs/decisions/` both exist, empty except for a `.gitkeep`
  each (git doesn't track empty directories).
- No existing `docs/spec*.md` or `docs/design/*.md` content is moved into
  either directory — see this issue's Non-goals.

Verify:
- `git status` after `git add` lists `specs/.gitkeep` and
  `specs/decisions/.gitkeep` as new files (git tracks files, not empty
  directories, so this is what actually confirms both directories exist and
  are tracked).

Files: `specs/.gitkeep` (new), `specs/decisions/.gitkeep` (new)

Dependencies: none.

---

- [ ] **T4 — Bootstrap `.specify/` from the iklo reference implementation**

Acceptance:
- `.specify/` exists with the same directory shape as the design doc's named
  reference implementation, `rsenna/iklo`'s own `.specify/` tree: `memory/`,
  `templates/`, `scripts/bash/`, `workflows/`, `integrations/`. (Written
  assuming a local `~/REPO/ME/iklo` checkout, since that's this issue's own
  wording and the maintainer's actual dev-machine layout; whoever implements
  this without that checkout should clone `rsenna/iklo` temporarily instead —
  there's no portable fetch mechanism for it beyond that.)
- `templates/`, `scripts/bash/`, `workflows/`, `integrations/` copy over
  verbatim (project-agnostic spec-kit tooling — no guiltty-specific content
  to adapt there).
- `.specify/memory/constitution.md` is **authored for guiltty**, not a
  copy-with-title-changed: restate `docs/spec.md`'s Boundaries
  Always/Ask-first/Never rules as constitution articles (e.g. "backend
  concerns never leak into `guiltty-core`", "ask first before a new external
  dependency or backend crate", "never panic on a recoverable error path in
  public API"), in the same spirit as iklo's constitution (principles that
  govern every spec/plan/task, amendments via an ADR under
  `specs/decisions/`) but with guiltty's own content, not iklo's.
- Before treating this as done, skim `.specify/scripts/bash/*.sh` for any
  tool dependency (e.g. `jq`) not already covered by `mise.toml`; if one
  exists, either add it to `mise.toml` as part of this task or note it
  explicitly as a follow-up (see this issue's Open questions) — don't
  silently ship scripts that fail on a clean checkout.

Verify:
- Per-directory diffs, not one combined `diff -rq` call (brace expansion
  turns `{templates,scripts,workflows,integrations}` into four separate
  path arguments, which `diff` can't take alongside a fifth `.specify/`
  operand):
  ```bash
  for d in templates scripts workflows integrations; do
    diff -rq ~/REPO/ME/iklo/.specify/"$d" .specify/"$d"
  done
  ```
  Each call should show no content differences for that copied,
  project-agnostic directory.
- Read `.specify/memory/constitution.md` end to end and confirm every
  article is genuinely about guiltty (no leftover "Iklo", "kebab-case
  identifiers", substrate/REPL language, or other iklo-specific content).

Files: `.specify/` (new tree — `memory/constitution.md`, `templates/*`, `scripts/bash/*`, `workflows/*`, `integrations/*`); `mise.toml` (conditional — only if the tool-dependency skim above finds something missing)

Dependencies: none at the file level; T3 is a soft dependency (the
constitution's own text points ADR amendments at `specs/decisions/`, which
T3 creates, so land T3 first even though nothing here hard-requires it).
Do last per the sequencing note above regardless.
