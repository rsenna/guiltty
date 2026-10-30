---
description: Convert existing tasks into actionable, dependency-ordered GitHub issues for the feature based on available design artifacts.
tools: ['github/github-mcp-server/list_issues', 'github/github-mcp-server/issue_write']
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty).

## Pre-Execution Checks

**Check for extension hooks (before tasks-to-issues conversion)**:
- Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read it and look for entries under the `hooks.before_taskstoissues` key
- If the YAML cannot be parsed or is invalid, skip hook checking silently and continue normally
- Filter out hooks where `enabled` is explicitly `false`. Treat hooks without an `enabled` field as enabled by default.
- For each remaining hook, do **not** attempt to interpret or evaluate hook `condition` expressions:
  - If the hook has no `condition` field, or it is null/empty, treat the hook as executable
  - If the hook defines a non-empty `condition`, skip the hook and leave condition evaluation to the HookExecutor implementation
- For each executable hook, output the following based on its `optional` flag:
  - **Optional hook** (`optional: true`):
    ```
    ## Extension Hooks

    **Optional Pre-Hook**: {extension}
    Command: `/{command}`
    Description: {description}

    Prompt: {prompt}
    To execute: `/{command}`
    ```
- **Mandatory hook** (`optional: false`):
  ```
  ## Extension Hooks

  **Mandatory Pre-Hook**: {extension}
  Command: `/{command}`
  Phase: `before_taskstoissues`
  Description: {description}

  Before invoking this hook, show the user its exact extension, phase, command, and description and ask for explicit approval. Do not invoke it unless the user approves. If approval is not given, stop before proceeding.
  ```

## Outline

1. Run `.specify/scripts/bash/check-prerequisites.sh --json --require-tasks --include-tasks` from repo root and parse `FEATURE_DIR` and `AVAILABLE_DOCS`. `FEATURE_DIR` is absolute; each `AVAILABLE_DOCS` entry is relative to `FEATURE_DIR`, so resolve it against `FEATURE_DIR` before reading or otherwise using it. For single quotes in args like "I'm Groot", use escape syntax: e.g 'I'\''m Groot' (or double-quote if possible: "I'm Groot").
1. **IF EXISTS**: Load `.specify/memory/constitution.md` for project principles and governance constraints.
1. Set `TASKS` to `FEATURE_DIR/tasks.md`.
1. Get the Git remote by running:

```bash
git config --get remote.origin.url
```

> [!CAUTION]
> ONLY PROCEED TO NEXT STEPS IF THE REMOTE IS A GITHUB URL

1. **Fetch existing issues for deduplication**: Normalize each row in `TASKS` by stripping its complete checkbox marker (`- [ ]`, `- [x]`, or `- [X]`) and any `[P]` / `[US#]` markers before parsing task IDs and descriptions. Report completed `[x]` / `[X]` rows as completed and skip them; build the set of task IDs to process from unchecked `- [ ]` rows only. For each issue returned by GitHub MCP `list_issues`, count it as a match only when its title has the canonical task-issue form `T<full task ID>: <description>` (for example, `T001: Create project structure`) and its body identifies this same feature directory and slug. Ignore arbitrary mentions of an ID, noncanonical titles, and issues scoped to another feature. Use the repository-relative `FEATURE_DIR` as the feature directory when it is within the repository, and its final path component as the feature slug. Omit `state` so both open and closed issues are included; request `perPage: 100` and follow cursor pagination with `after` set to the previous response's `endCursor`. Stop when every unchecked task ID has a matching issue or there are no more pages. If any unchecked task ID has no match, this may require scanning every repository issue; do not claim pagination is bounded in that case.
1. For a canonical issue title with this task ID, inspect its feature marker: an exact `FEATURE_DIR` and `FEATURE_SLUG` match is a duplicate; an explicit different marker is a sibling-feature issue and MUST be ignored. Only a markerless legacy issue is ambiguous: report its title and URL, ask the user before creating a possible duplicate, and skip only if the user confirms it belongs to this feature.
1. For each unchecked task in `TASKS`, use the GitHub MCP server to create a new issue in the repository that is representative of the Git remote. Create the issue with a single canonical title of the form `T001: <description>`, with the ID written once followed by the task description (for example, `- [ ] T001 Create project structure` becomes `T001: Create project structure`). Include a stable scope marker in its body: `Feature directory: <repository-relative FEATURE_DIR> (slug: <FEATURE_SLUG>)`.
   - **Skip** any task whose canonical title and body scope marker match an existing issue for this feature, and report it (for example, `T001 already has an issue for this feature, skipping`).
   - Only create issues for tasks without a matching feature-scoped canonical issue.

> [!CAUTION]
> UNDER NO CIRCUMSTANCES EVER CREATE ISSUES IN REPOSITORIES THAT DO NOT MATCH THE REMOTE URL

## Post-Execution Checks

**Check for extension hooks (after tasks-to-issues conversion)**:
Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read it and look for entries under the `hooks.after_taskstoissues` key
- If the YAML cannot be parsed or is invalid, skip hook checking silently and continue normally
- Filter out hooks where `enabled` is explicitly `false`. Treat hooks without an `enabled` field as enabled by default.
- For each remaining hook, do **not** attempt to interpret or evaluate hook `condition` expressions:
  - If the hook has no `condition` field, or it is null/empty, treat the hook as executable
  - If the hook defines a non-empty `condition`, skip the hook and leave condition evaluation to the HookExecutor implementation
- For each executable hook, output the following based on its `optional` flag:
  - **Optional hook** (`optional: true`):
    ```
    ## Extension Hooks

    **Optional Hook**: {extension}
    Command: `/{command}`
    Description: {description}

    Prompt: {prompt}
    To execute: `/{command}`
    ```
- **Mandatory hook** (`optional: false`):
  ```
  ## Extension Hooks

  **Mandatory Hook**: {extension}
  Command: `/{command}`
  Phase: `after_taskstoissues`
  Description: {description}

  Before invoking this hook, show the user its exact extension, phase, command, and description and ask for explicit approval. Do not invoke it unless the user approves. If approval is not given, stop workflow.
  ```
