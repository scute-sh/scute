use std::path::Path;

use crate::{CheckResult, Evidence, Expected, Measurement, Thresholds, derive_status};

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

/// # Errors
///
/// Returns an error if `cargo outdated` cannot be executed or produces
/// invalid output.
pub fn run(target: &Path, definition: &Definition) -> std::io::Result<CheckResult> {
    let outdated = fetch_outdated(target)?;
    Ok(check(
        &target.display().to_string(),
        &outdated,
        Some(definition),
    ))
}

/// # Errors
///
/// Returns an error if `cargo outdated` cannot be executed or produces
/// invalid output.
pub fn fetch_outdated(target: &Path) -> std::io::Result<Vec<OutdatedDep>> {
    let output = std::process::Command::new("cargo")
        .args(["outdated", "--format", "json", "--depth", "1"])
        .current_dir(target)
        .output()?;
    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(parse_cargo_outdated(&stdout))
}

#[must_use]
pub fn check(
    target: &str,
    outdated: &[OutdatedDep],
    definition: Option<&Definition>,
) -> CheckResult {
    let level = definition.and_then(|d| d.level);
    let evidence: Vec<Evidence> = outdated
        .iter()
        .filter(|dep| level.is_none_or(|l| dep.kind() >= l))
        .map(|dep| {
            Evidence::with_expected(
                &format!("outdated-{}", dep.kind()),
                &format!("{} {}", dep.name, dep.current),
                Expected::Text(dep.latest.to_string()),
            )
        })
        .collect();
    let observed = evidence.len() as u64;
    let thresholds = definition
        .and_then(|d| d.thresholds.clone())
        .unwrap_or(DEFAULT_THRESHOLDS);

    CheckResult {
        check: CHECK_NAME.into(),
        target: target.into(),
        status: derive_status(observed, &thresholds),
        measurement: Measurement {
            observed,
            thresholds,
        },
        evidence,
    }
}

#[must_use]
pub fn parse_cargo_outdated(output: &str) -> Vec<OutdatedDep> {
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

    fn dep(name: &str, current: &str, latest: &str) -> OutdatedDep {
        OutdatedDep {
            name: name.into(),
            current: current.parse().unwrap(),
            latest: latest.parse().unwrap(),
        }
    }

    #[test]
    fn no_outdated_deps_returns_pass_with_all_fields() {
        let result = check(".", &[], None);

        assert_eq!(result.check, "dependency-freshness");
        assert_eq!(result.status, Status::Pass);
        assert_eq!(result.measurement.observed, 0);
        assert_eq!(
            result.measurement.thresholds,
            Thresholds {
                warn: None,
                fail: Some(0)
            }
        );
        assert!(result.evidence.is_empty());
    }

    #[test]
    fn reports_outdated_dep_count() {
        let deps = vec![
            dep("rand", "0.7.3", "0.9.0"),
            dep("serde", "1.0.0", "1.1.0"),
        ];

        let result = check(".", &deps, None);

        assert_eq!(result.measurement.observed, 2);
        assert_eq!(result.status, Status::Fail);
    }

    #[test]
    fn evidence_contains_dep_name_current_and_latest() {
        let deps = vec![dep("rand", "0.7.3", "0.9.0")];

        let result = check(".", &deps, None);

        assert_eq!(result.evidence.len(), 1);
        assert_eq!(result.evidence[0].found, "rand 0.7.3");
        assert_eq!(
            result.evidence[0].expected,
            Some(Expected::Text("0.9.0".into()))
        );
    }

    #[test]
    fn outdated_deps_above_threshold_fails() {
        let deps = vec![
            dep("a", "1.0.0", "2.0.0"),
            dep("b", "1.0.0", "2.0.0"),
            dep("c", "1.0.0", "2.0.0"),
        ];

        let result = check(".", &deps, None);

        assert_eq!(result.measurement.observed, 3);
        assert_eq!(result.status, Status::Fail);
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

        let result = check(".", &deps, Some(&definition));

        assert_eq!(result.measurement.observed, 5);
        assert_eq!(result.status, Status::Fail);
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

        let result = check(".", &deps, Some(&definition));

        assert_eq!(result.measurement.observed, 2);
        assert_eq!(result.status, Status::Pass);
    }

    #[test]
    fn evidence_rule_reflects_outdated_kind() {
        let deps = vec![dep("a", "1.0.0", "2.0.0")];

        let result = check(".", &deps, None);

        assert_eq!(result.evidence[0].rule, "outdated-major");
    }

    #[test]
    fn major_level_excludes_minor_gap_deps() {
        let deps = vec![dep("a", "1.0.0", "2.0.0"), dep("b", "1.0.0", "1.1.0")];

        let definition = Definition {
            level: Some(Level::Major),
            ..Definition::default()
        };

        let result = check(".", &deps, Some(&definition));

        assert_eq!(result.measurement.observed, 1);
    }

    #[test]
    fn patch_level_includes_all_gaps() {
        let deps = vec![
            dep("a", "1.0.0", "2.0.0"),
            dep("b", "1.0.0", "1.1.0"),
            dep("c", "1.0.0", "1.0.1"),
        ];

        let definition = Definition {
            level: Some(Level::Patch),
            ..Definition::default()
        };

        let result = check(".", &deps, Some(&definition));

        assert_eq!(result.measurement.observed, 3);
    }

    #[test]
    fn minor_level_includes_major_and_minor_gaps() {
        let deps = vec![
            dep("a", "1.0.0", "2.0.0"),
            dep("b", "1.0.0", "1.1.0"),
            dep("c", "1.0.0", "1.0.1"),
        ];

        let definition = Definition {
            level: Some(Level::Minor),
            ..Definition::default()
        };

        let result = check(".", &deps, Some(&definition));

        assert_eq!(result.measurement.observed, 2);
    }
}
