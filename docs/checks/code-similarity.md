# code-similarity

## What it checks

Scans your codebase for structural code duplication by comparing token sequences across files. Finds copy-paste clones and near-identical functions that should probably be consolidated.

## Why it matters

Duplication is the silent debt multiplier. Every clone is a future bug where you fix one copy and forget the other. Coding agents are especially prone to this, generating "just one more helper" instead of reusing what already exists.

Catching it before it reaches a PR saves review cycles and keeps the codebase honest.

## Usage

### CLI

```sh
scute check code-similarity [OPTIONS] [FILES]...
```

| Argument/Option      | Description                                                                       |
| -------------------- | --------------------------------------------------------------------------------- |
| `[FILES]...`         | Focus files. Only report clones involving these files. Reads from stdin if piped.  |
| `--source-dir <DIR>` | Directory to scan for source files. Defaults to the working directory.             |

Scan the full project:

```sh
scute check code-similarity
```

Focus on specific files:

```sh
scute check code-similarity src/utils/format.rs
```

Pipe changed files from git:

```sh
git diff --name-only HEAD~1 | scute check code-similarity
```

### MCP tool

Tool name: `check_code_similarity`

| Parameter    | Type            | Required | Description                                                       |
| ------------ | --------------- | -------- | ----------------------------------------------------------------- |
| `files`      | array\<string\> | no       | Focus files. Only report clones involving these files.             |
| `source_dir` | string          | no       | Directory to scan for source files. Defaults to the project root. |

## Configuration

In your `.scute.yml`:

```yaml
checks:
  code-similarity:
    thresholds:
      warn: 70
      fail: 100
    min-tokens: 50
    skip-ignored-files: true
    test-thresholds:
      warn: 100
      fail: 130
```

### Thresholds

The `observed` value is the number of duplicated tokens in a clone group. The check reports one finding per clone group.

| Threshold | Default | Description                                    |
| --------- | ------- | ---------------------------------------------- |
| `warn`    | `70`    | Warn when duplicated tokens > this value       |
| `fail`    | `100`   | Fail when duplicated tokens > this value       |

With defaults: 70 tokens passes, 71 warns, 100 warns, 101 fails.

### Options

| Option               | Type       | Default              | Description                                                    |
| -------------------- | ---------- | -------------------- | -------------------------------------------------------------- |
| `min-tokens`         | integer    | `50`                 | Minimum token sequence length to consider as duplication        |
| `skip-ignored-files` | boolean    | `true`               | Skip files matched by `.gitignore` during file discovery        |
| `test-thresholds`    | thresholds | warn: 100, fail: 130 | Separate thresholds for clone groups where every occurrence lives in test code |

`min-tokens` controls sensitivity. Lower values catch smaller clones but produce more noise. 50 works well for most projects.

Test code tends to have more acceptable duplication (similar setups, assertion patterns). `test-thresholds` sets a more lenient bar for clone groups that live entirely in test code. This includes `tests/` directories, `*.test.*`/`*.spec.*` files, `#[cfg(test)]` modules, and `#[test]` functions. If any occurrence in a clone group is production code, the regular thresholds apply.

## Examples

### Fail: significant duplication

A coding agent added `format_timestamp` to `src/utils/format.rs`, but the same function already exists in `src/helpers/time.rs`:

```sh
scute check code-similarity src/utils/format.rs
```

```json
{
  "check": "code-similarity",
  "summary": { "evaluated": 12, "passed": 11, "warned": 0, "failed": 1, "errored": 0 },
  "findings": [
    {
      "target": "src/utils/format.rs:14",
      "status": "fail",
      "measurement": { "observed": 128, "thresholds": { "warn": 70, "fail": 100 } },
      "evidence": [
        {
          "location": "src/utils/format.rs:14-38",
          "found": "128 duplicated tokens, e.g. `fn format_timestamp(ts: i64) -> String {`"
        },
        {
          "location": "src/helpers/time.rs:7-31",
          "found": "128 duplicated tokens, e.g. `fn format_timestamp(ts: i64) -> String {`"
        }
      ]
    }
  ]
}
```

128 duplicated tokens. The `evidence` shows both sides of the clone with file paths, line ranges, and a snippet. An agent can read this and consolidate without asking for help.

### Pass: no duplication

After consolidating the duplicate:

```sh
scute check code-similarity src/utils/format.rs
```

```json
{
  "check": "code-similarity",
  "summary": { "evaluated": 8, "passed": 8, "warned": 0, "failed": 0, "errored": 0 },
  "findings": []
}
```

All clear.

## Scope & limitations

- **Supported languages:** Rust, TypeScript. More coming.
- **Structural, not semantic.** Compares token sequences. Whitespace and formatting don't matter. Renamed variables in otherwise-identical blocks still match. But two functions that do the same thing with different implementations won't be flagged.
- **Focus vs. scan.** The `files` parameter focuses the *results* (only report clones involving those files), but the full project is always scanned for matches. You're filtering the report, not the search.
- **Token granularity.** Very small clones (common patterns, boilerplate) are expected and usually not worth flagging. The `min-tokens` threshold controls what's worth reporting.
