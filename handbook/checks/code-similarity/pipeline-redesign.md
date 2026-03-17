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

The pipeline:

1. **Parse:** tree-sitter AST → our structural tree (language-agnostic).
   Tokens are leaves, context nodes are containers.
2. **Detect:** flatten the tree (collect leaves) → flat token sequence per
   file → suffix array → clone regions (token ranges).
3. **Evaluate:** for each token in a clone region, walk up to find context.
   Rules inspect context and decide the verdict.

### Node types

These represent structural roles that matter for similarity judgment:

| Node       | Data           | Role                                    |
| ---------- | -------------- | --------------------------------------- |
| Source     | file path      | Root per file                           |
| TestRegion | —              | Test code boundary                      |
| Contract   | contract name  | Contract implementation (trait, iface)  |
| Token      | text, line     | Leaf: normalized token for detection    |

**Assumption:** plain classes (no contract) and impl blocks are transparent
containers. Our model looks through them. A class only becomes a Contract node
when it implements a contract. May need revisiting.

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
