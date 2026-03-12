use std::path::Path;

use super::{FetchError, OutdatedDependency, PackageManager};

pub struct Pnpm;

impl PackageManager for Pnpm {
    fn is_project_root(&self, target: &Path) -> bool {
        target.join("pnpm-lock.yaml").exists()
    }

    fn fetch_outdated(&self, target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
        // pnpm outdated exits with code 1 when there ARE outdated deps.
        // We ignore the exit code and parse stdout directly.
        let output = std::process::Command::new("pnpm")
            .args(["outdated", "--format", "json"])
            .current_dir(target)
            .output()
            .map_err(|e| FetchError::Failed(format!("could not run pnpm outdated: {e}")))?;

        let stdout =
            String::from_utf8(output.stdout).map_err(|e| FetchError::Failed(e.to_string()))?;

        if stdout.trim().is_empty() || stdout.trim() == "{}" {
            return Ok(vec![]);
        }

        parse_outdated(&stdout)
    }
}

fn parse_outdated(json: &str) -> Result<Vec<OutdatedDependency>, FetchError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| FetchError::Failed(e.to_string()))?;

    let packages = root
        .as_object()
        .ok_or_else(|| FetchError::Failed("pnpm outdated returned non-object".into()))?;

    let mut outdated = Vec::new();
    for (name, info) in packages {
        if let Some(dep) = parse_entry(name, info) {
            outdated.push(dep);
        }
    }

    Ok(outdated)
}

fn parse_entry(name: &str, entry: &serde_json::Value) -> Option<OutdatedDependency> {
    let current = entry["current"]
        .as_str()
        .or_else(|| entry["wanted"].as_str())
        .and_then(|v| v.parse::<semver::Version>().ok())?;
    let latest = entry["latest"]
        .as_str()
        .and_then(|v| v.parse::<semver::Version>().ok())?;

    if latest <= current {
        return None;
    }

    Some(OutdatedDependency {
        name: name.into(),
        current,
        latest,
        location: Some("package.json".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_single_outdated_dependency() {
        let input = json!({
            "is-odd": {
                "current": "1.0.0",
                "latest": "3.0.1",
                "wanted": "1.0.0",
                "isDeprecated": false,
                "dependencyType": "dependencies"
            }
        });

        let deps = parse_outdated(&input.to_string()).unwrap();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "is-odd");
        assert_eq!(deps[0].current.to_string(), "1.0.0");
        assert_eq!(deps[0].latest.to_string(), "3.0.1");
    }

    #[test]
    fn rejects_non_object_root() {
        let result = parse_outdated("[1,2,3]");

        assert!(result.is_err());
    }

    #[test]
    fn skips_entries_with_unparseable_versions() {
        let input = json!({
            "is-odd": {
                "current": "not-semver",
                "latest": "3.0.1",
                "dependencyType": "dependencies"
            }
        });

        let deps = parse_outdated(&input.to_string()).unwrap();

        assert!(deps.is_empty());
    }

    #[test]
    fn skips_entries_where_latest_is_not_newer() {
        let input = json!({
            "is-odd": {
                "current": "3.0.1",
                "latest": "3.0.1",
                "dependencyType": "dependencies"
            }
        });

        let deps = parse_outdated(&input.to_string()).unwrap();

        assert!(deps.is_empty());
    }

    #[test]
    fn location_defaults_to_package_json() {
        let input = json!({
            "is-odd": {
                "current": "1.0.0",
                "latest": "3.0.1",
                "dependencyType": "dependencies"
            }
        });

        let deps = parse_outdated(&input.to_string()).unwrap();

        assert_eq!(deps[0].location.as_deref(), Some("package.json"));
    }
}
