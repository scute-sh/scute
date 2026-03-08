# dependency-freshness

## What it checks

Finds outdated dependencies in your project by comparing declared versions against the latest available. Reports how far behind each dependency is and at what granularity (major, minor, or patch).

## Why it matters

Stale dependencies accumulate security vulnerabilities and make future upgrades painful. The longer you wait, the bigger the jump. Tracking freshness continuously keeps upgrades small and routine instead of a dreaded migration project.

## Usage

### CLI

```sh
scute check dependency-freshness [PATH]
```

| Argument | Description                                                          |
| -------- | -------------------------------------------------------------------- |
| `[PATH]` | Path to the project directory. Defaults to the working directory.    |

```sh
scute check dependency-freshness
scute check dependency-freshness crates/
```

### MCP tool

Tool name: `check_dependency_freshness`

| Parameter | Type   | Required | Description                                                       |
| --------- | ------ | -------- | ----------------------------------------------------------------- |
| `path`    | string | no       | Path to the project directory. Defaults to the project root.      |

## Configuration

In your `.scute.yml`:

```yaml
checks:
  dependency-freshness:
    thresholds:
      warn: 3
      fail: 6
    config:
      level: minor
```

### How `level` and thresholds interact

The `level` sets which version component your thresholds apply to. Drift *above* the configured level always fails with zero tolerance. Drift *below* it is ignored.

| Dependency's drift | Configured level | What happens                                   |
| ------------------ | ---------------- | ---------------------------------------------- |
| Above level        | any              | **Always fails.** Zero tolerance, thresholds bypassed. |
| At level           | any              | Your `warn`/`fail` thresholds apply to the gap. |
| Below level        | any              | **Ignored.** Observed = 0, passes.             |

For example, with `level: minor, warn: 3, fail: 6`:

| Dependency              | Drift kind | Observed | Result      |
| ----------------------- | ---------- | -------- | ----------- |
| `serde 1.2.0 → 2.0.0`  | major      | 1        | **fail** (above level, zero tolerance) |
| `tokio 1.0.0 → 1.8.0`  | minor      | 8        | **fail** (at level, 8 > 6) |
| `anyhow 1.0.0 → 1.4.0` | minor      | 4        | **warn** (at level, 4 > 3) |
| `rand 1.0.0 → 1.1.0`   | minor      | 1        | pass (at level, 1 not > 3) |
| `log 0.4.0 → 0.4.9`    | patch      | 0        | pass (below level, ignored) |

### Thresholds

The `observed` value is the version gap for each dependency. Each dependency is evaluated independently.

| Threshold | Default | Description                                       |
| --------- | ------- | ------------------------------------------------- |
| `warn`    | none    | Warn when version gap > this value                |
| `fail`    | `0`     | Fail when version gap > this value. Default 0 means any drift at the configured level fails. |

### Config options

| Option  | Type   | Default | Values                    | Description                              |
| ------- | ------ | ------- | ------------------------- | ---------------------------------------- |
| `level` | string | `major` | `major`, `minor`, `patch` | Version component your thresholds govern |

With the default (`level: major`), only major version gaps are evaluated against your thresholds, minor and patch drift is invisible, and there's no "above level" to trigger zero tolerance. Setting `level: minor` brings minor gaps under your thresholds and makes major gaps always fail. Setting `level: patch` tracks everything and makes both major and minor gaps always fail.

## Examples

### Fail: major drift with default config

With defaults (`level: major`, `fail: 0`), any major version gap fails:

```sh
scute check dependency-freshness
```

```json
{
  "check": "dependency-freshness",
  "summary": { "evaluated": 2, "passed": 1, "warned": 0, "failed": 1, "errored": 0 },
  "findings": [
    {
      "target": "serde",
      "status": "fail",
      "measurement": { "observed": 2, "thresholds": { "fail": 0 } },
      "evidence": [
        {
          "rule": "outdated-major",
          "found": "serde 1.0.0",
          "expected": "3.0.0",
          "location": "Cargo.toml"
        }
      ]
    }
  ]
}
```

The `evidence` tells you the dependency name, current version, latest version, and which manifest declares it. The `rule` (`outdated-major`, `outdated-minor`, or `outdated-patch`) tells you the kind of gap.

### Fail: major drift above configured minor level

With `level: minor`, a major version gap bypasses your thresholds and fails with zero tolerance:

```json
{
  "target": "serde",
  "status": "fail",
  "measurement": { "observed": 1, "thresholds": { "fail": 0 } },
  "evidence": [
    { "rule": "outdated-major", "found": "serde 1.2.0", "expected": "2.0.0", "location": "Cargo.toml" }
  ]
}
```

Notice the thresholds in the output are `{ "fail": 0 }`, not your configured values. This is the zero-tolerance override for drift above your configured level.

### Pass: all dependencies fresh

```sh
scute check dependency-freshness crates/
```

```json
{
  "check": "dependency-freshness",
  "summary": { "evaluated": 2, "passed": 2, "warned": 0, "failed": 0, "errored": 0 },
  "findings": []
}
```

## Scope & limitations

- **Supported ecosystems:** Cargo (Rust). npm and other ecosystems are planned.
- **Queries crates.io.** Requires network access to fetch latest versions.
- **Direct dependencies only.** Transitive dependencies are not evaluated.
- **Workspace support.** In a Cargo workspace, all workspace members are evaluated. The `location` field in evidence tells you which `Cargo.toml` declares the dependency.
