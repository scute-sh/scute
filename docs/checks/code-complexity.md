# code-complexity

## What it checks

Scores each function's cognitive complexity following [G. Ann Campbell's
spec](https://www.sonarsource.com/docs/CognitiveComplexity.pdf). Unlike
cyclomatic complexity (which counts execution paths for testability), cognitive
complexity measures how hard code is for a human to understand.

## Why it matters

Functions accumulate complexity one `if` at a time. By the time someone
complains, the function is already 80 lines of nested spaghetti. Catching it
per-function, per-commit keeps code reviewable and refactoring small.

Coding agents are especially prone to piling logic into a single function.
A clear score and per-line evidence gives them the signal they need to split
things up before asking for review.

## How scoring works

Every function starts at 0. Six cognitive drivers add to the score:

| Driver | Increment | When |
| --- | --- | --- |
| Flow break | +1 | `if`, `for`, `while`, `loop`, `match` at nesting depth 0 |
| Nesting | +1 + depth | Same constructs, but nested inside other control flow |
| Else | +1 | `else` or `else if` branch |
| Boolean logic | +1 per sequence change | `a && b` = +1, `a && b \|\| c` = +2 |
| Recursion | +1 | Direct recursive call |
| Jump | +1 | Labeled `break` or `continue` |

Key behaviors:

- **Nesting multiplies.** An `if` inside a `for` inside a `match` costs 1 + nesting depth, not just 1. This is the main driver of high scores.
- **Else-if chains are flat.** `if / else if / else if / else` does not compound nesting. Each branch costs +1.
- **Closures inherit nesting.** A closure bumps nesting depth by 1 for its contained code. It's not a fresh scope.
- **Logical operators count sequence changes.** `a && b && c` is one sequence (+1). `a && b || c && d` has two changes (+3).

## Usage

### CLI

```sh
scute check code-complexity [PATHS]...
```

| Argument    | Description                                                                                  |
| ----------- | -------------------------------------------------------------------------------------------- |
| `[PATHS]...` | Files or directories to check. Directories are walked for supported files. Reads from stdin if piped. Defaults to the working directory. |

Scan the full project:

```sh
scute check code-complexity
```

Check specific files:

```sh
scute check code-complexity src/parser.rs src/engine.rs
```

Check a directory:

```sh
scute check code-complexity src/
```

Pipe changed files from git:

```sh
git diff --name-only HEAD~1 | scute check code-complexity
```

### MCP tool

Tool name: `check_code_complexity`

| Parameter | Type            | Required | Description                                                                      |
| --------- | --------------- | -------- | -------------------------------------------------------------------------------- |
| `paths`   | array\<string\> | no       | Files or directories to check. Defaults to the project root. |

## Configuration

In your `.scute.yml`:

```yaml
checks:
  code-complexity:
    thresholds:
      warn: 5
      fail: 10
    exclude:
      - 'generated/**'
```

### Thresholds

The `observed` value is the cognitive complexity score of a single function. One finding per function.

| Threshold | Default | Description                                    |
| --------- | ------- | ---------------------------------------------- |
| `warn`    | `5`     | Warn when score > this value                   |
| `fail`    | `10`    | Fail when score > this value                   |

At score 5 you can follow the logic in one pass. At 10+ you need to re-read to track control flow. These defaults are calibrated for new code, not legacy.

With defaults: score 5 passes, 6 warns, 10 warns, 11 fails.

### Options

| Option    | Type            | Default | Description                                    |
| --------- | --------------- | ------- | ---------------------------------------------- |
| `exclude` | array\<string\> | `[]`    | Glob patterns for files to skip during scanning |

`exclude` patterns follow `.gitignore` semantics. Useful for generated code, vendored dependencies, or other directories you don't control.

### Evidence rules

Each line that contributes to a function's score produces an evidence entry. The `rule` field names the cognitive driver, `found` describes what's happening, and `expected` suggests what to do about it.

| Rule | found example | expected |
| --- | --- | --- |
| `flow break` | `'for' loop (+1)` | — |
| `nesting` | `'if' nested 2 levels: 'for > if > if' (+3)` | extract inner block into a function |
| `else` | `'else' branch (+1)` | use a guard clause or early return |
| `boolean logic` | `mixed '&&' and '\|\|' operators (+2)` | extract into a named boolean |
| `recursion` | `recursive call to 'process' (+1)` | consider iterative approach |
| `jump` | `'break' to label 'outer (+1)` | restructure to avoid labeled jump |

The nesting chain reads left-to-right (outside to inside) and stops at closure boundaries: `'if' nested 2 levels: 'closure > if' (+3)`.

## Examples

### Warn: moderately complex function

A function with nested control flow that's starting to get hard to follow:

```sh
scute check code-complexity src/process.rs
```

```json
{
  "check": "code-complexity",
  "summary": { "evaluated": 4, "passed": 3, "warned": 1, "failed": 0, "errored": 0 },
  "findings": [
    {
      "target": "src/process.rs:10:process",
      "status": "warn",
      "measurement": { "observed": 7, "thresholds": { "warn": 5, "fail": 10 } },
      "evidence": [
        {
          "rule": "flow break",
          "found": "'for' loop (+1)",
          "location": "src/process.rs:11"
        },
        {
          "rule": "nesting",
          "found": "'if' nested 1 level: 'for > if' (+2)",
          "expected": "extract inner block into a function",
          "location": "src/process.rs:13"
        },
        {
          "rule": "nesting",
          "found": "'if' nested 2 levels: 'for > if > if' (+3)",
          "expected": "extract inner block into a function",
          "location": "src/process.rs:14"
        },
        {
          "rule": "else",
          "found": "'else' branch (+1)",
          "expected": "use a guard clause or early return",
          "location": "src/process.rs:17"
        }
      ]
    }
  ]
}
```

The `target` is `file:line:function_name`. Each evidence entry points to a specific line, names the cognitive driver, shows its cost, and suggests a fix. An agent can read this and extract the nested block into a helper function.

### Pass: simple functions

```sh
scute check code-complexity src/utils.rs
```

```json
{
  "check": "code-complexity",
  "summary": { "evaluated": 3, "passed": 3, "warned": 0, "failed": 0, "errored": 0 },
  "findings": []
}
```

All functions score within thresholds. Nothing to fix.

## Scope & limitations

- **Supported languages:** Rust. TypeScript and JavaScript support is planned.
- **Per-function, not per-file.** Each function is evaluated and reported independently. The `target` includes the function name and line number.
- **Structural, not semantic.** Measures control flow structure. Two functions that are equally hard to understand but use different constructs may score differently.
- **Scoped analysis.** Only the files and directories you pass are analyzed. Pass nothing to scan the whole project.
- **Graceful with syntax errors.** Tree-sitter recovers from parse errors. Malformed code won't crash the check.
