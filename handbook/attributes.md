# Project Attributes

What this project commits to. These attributes guide every decision, from
architecture to API design to how we write a commit message. Some are
enforceable through automated checks, others are principles we hold ourselves
to. Both matter.

This document is context for anyone contributing, whether human or agent.

---

## Developer-First

Developer experience is a primary design constraint. APIs, CLIs, error
messages, configuration, documentation: all of it should feel obvious to use
and hard to misuse.

Agents are part of the developer. Not a separate audience, not an
afterthought. A developer today works with coding agents the same way they
work with an editor or a terminal. We design for both with the same care:
structured inputs and outputs, clear contracts, unambiguous interfaces.

## Deterministic

Outputs are reproducible. Same input, same result, every time, on any machine.
We don't rely on non-deterministic processes (LLMs, heuristics, sampling) for
core functionality. When something says "pass" or "fail," it's a fact.

## Composable

Pick what you need. Leave what you don't. Extend what's missing. Individual
pieces are useful on their own and better together. Nothing requires adopting
the whole system.

## Cohesive

Composable doesn't mean disconnected. Consistent naming, shared conventions,
predictable behavior. Learning one part gives intuition about the rest.

## Opinionated

Some ways of building software are better than others. Our defaults reflect
that. Opinionated doesn't mean inflexible: defaults are overridable, but the
defaults matter. Some teams will disagree and choose not to use us. That's
fine.

## Pragmatic

Theory informs practice, but practice wins when they conflict. We ship things
that work in real codebases, with real teams, under real constraints. Purity
is not a goal.

## Rigorous

DX makes the right thing easy, not the wrong thing acceptable. We don't skip
validation because it's simpler. We don't approximate when precision is
possible. When ease and correctness conflict, correctness wins.

## Privacy-First

No telemetry. No phone-home. No cloud dependency. Code stays on the user's
machines. Compliant with GDPR, Loi 25 (Quebec), and the principle that your
data is yours.

## Auditable

Every decision the tool makes is traceable. Why did this check fail? What
rule triggered it? What threshold was exceeded? Who configured it? Teams with
strict compliance requirements use this without compromise.

## Secure

We don't introduce attack surface. Dependencies are minimal and vetted. Inputs
are validated. Outputs don't leak sensitive information. Security is a
constraint on every decision, not a feature to add later.

## Maintainable

The codebase is a living example of what we advocate. Clean, readable,
well-tested. What we expect of the software we check, we expect of ourselves.

## Accessible

We remove barriers to participation, both sensory and cognitive. Every
interface we produce is designed for the full range of human ability from the
start, not retrofitted. Clear language over jargon. Predictable behavior over
clever behavior.

## Stable

APIs don't break without reason. Upgrades come with migration paths. Semver
is respected. Users and agents depend on this tool without fear that an update
will silently change behavior.

## Open

Fully open source. Not open core. Not freemium. Transparent development
process, public roadmap, open discussions. Sustainability comes from community
and sponsorship, not from locking features behind a paywall.

## First Principles

When in doubt, go back to fundamentals. Why does this exist? What problem
does it actually solve? Does this abstraction earn its complexity?

Ship the smallest useful thing, observe how it's used, then evolve. Every
addition earns its place through real usage, not speculation. We question
inherited assumptions and cargo-culted practices, including our own.
