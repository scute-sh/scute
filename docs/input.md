# Choosing files

How to point a check at files, and what it does with each one. This applies to
the checks that read source files:
[`code-complexity`](checks/code-complexity.md) and
[`code-similarity`](checks/code-similarity.md).

## Handing over files

On the CLI, pass files or directories as arguments:

```sh
scute check code-complexity src/parser.rs src/engine/
```

Or feed them through stdin, one per line, with no arguments:

```sh
scute check code-complexity < files-to-check.txt
```

Arguments win: when there are any, stdin is ignored. Relative paths resolve
against the working directory.

Over MCP, pass a list (`paths` for `check_code_complexity`, `files` for
`check_code_similarity`). Relative paths resolve against `project_root`.

## What happens to each path

- **Directories get walked** for supported files.
- **Files in a supported language get checked.** Anything else you pass
  (`Cargo.toml`, `README.md`, images) is skipped, so you can hand over every
  file in a change without filtering it first.
- **A path that doesn't exist gets an error finding** naming it, next to the
  results for the other paths.
- **Files matching an [`exclude`](configuration.md#excluding-files) pattern
  are skipped.**
- **Nothing left to check passes,** with `0 evaluated` in the summary.

## Handing over nothing

With no files, a check covers the whole project:

- On the CLI: no arguments and nothing on stdin, an empty pipe included.
- Over MCP: leave the list out. An empty list (`[]`) checks nothing.

## Checking a change's files

To check only the files a change touched, go through `xargs`:

```sh
git diff -z --diff-filter=d --name-only origin/main...HEAD | xargs -0r scute check code-complexity
```

- `-r` skips the check when there are no files.
- `git diff -z` with `xargs -0` keeps names with spaces, quotes, or accents
  intact. Without `-z`, git quotes a name like `café.rs` and the check can't
  find it.
- On a very long list, `xargs` splits the files across several runs, each
  printing its own report.

Ready-made setups live in [Git hooks](integrations/git-hooks.md) and
[CI](integrations/ci.md).
