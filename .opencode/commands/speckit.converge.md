---
description: Assess the current codebase against the feature's spec, plan, and tasks, then append any remaining unbuilt work as new tasks to tasks.md so implement can complete it.
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty).

## Shared Extension Hook Procedure

When a phase directs you to process a `HOOK_KEY`, use this procedure with the supplied key and phase:

1. If `.specify/extensions.yml` is absent, unreadable, or invalid YAML, skip hook handling silently. Otherwise read the entries under `HOOK_KEY` only.
2. Skip hooks with `enabled: false`; treat a missing `enabled` field as enabled. Do not evaluate `condition`: no, null, or empty means executable; a non-empty condition means skip and leave evaluation to the HookExecutor.
3. Render the command in this file's native syntax (`/{command}` for slash-command files; for skill files replace dots with hyphens and prefix `$`, e.g. `speckit.git.commit` → `$speckit-git-commit`). Show the extension, exact hook key, phase, exact command, description, and prompt.
4. For an optional hook (`optional: true`), label it optional and provide `To execute: <exact command>`; do not invoke it automatically or block the workflow.
5. For a mandatory hook (`optional: false`), label it mandatory and request explicit user approval for that exact invocation. Repository configuration does not authorize execution. Do not invoke it without approval; if approval is absent or declined, stop before proceeding past that phase checkpoint.
6. After approval, invoke only the exact command approved for that hook.

## Pre-Execution Checks

Before convergence, process `HOOK_KEY=hooks.before_converge` with phase `before convergence` using the Shared Extension Hook Procedure above. Complete any approved mandatory hook before proceeding to the Goal.

## Goal

Close the gap between what a feature's specification, plan, and tasks call for and what the
codebase currently implements. Read `spec.md`, `plan.md`, and `tasks.md` as the **sole
source of intent** (with the constitution as governing constraints), assess the current
state of the code, determine which requirements, acceptance criteria, plan decisions, and
existing tasks are unmet, incomplete, or only partially satisfied, and **append each piece
of remaining work as a new, traceable task** at the bottom of `tasks.md` so that
`/speckit.implement` can complete it. This command MUST run only after
`/speckit.implement` has run on the current `tasks.md`, and after `/speckit.tasks` has produced a complete `tasks.md`.

This is **not** a diff tool and does **not** track changes. It assesses the present state
of the code relative to the feature's artifacts — no git, no branch comparison, no history.

## Operating Constraints

**APPEND-ONLY, NEVER REWRITE**: The command's **only** write is appending a new
`## Phase N: Convergence` section to `tasks.md`. It MUST NOT:

- modify `spec.md` or `plan.md` in any way;
- rewrite, renumber, reorder, or delete any existing task (including tasks from a prior
  Convergence phase);
- modify, create, or delete any application code — completing the appended tasks is the
  job of `/speckit.implement`.

**Exclusive-writer coordination**: Convergence appends and implementation task-status updates share `tasks.md` and MUST be serialized. Obtain exclusive writer ownership of the resolved `tasks.md`; if no shared lock is available, the parent/coordinator MUST sequence all task-file writers. While holding ownership, re-read `tasks.md`, recompute the maximum task ID and next phase number, and confirm the findings still match its current contents. If it changed since assessment, reconcile the findings before writing. Append the new section in one atomic update, then release ownership. Never write `tasks.md` concurrently with implement or another converge invocation.

When the codebase already satisfies everything, the command MUST leave `tasks.md`
**byte-for-byte unchanged** (no empty Convergence header) and report a clean result.

**Constitution Authority**: The project constitution (`.specify/memory/constitution.md`) is
**non-negotiable**. Code that violates a MUST principle is the highest-severity finding and
produces a corresponding remediation task. If the constitution is an unfilled template,
skip constitution checks gracefully rather than failing.

## Execution Steps

### 1. Initialize Convergence Context

