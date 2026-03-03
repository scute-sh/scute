---
name: feature-development
description: Autonomous end-to-end feature development with phased validation gates. Use when building major new functionality or adding substantial features.
---

# Feature Development

STARTER_CHARACTER = 🏗️

Multi-phase workflow for complex features: requirements → design → TDD → hardening → verification. Each phase has a hard validation gate. No phase may be skipped. No gate may be skipped.

**Gates that require `/refinement-loop` cannot be self-approved.** You must actually invoke the skill and run it to completion. Reading the checks yourself and deciding "these look fine" is not a substitute — the refinement loop exists to force iteration you wouldn't do on your own.

## Phases

1. **Understand** — Clarify what we're building (gate: acceptance criteria)
2. **Assess** — Evaluate existing code we'll touch (gate: scout report)
3. **Sketch** — Lightweight design (gate: slices defined)
4. **Build** — TDD-driven implementation (gate: slice passes criteria)
5. **Harden** — Edge cases, production-readiness (gate: checklists pass)
6. **Verify** — Final validation (gate: all criteria demonstrated)

---

## Phase 1: Understand

**Goal**: Know exactly what success looks like before writing code.

1. Restate the requirement in your own words
2. Identify:
   - **Inputs**: What data/events trigger this feature?
   - **Outputs**: What should change? What should users see?
   - **Boundaries**: What's explicitly out of scope?
   - **Unknowns**: What needs clarification?
3. If unknowns exist, ask the user — don't guess on important decisions
4. Write acceptance criteria to `playground/{feature-name}-criteria.md`:

   ```
   # [Feature Name] Acceptance Criteria

   ## Must Have
   - [ ] Criterion 1 (specific, testable)
   - [ ] Criterion 2

   ## Out of Scope
   - Item explicitly excluded
   ```

Use a short, hyphenated feature name (e.g., `user-auth`, `invoice-export`) consistently across all playground files.

### Gate: Criteria Validation

**Invoke `/refinement-loop`** on `playground/{feature-name}-criteria.md` with these validation criteria:

- Specific enough to test? (not "works well" but "returns X when given Y")
- Complete? Walk through user's journey — any gaps?
- Bounded? Clear what's NOT included?

**Do not proceed to Phase 2 until every criterion passes every check.**

---

## Phase 2: Assess

**Goal**: Understand existing code we'll touch and identify what needs improving. No design work — that emerges from TDD.

**Greenfield** (no existing files to modify): No scout report needed, but review the surrounding codebase for conventions, patterns, test infrastructure, and architecture the new code must align with. Then proceed to Phase 3.

**Modifying existing code**:

1. **Read the file(s)** you'll modify
2. **Evaluate current state**:
   - Code clarity — readability for someone unfamiliar with this area
   - Test coverage — existing tests for the areas we'll touch
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

### Gate: Scout Validation

**Invoke `/refinement-loop`** on `playground/{feature-name}-scout.md` with these validation criteria:

- **Observations describe behavior, not gaps.** Litmus test: would this observation be noteworthy if you were building a completely different feature? If not, you're describing the code's relationship to your plans, not the code itself.
  Anti-patterns: "No X wrapper", "Does not expose Y", "Will need to", "Should be extended"
- **Improvements address existing problems, not feature prerequisites.** Each improvement fixes debt that exists independent of the feature.

**Do not proceed to Phase 3 until every line passes every check.**

Once validated, scout report improvements become **preparatory slices** in Phase 3. They go through the same build loop and gates as feature slices.

---

## Phase 3: Sketch

**Goal**: Enough structure to start, not a complete design. Implementation details (function signatures, endpoint paths, field names, validation) emerge from TDD in Phase 4 — do not pre-decide them here.

1. Identify areas/modules involved (not specific classes or functions)
2. Sketch the user-visible flow (what the user does → what happens → what they see)
3. Note integration points with existing code
4. Define **slices** in two categories, preparatory first:

   **Preparatory slices** (from scout report improvements):
   - Each addresses one improvement from the scout report
   - **Code-observable**: Tests prove the improvement (characterization tests, better structure)
   - **Focused**: One concern per slice (one refactoring, one test coverage gap)
   - Ordered before feature slices — they prepare the ground

   **Feature slices**:
   - **User-observable**: A stakeholder could see it working
   - **End-to-end**: Cuts through all layers (not split by layer)
   - **Small**: Completable in one focused session

Write to `playground/{feature-name}-sketch.md`:

