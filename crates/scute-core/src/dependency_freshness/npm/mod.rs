use std::path::Path;

use super::{FetchError, OutdatedDependency};

pub(super) fn is_npm_project(target: &Path) -> bool {
    target.join("package.json").exists()
}

pub(super) fn fetch_outdated(target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
    let output = std::process::Command::new("npm")
        .args(["outdated", "--json"])
        .current_dir(target)
        .output()
        .map_err(|e| FetchError::Failed(format!("could not run npm outdated: {e}")))?;

    // npm outdated exits with code 1 when there ARE outdated deps.
    // It only fails with non-json output on actual errors.
    let stdout = String::from_utf8(output.stdout).map_err(|e| FetchError::Failed(e.to_string()))?;

    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    parse_outdated(&stdout, target)
}

fn parse_outdated(json: &str, target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| FetchError::Failed(e.to_string()))?;

    let packages = root
        .as_object()
        .ok_or_else(|| FetchError::Failed("npm outdated returned non-object".into()))?;

    let project_identity = ProjectIdentity::from_target(target);
    let mut outdated = Vec::new();

    for (name, info) in packages {
        let current = info["current"]
            .as_str()
            .or_else(|| info["wanted"].as_str())
            .and_then(|v| v.parse::<semver::Version>().ok());
        let latest = info["latest"]
            .as_str()
            .and_then(|v| v.parse::<semver::Version>().ok());

        let (Some(current), Some(latest)) = (current, latest) else {
            continue;
        };

        if latest <= current {
            continue;
        }

        let location = info["dependent"]
            .as_str()
            .and_then(|dependent_name| project_identity.resolve_location(dependent_name));

        outdated.push(OutdatedDependency {
            name: name.clone(),
            current,
            latest,
            location,
        });
    }

    Ok(outdated)
}

struct ProjectIdentity {
    dir_name: Option<String>,
    package_name: Option<String>,
}

impl ProjectIdentity {
    fn from_target(target: &Path) -> Self {
        let dir_name = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());

        let package_name = std::fs::read_to_string(target.join("package.json"))
            .ok()
            .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
            .and_then(|value| value["name"].as_str().map(String::from));

        Self {
            dir_name,
            package_name,
        }
    }

    fn resolve_location(&self, dependent_name: &str) -> Option<String> {
        // npm uses the directory name as `dependent`, not the package.json name
        let matches_dir = self.dir_name.as_deref() == Some(dependent_name);
        let matches_package = self.package_name.as_deref() == Some(dependent_name);

        if matches_dir || matches_package {
            return Some("package.json".into());
        }

        None
    }
}
