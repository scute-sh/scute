# Reading results

Every check reports in the same JSON shape. The CLI prints it on stdout, and
the MCP server returns it as the tool's structured content.

## The report

```json
{
  "check": "code-complexity",
  "summary": { "evaluated": 14, "passed": 12, "warned": 1, "failed": 0, "errored": 1 },
  "findings": [
    {
      "target": "src/engine.rs:87:resolve_config",
      "status": "warn",
      "measurement": { "observed": 7, "thresholds": { "warn": 5, "fail": 10 } },
      "evidence": [
        {
          "rule": "nesting",
          "found": "'if' nested 2 levels: 'for > match > if' (+3)",
          "expected": "extract inner block into a function",
          "location": "src/engine.rs:92"
        }
      ]
    },
    {
      "target": "src/old.rs",
      "status": "error",
      "error": {
        "code": "invalid_target",
        "message": "cannot read src/old.rs: No such file or directory (os error 2)",
        "recovery": "check that the path exists and is readable"
      }
    }
  ]
}
```

- **`check`**: the check that ran.
- **`summary`**: how many things the check evaluated, and how each one came
  out. What counts as one depends on the check: a function, a clone group, a
  commit message, a dependency.
- **`findings`**: everything that didn't pass. Passing evaluations only show
  up in the summary, so an empty array means all clear.

A finding has a `target` (what was evaluated, often `file:line`) and a
`status`:

- **`warn` or `fail`**: `measurement.observed` crossed the matching
  threshold. `evidence` explains why, each entry with what was `found`, and
  when the check knows them, the `rule` it broke, a `location`, and what was
  `expected` instead (a hint or a list of accepted values).
- **`error`**: the check couldn't evaluate this target, like a file that
  doesn't exist. `error` has a `code`, a `message`, and a `recovery` hint. The
  other targets still get evaluated.

Each check's page explains what `observed` measures and what its thresholds
mean.

## When a check can't run at all

Some problems stop the whole check, like a JavaScript project without a lock
file for `dependency-freshness` to read. The report then carries an `error`
instead of a summary and findings:

```json
{
  "check": "dependency-freshness",
  "error": {
    "code": "invalid_target",
    "message": "found package.json but no lock file — run `npm install` or `pnpm install` first",
    "recovery": "check that you're pointing at a project root"
  }
}
```

On the CLI, problems that come up before any check starts, like an invalid
`.scute.yml` or an unknown check, print the same `error` object without
`check`. Over MCP, they come back as a report with `check` and `error`.

## Exit codes

| Code | Meaning                                                           |
| ---- | ----------------------------------------------------------------- |
| `0`  | Nothing failed. Warnings don't change the exit code.              |
| `1`  | At least one finding failed.                                      |
| `2`  | Something errored: a finding, the whole check, or the CLI usage.  |

`2` wins over `1`, so a run with both errors and failures exits with `2`.

Over MCP, the tool result is flagged as an error when anything failed or
errored.
