#[doc(hidden)]
pub mod cargo;
#[doc(hidden)]
pub mod npm;
#[doc(hidden)]
pub mod pnpm;

use std::path::Path;

use serde::Deserialize;

use crate::{Evaluation, Evidence, ExecutionError, Expected, Outcome, Status, Thresholds};

pub const CHECK_NAME: &str = "dependency-freshness";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
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

const ZERO_TOLERANCE: Thresholds = Thresholds {
    warn: None,
    fail: Some(0),
};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub level: Option<Level>,
    pub thresholds: Option<Thresholds>,
}

#[derive(Debug)]
pub struct OutdatedDependency {
    pub name: String,
    pub current: semver::Version,
    pub latest: semver::Version,
    pub location: Option<String>,
}

impl OutdatedDependency {
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
            Level::Major => self.latest.major.saturating_sub(self.current.major),
            Level::Minor => self.latest.minor.saturating_sub(self.current.minor),
            Level::Patch => self.latest.patch.saturating_sub(self.current.patch),
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
        Evidence {
            rule: Some(format!("outdated-{}", self.kind())),
            location: self.location.clone(),
            found: format!("{} {}", self.name, self.current),
            expected: Some(Expected::Text(self.latest.to_string())),
        }
    }
}

/// Run the dependency-freshness check against a project directory.
///
/// Discovers supported package managers (Cargo, npm, pnpm) and checks each one found.
///
/// # Errors
///
/// Returns `Err` when the target path doesn't exist, no supported project
/// is found, or the dependency data can't be fetched.
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
pub enum FetchError {
    /// Target path doesn't contain a valid project.
    InvalidTarget(String),
    /// Something else went wrong. The String carries details for debugging.
    Failed(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTarget(msg) => write!(f, "invalid target: {msg}"),
            Self::Failed(msg) => write!(f, "fetch failed: {msg}"),
        }
    }
}

fn classify_error(err: FetchError) -> ExecutionError {
    match err {
        FetchError::InvalidTarget(msg) => ExecutionError {
            code: "invalid_target".into(),
            message: msg,
            recovery: "pass a directory containing a Cargo.toml or package.json".into(),
        },
        FetchError::Failed(msg) => ExecutionError {
            code: "tool_failed".into(),
            message: msg,
            recovery: "check network connectivity and project setup, then try again".into(),
        },
    }
}

/// A package manager that can detect project roots and fetch outdated
/// dependencies.
#[doc(hidden)]
pub trait PackageManager: Send + Sync {
    /// Returns true for standalone projects and workspace roots, false for
    /// workspace members whose root is an ancestor.
    fn is_project_root(&self, dir: &Path) -> bool;

    /// Returns dependencies with locations relative to `dir`.
    fn fetch_outdated(&self, dir: &Path) -> Result<Vec<OutdatedDependency>, FetchError>;
}

/// A discovered manifest file paired with its package manager.
struct Manifest {
    dir: std::path::PathBuf,
    pm: Box<dyn PackageManager>,
}

impl Manifest {
    fn detect(path: &Path) -> Option<Self> {
        let dir = path.parent()?.to_path_buf();
        let pm: Box<dyn PackageManager> = match path.file_name()?.to_str()? {
            "Cargo.toml" => Box::new(cargo::Cargo),
            "package.json" if dir.join("pnpm-lock.yaml").exists() => Box::new(pnpm::Pnpm),
            "package.json" => Box::new(npm::Npm),
            _ => return None,
        };
        Some(Self { dir, pm })
    }

    fn is_project_root(&self) -> bool {
        self.pm.is_project_root(&self.dir)
    }

    fn fetch_outdated(&self) -> Result<Vec<OutdatedDependency>, FetchError> {
        self.pm.fetch_outdated(&self.dir)
    }
}

