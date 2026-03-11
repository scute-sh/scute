use std::collections::HashMap;
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

    let layout = WorkspaceLayout::from_target(target);
    let mut outdated = Vec::new();

    for (name, info) in packages {
        // npm returns an object for single-dependent packages,
        // an array when multiple workspace members depend on the same package.
        let entries: &[serde_json::Value] = match info {
            serde_json::Value::Array(arr) => arr,
            single @ serde_json::Value::Object(_) => std::slice::from_ref(single),
            _ => continue,
        };

        for entry in entries {
            if let Some(dependency) = parse_entry(name, entry, &layout) {
                outdated.push(dependency);
            }
        }
    }

    Ok(outdated)
}

fn parse_entry(
    name: &str,
    entry: &serde_json::Value,
    layout: &WorkspaceLayout,
) -> Option<OutdatedDependency> {
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

    let location = entry["dependent"]
        .as_str()
        .and_then(|dependent| layout.resolve_location(dependent));

    Some(OutdatedDependency {
        name: name.into(),
        current,
        latest,
        location,
    })
}

struct WorkspaceLayout {
    root_dir_name: Option<String>,
    root_package_name: Option<String>,
    members: HashMap<String, String>,
}

impl WorkspaceLayout {
    fn from_target(target: &Path) -> Self {
        let root_dir_name = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());

        let package_json = std::fs::read_to_string(target.join("package.json"))
            .ok()
            .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok());

        let root_package_name = package_json
            .as_ref()
            .and_then(|v| v["name"].as_str().map(String::from));

        let members = package_json
            .as_ref()
            .and_then(|v| v["workspaces"].as_array())
            .map(|patterns| discover_members(target, patterns))
            .unwrap_or_default();

        Self {
            root_dir_name,
            root_package_name,
            members,
        }
    }

    fn resolve_location(&self, dependent: &str) -> Option<String> {
        if let Some(manifest) = self.members.get(dependent) {
            return Some(manifest.clone());
        }

        let matches_dir = self.root_dir_name.as_deref() == Some(dependent);
        let matches_package = self.root_package_name.as_deref() == Some(dependent);

        if matches_dir || matches_package {
            return Some("package.json".into());
        }

        None
    }
}

fn discover_members(target: &Path, patterns: &[serde_json::Value]) -> HashMap<String, String> {
    let mut members = HashMap::new();

    for pattern in patterns.iter().filter_map(|v| v.as_str()) {
        for dir in resolve_workspace_pattern(target, pattern) {
            if !dir.join("package.json").exists() {
                continue;
            }
            let Some(basename) = dir.file_name() else {
                continue;
            };
            let Ok(relative) = dir.strip_prefix(target) else {
                continue;
            };
            members.insert(
                basename.to_string_lossy().into_owned(),
                format!("{}/package.json", relative.display()),
            );
        }
    }

    members
}

fn resolve_workspace_pattern(target: &Path, pattern: &str) -> Vec<std::path::PathBuf> {
    if !pattern.contains('*') {
        return vec![target.join(pattern)];
    }

    // For glob patterns like "packages/*", list the parent directory.
    let Some(prefix) = pattern.strip_suffix("/*") else {
        return vec![];
    };

    let parent = target.join(prefix);
    let Ok(entries) = std::fs::read_dir(parent) else {
        return vec![];
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}
