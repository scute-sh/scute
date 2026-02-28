use std::path::Path;

use crate::{CheckResult, Evidence, Expected, Measurement, Thresholds, derive_status};

pub const CHECK_NAME: &str = "dependency-freshness";

const DEFAULT_THRESHOLDS: Thresholds = Thresholds {
    warn: None,
    fail: Some(0),
};

#[derive(Debug)]
pub struct OutdatedDep {
    pub name: String,
    pub current: String,
    pub latest: String,
}

/// # Errors
///
/// Returns an error if `cargo outdated` cannot be executed or produces
/// invalid output.
pub fn run(target: &Path) -> std::io::Result<CheckResult> {
    let outdated = fetch_outdated(target)?;
    Ok(check(&target.display().to_string(), &outdated))
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
pub fn check(target: &str, outdated: &[OutdatedDep]) -> CheckResult {
    let evidence: Vec<Evidence> = outdated
        .iter()
        .map(|dep| {
            Evidence::with_expected(
                "outdated",
                &format!("{} {}", dep.name, dep.current),
                Expected::Text(dep.latest.clone()),
            )
        })
        .collect();
    let observed = evidence.len() as u64;
    let thresholds = DEFAULT_THRESHOLDS;

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
                if current != latest {
                    deps.push(OutdatedDep {
                        name: name.into(),
                        current: current.into(),
                        latest: latest.into(),
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
            current: current.into(),
            latest: latest.into(),
        }
    }

    #[test]
    fn no_outdated_deps_returns_pass_with_all_fields() {
        let result = check(".", &[]);

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
    fn observed_counts_outdated_deps() {
        let deps = vec![
            dep("rand", "0.7.3", "0.9.0"),
            dep("serde", "1.0.0", "1.1.0"),
        ];

        let result = check(".", &deps);

        assert_eq!(result.measurement.observed, 2);
        assert_eq!(result.status, Status::Fail);
    }

    #[test]
    fn evidence_carries_name_current_and_latest() {
        let deps = vec![dep("rand", "0.7.3", "0.9.0")];

        let result = check(".", &deps);

        assert_eq!(result.evidence.len(), 1);
        assert_eq!(result.evidence[0].rule, "outdated");
        assert_eq!(result.evidence[0].found, "rand 0.7.3");
        assert_eq!(
            result.evidence[0].expected,
            Some(Expected::Text("0.9.0".into()))
        );
    }
}
