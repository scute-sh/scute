---
name: sketch-slices
description: Decomposes work into ordered, shippable slices. Use when breaking down features, planning implementation order, or slicing work into deliverable increments.
---

# Sketch Slices

STARTER_CHARACTER = ✂️

Enough structure to start, not a complete design. Implementation details (function signatures, endpoint paths, field names) emerge from building — do not pre-decide them here.

## Process

1. Sketch the user-visible flow (what the user does → what happens → what they see)
2. Note integration points with existing code
3. Define **slices** in two categories, preparatory first:

   **Preparatory slices** (from scout report improvements, if one exists):
   - Each addresses one improvement from the scout report
   - **Code-observable**: Tests prove the improvement (characterization tests, better structure)
   - **Focused**: One concern per slice (one refactoring, one test coverage gap)
   - Ordered before feature slices — they prepare the ground

   **Feature slices**:
   - **Persona-driven**: Written as "[Persona] does [action] and sees [result]" — if you can't name the persona, it's a task, not a slice
   - **End-to-end**: Cuts through all layers (not split by layer)
   - **Small**: Completable in one focused session

5. Write to `playground/{feature-name}-sketch.md`:

   ```
   # [Feature] Design Sketch

   ## Areas Involved
   - [Area/module]: [what it's responsible for]

   ## User Flow
   1. User does [action] → 2. System [responds] → 3. User sees [result]

   ## Slices (ordered)

   ### Preparatory (from scout report)
   - [Code improvement]: addresses [scout item X]

   ### Feature
   - [Persona] [does action] and [sees result]: addresses [criterion X]
   - [Persona] [does action] and [sees result]: addresses [criterion Y]
   ```

**Stay high-level.** The sketch should read like a conversation about the feature, not a technical spec. If you're writing function signatures, endpoint paths, or database field names, pull back.

**Anti-patterns**:
- Slices by layer ("Backend first, then Frontend") — tasks, not slices
- Slices by technical concern ("Score a function", "Add language support") — no persona, no observable outcome
- **Litmus test**: Can you demo it to a persona and they'd care? If not, re-slice.

## Gate

_Invoke_ `/refinement-loop` on the sketch with these checks:

- **Preparatory slices**: Each maps to a scout report improvement? One concern per slice?
- **Feature slices**: Names a persona doing something? Could demo to them and they'd care? Maps to an acceptance criterion? Touches all relevant layers?
- **Both**: Free of implementation details? (no function names, no endpoint paths, no schema details)

Do not consider this done until every slice passes every check.
