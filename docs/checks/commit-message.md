# commit-message

## What it checks

Validates a commit message against the [Conventional Commits](https://www.conventionalcommits.org/) spec. Catches malformed subjects, missing types, empty descriptions, and other structural violations.

## Why it matters

Consistent commit messages power changelogs, semantic versioning, and automated releases. They also make `git log` actually useful when you're debugging at 2am.

Coding agents need a clear spec and a fast signal when they get it wrong. This check gives them both.

## Usage

### CLI

```sh
scute check commit-message [MESSAGE]
```

Pass the full commit message as an argument:

```sh
scute check commit-message "feat(auth): add OAuth2 support"
```

### MCP tool

Tool name: `check_commit_message`

| Parameter | Type   | Required | Description                         |
| --------- | ------ | -------- | ----------------------------------- |
| `message` | string | yes      | The full commit message to validate |

## Configuration

In your `.scute.yml`:

```yaml
checks:
  commit-message:
    thresholds:
      fail: 0
    config:
      types: [feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert]
```

### Thresholds

The check reports `observed: 1` if any rule is violated, `observed: 0` if the message is clean. It's a pass/fail signal, not a violation count.

| Threshold | Default | Description                                         |
| --------- | ------- | --------------------------------------------------- |
| `warn`    | none    | Warn when observed > this value                     |
| `fail`    | `0`     | Fail when observed > this value                     |

At the default (`fail: 0`), any violation fails the check. To get warnings instead, set only `warn: 0`:

```yaml
checks:
  commit-message:
    thresholds:
      warn: 0   # flag violations as warnings, don't fail
```

Multiple rules can fire on the same message (e.g. `unknown-type` and `empty-description`), but `observed` stays 1. The individual violations show up in the `evidence` array.

### Config options

| Option  | Type            | Default                                                                     | Description                        |
| ------- | --------------- | --------------------------------------------------------------------------- | ---------------------------------- |
| `types` | array\<string\> | `[feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert]`  | Allowed commit types               |

Override `types` to add project-specific types or restrict the list:

```yaml
checks:
  commit-message:
    config:
      types: [feat, fix, hotfix, chore]
```

### Rules

Each rule that fires produces an entry in the `evidence` array with the rule name, what was found, and what was expected.

| Rule                   | What it catches                                  |
| ---------------------- | ------------------------------------------------ |
| `subject-format`       | Subject doesn't match `type(scope): description` |
| `unknown-type`         | Type not in the allowed list                     |
| `empty-description`    | Missing description after the colon              |
| `empty-scope`          | Empty parentheses `()` with no scope inside      |
| `body-separator`       | Missing blank line between subject and body      |
| `footer-format`        | Footer doesn't follow `token: value` or `token #value` |
| `breaking-change-case` | `breaking change` instead of `BREAKING CHANGE`   |

## Examples

### Fail: malformed subject

```sh
scute check commit-message "added stuff"
```

```json
{
  "check": "commit-message",
  "summary": { "evaluated": 1, "passed": 0, "warned": 0, "failed": 1, "errored": 0 },
  "findings": [
    {
      "target": "added stuff",
      "status": "fail",
      "measurement": { "observed": 1, "thresholds": { "fail": 0 } },
      "evidence": [
        { "rule": "subject-format", "found": "added stuff", "expected": "type(scope): description" }
      ]
    }
  ]
}
```

The `evidence` tells you the rule, what it found, and what it expected. An agent reads this and knows exactly how to fix it.

### Pass: well-formed message

```sh
scute check commit-message "feat(auth): add OAuth2 support"
```

```json
{
  "check": "commit-message",
  "summary": { "evaluated": 1, "passed": 1, "warned": 0, "failed": 0, "errored": 0 },
  "findings": []
}
```

Empty `findings`, clean summary. Nothing to fix.

## Scope & limitations

- Validates structure only, not content. `feat: asdfghjkl` passes.
- Scopes are optional. Both `feat: add login` and `feat(auth): add login` are valid.
- Type matching is case-insensitive. `Feat: add login` is valid.
- Multi-line messages are supported. Validates subject line, body separation, and footer format.
- Git comment lines (`#`-prefixed) are stripped before validation.
- Does not enforce max subject length (yet).
