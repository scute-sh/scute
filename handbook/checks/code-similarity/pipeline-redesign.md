# Code Similarity — Pipeline Redesign

## The problem

The code-similarity check finds structurally similar code. But similarity alone
isn't the verdict. The same similarity score means different things depending on
the structural context: where the code lives, what role it plays.

Today, the pipeline produces context-free clone groups ("these regions share N
tokens"), then evaluation reconstructs context from scratch by re-parsing. Each
context type is a hardcoded path. The pipeline doesn't carry structural
information forward.

## Core insight

Tree-sitter gives us an AST per language. We need a **language-agnostic
structural tree** designed for solving the code similarity problem. When we
parse a codebase, the output should be rich enough that everything downstream
(detection, evaluation) just reads the data.

## The model: a structural tree with tokens as leaves

We build our own tree from the tree-sitter AST. It captures structural context
(test regions, contract implementations) as container nodes, with normalized
tokens as leaves. Each token knows its parent, so context is always one walk
away.

```
Source("a.rs")
  └── Contract("Render")
        ├── Token("fn")
        ├── Token("$ID")
        ├── Token("(")
        ├── Token("&")
        ├── Token("$ID")
        ...

Source("tests/a.rs")
  └── TestRegion
        ├── Token("fn")
        ├── Token("$ID")
        ...
```

### The pipeline

Four distinct steps. Each has a clear input/output contract. No step
should reach into another step's concerns.

#### 1. Parse

Source files → structural trees.

Language-specific rules (`SimilarityRules` trait) map tree-sitter AST
nodes to our model. The generic walker drives the builder; the language
rules classify each node. Two classify functions per language:

- `classify_file(path)` → optional container for the whole file
  (e.g. `tests/` directory → TestRegion)
- `classify_node(node, src)` → `Option<NodeKind>`: container, token,
  comment/decoration, or `None` (walker uses default behavior)

The walker is language-agnostic. It handles unnamed nodes (operators,
punctuation) and default recursion. Languages only classify named nodes
they have an opinion about.

After this step: one `StructuralTree` per file. All structural context
is captured. No re-parsing happens downstream.

#### 2. Detect

Structural trees → matched token ranges.

Flatten each tree's leaves into a token sequence. Each position in the
flat sequence retains its tree node index. Concatenate all sequences
with sentinels, build a suffix array, extract maximal repeated regions.

Output: clone groups where each occurrence is a **token range**
(source index + start position + length in the flat sequence). Not
lines. The token range maps directly back to tree nodes.

Lines are never computed here. Detection works with tokens.

#### 3. Evaluate

Matched token ranges + structural trees → verdicts.

For each occurrence in a clone group, read the tree node indices of its
tokens. Walk up to find structural context (contract name, test region).
Rules inspect context and decide:

- All occurrences inside the same contract → exclude
- All occurrences in test regions → apply test thresholds
- Otherwise → apply production thresholds

Because occurrences carry exact token positions (not line ranges),
there's no ambiguity about which tokens belong to the clone.

**Boundary-crossing clones.** A clone's token sequence can span across
structural boundaries. The `}` closing a previous block is a real
matched token, not a display artifact. When a clone includes tokens
from both inside and outside a container, the evaluation must handle
mixed context. The exact strategy (majority vote, any-outside-breaks-
context, etc.) is a rule design question, not a data structure question.
The data is precise: we know exactly which tokens are in which context.

#### 4. Format

Verdicts → actionable feedback (`Evaluation` with evidence).

Only here do we convert token ranges to line numbers, extract source
snippets for evidence, and build the output structures. This is a
display concern, not an algorithmic one.

### Node types

These represent structural roles that matter for similarity judgment:

| Node       | Data           | Role                                    |
| ---------- | -------------- | --------------------------------------- |
| Source     | file path      | Root per file                           |
| TestRegion | —              | Test code boundary                      |
| Contract   | contract names | Contract implementation (trait, iface)  |
| Token      | text, line     | Leaf: normalized token for detection    |

**Assumption:** plain classes (no contract) and impl blocks are transparent
containers. Our model looks through them. A class only becomes a Contract node
when it implements a contract. May need revisiting.

### Representation: arena with parent indices

The tree is stored as a flat `Vec<Node>` (arena). Each node holds its
`NodeKind` and a parent index. Tokens are leaves; containers hold child
indices.

```rust
struct StructuralTree {
    nodes: Vec<Node>,
}

struct Node {
    kind: NodeKind,
    parent: Option<usize>,   // index into arena
    children: Vec<usize>,
}

enum NodeKind {
    Source { path: String },
    TestRegion,
    Contract { names: Vec<String> },
    Token { text: String, start_line: usize, end_line: usize },
}
```

Walk-up: follow `parent` indices until root. 2-4 hops per token.

**Alternatives considered:**

- **Flat context on tokens** (`token.contract: Option<String>`,
  `token.in_test: bool`). Simplest for today's two context types, but every
  new context type adds another field + evaluation branch. Same scaling
  problem we're trying to escape.
- **Pointer-based / doubly-linked.** Fights Rust's ownership model
  (`Rc<RefCell<>>` or unsafe). The arena gives the same walk-up capability
  with plain `usize` indices, zero unsafe, and contiguous memory.
- **Recursive enum tree.** Natural for top-down traversal, but walk-up
  requires either parent pointers (back to arena) or flattening with context
  at flatten time (back to flat tokens).

**Performance at scale (>1M LoC):** the arena is not on the hot path. The
suffix array runs on the flattened token sequence, no indirection. The arena
is touched during parsing (build once, linear), flattening (one pass,
linear), and evaluation (walk-up only for tokens in detected clone groups,
a tiny subset). 2-4 `Vec` index lookups per walk-up, contiguous memory,
cache-friendly.

### Key properties

- **Context lookup:** walk up from any token. O(depth), depth is 2-4 levels.
- **Flattening:** collect leaves in order. O(n), n = token count.
- **Detection alignment:** clones are token ranges, not node boundaries.
  No assumption that clones align with functions or any structural unit.
- **Adding a context type:** add a node type, teach the parser to emit it as
  a container, write a rule that checks for it on walk-up.

## Why not a graph?

We explored a graph-based model where nodes represent entities (contracts,
functions) and edges represent relationships (implements, contains, similar).
The graph was disqualified for our use case:

1. **Detection output can't be edges.** Clone detection produces arbitrary
   token regions via suffix arrays. These regions don't map to nodes. A clone
   might be half a function, span two functions, or be top-level code. You
   can't draw a `similar` edge when there's no node to attach it to.

2. **"Test" isn't an entity.** The graph models relationships between
   entities. Contract is a real entity with identity (a trait that exists,
   referenced by multiple impls). Test context is a location property, not an
   entity. Modeling it as a node with edges (`F --in_context--> Test`) felt
   wrong because there's nothing meaningful to point to. Test context is
   inherently hierarchical: containment, not relationship.

3. **Direct edges lose their advantage with tokens as leaves.** The graph's
   appeal was direct edges like `Function --implements--> Contract`, skipping
   hierarchical traversal. But once tokens are leaves (not functions), there
   are no function nodes to attach edges to. Putting `implements` on every
   token is redundant. Putting it on the Contract container is just... the
   tree.

4. **Cross-cutting queries aren't needed.** "Find all impls of Render across
   the codebase" is a natural graph query, but no evaluation rule needs it.
   Every rule asks "what is the context of THIS region?", which is a
   walk-up query. That's what trees do best.

The graph is more powerful in general, but that power doesn't serve our
problem. The tree is simpler and directly fits the query pattern we need.

## Contract name resolution (parsing concern)

Two problems that affect the tree equally:

**Qualification:** the same contract can appear with different names depending
on imports. `impl Render for Html` (imported) vs `impl crate::render::Render
for Html` (fully qualified). Tree-sitter gives exactly what's written. Short
names via import need `use` resolution to canonicalize.

**Collision:** different contracts can share a name. `renderer::Render` vs
`serializer::Render`. If both imported as `Render`, we'd incorrectly treat
them as the same contract (false negative).

Pragmatic starting point: use the name as written. Collision risk is low in
practice, and the failure mode (false negative) is less harmful than false
positive. Improve resolution later.

## Known limitation: multiple contracts per region

In Rust, each `impl Trait for Type` block is a separate syntactic unit.
The tree naturally gets one Contract node per trait, each owning its own
tokens. Ideal representation.

Most OOP languages don't have this luxury. A class body is a single
syntactic unit that satisfies all its contracts at once:
`class Foo implements A, B`, `class Foo extends Base implements A`.
There's no syntactic boundary between "tokens for A" and "tokens for B."
Splitting them would require semantic analysis (knowing which methods
satisfy which interface).

Current model: a Contract node holds all contract names for the region.
All tokens belong to all contracts. The evaluation checks set intersection
across occurrences, so two classes sharing any contract get excluded.

Ideal future: per-contract token ownership, so similarity is evaluated
per-contract rather than per-class. Requires type information from a
language server or type checker.