```
# [Feature] Design Sketch

## Areas Involved
- [Area/module]: [what it's responsible for in this feature]

## User Flow
1. User does [action] → 2. System [responds] → 3. User sees [result]

## Slices (ordered)

### Preparatory (from scout report)
- [Code improvement]: addresses [scout item X]

### Feature
- [User-observable outcome]: addresses [criterion X]
- [Next outcome]: addresses [criterion Y]
```

**Stay high-level.** The sketch should read like a conversation about the feature, not like a technical spec. If you're writing function signatures, endpoint paths, or database field names, you've gone too deep — pull back.

**Anti-pattern**: Slices by layer ("Backend first, then Frontend") are tasks, not slices. Re-slice until each delivers observable value.

### Gate: Sketch Validation

**Invoke `/refinement-loop`** on `playground/{feature-name}-sketch.md` with these validation criteria:

- **Preparatory slices**: Each maps to a scout report improvement? One concern per slice?
- **Feature slices**: Could demo to a stakeholder? Maps to an acceptance criterion? Touches all relevant layers?
- **Both**: Free of implementation details? (no function names, no endpoint paths, no schema details)

**Do not proceed to Phase 4 until every slice passes every check.**

---

## Phase 4: Build

**Goal**: Working, tested implementation.

### Test Strategy

Choose based on slice type:

- **Preparatory** (refactoring, test gaps) → `/tdd` for characterization, then `/refactoring`
- **Behavior-heavy** (user flows, rules) → `/bdd-with-approvals`
- **Algorithm/logic-heavy** → `/tdd`
- **Integration/API** → `/tdd` with contract tests
- **UI/Output formatting** → `/approval-tests`

The invoked skill handles red-green-refactor. This skill handles orchestration.

### Build Loop (per slice)

For each slice from the sketch:

#### Preparatory slices (refactoring, test coverage)

1. **Characterization tests first** — if modifying untested code, use `/tdd` to add tests that capture current behavior before changing anything. These tests are the safety net for the refactoring that follows. Do not skip this step.
2. **Refactor** — invoke `/refactoring` on the affected area. Follow the refactoring skill's full process: small steps, tests green after each change.
3. **Verify no regressions** — read back the refactored code with fresh eyes and check that the refactoring didn't degrade:
   - Error handling still intact? No swallowed exceptions?
   - Logging still captures key events? No lost observability?
   - No sensitive data exposed through new code paths?
   - Performance characteristics preserved?
4. **Run every gate check below** — all must pass before moving on.

#### Feature slices

1. **Identify the outermost layer** for this slice — where the user interaction changes. This is where you start, not where "the real logic" lives:
   - Web feature → the UI component where the new affordance appears
   - API feature → the controller/endpoint
   - CLI feature → the command handler
     Even if it's "just" adding a prop and a menu item, that's your starting point. The outermost layer defines what inner layers must provide.
2. **Write your first test for that outermost layer.** Invoke the chosen test skill. Stub everything below — hardcoded data, fake responses, in-memory storage. The outermost layer should work with stubs before any inner layer exists.
3. **Work inward one layer at a time.** Each layer gets its own tests. Replace the stub from the layer above with real implementation. Then stub the next layer down. Repeat until you reach storage.

**If you find yourself reasoning about why outside-in doesn't apply to this specific case, you are rationalizing. Common excuses:**

- "The outermost layer has no tests yet" → That's exactly why you start there. You're building the test infrastructure as part of the feature.
- "The real logic/work is in [deeper layer]" → Outside-in discovers what inner layers need. Inside-out guesses. You're drawn to where business rules live because it feels like "real work." The UI feels like "just wiring." This is backwards.
- "I'll be pragmatic" → Outside-in IS pragmatic. It catches integration issues early and prevents building the wrong API.
- "The backend needs to exist first" → No. That's what stubs are for. The UI works with stubs before any backend exists.
- "This component already exists, so I can skip this layer" → The outermost layer is the component that USES the shared one. A reusable dialog still needs integration: new props, new menu items, new wiring. Start there.

4. **Read back code** — is it clear? If not, refactor before moving on.
5. **Run every gate check below** — all must pass before moving on.

All slices done → Phase 5.

### Stop If You Catch Yourself

These are not suggestions. If you detect any of these, stop immediately and correct course:

