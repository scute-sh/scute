---
name: issue-workflow
description: End-to-end GitHub issue lifecycle: pick up an issue, create a worktree, build the fix or feature, open a PR, watch CI, and iterate until green. Use when picking up, fixing, or implementing a GitHub issue or ticket.
---

# Issue Workflow

STARTER_CHARACTER = 🎫

Full lifecycle for working on a GitHub issue.

## 1. Understand the Issue

1. Read the issue and its comments
2. Restate the problem and the expected outcome in your own words
3. Identify blindspots — if something is ambiguous or underspecified, ask the user before proceeding. Do not assume.
4. Assign the issue to the user

You should be able to explain what needs to change and why, with no open questions.

## 2. Define Acceptance Criteria

Do not explore or read the codebase yet. Criteria describe the user's need, not the implementation. The issue itself has all the context you need.

_Invoke_ `/acceptance-criteria` to define what done looks like. Update the issue description to include the criteria — they are the shared contract and belong with the issue, not in a local file.

## 3. Prepare the Work

Assess issue complexity:

- **Substantial feature or complex change** → _invoke_ `/scout-codebase` (if modifying existing code), then _invoke_ `/sketch-slices` to decompose into scenarios
- **Small/focused fix** → skip to the next step

Enter a worktree for this issue using `EnterWorktree` — do not create one manually with git commands. Name it after the issue (e.g., `fix-login-redirect`, `add-export-csv`).

## 4. Build and Ship

Repeat for each slice (or the single fix):

### Build

Do not design upfront. No data models, no architecture decisions, no technical plans. Start the first scenario and let implementation emerge from the tests. The right design will reveal itself as you make tests pass.

_Invoke_ `/atdd` for substantial work, `/tdd` or the appropriate testing skill for small fixes.

### Commit and Open PR

A slice is not done until it has a PR. This is not optional.

1. Commit all changes for this slice
2. Push the branch
3. Open a PR:
   - Title: short, under 70 characters
   - Body: use the repo's PR template if one exists, otherwise write a concise description. Reference the issue with `Part of #<number>`.
4. Watch CI checks until they complete
   - If checks fail: read the failure logs, fix, commit, push, watch again
   - After 3 failed fix attempts, stop and ask the user for guidance
5. Report the PR URL to the user

**⛔ STOP.** Do not start the next slice. Wait for the user to confirm the PR is reviewed and merged. Then reset the worktree to latest main (`git reset --hard origin/main`) and proceed.

Each PR must not break existing behavior.

## 5. Harden and Close

Once all scenarios are complete:

1. _Invoke_ `/harden` on all files touched
2. _Invoke_ `/verify-criteria` against the criteria on the issue
3. Push, open a final PR with `Closes #<number>` in the body
4. Watch CI until green

## 6. Clean Up

Delete any `playground/` working files created during this workflow (sketches, scout reports, criteria files). These are working artifacts, not project files — they must not be committed.

Do not clean up the worktree — the user may want to keep it for follow-up work.

## When Done

Report to the user:
- PR URL(s)
- Summary of what was done
- Any decisions made along the way that the user should know about
