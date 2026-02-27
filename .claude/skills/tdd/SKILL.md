---
name: tdd
description: Test-driven development (TDD) process used when writing code. Use whenever you are adding any new code, unless the user explicitly asks to skip TDD or the code is exploratory/spike.
---

# Test-Driven Development Process

TDD is a design technique that uses tests as a tool. Design emerges from usage, not speculation. Short feedback loops let you course-correct immediately. The resulting architecture is testable by design, not retrofitted. We are not trying to rush towards a feature completion, it's important that the code is correct and well-designed, it's crucial to be thorough and only add what tests demand. 

When starting, announce: "Using TDD skill in mode: [auto|human]"

MODE (user specifies, default: auto)
- auto: DO NOT ask for confirmation or approval. Proceed through all steps without stopping.
- human: wait for confirmation at key points

STARTER_CHARACTER = 🔴 for red test, 🌱 for green, 🌀 when refactoring, always followed by a space

## The Cycle

RED → GREEN → REFACTOR. Every test.

Anti-pattern: RED → GREEN → RED → GREEN → ... → REFACTOR at the end.

## Core Rules

1. ALL code changes follow TDD. Feature requests mid-stream are NOT exceptions. Write test first, then code.
2. **One test at a time.** Write one failing test, make it pass, refactor. Then think about the next test. Never write multiple tests at once.
3. **The production API emerges from tests.** Do not design method signatures, interfaces, or class structures ahead of what a test demands. If no test has required a parameter, don't add it. If no test has required a class, don't create it.
4. Predict failures. State what we expect to fail before running tests.
5. Two-step red phase:
   - First: Make it fail to compile (class/method doesn't exist)
   - Second: Make it compile but fail the assertion (return wrong value)
6. Minimal code to pass. Just enough to make the test green. If no test requires it, don't write it. When a test verifies a collaboration (A calls B), "pass" means B's contract exists — not that B works internally. Create the signature, return a hardcoded value. B's behavior emerges from B's own tests.
7. No comments in production code. Keep it clean unless specifically asked.
8. Run all tests every time. Not just the one you're working on.
9. Refactor after every green, not at the end.
10. Test behavior, not implementation. Check responses or state, not method calls.
11. Push back when something seems wrong or unclear.

## Test Planning

1. Think about what the code should do from the **caller's perspective** — not how it works internally
2. Sketch the first few tests as single-line `[TEST]` comments. Start with the simplest case. This is a starting direction, not a comprehensive list.
3. **Self-check: are you describing what happens, or how it happens?** Each test should name a behavior or outcome. If you catch yourself naming error classes, internal functions, database operations, or implementation mechanisms — you're pre-designing the solution. Rewrite the test.
   ```
   # These presuppose implementation — rewrite them
   [TEST] Throws ForbiddenError when user is not owner
   [TEST] Calls mapsDao.transferOwnership
   [TEST] Emits transfer tracking event
   ```
   These lock in error types, internal calls, and infrastructure before a single test runs. Describe what the caller observes instead — leave the how to emerge from making tests pass.
4. **This list will evolve.** New tests emerge during implementation. Tests you planned may turn out wrong. That's expected.
5. **Do not** plan all edge cases upfront. Comprehensive coverage (ZOMBIES) happens in Final Evaluation after the core behavior exists.
6. If MODE is human, wait for confirmation after test planning

## Implementation Phase

1. Replace the next [TEST] comment directly with a failing test. No intermediate markers.
2. Test should be in format given-when-then (do not add as comments), with empty line separating them
3. Think through the expected value BEFORE writing the assertion. Trace the logic step by step.
4. Predict what will fail
5. Run tests, see compilation error (if testing something new)
6. Add minimal code to compile
7. Predict assertion failure
8. Run tests, see assertion failure
9. Add minimal code to pass
10. Predict whether the tests will pass and why. Run tests, see green
11. Simplify. For each line/expression you just added, ask: "Does a failing test require this?"
    - If no test requires it, delete it or if it's necessary, add a test comment to write that test
    - Run tests after each simplification
    - Repeat until every line is justified by a test
12. Refactor. Zoom out — evaluate the effect of your changes on the surrounding code, not just the diff.
    - "What did this change do to the code around it?" Is the receiving function/class/module still cohesive? Growing too many parameters, responsibilities, or cases?
    - Missing domain concept? Duplication? Abstraction waiting to emerge?
    - New abstractions are allowed. New behavior is not.
    - If improving: `🌀 Refactoring: [list improvements]`
    - If clean: `🌀 Clean`
    - One change at a time, run tests after each
13. Go to step 1 for the next [TEST] comment. Repeat until all planned tests are passing.

## Final Evaluation

1. Now walk through [ZOMBIES](references/zombies.md) for comprehensive coverage:
   - **Z**ero/empty cases? **O**ne item? **M**any items? **B**oundary transitions? **I**nterface clarity? **E**xceptions/errors? **S**imple overlooked cases?
2. If there are gaps, add `[TEST]` comments for the missing cases and run them through the full Implementation Phase (red → green → refactor, one at a time).
3. Is anything still hardcoded in the code that shouldn't be? Fix it, analyze test gaps and go back to previous stages if needed.
4. Analyze code expressiveness and quality. If there's anything to improve, go to refactoring phase.