- **Adding to long functions** — extract first, then add
- **Copy-pasting with modifications** — extract shared logic
- **Skipping tests "temporarily"** — write the test first
- **Building bottom-up** — start from user interaction, work inward
- **Ordering work inside-out** — if your plan reads "infrastructure → service → controller → UI," reverse it. The order you build is the order you list.
- **"The real work is in [deeper layer]"** — this is bottom-up rationalized as pragmatism
- **"I'll refactor later"** — refactor now while context is fresh

### Gate: Slice Complete

**All four** checks must pass before moving to next slice:

1. All tests pass
2. Code read-back: comfortable explaining this in a code review?
3. Update tracking:
   - **Preparatory**: Mark item complete in `playground/{feature-name}-scout.md`
   - **Feature**: Mark satisfied criteria in `playground/{feature-name}-criteria.md`
4. Commit with message describing the change

### Course Correction

After each slice gate passes, before starting the next:

- Does what you learned invalidate remaining slices? → return to Phase 3, re-sketch remaining slices only
- Is an acceptance criterion wrong or ambiguous? → return to Phase 1, clarify with the user

Completed work stays — only adjust what's ahead.

---

## Phase 5: Harden

**Goal**: Production-ready, resilient code. Complete **all four** sub-steps.

### 5.1 Edge Cases (ZOMBIES)

Walk through **every** category, add tests for gaps found:

- **Z**ero/empty — null, empty string, zero, empty collection
- **O**ne — single item
- **M**any — multiple items, ordering
- **B**oundaries — min/max, off-by-one
- **I**nterfaces — API contracts, type precision
- **E**xceptions — error paths, failure modes
- **S**imple — obvious cases easily overlooked

### 5.2 Production Checklist

See [references/production-checklist.md](references/production-checklist.md) for full checklist.

Address **every** item (mark N/A with reason if not applicable):

- [ ] Error handling: graceful degradation, meaningful messages
- [ ] Logging: key events, debug info, no sensitive data
- [ ] Observability and analytics: can you tell if it's working and valuable?
- [ ] Resilience: timeouts, retries where appropriate
- [ ] Performance: latency acceptable, no obvious bottlenecks
- [ ] Security: OWASP Top 10 walkthrough
- [ ] Accessibility (if user-facing)

### 5.3 Refactoring Pass

Invoke `/refactoring` on all files touched. This is a required step, not optional.

### 5.4 Writing Style Review

If the feature includes user-facing text, invoke `/writing-style` on all text added or modified. Mark N/A if purely backend.

User-facing text includes:

- API docs (markdown files, rustdoc, jsdoc, etc.)
- Documentation (Readme, changelog, handbook, and other readme files)
- UI text, CLI output, MCP messages, etc.
- Test names, error messages, etc.
- Logging messages, alerting, observability, etc.

### Gate: Hardening Complete

**All five** checks must pass. **Do not proceed to Phase 6 until they do.**

- [ ] ZOMBIES walked through every category, tests added for gaps
- [ ] Production checklist: every item addressed (or N/A with reason)
- [ ] Refactoring pass complete
- [ ] Writing style reviewed (or N/A)
- [ ] All tests pass

---

## Phase 6: Verify

**Goal**: Confirm feature is complete and correct.

### Criteria Demonstration

Open `playground/{feature-name}-criteria.md`. For **every** criterion:

1. State the criterion
2. Cite the test(s) that prove it. If the criterion can't be verified by a test, explain why and provide alternative evidence.
3. Mark VERIFIED or FAILED

If any criterion is FAILED, return to the appropriate phase and fix it.

### Final Checks

Complete **all five**:

1. Run full test suite — all must pass
2. Review all commits — do they tell a coherent story?
3. Self-review: Read the diff as someone else's code. Address any comments you'd leave.
4. What could break in production that tests don't cover?
5. Update docs if the feature affects public APIs, user-facing behavior, or onboarding

### Gate: Feature Complete

**All five** checks must pass:

- [ ] Every acceptance criterion VERIFIED with evidence
- [ ] All tests pass
- [ ] Self-review complete, no outstanding concerns
- [ ] Documentation updated (or N/A)
- [ ] Summary written: what was built, key decisions, trade-offs

Only after all gates pass: Feature is complete.

### Cleanup

Delete playground files (`{feature-name}-criteria.md`, `{feature-name}-scout.md`, `{feature-name}-sketch.md`).

---

## When to Check In With User

Stay autonomous, but stop and ask when:

- Requirements have genuine ambiguity affecting architecture
- Trade-off with no clear winner
- Scope creep detected
- Blocked by external factors
- Scout report reveals significant debt

Don't ask about: routine implementation, refactoring choices, test structure, naming.
