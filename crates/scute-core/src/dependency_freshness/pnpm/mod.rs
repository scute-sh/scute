use std::path::Path;

use super::{FetchError, OutdatedDependency, PackageManager};

pub struct Pnpm;

impl PackageManager for Pnpm {
    fn is_project_root(&self, target: &Path) -> bool {
        // pnpm only creates the lockfile at the workspace/project root,
        // never inside individual members, so its presence is sufficient.
        target.join("pnpm-lock.yaml").exists()
    }

    fn fetch_outdated(&self, target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
        let output = std::process::Command::new("pnpm")
            .args(["outdated", "--recursive", "--format", "json"])
            .current_dir(target)
            .output()
            .map_err(|e| FetchError::Failed(format!("could not run pnpm outdated: {e}")))?;

        let stdout =
            String::from_utf8(output.stdout).map_err(|e| FetchError::Failed(e.to_string()))?;

        if stdout.trim().is_empty() || stdout.trim() == "{}" {
            check_for_silent_failure(output.status, &output.stderr)?;
            return Ok(vec![]);
        }

        // Canonicalize so path comparisons match pnpm's absolute paths,
        // which go through resolved symlinks (e.g. /tmp → /private/tmp on macOS).
        let root = target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf());

        parse_outdated(&stdout, &root)
    }
}

/// pnpm exits non-zero with empty stdout when it genuinely fails.
/// Distinguish that from "no outdated deps" by checking stderr.
fn check_for_silent_failure(
    status: std::process::ExitStatus,
    stderr: &[u8],
) -> Result<(), FetchError> {
    if status.success() {
        return Ok(());
    }
    let msg = String::from_utf8_lossy(stderr);
    if !msg.trim().is_empty() {
        return Err(FetchError::Failed(msg.trim().to_string()));
    }
    Ok(())
}

fn parse_outdated(json: &str, root: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
    let top: serde_json::Value =
        serde_json::from_str(json).map_err(|e| FetchError::Failed(e.to_string()))?;

    let packages = top
        .as_object()
        .ok_or_else(|| FetchError::Failed("pnpm outdated returned non-object".into()))?;

    let mut outdated = Vec::new();
    for (name, info) in packages {
        outdated.extend(parse_entry(name, info, root));
    }

    Ok(outdated)
}

fn parse_entry(name: &str, entry: &serde_json::Value, root: &Path) -> Vec<OutdatedDependency> {
    let Some(current) = entry["current"]
        .as_str()
        .or_else(|| entry["wanted"].as_str())
        .and_then(|v| v.parse::<semver::Version>().ok())
    else {
        return vec![];
    };
    let Some(latest) = entry["latest"]
        .as_str()
        .and_then(|v| v.parse::<semver::Version>().ok())
    else {
        return vec![];
    };

    if latest <= current {
        return vec![];
    }

    match entry["dependentPackages"].as_array() {
        Some(dependents) => dependents
            .iter()
            .map(|dep| {
                let location = dep["location"]
                    .as_str()
                    .and_then(|loc| resolve_location(loc, root));
                OutdatedDependency {
                    name: name.into(),
                    current: current.clone(),
                    latest: latest.clone(),
                    location,
                }
            })
            .collect(),
        None => vec![OutdatedDependency {
            name: name.into(),
            current,
            latest,
            location: Some("package.json".into()),
        }],
    }
}

fn resolve_location(location: &str, root: &Path) -> Option<String> {
    let dep_path = Path::new(location);
    if !dep_path.is_absolute() {
        return None;
    }
    let relative = dep_path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        Some("package.json".into())
    } else {
        Some(format!("{}/package.json", relative.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use test_case::test_case;

    fn any_root() -> PathBuf {
        PathBuf::from("/project")
    }

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

        let deps = parse_outdated(&input.to_string(), &any_root()).unwrap();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "is-odd");
        assert_eq!(deps[0].current.to_string(), "1.0.0");
        assert_eq!(deps[0].latest.to_string(), "3.0.1");
    }

    #[test]
    fn rejects_non_object_root() {
        let result = parse_outdated("[1,2,3]", &any_root());

        assert!(result.is_err());
    }

    #[test_case("not-semver", "3.0.1" ; "unparseable_version")]
    #[test_case("3.0.1", "3.0.1" ; "latest_not_newer")]
    fn skips_entry(current: &str, latest: &str) {
        let input = json!({ "is-odd": { "current": current, "latest": latest } });

        let deps = parse_outdated(&input.to_string(), &any_root()).unwrap();

        assert!(deps.is_empty());
    }

    #[test]
    fn no_dependents_defaults_location_to_package_json() {
        let input = json!({
            "is-odd": { "current": "1.0.0", "latest": "3.0.1" }
        });

        let deps = parse_outdated(&input.to_string(), &any_root()).unwrap();

        assert_eq!(deps[0].location.as_deref(), Some("package.json"));
    }

    #[test_case("/tmp/my-project", "package.json" ; "root_to_package_json")]
    #[test_case("/tmp/my-project/apps/web", "apps/web/package.json" ; "member_to_relative_manifest")]
    fn resolves_dependent_location(location: &str, expected: &str) {
        let root = PathBuf::from("/tmp/my-project");
        let input = json!({
            "is-odd": {
                "current": "1.0.0",
                "latest": "3.0.1",
                "dependentPackages": [{ "name": "any", "location": location }]
            }
        });

        let deps = parse_outdated(&input.to_string(), &root).unwrap();

        assert_eq!(deps[0].location.as_deref(), Some(expected));
    }

    #[test]
    fn shared_dep_produces_one_entry_per_dependent() {
        let root = PathBuf::from("/tmp/my-project");
        let input = json!({
            "is-odd": {
                "current": "1.0.0",
                "latest": "3.0.1",
                "dependentPackages": [
                    { "name": "@test/web", "location": "/tmp/my-project/apps/web" },
                    { "name": "@test/utils", "location": "/tmp/my-project/packages/utils" }
                ]
            }
        });

        let deps = parse_outdated(&input.to_string(), &root).unwrap();

        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].location.as_deref(), Some("apps/web/package.json"));
        assert_eq!(
            deps[1].location.as_deref(),
            Some("packages/utils/package.json")
        );
    }
}
