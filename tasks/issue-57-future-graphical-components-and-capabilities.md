# Tasks: Issue #57 — future graphical components and capabilities

Source: https://github.com/rsenna/guiltty/issues/57

**Priority: future / not urgent.** Issue #57 explicitly says this work does not block guiltty's current roadmap. Do not start implementation until Loom has a terminal Orb design and confirms it has pixel-rendering use cases that justify a graphical toolkit. This breakdown is for future planning, not a request to reprioritize v0 viewport, zoom/pan, or other existing work.

**Cross-repo boundary:** Loom-side work (building `orb-tui`/a Kitty-capable renderer, deciding pixel-valued views, and consuming guiltty behind `loom-orb-kitty`) belongs in `rsenna/loom`, not this repository. Loom's `loom-core`, `loom-nu`, and `loom-orb` must remain free of guiltty/Kitty types.

**Quality gate for implementation tasks:** `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `cargo llvm-cov --workspace --summary-only --fail-under-lines 90`.

## Sequencing and gates

T1 is an external readiness gate. T2 is a 🔒 design/approval gate; do not implement components or event handling before the API, crate boundary, and input model are approved. T3–T7 depend on T2 and should be split into focused PRs. T8 is a later release/integration gate and requires explicit maintainer approval for publication or a stable tag.

---

- [ ] **T1 — Reconfirm Loom readiness before starting implementation** 🔒

Acceptance:
- Loom has a documented terminal Orb design or implementation and has confirmed that at least one intended view genuinely needs pixel graphics (rather than an ordinary text TUI).
- Loom's Rust MSRV/edition mismatch with guiltty (Loom 1.92/2024 vs. guiltty 1.96/2021, as reported by issue #57) is resolved or explicitly accepted by both maintainers.
- A Loom-side integration issue/task exists for the adapter and keeps the dependency in `loom-orb-kitty`, behind Loom's `Renderer` contract.
- If these conditions are not met, leave all implementation tasks unchecked; this issue remains future work.

Verify:
- Review the linked Loom terminal-renderer proposal/implementation and its tracked integration task with the Loom maintainer.
- Run the guiltty quality gate above if this task changes repository files; otherwise record the external readiness evidence in the issue comment/PR.

Files: external Loom issue/PR; `tasks/issue-57-future-graphical-components-and-capabilities.md` (status only, if needed).

Dependencies: none; blocks T2–T8.

---

- [ ] **T2 — Design and approve the backend-agnostic component and input API** 🔒

Acceptance:
- Add a design/ADR under `specs/decisions/` before implementation. It decides whether components belong in `guiltty-core` or a new crate (new crate boundary is ask-first), how components render through the backend-agnostic surface, and how state/ownership and focus are represented.
- Decide whether keyboard input uses Kitty keyboard protocol directly or a backend-agnostic event abstraction; specify key/modifier/repeat/release behavior, resize and cell/pixel-size queries, and focus-in/out. Mouse support remains optional and is not included without a separate scope decision.
- Decide the supported character repertoire and visible fallback for text. Structured values commonly contain lowercase, punctuation, and possibly non-ASCII; unsupported characters must not be silently omitted from a view.
- Define a synchronous/simple event-loop integration; no forced async runtime.
- Specify that UI/event types do not leak into `guiltty-core` if the approved crate boundary places them elsewhere, and that Kitty-specific event decoding stays out of backend-agnostic APIs.
- Record the intended Loom adapter boundary: only `loom-orb-kitty` depends on guiltty, not Loom core crates.

Verify:
- Review the design against `docs/spec.md` Boundaries, `docs/intent/kitty-graphics-ui-toolkit.md`, the current `Backend` trait, and Loom's `Renderer` contract.
- Run the guiltty quality gate above if repository files change.

Files: `specs/decisions/` (new approved decision), `docs/design/` (if the design is too large for an ADR).

Dependencies: T1. Ask first before adding a crate boundary or external dependency.

---

- [ ] **T3 — Add shared component composition, focus, and keyboard navigation primitives** 🔒

Acceptance:
- Implement the component/container and focus model selected by T2, with deterministic keyboard navigation and explicit state transitions.
- Component APIs remain backend-agnostic; no Kitty protocol types appear in public component signatures.
- Tests assert focus order, focus changes, key dispatch, and behavior at boundaries (first/last component, disabled/unfocusable component if supported).
- No mouse handling unless separately approved.

Verify:
- Unit tests cover focus/navigation transitions and errors/boundaries.
- Run the guiltty quality gate above.

Files: the crate/module boundary selected by T2; public API docs; tests.

Dependencies: T2.

---

- [ ] **T4 — Implement text input and button components** 🔒

Acceptance:
- Text input supports single-line editing, cursor movement, selection, and the editing keys chosen in T2.
- Text input renders characters according to T2's explicit text-repertoire/fallback contract, including lowercase and punctuation needed for ordinary commands.
- Button exposes focus and activation through the approved keyboard model.
- Rendering and input behavior are testable without launching a terminal; terminal-specific presentation remains in backend crates.
- Tests cover editing, cursor/selection bounds, activation, and focus behavior.

Verify:
- Deterministic unit tests assert state changes and rendered canvas output for representative inputs and boundaries.
- Run the guiltty quality gate above.

Files: component crate/module selected by T2; tests and public API docs.

Dependencies: T3.

---

- [ ] **T5 — Implement selectable lists and table/record views** 🔒

Acceptance:
- Selectable list supports keyboard-driven selection and scrolling for content larger than its visible region.
- Table/record view lays out structured rows/columns with documented clipping and overflow behavior.
- Both consume viewport/crop/scroll capabilities through the API selected by T2; do not duplicate the v0 viewport design or add Kitty-specific placement to core.
- Structured values retain text fidelity: tests cover lowercase, punctuation, and representative non-ASCII input, asserting supported characters render and unsupported characters follow T2's visible fallback rather than disappearing silently.
- Tests cover selection/scroll boundaries and stable column/record layout for representative content.

Verify:
- Unit tests assert selected item, visible window, clipping, and layout at empty/single/overflow boundaries.
- Run the guiltty quality gate above.

Files: component crate/module selected by T2; viewport API from the existing viewport follow-up; tests and docs.

Dependencies: T2, T3, and the viewport readiness gate: before starting this task, the viewport follow-up in `docs/design/viewport-regions-zoom-scroll.md` must be broken into checkboxed, dependency-ordered V1–V3 tasks and those tasks must be merged. `tasks/plan.md` currently says that breakdown has not happened; leave this task blocked until it does. Do not reprioritize viewport work from this Loom issue.

---

- [ ] **T6 — Add the approved backend-agnostic input/event surface** 🔒

Acceptance:
- Implement only the event set approved in T2: Kitty keyboard protocol support or an agreed backend-agnostic abstraction, resize, cell/pixel-size queries, and focus-in/out.
- Keep protocol parsing in `guiltty-kitty`; keep event types consumed by components independent of Kitty types.
- Provide an application-driven synchronous/simple loop; do not introduce an async runtime by default.
- Mouse events remain out of scope unless separately approved.
- Tests cover parsing/translation, modifiers, key release/repeat where selected, malformed input, resize, focus transitions, and unknown events.

Verify:
- Unit/protocol tests use byte fixtures and deterministic event sequences; no terminal required for core behavior.
- Run the guiltty quality gate above and a real-terminal smoke test for Kitty protocol behavior.

Files: event API crate/module selected by T2; `crates/guiltty-kitty/` for Kitty decoding only; tests/docs.

Dependencies: T2 and T3. Any dependency or crate-boundary change is ask-first.

---

- [ ] **T7 — Add optional graphical views only for validated pixel-valued use cases** 🔒

Acceptance:
- Add image/thumbnail, chart/sparkline, or file-preview views only after Loom confirms which structured values require pixels.
- Each view has a bounded input contract and integrates with the component/viewport API rather than adding Loom-specific types to guiltty.
- At least one consumer-facing example demonstrates the confirmed use case; ordinary lists/tables remain usable without graphical views.

Verify:
- Unit tests assert geometry, clipping, and pixel-buffer results for each implemented view.
- Run the guiltty quality gate above and visually verify the example in a Kitty-compatible terminal.

Files: component crate/module selected by T2; `examples/`; tests/docs.

Dependencies: T1–T5 and a confirmed Loom pixel-valued use case. Separate PR per view or tightly related view set.

---

- [ ] **T8 — Validate Loom adapter integration and decide release form** 🔒

Acceptance:
- In Loom, implement the dependency only in `loom-orb-kitty` behind its `Renderer` trait; `loom-core`, `loom-nu`, and `loom-orb` remain toolkit-free.
- Validate structured-value rendering, resize/focus/input integration, and graceful operation under the agreed synchronous/simple event loop.
- Choose a stable git tag or published, versioned crate only after public APIs are stable. Publication is not authorized by this issue; ask the maintainer before publishing or changing release policy.
- Confirm transitive-license compatibility, including `kittage`, as Loom's issue requests.

Verify:
- Run both repositories' documented quality gates and the Loom adapter's real-terminal smoke test.
- Review dependency graph and license notices before any release action.

Files: Loom `loom-orb-kitty` adapter and integration tests; guiltty release metadata only after approval.

Dependencies: T1–T7 and explicit maintainer approval for publishing/tagging.
