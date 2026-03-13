---
name: scout-codebase
description: Assesses existing code before modifying it: evaluates clarity, test coverage, and technical debt, then produces a scout report with improvement items. Use when evaluating code quality, assessing an area before changes, or scouting unfamiliar code.
---

# Scout Codebase

STARTER_CHARACTER = 🔭

Understand existing code you'll touch and identify what needs improving. No design work — that comes later.

## Process

1. **Read the file(s)** you'll modify
2. **Evaluate current state:**
   - Code clarity — readability for someone unfamiliar with this area
   - Test coverage — existing tests for the areas you'll touch
   - Technical debt — long functions, duplication, poor names
3. **Write scout report** to `playground/{feature-name}-scout.md`:

   ```
   # Scout Report: [area/files being modified]

   ## Current State
   - [Observations about existing code quality]

   ## Test Coverage
   - [Existing tests for this area, or lack thereof]

   ## Improvements
   - [ ] [Specific improvement — refactoring, test coverage gap, technical debt]
   ```

## Gate

_Invoke_ `/refinement-loop` on the scout report with these checks:

- **Observations describe behavior, not gaps.** Litmus test: would this observation be noteworthy if you were building something completely different? If not, you're describing the code's relationship to your plans, not the code itself.
  Anti-patterns: "No X wrapper", "Does not expose Y", "Will need to", "Should be extended"
- **Improvements address existing problems, not feature prerequisites.** Each improvement fixes debt that exists independent of any planned work.

Do not consider this done until every line passes every check.
