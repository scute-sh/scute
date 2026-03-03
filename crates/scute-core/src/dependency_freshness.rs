use std::path::Path;

use crate::{Evaluation, Evidence, ExecutionError, Expected, Outcome, Thresholds};

pub const CHECK_NAME: &str = "dependency-freshness";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Patch,
    Minor,
    #[default]
    Major,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Patch => f.write_str("patch"),
            Self::Minor => f.write_str("minor"),
            Self::Major => f.write_str("major"),
        }
    }
}

const DEFAULT_THRESHOLDS: Thresholds = Thresholds {
    warn: None,
    fail: Some(0),
};

#[derive(Debug, Default)]
pub struct Definition {
    pub level: Option<Level>,
    pub thresholds: Option<Thresholds>,
}

#[derive(Debug)]
pub struct OutdatedDep {
    pub name: String,
    pub current: semver::Version,
    pub latest: semver::Version,
}

impl OutdatedDep {
    #[must_use]
    pub fn kind(&self) -> Level {
        if self.current.major != self.latest.major {
            Level::Major
        } else if self.current.minor != self.latest.minor {
            Level::Minor
        } else {
            Level::Patch
        }
    }
}

/// Run the dependency-freshness check against a Cargo project.
///
/// # Errors
///
/// Returns `Err` when the target path doesn't exist, `cargo-outdated`
/// isn't installed, or the external tool crashes.
pub fn check(target: &Path, definition: &Definition) -> Result<Vec<Evaluation>, ExecutionError> {
    let resolved = target.canonicalize().map_err(|_| ExecutionError {
        code: "invalid_target".into(),
        message: format!("path does not exist: {}", target.display()),
        recovery: "provide a valid directory path".into(),
    })?;

    let outdated = fetch_outdated(&resolved).map_err(classify_error)?;

    Ok(vec![Evaluation {
        target: resolved.display().to_string(),
        outcome: evaluate(&outdated, definition),
    }])
}

#[derive(Debug)]
#[doc(hidden)]
pub enum FetchError {
    NotFound(std::io::Error),
    ToolFailed(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(e) => write!(f, "cargo-outdated not found: {e}"),
            Self::ToolFailed(msg) => write!(f, "cargo outdated failed: {msg}"),
        }
    }
}

fn classify_error(err: FetchError) -> ExecutionError {
    match err {
        FetchError::NotFound(io_err) => {
            if io_err.kind() == std::io::ErrorKind::NotFound {
                ExecutionError {
                    code: "missing_tool".into(),
                    message: "cargo-outdated is not installed".into(),
                    recovery: "install it with: cargo install cargo-outdated".into(),
                }
            } else {
                ExecutionError {
                    code: "tool_failed".into(),
                    message: format!("could not run cargo-outdated: {io_err}"),
                    recovery:
                        "check that cargo-outdated is installed: cargo install cargo-outdated"
                            .into(),
                }
            }
        }
        FetchError::ToolFailed(stderr) => {
            if stderr.contains("not a cargo project")
                || stderr.contains("could not find `Cargo.toml`")
            {
                ExecutionError {
                    code: "invalid_target".into(),
                    message: "target is not a valid Cargo project".into(),
                    recovery: "pass a directory containing a Cargo.toml".into(),
                }
            } else {
                ExecutionError {
                    code: "tool_failed".into(),
                    message: format!("cargo-outdated failed: {stderr}"),
                    recovery: "verify cargo-outdated works by running: cargo outdated --format json --depth 1".into(),
                }
            }
        }
    }
}

#[doc(hidden)]
pub fn fetch_outdated(target: &Path) -> Result<Vec<OutdatedDep>, FetchError> {
    let output = std::process::Command::new("cargo")
        .args(["outdated", "--format", "json", "--depth", "1"])
        .current_dir(target)
        .output()
        .map_err(FetchError::NotFound)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(FetchError::ToolFailed(stderr));
    }

    let stdout =
        String::from_utf8(output.stdout).map_err(|e| FetchError::ToolFailed(e.to_string()))?;
    Ok(parse_cargo_outdated(&stdout))
}

fn evaluate(outdated: &[OutdatedDep], definition: &Definition) -> Outcome {
    let level = definition.level.unwrap_or_default();
    let evidence: Vec<Evidence> = outdated
        .iter()
        .filter(|dep| dep.kind() >= level)
        .map(|dep| {
            Evidence::with_expected(
                &format!("outdated-{}", dep.kind()),
                &format!("{} {}", dep.name, dep.current),
                Expected::Text(dep.latest.to_string()),
            )
        })
        .collect();
    let observed = evidence.len() as u64;
    let thresholds = definition.thresholds.clone().unwrap_or(DEFAULT_THRESHOLDS);

    Outcome::completed(observed, thresholds, evidence)
}

