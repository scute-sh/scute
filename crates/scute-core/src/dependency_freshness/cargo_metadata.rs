use std::path::Path;

use super::FetchError;

#[derive(Debug)]
pub(super) struct DirectDep {
    pub name: String,
    pub version: semver::Version,
    manifest_path: Option<String>,
}

impl DirectDep {
    pub fn location_relative_to(&self, base: &Path) -> Option<String> {
        self.manifest_path.as_ref().map(|abs| {
            Path::new(abs)
                .strip_prefix(base)
                .unwrap_or(Path::new(abs))
                .to_string_lossy()
                .into_owned()
        })
    }
}

pub(super) fn collect_direct_deps(target: &Path) -> Result<Vec<DirectDep>, FetchError> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(target)
        .output()
        .map_err(|e| FetchError::Failed(format!("could not run cargo metadata: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("could not find `Cargo.toml`") {
            return Err(FetchError::InvalidTarget(
                "target is not a valid Cargo project".into(),
            ));
        }
        return Err(FetchError::Failed(format!(
            "cargo metadata failed: {stderr}"
        )));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|e| FetchError::Failed(e.to_string()))?;
    parse_metadata(&stdout)
}

fn parse_metadata(json: &str) -> Result<Vec<DirectDep>, FetchError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| FetchError::Failed(e.to_string()))?;

    let member_ids: Vec<&str> = root["workspace_members"]
        .as_array()
        .ok_or_else(|| FetchError::Failed("missing workspace_members".into()))?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();

    let nodes = root["resolve"]["nodes"]
        .as_array()
        .ok_or_else(|| FetchError::Failed("missing resolve.nodes".into()))?;

    let packages = root["packages"]
        .as_array()
        .ok_or_else(|| FetchError::Failed("missing packages".into()))?;

    Ok(nodes
        .iter()
        .filter(|node| {
            let id = node["id"].as_str().unwrap_or_default();
            member_ids.contains(&id)
        })
        .flat_map(|node| collect_member_deps(node, packages))
        .collect())
}

fn collect_member_deps(node: &serde_json::Value, packages: &[serde_json::Value]) -> Vec<DirectDep> {
    let node_id = node["id"].as_str().unwrap_or_default();

    let manifest_path = packages
        .iter()
        .find(|p| p["id"].as_str() == Some(node_id))
        .and_then(|p| p["manifest_path"].as_str())
        .map(String::from);

    let Some(node_deps) = node["deps"].as_array() else {
        return vec![];
    };

    node_deps
        .iter()
        .filter(|dep| is_direct_dep(dep))
        .filter_map(|dep| dep["pkg"].as_str())
        .filter_map(|pkg_id| {
            let mut direct = resolve_package(pkg_id, packages)?;
            direct.manifest_path.clone_from(&manifest_path);
            Some(direct)
        })
        .collect()
}

/// A dep is "direct" if it has at least one `dep_kind` that is null (normal)
/// or "dev", and its source is from crates.io (not a path/git dep).
fn is_direct_dep(dep: &serde_json::Value) -> bool {
    let Some(dep_kinds) = dep["dep_kinds"].as_array() else {
        return false;
    };

    dep_kinds.iter().any(|dk| {
        let kind = dk["kind"].as_str();
        kind.is_none() || kind == Some("dev")
    })
}

fn resolve_package(pkg_id: &str, packages: &[serde_json::Value]) -> Option<DirectDep> {
    let pkg = packages.iter().find(|p| p["id"].as_str() == Some(pkg_id))?;

    // Only include crates.io deps (source starts with "registry+")
    let source = pkg["source"].as_str()?;
    if !source.starts_with("registry+") {
        return None;
    }

    let name = pkg["name"].as_str()?;
    let version = pkg["version"].as_str()?.parse::<semver::Version>().ok()?;

    Some(DirectDep {
        name: name.into(),
        version,
        manifest_path: None,
    })
}
