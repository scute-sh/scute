---
name: feature-development
description: Autonomous end-to-end feature development with phased validation gates. Use when building major new functionality or adding substantial features.
---

# Feature Development

STARTER_CHARACTER = 🏗️

Structured workflow for complex features. Each step invokes a focused skill with its own gate. No step may be skipped. No gate may be skipped.

## 1. Define Acceptance Criteria

_Invoke_ `/acceptance-criteria`. Write criteria to `playground/{feature-name}-criteria.md`.

Use a short, hyphenated feature name (e.g., `user-auth`, `invoice-export`) consistently across all playground files.

## 2. Assess Existing Code

If modifying existing code, _invoke_ `/scout-codebase`. Write scout report to `playground/{feature-name}-scout.md`. Once validated, scout report improvements become preparatory scenarios in step 3.

If greenfield, review the surrounding codebase for conventions, patterns, test infrastructure, and architecture the new code must align with.

## 3. Sketch Scenarios

_Invoke_ `/sketch-slices`. Write sketch to `playground/{feature-name}-sketch.md`.

## 4. Build

_Invoke_ `/atdd` for each scenario from the sketch — preparatory first, then feature scenarios.

## 5. Harden

_Invoke_ `/harden` on all files touched.

## 6. Verify

_Invoke_ `/verify-criteria` against `playground/{feature-name}-criteria.md`.

## Cleanup

Remove playground files from disk (`{feature-name}-criteria.md`, `{feature-name}-scout.md`, `{feature-name}-sketch.md`). These are working files, not tracked by git.

## When to Check In With User

Stay autonomous, but stop and ask when:

- Requirements have genuine ambiguity affecting architecture
- Trade-off with no clear winner
- Scope creep detected
- Blocked by external factors
- Scout report reveals significant debt

Don't ask about: routine implementation, refactoring choices, test structure, naming.
