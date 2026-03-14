use std::collections::HashMap;
use std::path::Path;

use super::{FetchError, OutdatedDependency, PackageManager};

pub struct Npm;

impl PackageManager for Npm {
    /// Ask `npm query :root` whether this directory is the project root.
    /// Returns true for both standalone projects and workspace roots,
    /// false for workspace members (whose root is an ancestor).
    fn is_project_root(&self, target: &Path) -> bool {
        super::run_and_check_root("npm", &["query", ":root"], target, |v| {
            v.as_array()?
                .first()?
                .get("path")?
                .as_str()
                .map(String::from)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_layout() -> WorkspaceLayout {
        WorkspaceLayout {
            root_dir_name: Some("my-project".into()),
            root_package_name: Some("my-project".into()),
            members: HashMap::new(),
        }
    }

    fn workspace_layout() -> WorkspaceLayout {
        let mut members = HashMap::new();
        members.insert("web".into(), "apps/web/package.json".into());
        members.insert("@test/web".into(), "apps/web/package.json".into());
        WorkspaceLayout {
            root_dir_name: Some("my-project".into()),
            root_package_name: Some("my-project".into()),
            members,
        }
    }

    #[test]
    fn rejects_non_object_root() {
        let result = parse_outdated("[1,2,3]", &empty_layout());
        assert!(result.is_err());
    }

    #[test]
    fn falls_back_to_wanted_when_current_is_missing() {
        let input = json!({
            "is-odd": {
                "wanted": "1.0.0",
                "latest": "3.0.1",
                "dependent": "my-project"
            }
        });

        let deps = parse_outdated(&input.to_string(), &empty_layout()).unwrap();

        assert_eq!(deps[0].current.to_string(), "1.0.0");
    }

    #[test]
    fn parses_array_entries_for_shared_deps() {
        let input = json!({
            "is-odd": [
                { "current": "1.0.0", "latest": "3.0.1", "dependent": "web" },
                { "current": "1.0.0", "latest": "3.0.1", "dependent": "my-project" }
            ]
        });

        let deps = parse_outdated(&input.to_string(), &workspace_layout()).unwrap();

        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn skips_entries_with_unparseable_versions() {
        let input = json!({
            "is-odd": {
                "current": "not-semver",
                "latest": "3.0.1",
                "dependent": "my-project"
            }
        });

        let deps = parse_outdated(&input.to_string(), &empty_layout()).unwrap();

        assert!(deps.is_empty());
    }

    #[test]
    fn resolves_root_dep_by_dir_name() {
        let layout = empty_layout();
        assert_eq!(
            layout.resolve_location("my-project"),
            Some("package.json".into())
        );
    }

    #[test]
    fn resolves_workspace_member_by_basename() {
        let layout = workspace_layout();
        assert_eq!(
            layout.resolve_location("web"),
            Some("apps/web/package.json".into())
        );
    }

    #[test]
    fn resolves_workspace_member_by_package_name() {
        let layout = workspace_layout();
        assert_eq!(
            layout.resolve_location("@test/web"),
            Some("apps/web/package.json".into())
        );
    }

    #[test]
    fn unknown_dependent_returns_none() {
        let layout = empty_layout();
        assert_eq!(layout.resolve_location("totally-unknown"), None);
    }
}
