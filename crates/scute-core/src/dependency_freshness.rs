use std::path::Path;

use crate::{Evaluation, Evidence, ExecutionError, Expected, Outcome, Status, Thresholds};

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

    fn gap(&self) -> u64 {
        match self.kind() {
            Level::Major => self.latest.major - self.current.major,
            Level::Minor => self.latest.minor - self.current.minor,
            Level::Patch => self.latest.patch - self.current.patch,
        }
    }

    fn measure_gap(&self, level: Level, configured_thresholds: &Thresholds) -> (u64, Thresholds) {
        use std::cmp::Ordering;
        match self.kind().cmp(&level) {
            Ordering::Greater => (self.gap(), ZERO_TOLERANCE),
            Ordering::Equal => (self.gap(), configured_thresholds.clone()),
            Ordering::Less => (0, configured_thresholds.clone()),
        }
    }

    fn to_evidence(&self) -> Evidence {
        Evidence::with_expected(
            &format!("outdated-{}", self.kind()),
            &format!("{} {}", self.name, self.current),
            Expected::Text(self.latest.to_string()),
        )
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

    Ok(evaluate(&resolved, &outdated, definition))
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

const ZERO_TOLERANCE: Thresholds = Thresholds {
    warn: None,
    fail: Some(0),
};

fn evaluate(target: &Path, outdated: &[OutdatedDep], definition: &Definition) -> Vec<Evaluation> {
    let level = definition.level.unwrap_or_default();
    let configured_thresholds = definition.thresholds.clone().unwrap_or(DEFAULT_THRESHOLDS);

    if outdated.is_empty() {
        return vec![Evaluation {
            target: target.display().to_string(),
            outcome: Outcome::completed(0, configured_thresholds, vec![]),
        }];
    }

    outdated
        .iter()
        .map(|dependency| evaluate_dependency(dependency, level, &configured_thresholds))
        .collect()
}

fn evaluate_dependency(
    dependency: &OutdatedDep,
    level: Level,
    configured_thresholds: &Thresholds,
) -> Evaluation {
    let (observed, effective_thresholds) = dependency.measure_gap(level, configured_thresholds);
    let status = crate::derive_status(observed, &effective_thresholds);
    let evidence = if status == Status::Pass {
        vec![]
    } else {
        vec![dependency.to_evidence()]
    };

    Evaluation {
        target: dependency.name.clone(),
        outcome: Outcome::Completed {
            status,
            observed,
            thresholds: effective_thresholds,
            evidence,
        },
    }
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
    use crate::Expected;
    use googletest::prelude::*;

    fn evaluate_all(deps: &[OutdatedDep], definition: &Definition) -> Vec<Evaluation> {
        evaluate(Path::new("/any"), deps, definition)
    }

    fn evaluate_one(dep: OutdatedDep, definition: &Definition) -> Evaluation {
        let mut evals = evaluate_all(&[dep], definition);
        assert_eq!(evals.len(), 1, "expected exactly one evaluation");
        evals.remove(0)
    }

    fn dep(name: &str, current: &str, latest: &str) -> OutdatedDep {
        OutdatedDep {
            name: name.into(),
            current: current.parse().unwrap(),
            latest: latest.parse().unwrap(),
        }
    }

    fn patch_level_with_thresholds(warn: u64, fail: u64) -> Definition {
        Definition {
            level: Some(Level::Patch),
            thresholds: Some(Thresholds {
                warn: Some(warn),
                fail: Some(fail),
            }),
        }
    }

    fn major_level_with_thresholds(warn: u64, fail: u64) -> Definition {
        Definition {
            level: Some(Level::Major),
            thresholds: Some(Thresholds {
                warn: Some(warn),
                fail: Some(fail),
            }),
        }
    }

    fn extract_observed(outcome: &Outcome) -> u64 {
        match outcome {
            Outcome::Completed { observed, .. } => *observed,
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    fn extract_evidence(outcome: &Outcome) -> &[Evidence] {
        match outcome {
            Outcome::Completed { evidence, .. } => evidence,
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn single_major_dep_at_default_level_fails() {
        let eval = evaluate_one(dep("a", "1.0.0", "2.0.0"), &Definition::default());

        assert!(eval.is_fail());
    }

    #[test]
    fn major_gap_is_observed_value() {
        let eval = evaluate_one(dep("a", "1.0.0", "3.0.0"), &Definition::default());

        assert_that!(extract_observed(&eval.outcome), eq(2));
    }

    #[test]
    fn evaluation_target_is_dep_name() {
        let eval = evaluate_one(dep("serde", "1.0.0", "2.0.0"), &Definition::default());

        assert_eq!(eval.target, "serde");
    }

    #[test]
    fn same_level_gap_within_warn_threshold_passes() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.1"), &definition);

        assert!(eval.is_pass());
    }

    #[test]
    fn passing_evaluation_has_no_evidence() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.1"), &definition);

        assert_that!(extract_evidence(&eval.outcome), is_empty());
    }

    #[test]
    fn same_level_gap_between_warn_and_fail_warns() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.4"), &definition);

        assert!(eval.is_warn());
    }

    #[test]
    fn same_level_gap_above_fail_threshold_fails() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.8"), &definition);

        assert!(eval.is_fail());
    }

    #[test]
    fn non_passing_evidence_includes_rule_and_versions() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.4"), &definition);

        assert_that!(
            extract_evidence(&eval.outcome)[0].rule,
            some(eq("outdated-patch"))
        );
    }

    #[test]
    fn evidence_found_shows_dep_name_and_current_version() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("serde", "1.0.0", "1.0.4"), &definition);

        let evidence = extract_evidence(&eval.outcome);
        assert_eq!(evidence[0].found, "serde 1.0.0");
    }

    #[test]
    fn evidence_expected_shows_latest_version() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.4"), &definition);

        let evidence = extract_evidence(&eval.outcome);
        assert_eq!(evidence[0].expected, Some(Expected::Text("1.0.4".into())));
    }

    #[test]
    fn superior_drift_fails_with_zero_tolerance() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.1", "1.1.0"), &definition);

        assert!(eval.is_fail());
    }

    #[test]
    fn superior_drift_uses_gap_at_superior_level() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.1", "1.1.0"), &definition);

        assert_that!(extract_observed(&eval.outcome), eq(1));
    }

    #[test]
    fn superior_drift_evidence_uses_superior_kind() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.1", "1.1.0"), &definition);

        assert_that!(
            extract_evidence(&eval.outcome)[0].rule,
            some(eq("outdated-minor"))
        );
    }

    #[test]
    fn kind_below_configured_level_passes() {
        let definition = major_level_with_thresholds(1, 3);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.5"), &definition);

        assert!(eval.is_pass());
    }

    #[test]
    fn kind_below_configured_level_has_zero_observed() {
        let definition = major_level_with_thresholds(1, 3);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.5"), &definition);

        assert_that!(extract_observed(&eval.outcome), eq(0));
    }

    #[test]
    fn no_outdated_deps_returns_passing_evaluation() {
        let evals = evaluate_all(&[], &Definition::default());

        assert_eq!(evals.len(), 1);
        assert!(evals[0].is_pass());
    }

    #[test]
    fn multiple_deps_return_one_evaluation_per_dep() {
        let deps = [dep("a", "1.0.0", "2.0.0"), dep("b", "1.0.0", "3.0.0")];

        let evals = evaluate_all(&deps, &Definition::default());

        assert_eq!(evals.len(), 2);
        assert_eq!(evals[0].target, "a");
        assert_eq!(evals[1].target, "b");
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
