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

- **Ambiguous test names.** Names don't communicate the scenario. Not
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
- **Test pyramid violations.** Behavior verified at the wrong level.
  Integration tests asserting on domain-level details that belong in unit
  tests, or unit tests mocking so heavily they're testing wiring instead of
  logic. Each layer should own its own concerns: unit tests verify behavior,
  integration tests verify that layers talk to each other.
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
- **Premature optimization.** Adding complexity to avoid trivial costs.
  Extra variables, match arms, or type gymnastics to save an allocation
  that doesn't matter. Simplicity wins until profiling says otherwise.
- **Poor variable names.** Names that don't communicate intent or that mislead.
- **Formatting and linting not enforced.** Tools exist but aren't wired into
  the workflow. The codebase drifts from its own standards one PR at a time.
  Trivially automatable, yet consistently overlooked.

## Architecture

- **Separation of concerns violated.** Responsibilities leak across boundaries
  (e.g. configuration logic embedded in domain code).
- **SOLID / Tell Don't Ask violations.** Objects exposing internals instead of
  encapsulating behavior. Dependency inversion ignored.
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

## Documentation

- **Language-level docs ignored.** Doc comments (GoDoc, Rustdoc, JSDoc) left
  empty or missing on public interfaces.
- **CLI docs drift from implementation.** The documented CLI commands don't
  match what the tool actually does. The source of truth and the docs diverge.

---

## How to Read This

Each entry describes a **problem**, not a solution. Some map cleanly to
deterministic checks (commit message format, test complexity metrics). Others
are judgment calls that may need heuristic proxies (test readability, naming
quality). A few sit at the boundary of what static analysis can catch.

The value is in the catalog itself: knowing what goes wrong tells us where
enforcement matters most.
