use std::path::Path;

use super::FetchError;
use super::OutdatedDependency;

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

    let obj = root
        .as_object()
        .ok_or_else(|| FetchError::Failed("npm outdated returned non-object".into()))?;

    let mut outdated = Vec::new();

    for (name, info) in obj {
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
            .and_then(|dep_name| resolve_location(dep_name, target));

        outdated.push(OutdatedDependency {
            name: name.clone(),
            current,
            latest,
            location,
        });
    }

    Ok(outdated)
}

fn resolve_location(dependent_name: &str, target: &Path) -> Option<String> {
    // Try the root package.json first
    let root_pkg_path = target.join("package.json");
    let root_name = std::fs::read_to_string(&root_pkg_path)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| v["name"].as_str().map(String::from));

    // npm uses the directory name as `dependent`, not the package.json name
    let dir_name = target.file_name().map(|n| n.to_string_lossy().into_owned());

    if dir_name.as_deref() == Some(dependent_name) || root_name.as_deref() == Some(dependent_name) {
        return Some("package.json".into());
    }

    None
}