#[must_use]
fn parse_cargo_outdated(output: &str) -> Vec<OutdatedDep> {
    let mut deps = Vec::new();
    for line in output.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(entries) = v["dependencies"].as_array() {
            for entry in entries {
                let name = entry["name"].as_str().unwrap_or("");
                let current = entry["project"].as_str().unwrap_or("");
                let latest = entry["latest"].as_str().unwrap_or("");
                if latest == "---" || latest == "Removed" {
                    continue;
                }
                let (Ok(current), Ok(latest)) = (
                    current.parse::<semver::Version>(),
                    latest.parse::<semver::Version>(),
                ) else {
                    continue;
                };
                if current != latest {
                    deps.push(OutdatedDep {
                        name: name.into(),
                        current,
                        latest,
                    });
                }
            }
        }
    }
    deps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Status;
    use googletest::prelude::*;

    struct Completed {
        status: Status,
        observed: u64,
        thresholds: Thresholds,
        evidence: Vec<Evidence>,
    }

    fn unwrap_completed(outcome: Outcome) -> Completed {
        match outcome {
            Outcome::Completed {
                status,
                observed,
                thresholds,
                evidence,
            } => Completed {
                status,
                observed,
                thresholds,
                evidence,
            },
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn no_outdated_deps_returns_pass_with_all_fields() {
        let c = unwrap_completed(evaluate(&[], &Definition::default()));

        assert_eq!(c.status, Status::Pass);
        assert_eq!(c.observed, 0);
        assert_eq!(
            c.thresholds,
            Thresholds {
                warn: None,
                fail: Some(0)
            }
        );
        assert!(c.evidence.is_empty());
    }

    #[test]
    fn reports_outdated_dep_count() {
        let deps = vec![dep("a", "1.0.0", "2.0.0"), dep("b", "2.0.0", "3.0.0")];

        let c = unwrap_completed(evaluate(&deps, &Definition::default()));

        assert_eq!(c.observed, 2);
    }

    #[test]
    fn outdated_deps_above_threshold_fails() {
        let deps = vec![dep("a", "1.0.0", "2.0.0")];

        let c = unwrap_completed(evaluate(&deps, &Definition::default()));

        assert_eq!(c.status, Status::Fail);
    }

    #[test]
    fn custom_fail_threshold_overrides_default() {
        let deps: Vec<OutdatedDep> = (0..5)
            .map(|i| {
                dep(
                    &format!("d{i}"),
                    &format!("{i}.0.0"),
                    &format!("{}.0.0", i + 1),
                )
            })
            .collect();
        let definition = Definition {
            thresholds: Some(Thresholds {
                warn: None,
                fail: Some(3),
            }),
            ..Definition::default()
        };

        let c = unwrap_completed(evaluate(&deps, &definition));

        assert_eq!(c.observed, 5);
        assert_eq!(c.status, Status::Fail);
    }

    #[test]
    fn observed_below_warn_threshold_passes() {
        let deps = vec![dep("a", "1.0.0", "2.0.0"), dep("b", "2.0.0", "3.0.0")];
        let definition = Definition {
            thresholds: Some(Thresholds {
                warn: Some(3),
                fail: Some(10),
            }),
            ..Definition::default()
        };

        let c = unwrap_completed(evaluate(&deps, &definition));

        assert_eq!(c.observed, 2);
        assert_eq!(c.status, Status::Pass);
    }

    #[test]
    fn evidence_contains_dep_name_current_and_latest() {
        let deps = vec![dep("a", "1.0.0", "2.0.0")];

        let c = unwrap_completed(evaluate(&deps, &Definition::default()));

        assert_eq!(c.evidence.len(), 1);
        assert_eq!(c.evidence[0].found, "a 1.0.0");
        assert_eq!(c.evidence[0].expected, Some(Expected::Text("2.0.0".into())));
    }

    #[test]
    fn evidence_rule_reflects_outdated_kind() {
        let deps = vec![dep("a", "1.0.0", "2.0.0")];

        let c = unwrap_completed(evaluate(&deps, &Definition::default()));

        assert_that!(c.evidence[0].rule, some(eq("outdated-major")));
    }

    #[test]
    fn no_definition_defaults_to_major_level() {
        let c = unwrap_completed(evaluate(&deps_at_every_level(), &Definition::default()));

        assert_eq!(c.observed, 1);
    }

    #[test]
    fn major_level_excludes_minor_gap_deps() {
        let definition = Definition {
            level: Some(Level::Major),
            ..Definition::default()
        };

        let c = unwrap_completed(evaluate(&deps_at_every_level(), &definition));

        assert_eq!(c.observed, 1);
    }

    #[test]
    fn minor_level_includes_major_and_minor_gaps() {
        let definition = Definition {
            level: Some(Level::Minor),
            ..Definition::default()
        };

        let c = unwrap_completed(evaluate(&deps_at_every_level(), &definition));

        assert_eq!(c.observed, 2);
    }

    #[test]
    fn patch_level_includes_all_gaps() {
        let definition = Definition {
            level: Some(Level::Patch),
            ..Definition::default()
        };

        let c = unwrap_completed(evaluate(&deps_at_every_level(), &definition));

        assert_eq!(c.observed, 3);
    }

    fn dep(name: &str, current: &str, latest: &str) -> OutdatedDep {
        OutdatedDep {
            name: name.into(),
            current: current.parse().unwrap(),
            latest: latest.parse().unwrap(),
        }
    }

    fn deps_at_every_level() -> Vec<OutdatedDep> {
        vec![
            dep("a", "1.0.0", "2.0.0"),
            dep("b", "1.0.0", "1.1.0"),
            dep("c", "1.0.0", "1.0.1"),
        ]
    }

    #[test]
    fn nonexistent_path_returns_invalid_target_error() {
        let result = check(Path::new("/nonexistent/path"), &Definition::default());

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "invalid_target");
    }

    #[test]
    fn command_not_found_classifies_as_missing_tool() {
        let err = FetchError::NotFound(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No such file or directory",
        ));

        let result = classify_error(err);

        assert_eq!(result.code, "missing_tool");
    }

    #[test]
    fn cargo_toml_not_found_classifies_as_invalid_target() {
        let err = FetchError::ToolFailed("could not find `Cargo.toml`".into());

        let result = classify_error(err);

        assert_eq!(result.code, "invalid_target");
    }

    #[test]
    fn unknown_tool_failure_classifies_as_tool_failed() {
        let err = FetchError::ToolFailed("segfault or something".into());

        let result = classify_error(err);

        assert_eq!(result.code, "tool_failed");
    }
}