fn collect_projects(target: &Path) -> Result<Vec<Manifest>, FetchError> {
    let walker = ignore::WalkBuilder::new(target)
        .standard_filters(true)
        .build();

    let manifests: Vec<_> = walker
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .filter_map(|entry| Manifest::detect(entry.path()))
        .collect();

    std::thread::scope(|scope| {
        let handles: Vec<_> = manifests
            .into_iter()
            .map(|manifest| {
                let dir = manifest.dir.display().to_string();
                (
                    scope.spawn(move || manifest.is_project_root().then_some(manifest)),
                    dir,
                )
            })
            .collect();

        let mut projects = Vec::new();
        for (handle, dir) in handles {
            match handle.join() {
                Ok(Some(manifest)) => projects.push(manifest),
                Ok(None) => {}
                Err(_) => {
                    return Err(FetchError::Failed(format!(
                        "root detection failed for {dir}"
                    )));
                }
            }
        }
        Ok(projects)
    })
}

/// Walk `target` for supported package managers, identify project roots,
/// and collect outdated dependencies from each one.
///
/// Dependency locations are prefixed with the project's relative path
/// from `target`, so callers always get target-relative paths.
///
/// Fails fast: if any project root errors out, the whole call fails.
#[doc(hidden)]
pub fn fetch_outdated(target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
    let projects = collect_projects(target)?;

    if projects.is_empty() {
        return Err(FetchError::InvalidTarget(
            "no supported project found".into(),
        ));
    }

    let mut all_outdated = Vec::new();

    for project in &projects {
        let mut deps = project.fetch_outdated()?;
        let prefix = project.dir.strip_prefix(target).unwrap_or(&project.dir);

        if !prefix.as_os_str().is_empty() {
            for dep in &mut deps {
                dep.location = dep
                    .location
                    .as_ref()
                    .map(|loc| format!("{}/{loc}", prefix.display()));
            }
        }

        all_outdated.extend(deps);
    }

    Ok(all_outdated)
}

fn evaluate(
    target: &Path,
    outdated: &[OutdatedDependency],
    definition: &Definition,
) -> Vec<Evaluation> {
    let level = definition.level.unwrap_or_default();
    let configured_thresholds = definition.thresholds.clone().unwrap_or(DEFAULT_THRESHOLDS);

    if outdated.is_empty() {
        return vec![Evaluation::completed(
            target.display().to_string(),
            0,
            configured_thresholds,
            vec![],
        )];
    }

    outdated
        .iter()
        .map(|dependency| evaluate_dependency(dependency, level, &configured_thresholds))
        .collect()
}

