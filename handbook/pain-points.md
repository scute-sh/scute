# Pain Points

Common bad practices across the software delivery lifecycle. Each entry is a
problem that erodes codebase health over time, and a potential opportunity for
a Scute fitness check. Coding agents push these problems to unmanageable
scale because they cut the same corners, just faster, more consistently, and
with more creative workarounds.

This is a living backlog. Not everything here is automatable today. What
matters is mapping the problem space so we know where enforcement can add
value.

---

## Commits

- **Message conventions ignored.** Commit messages that bypass agreed-upon
  standards (e.g. Conventional Commits). The format is known and
  machine-checkable.
- **Oversized commits.** Too many changes bundled into a single commit, making
  review harder and bisecting less useful.
- **Breaking changes unannounced.** Public API signatures changed without
  corresponding version bumps, changelog entries, or migration notes.

## Tests

- **Ambiguous test names.** (×1) Names don't communicate the scenario. Not
  structured around Given/When/Then or any recognizable pattern.
- **Bloated test bodies.** Tests that are too long to scan. The story they tell
  is buried under setup noise and assertion volume.
- **Logic in tests.** Loops, conditionals, and computation inside test code.
  Tests should be linear and declarative.
- **Magic values.** Unnamed literals scattered through assertions with no
  indication of what they represent or why that value matters.
- **No test helpers or factories.** Repeated, verbose setup instead of
  expressive helpers that name the intent.
- **Duplicate test coverage.** Multiple tests validating the same behavior
  without adding signal. Noise that slows the suite and obscures gaps.
- **Over-specified test inputs.** Test data more complex than what the test
  actually proves. The extra detail couples the test to unrelated features,
  making it fragile: changes to those features break tests that weren't
  testing them.
- **Helpers that hide the story.** Test helpers that wrap both the action and
  the assertion, making individual tests opaque. Helpers should reduce
  construction boilerplate, not obscure the test narrative.
- **Weak assertions.** Generic boolean checks instead of specific value
  comparisons. They hide the expected value and produce useless failure
  messages.
- **Test pyramid violations.** (×1) Behavior verified at the wrong level.
  Each layer should own its own concerns.
  - Integration tests asserting on domain-level details that belong in unit
    tests.
  - Unit tests mocking so heavily they're testing wiring instead of logic.
  - Unit tests with handcrafted external tool output testing assumptions
    about a contract instead of border tests validating the real interaction.
- **Core logic left untested.** Tests skipped entirely, jumping straight to
  implementation. TDD isn't mandatory, but core logic without tests is a
  fitness problem.

## Code Quality

- **Clean code violations.** General readability and maintainability rules
  broken: long functions, deep nesting, unclear control flow.
- **Complex conditions.** Compound boolean expressions, especially negated
  ones, that obscure intent. An intermediate variable that names what the
  condition means is worth the extra line.
- **Duplication (DRY).** Copy-pasted logic instead of extracting shared
  behavior. Includes structural duplication: same operation applied to
  different data, with only the data source varying.
- **Duplicated domain strings.** The same concept hardcoded as a string
  literal in multiple places. The coupling is implicit and breaks silently
  when one occurrence changes without the other.
- **Overparameterization.** (×1) Extracting a helper but keeping the
  hardcoded value as a parameter instead of encapsulating it. If every
  call site passes the same value, the helper should own that value.
- **Solution-space anchoring.** (×1) Fixating on how existing code works
  (helpers, models, constructors) instead of stepping back to the problem
  space. Leads to contorted designs that serve the implementation rather than
  the user. The question should always be "what does the consumer need?" not
  "how do I make the data fit the current helper?"
- **Premature design.** (×2) Designing based on anticipated needs instead of
  letting usage drive the shape. If nothing uses it yet, it shouldn't exist yet.
- **Premature optimization.** Adding complexity to avoid trivial costs.
  Extra variables, match arms, or type gymnastics to save an allocation
  that doesn't matter. Simplicity wins until profiling says otherwise.
- **Poor variable names.** (×3) Names that don't communicate intent or that mislead.
  Defaulting to abbreviations instead of treating naming as a design decision.
  Repeatedly corrected on the same pattern.
- **Formatting and linting not enforced.** (×2) Tools exist but aren't wired
  into the workflow. The codebase drifts from its own standards one PR at a
  time. Trivially automatable, yet consistently overlooked.

## Architecture

- **Separation of concerns violated.** Responsibilities leak across boundaries
  (e.g. configuration logic embedded in domain code).
