use std::collections::HashMap;
use std::path::Path;

use super::{FetchError, OutdatedDependency, PackageManager};

pub(super) struct Npm;

impl PackageManager for Npm {
    /// Ask `npm query :root` whether this directory is the project root.
    /// Returns true for both standalone projects and workspace roots,
    /// false for workspace members (whose root is an ancestor).
    fn is_project_root(&self, target: &Path) -> bool {
        let Ok(output) = std::process::Command::new("npm")
            .args(["query", ":root"])
            .current_dir(target)
            .output()
        else {
            return false;
        };

        if !output.status.success() {
            return false;
        }

        let root_path = String::from_utf8(output.stdout)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| {
                v.as_array()?
                    .first()?
                    .get("path")?
                    .as_str()
                    .map(String::from)
            });

        let Some(canonical_target) = target.canonicalize().ok() else {
            return false;
        };

        root_path
            .as_deref()
            .is_some_and(|root| Path::new(root) == canonical_target)
    }

    fn fetch_outdated(&self, target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
        let output = std::process::Command::new("npm")
            .args(["outdated", "--json"])
            .current_dir(target)
            .output()
            .map_err(|e| FetchError::Failed(format!("could not run npm outdated: {e}")))?;

        // npm outdated exits with code 1 when there ARE outdated deps.
        // It only fails with non-json output on actual errors.
        let stdout =
            String::from_utf8(output.stdout).map_err(|e| FetchError::Failed(e.to_string()))?;

        if stdout.trim().is_empty() {
            return Ok(vec![]);
        }

        let layout = WorkspaceLayout::from_target(target);
        parse_outdated(&stdout, &layout)
    }
}

fn parse_outdated(
    json: &str,
    layout: &WorkspaceLayout,
) -> Result<Vec<OutdatedDependency>, FetchError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| FetchError::Failed(e.to_string()))?;

    let packages = root
        .as_object()
        .ok_or_else(|| FetchError::Failed("npm outdated returned non-object".into()))?;

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
            if let Some(dependency) = parse_entry(name, entry, layout) {
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

    let dirs = patterns
        .iter()
        .filter_map(|v| v.as_str())
        .flat_map(|pattern| resolve_workspace_pattern(target, pattern));

    for dir in dirs {
        register_member(target, &dir, &mut members);
    }

    members
}

fn resolve_workspace_pattern(target: &Path, pattern: &str) -> Vec<std::path::PathBuf> {
    let full = format!("{}/{pattern}", target.display());
    glob::glob(&full)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|p| p.is_dir())
        .collect()
}

fn register_member(target: &Path, dir: &Path, members: &mut HashMap<String, String>) {
    if !dir.join("package.json").exists() {
        return;
    }
    let Ok(relative) = dir.strip_prefix(target) else {
        return;
    };
    let manifest = format!("{}/package.json", relative.display());

    // Index by directory basename. npm currently uses directory names
    // in the `dependent` field of `npm outdated --json`.
    if let Some(basename) = dir.file_name() {
        members.insert(basename.to_string_lossy().into_owned(), manifest.clone());
    }

    // Also index by package name, in case npm changes behavior.
    if let Some(pkg_name) = read_package_name(dir) {
        members.insert(pkg_name, manifest);
    }
}

fn read_package_name(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&contents).ok()?;
    value["name"].as_str().map(String::from)
}
