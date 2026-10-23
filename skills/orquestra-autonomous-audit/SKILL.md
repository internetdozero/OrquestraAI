---
name: orquestra-autonomous-audit
description: Time-boxed, resumable code audit and hardening. Runs bounded multi-scope sweeps, fixes verified defects with reproduction tests and atomic local commits, records negative passes and checkpoints, and optionally records backlog ideas.
---

# Autonomous Code Audit

Run a structured audit of the requested repository scopes. The duration is an audit budget, not a promise that one model invocation or harness process will remain alive until the deadline. Work in bounded cycles, preserve progress in the ledger, and resume from it when the skill is invoked again. Never claim the full time window was completed unless it actually elapsed.

## 1. Parameters and defaults

When activated through the host's registered skill/command entry point, accept:

- **Duration:** A requested wall-clock budget such as `60m`, `120m`, or `180m`; default `120m`. Respect the host's execution limits. Do not guess a slash-command alias from this skill name; the host controls command registration.
- **Scopes:** A comma-separated list such as `security, api, database`. If omitted, use these four scopes:
  1. **Security & Hardening:** path/symlink traversal, injection, input validation, secret exposure, unsafe deserialization.
  2. **Performance & Resources:** leaks, unbounded reads/parsing, hot-path contention, event-loop stalls, unnecessary re-renders.
  3. **Reliability & Logic:** panics on external input, unhandled errors, races, configuration drift, edge cases.
  4. **Workflow & DX:** objectively dead code, redundant configuration, project file-size limits, broken contracts.

Normalize requested scope names to these canonical labels in the ledger. Do not add scopes outside the operator's request without asking.

## 2. Safety and scope invariants

1. **Bounded, resumable execution:** Never stop merely because one defect was fixed. After each completed defect, checkpoint it and continue to the next declared scope while the current invocation and execution limits permit. If the invocation must end, leave a precise checkpoint for the next activation. Do not use `sleep`, `timeout`, or other idle waiting to consume the budget; do not claim to run continuously between invocations.
2. **Stop conditions:** Stop when the requested budget expires, the operator cancels, the host limits execution, all requested scopes and any explicitly authorized deepening work are complete, or no safe, actionable work remains. If blocked by missing permission, dependencies, or unavailable services, record the blocker and stop that action. Do not keep working solely to fill time.
3. **No manufactured work or scope creep:** A negative pass is a valid result. Do not create tests solely to increase coverage unless they address a demonstrated risk within scope. Treat larger refactors and unrelated backlog items as recommendations, not authorization to implement them. Deepening vectors below are optional and may be used only when in scope and within the operator's audit request.
4. **Preserve workspace state:** At kickoff, inspect the current branch and working tree. Record pre-existing changes and never overwrite, stage, or commit them. If ownership of a changed file or hunk is unclear, leave it untouched and report the ambiguity. Stage only files/hunks created for the current defect; never use broad staging such as `git add -A`.
5. **Atomic local commits:** One verified defect per commit, using a concise Conventional Commit subject (`fix(scope): ...`, `perf(scope): ...`, etc.). Never push, merge, deploy, or perform destructive/external actions unless the operator explicitly authorizes that specific action.
6. **Reproduction evidence:** For a behavioral fix, add or identify a test that fails for the defect before the patch and passes after it. Confirm the pre-patch failure is due to the target behavior, not a broken test or environment. For non-behavioral cleanup, capture a relevant check demonstrating the issue and its removal. If proof cannot be obtained safely, do not claim the defect is fixed; record the limitation.
7. **Verification proportional to change:** Run the focused test/check and relevant project gates. Run broader suites when feasible and relevant; report what was not run. Do not weaken tests or suppress errors to obtain a pass.
8. **Scratch-file safety:** Do not write temporary files to RAM-backed `/tmp/`. Prefer repository-local fixtures or `/var/tmp/testing/` when a temporary file is necessary.

## 3. Audit cycle

For each declared scope:

1. Inspect relevant code and tests; keep a concise record of files and vectors examined.
2. Decide whether a finding is reproducible and in scope. Do not report speculation as a defect.
3. If no defect is found after the planned scan, record a formal negative pass and move on.
4. For a confirmed defect, create/confirm the reproduction, verify it fails for the expected reason, apply the smallest safe patch, and run the relevant checks.
5. Review the exact diff and stage only the task-owned changes. Commit one defect at a time; never include unrelated or pre-existing changes.
6. Append the completed result to the audit ledger, then continue with the next declared scope if execution remains available.

A failed check is a diagnosis point, not a reason to skip it. Fix the cause when safely in scope; otherwise preserve the failure details and checkpoint the blocker.

## 4. Negative passes and findings

Do not manufacture artificial or trivial edits to avoid a negative result. Record negative passes with the current HEAD and concrete scan evidence:

```markdown
- **Commit:** `<current_HEAD_short_hash>`
- **Files:** none
- **Scope:** `<Security & Hardening | Performance & Resources | Reliability & Logic | Workflow & DX>`
- **Diagnosis:** Negative pass. Inspected `<vectors/files>`; no reproducible in-scope defect found.
```

For a finding, describe the root cause and fix, not just the symptom. Distinguish verified defects, suspected risks, and blocked checks in the diagnosis.

## 5. Ledger and resume checkpoints

Use the repository-local ledger:
`./.orquestra/audits/audit-YYYY-MM-DD.md` (create the directory if needed, after checking existing state).

Each completed finding or negative pass uses this four-field schema:

```markdown
- **Commit:** `<hash, or current HEAD for a negative pass>`
- **Files:** `<file1>, <file2>` (or `none`)
- **Scope:** `<canonical scope label>`
- **Diagnosis:** `<concise evidence-based diagnosis or negative-pass evidence>`
```

Maintain a `## Shift Checkpoint` section with:

- requested budget and scopes;
- scopes completed and remaining;
- current investigation or finding status;
- last completed commit/check;
- the single next action needed to resume;
- blockers and checks not run.

On activation or after context compaction, inspect the ledger, Git status, and recent commits before continuing. Reconcile any commit that exists without a ledger entry before making new changes; do not duplicate its patch or commit. Update the checkpoint after each completed commit/pass and before ending an invocation. At the end, summarize verified results, remaining scopes, blockers, and the exact resume point. A checkpoint is not evidence that the full requested duration elapsed.

## 6. Optional deepening and backlog integration

After declared scopes are complete, stop unless the operator explicitly authorized additional deepening. If authorized, choose only relevant vectors:

- test gaps tied to a demonstrated risk;
- hostile or malformed inputs to in-scope public entry points;
- type/lint issues directly related to the declared scopes;
- existing backlog items explicitly selected by the operator.

Do not implement a broad architectural improvement merely because it appears in a backlog. Record it as a recommendation instead.

When `idea-vault` is connected, it may be used to record a concrete, non-trivial follow-up idea or post an alert for a verified issue requiring another specialist. Do not duplicate existing items; do not change an item's lifecycle unless the operator asked to work on that item. If the MCP is unavailable, record relevant recommendations under `### Open Backlog & Future Tasks` in the ledger. Optional integrations must never block the audit.