- **SOLID / Tell Don't Ask violations.** (×3) Objects exposing internals
  instead of encapsulating behavior.
  - Dependency inversion ignored: high-level policy coupled to low-level
    detail (e.g. check logic welded to a specific tool's output format).
  - SRP violated: parsing, business logic, and output construction bundled
    in one function.
  - OCP violated: no extension point for new tool adapters, requiring
    modification instead of extension.
  - Law of Demeter violated: callers reaching through chains of fields
    (`result.evaluation.status`) instead of asking the object what they
    actually need (`result.is_fail()`). Exposes internal structure and
    breaks when it changes.
- **Layer boundary violations.** Imports that cross architectural layers
  directly. This goes beyond a single check and applies broadly across module
  boundaries.
- **UI concerns leaking into domain.** Serialization formats, framework types,
  or presentation logic creeping into core domain code. A domain module
  returning `serde_json::Value` instead of its own types is a classic example.
  The domain should be infrastructure-agnostic; serialization belongs at the
  edges.

## Dependencies

- **Unused dependencies.** Libraries added speculatively or left behind after
  a refactor. They inflate build times, expand the attack surface, and mislead
  readers about what the code actually uses.
- **Duplicate dependencies.** The same library declared in multiple dependency
  sections or at multiple versions. Causes confusion about which version is
  active and bloats the dependency tree.
- **Dependency freshness.** Dependencies falling behind on major versions.
  Stale deps accumulate security vulnerabilities and make future upgrades
  painful.

## API Design

- **Duplicated function signatures.** Multiple functions that do nearly the
  same thing (e.g. `Check` vs `CheckWithConfig`), adding cognitive load instead
  of a single composable interface.
- **Breaking public API changes.** Signature changes to public APIs without
  versioning discipline.

## Process

- **Superficial application of agreed conventions.** (×1) Convention is understood
  in principle but applied inconsistently. Fixes address only the examples
  explicitly called out, missing the same issue elsewhere. The pattern is
  understood, but the rigor to apply it exhaustively is missing.
- **Checklists skipped under momentum.** (×9) Established workflows and
  checklists exist but get bypassed when focus is on "just get the thing done."
  The process is known, the trigger is clear, but urgency wins over discipline.
  Especially common with agents who optimize for task completion over process
  compliance.
- **Exploratory poking instead of testing.** (×1) When exploring how something
  behaves (an API, an edge case, a library quirk), the agent reads source or
  writes throwaway scripts instead of writing a test. The test harness gives
  faster feedback and the answer persists as documentation.
- **Docs/examples not consulted before building.** (×2) Framework documentation
  and official examples show the idiomatic pattern, but the agent invents a
  manual approach instead of reading the docs first. Wastes time and produces
  non-idiomatic code that needs to be redone.
- **Available tools not used contextually.** (×1) A tool exists, is described,
  and is clearly relevant to the current action, but the agent doesn't reach
  for it because it's not on a written checklist. The tool descriptions are in
  context but treated as reference material instead of active capabilities.
  Agents follow static lists instead of thinking "what tools do I have for
  this?"

## Documentation

- **Language-level docs ignored.** Doc comments (GoDoc, Rustdoc, JSDoc) left
  empty or missing on public interfaces.
- **CLI docs drift from implementation.** The documented CLI commands don't
  match what the tool actually does. The source of truth and the docs diverge.
- **User-facing copy written without verifying behavior.** (×1) Help text,
  descriptions, or argument docs describe behavior the code doesn't actually
  have. The copy is plausible but wrong because nobody checked the
  implementation before writing it.

## Agent-Driven Codebase Decay

The entries above describe bad practices that agents amplify. The entries below
are different: they describe **emergent properties** of codebases where agents
are the primary code producers. Individual commits look fine. The damage is
cumulative and only visible when you zoom out.

The root cause is the same across all of them: **agents optimize locally, not
globally.** They solve the task in front of them without considering the
codebase as a whole. A human dev with taste would feel the drift, open a file
and think "this is getting unwieldy." An agent never has that moment.

- **Additive-only growth.** Agents add code but never subtract. No proactive
  refactoring, no splitting a module that's taken on too much responsibility.
  The codebase grows linearly with features instead of logarithmically. Files
  get longer, modules accumulate concerns, but nothing gets restructured
  unless someone explicitly asks. Over time, the cost of each new change
  increases because nothing was simplified along the way.
- **Structural duplication.** Not copy-paste (that's detectable with basic
  tooling). Agents write new functions that do 80% of what an existing
  function does because they didn't look hard enough or didn't think about
  composability. Three slightly different implementations of the same logic,
  each locally clean, collectively a maintenance burden. The duplication is
  semantic, not textual, which makes it invisible to naive detection.
- **Global incoherence.** Each change is locally correct but globally
  inconsistent. One module uses pattern A, another uses pattern B for the
  same problem. No ubiquitous language, no conceptual integrity across
  modules. The codebase reads like it was written by a different person every
  day, because it effectively was. Nobody is minding the whole.
- **Unnecessary abstraction.** Interfaces with one implementation. Factories
  that build one thing. Strategy patterns where a plain function would do.
  Agents apply patterns from their training data regardless of whether the
  current context warrants them. The result is over-engineered code that's
  harder to read and harder to change than the naive version would have been.
- **Module bloat.** Files and modules keep growing instead of being split.
  Public API surface expands disproportionally to functionality. No agent
  looks at a module and thinks "this has too many responsibilities." They
  add the next function where it seems to fit, and the module quietly
  becomes a god object.
- **Hidden change coupling.** Files that always change together but aren't
  co-located. The import graph looks clean, but in practice, touching
  module A means you always need to touch module B. These invisible
  dependencies accumulate because agents don't track cross-cutting change
  patterns across sessions. The coupling only surfaces when a "simple
  change" cascades into five files across three modules.

---

## How to Read This

Each entry describes a **problem**, not a solution. Some map cleanly to
deterministic checks (commit message format, test complexity metrics). Others
are judgment calls that may need heuristic proxies (test readability, naming
quality). A few sit at the boundary of what static analysis can catch.

The value is in the catalog itself: knowing what goes wrong tells us where
enforcement matters most.
