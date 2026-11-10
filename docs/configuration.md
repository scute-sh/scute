# Configuration

Every check works out of the box. To tune one, add a `.scute.yml`:

```yaml
checks:
  code-complexity:
    thresholds:
      warn: 8
      fail: 15
    exclude:
      - 'generated/**'
```

Each check's page lists its defaults and its own options. This page covers
what they all share.

## Where Scute finds it

Scute looks for `.scute.yml` in the directory it runs from (the working
directory on the CLI, `project_root` over MCP), then in each parent
directory. It stops at the first directory holding a `.git` folder, or at your
home directory. Without a file, every check uses its defaults.

## Thresholds

A check measures something for each thing it evaluates (`observed` in the
[report](output.md)), then compares it against `warn` and `fail`:

- **`fail`** when `observed` is above `fail`.
- **`warn`** when it's above `warn`, but not above `fail`.
- **`pass`** otherwise. A value equal to a threshold passes.

A threshold you leave out never triggers. `thresholds` replaces the defaults
as a whole, so setting only `fail` turns warnings off:

```yaml
checks:
  code-complexity:
    thresholds:
      fail: 15 # no warn: nothing warns anymore
```

Setting `warn` above `fail` flips the direction: lower becomes worse, for
measurements like coverage where less is the problem.

## Excluding files

The checks that read source files ([`code-complexity`](checks/code-complexity.md)
and [`code-similarity`](checks/code-similarity.md)) take an `exclude` list:

```yaml
checks:
  code-similarity:
    exclude:
      - 'generated/**'
      - 'vendor/'
```

- Patterns follow `.gitignore` semantics and match from the project root,
  whatever paths you hand over.
- A file matching a pattern is skipped even when you name it directly, so a
  hook passing staged files won't flag your generated code.

Useful for generated code, vendored dependencies, or other directories you
don't control.