fn evaluate_dependency(
    dependency: &OutdatedDependency,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Expected, Status};
    use googletest::prelude::*;
    use test_case::test_case;

    fn evaluate_all(deps: &[OutdatedDependency], definition: &Definition) -> Vec<Evaluation> {
        evaluate(Path::new("/any"), deps, definition)
    }

    fn evaluate_one(dep: OutdatedDependency, definition: &Definition) -> Evaluation {
        let mut evals = evaluate_all(&[dep], definition);
        assert_eq!(evals.len(), 1, "expected exactly one evaluation");
        evals.remove(0)
    }

    fn dep(name: &str, current: &str, latest: &str) -> OutdatedDependency {
        OutdatedDependency {
            name: name.into(),
            current: current.parse().unwrap(),
            latest: latest.parse().unwrap(),
            location: None,
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

    fn extract_status(outcome: &Outcome) -> Status {
        match outcome {
            Outcome::Completed { status, .. } => *status,
            other @ Outcome::Errored(_) => panic!("expected Completed, got {other:?}"),
        }
    }

    fn extract_observed(outcome: &Outcome) -> u64 {
        match outcome {
            Outcome::Completed { observed, .. } => *observed,
            other @ Outcome::Errored(_) => panic!("expected Completed, got {other:?}"),
        }
    }

    fn extract_evidence(outcome: &Outcome) -> &[Evidence] {
        match outcome {
            Outcome::Completed { evidence, .. } => evidence,
            other @ Outcome::Errored(_) => panic!("expected Completed, got {other:?}"),
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
    fn evaluation_target_is_dependency_name() {
        let eval = evaluate_one(dep("serde", "1.0.0", "2.0.0"), &Definition::default());

        assert_eq!(eval.target, "serde");
    }

    #[test_case("1.0.1", Status::Pass ; "below warn threshold passes")]
    #[test_case("1.0.4", Status::Warn ; "between thresholds warns")]
    #[test_case("1.0.8", Status::Fail ; "above fail threshold fails")]
    fn same_level_gap_at_patch_level(latest: &str, expected: Status) {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", latest), &definition);

        assert_eq!(extract_status(&eval.outcome), expected);
    }

    #[test]
    fn passing_evaluation_has_no_evidence() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.1"), &definition);

        assert_that!(extract_evidence(&eval.outcome), is_empty());
    }

    #[test]
    fn non_passing_evidence_includes_rule_found_and_expected() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("serde", "1.0.0", "1.0.4"), &definition);

        let evidence = &extract_evidence(&eval.outcome)[0];
        assert_that!(evidence.rule, some(eq("outdated-patch")));
        assert_eq!(evidence.found, "serde 1.0.0");
        assert_eq!(evidence.expected, Some(Expected::Text("1.0.4".into())));
    }

    #[test]
    fn superior_drift_fails_with_gap_at_superior_level() {
        let definition = patch_level_with_thresholds(2, 5);

        let eval = evaluate_one(dep("a", "1.0.1", "1.1.0"), &definition);

        assert!(eval.is_fail());
        assert_that!(extract_observed(&eval.outcome), eq(1));
        assert_that!(
            extract_evidence(&eval.outcome)[0].rule,
            some(eq("outdated-minor"))
        );
    }

    #[test]
    fn kind_below_configured_level_passes_with_zero_observed() {
        let definition = major_level_with_thresholds(1, 3);

        let eval = evaluate_one(dep("a", "1.0.0", "1.0.5"), &definition);

        assert!(eval.is_pass());
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

    #[test_case(FetchError::InvalidTarget("msg".into()), "invalid_target" ; "invalid target")]
    #[test_case(FetchError::Failed("msg".into()), "tool_failed" ; "failed")]
    fn classify_error_maps_to_correct_code(err: FetchError, expected_code: &str) {
        let result = classify_error(err);

        assert_eq!(result.code, expected_code);
    }

    #[test]
    fn mixed_levels_evaluate_independently() {
        let deps = [dep("a", "1.0.0", "1.0.5"), dep("b", "1.0.0", "2.0.0")];
        let definition = Definition {
            level: Some(Level::Patch),
            thresholds: Some(Thresholds {
                warn: Some(2),
                fail: Some(10),
            }),
        };

        let evals = evaluate_all(&deps, &definition);

        assert!(evals[0].is_warn(), "patch drift at patch level should warn");
        assert!(
            evals[1].is_fail(),
            "major drift at patch level should fail (superior drift)"
        );
    }

    #[test]
    fn gap_saturates_on_downgrade() {
        let d = dep("a", "2.0.0", "1.0.0");

        assert_eq!(d.gap(), 0);
    }

    #[test]
    fn evidence_carries_manifest_location() {
        let definition = patch_level_with_thresholds(2, 5);
        let d = OutdatedDependency {
            name: "serde".into(),
            current: "1.0.0".parse().unwrap(),
            latest: "1.0.4".parse().unwrap(),
            location: Some("crates/scute-mcp/Cargo.toml".into()),
        };

        let eval = evaluate_one(d, &definition);

        let evidence = extract_evidence(&eval.outcome);
        assert_eq!(
            evidence[0].location.as_deref(),
            Some("crates/scute-mcp/Cargo.toml")
        );
    }
}