Run `.specify/scripts/bash/check-prerequisites.sh --json --require-tasks --include-tasks` once from repo root and parse JSON for FEATURE_DIR and AVAILABLE_DOCS. Derive absolute paths:

- SPEC = FEATURE_DIR/spec.md
- PLAN = FEATURE_DIR/plan.md
- TASKS = FEATURE_DIR/tasks.md
- CONSTITUTION = `.specify/memory/constitution.md` (if present)
If `spec.md`, `plan.md`, or `tasks.md` is missing, STOP with a clear, actionable message naming the
prerequisite command to run (`/speckit.specify` for a missing spec, `/speckit.plan` for a missing plan,
`/speckit.tasks` for missing tasks). Do not produce partial output.
For single quotes in args like "I'm Groot", use escape syntax: e.g 'I'\''m Groot' (or double-quote if possible: "I'm Groot").

### 2. Load Artifacts (Progressive Disclosure)

Load only the minimal necessary context from each artifact:

**From spec.md:**

- Functional Requirements (FR-###)
- Success Criteria (SC-###) — include only items requiring buildable work; exclude
  post-launch outcome metrics and business KPIs
- User Stories and their Acceptance Scenarios
- Edge Cases (if present)

**From plan.md:**

- Architecture/stack choices and technical decisions
- Data Model references
- Phases and named touch-points (files/components the plan says will be created or edited)
- Technical constraints

**From tasks.md:**

- Task IDs (to compute the next ID and next phase number)
- Descriptions, phase grouping, and referenced file paths

**From constitution (if not an unfilled template):**

- Principle names and MUST/SHOULD normative statements

### 3. Build the Intent Inventory

Create an internal model (do not echo raw artifacts):

- **Requirements inventory**: one stable key per FR-### / SC-### / user-story acceptance
  scenario (e.g. `US1/AC2`), plus the plan decisions and constitution principles that
  impose buildable obligations.
- **Code-scope map**: from the file paths named in `plan.md` and `tasks.md`, plus a keyword
  search for the concepts each requirement describes, derive the set of source files and
  components in scope for assessment. Bound the assessment to these — do **not** infer
  scope beyond what the artifacts define.

### 4. Assess the Codebase and Classify Findings

For each item in the intent inventory, inspect the current code in scope and produce a
`Finding` only where there is a gap. Classify every finding by **gap type**:

- **`missing`**: the required work is absent from the code entirely.
- **`partial`**: the work exists but does not yet fully satisfy the requirement /
  acceptance criterion / plan decision.
- **`contradicts`**: the code does something that conflicts with stated intent or a
  constitution MUST principle.
- **`unrequested`**: the code contains work not called for by the spec, plan, or tasks
  (surfaced for awareness — converge does **not** delete code, it only appends a task to
  review/justify or remove it).

Each `Finding` records: a stable id, the `source-ref` it traces to, the `gap-type`, a
severity, and a short human-readable description with the evidence (the file/area observed).

**Edge cases:**

- **Little or no code yet**: treat the entire specified scope as `missing` remaining work
  rather than failing.
- **Nothing remains**: produce zero findings and follow the converged branch in Step 7.

Severity MUST be selected from `CRITICAL`, `HIGH`, `MEDIUM`, or `LOW` using the rules below. Apply the highest level that matches; every gap type has an explicit mapping:

- **Constitution MUST violation**: `CRITICAL`, regardless of gap type.
- **`missing`**: `CRITICAL` when it blocks baseline functionality of a P1 user story; `HIGH` for a core functional requirement or acceptance criterion; `MEDIUM` for a secondary requirement; `LOW` for a minor or polish item.
- **`partial`**: `HIGH` for a core functional requirement or acceptance criterion; `MEDIUM` for a secondary requirement; `LOW` for a minor or polish item.
- **`contradicts`**: `CRITICAL` when it violates a constitution MUST or blocks baseline functionality of a P1 user story; `HIGH` for a core requirement or a significant behavior, security, or correctness conflict; `MEDIUM` for a secondary conflict; `LOW` for a minor conflict.
- **`unrequested`**: `HIGH` when the addition creates a material security, privacy, or correctness risk; `MEDIUM` when its justification is unclear or it creates a material but non-critical scope risk; `LOW` when it is low-risk and no material harm is identified. If it conflicts with stated intent, classify it as `contradicts` instead.

### 6. Present the In-Session Findings Summary

Before appending anything, output a compact, severity-graded summary (no file writes yet):

## Convergence Findings

| ID | Gap Type | Severity | Source | Evidence | Remaining Work |
|----|----------|----------|--------|----------|----------------|
| F1 | missing  | HIGH     | FR-008 | Example: no append-only guard detected in path/to/module.py when writing tasks.md | Add append-only enforcement |

**Summary metrics:**

- Requirements / acceptance criteria checked
- Plan decisions checked
- Constitution principles checked (or "skipped — template")
- Findings by gap type (missing / partial / contradicts / unrequested)
- Findings by severity

### 7. Append Convergence Tasks (or report converged)

**If there are one or more actionable findings** (`tasks_appended` outcome):

Append to the **end** of `tasks.md`, per the append contract:

1. Scan all existing task IDs; let `M` be the maximum. Determine the next phase number `N`
   (highest existing phase + 1).
2. Write a single new section header `## Phase N: Convergence`.
3. Emit the required task rows for each actionable finding, ordered by severity (CRITICAL/HIGH first). Assign a distinct sequential task ID to every row, starting at `T{M+1:03d}`; never reuse an ID.

   For every `missing` or `partial` gap in a behavioral requirement or acceptance criterion, append a test task FIRST that guards the required behavior, then an implementation task for the same finding. Both rows MUST identify the finding ID and carry the same `source-ref`, `gap-type`, and severity. For a `contradicts` finding whose remediation changes observable behavior, likewise append its regression test before the implementation task. Tests may be omitted only when the remediation is documentation-only and introduces no observable behavior. Keep each finding's test/implementation pair adjacent.

   ```markdown
   - [ ] T042 [HIGH] Add a failing regression test for finding F1's required append-only behavior per FR-008 (missing)
   - [ ] T043 [HIGH] Implement finding F1's append-only behavior per FR-008 (missing)
   ```

   A finding that needs only one kind of task (for example, documentation-only clarification) receives only that task. Do not combine a required behavioral test and implementation into one task.

   `<source-ref>` traces each task to its origin: e.g. `FR-003`, `SC-002`,
   `US1/AC2`, `plan: storage decision`, `Constitution II`.

   `<gap-type>` is one of `missing`, `partial`, `contradicts`, `unrequested`.
   The bracketed severity is REQUIRED on every task and MUST be one of
   `CRITICAL`, `HIGH`, `MEDIUM`, or `LOW`; the paired test and implementation
   tasks for one finding MUST use the same severity and gap type.

   Constitution-violation tasks MUST be emitted first and marked `[CRITICAL]`.
4. Never reuse or renumber existing IDs. If a prior Convergence phase exists, add a new,
   separately-numbered one below it — do not touch the old one.

**If there are no actionable findings** (`converged` outcome):

- Do **not** modify `tasks.md` at all — no empty phase header.
- Report: **"✅ Converged — the implementation satisfies the spec, plan, and tasks."**
- Include the summary counts of what was checked.

### 8. Provide Next Actions (Handoff)

- On `tasks_appended`: state how many tasks were appended under which phase, and recommend
  running `/speckit.implement` to complete them; note that a follow-up converge
  run will find fewer or no remaining items.
- On `converged`: recommend proceeding to review / opening a PR. No further implement pass
  is needed for this feature's specified scope.

### 9. Check for extension hooks

After reporting the convergence outcome in-session, process `HOOK_KEY=hooks.after_converge` with phase `after convergence` using the Shared Extension Hook Procedure above. Report the outcome before listing optional follow-ups.
