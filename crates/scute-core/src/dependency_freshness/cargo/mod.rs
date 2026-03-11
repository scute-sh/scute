mod crates_io;
mod metadata;

use std::path::Path;

use super::{FetchError, OutdatedDependency, PackageManager};

pub(super) struct Cargo;

impl PackageManager for Cargo {
    /// Ask `cargo metadata` whether this directory is the workspace root.
    /// Returns true for both standalone projects and workspace roots,
    /// false for workspace members (whose root is an ancestor).
    fn is_project_root(&self, target: &Path) -> bool {
        let Ok(output) = std::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .current_dir(target)
            .output()
        else {
            return false;
        };

        if !output.status.success() {
            return false;
        }

        let workspace_root = String::from_utf8(output.stdout)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["workspace_root"].as_str().map(String::from));

        let Some(canonical_target) = target.canonicalize().ok() else {
            return false;
        };

        workspace_root
            .as_deref()
            .is_some_and(|root| Path::new(root) == canonical_target)
    }

    fn fetch_outdated(&self, target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
        let direct_deps = metadata::collect_direct_deps(target)?;

        let latest_versions = fetch_latest_versions(&direct_deps);

        let mut outdated = Vec::new();
        let mut errors = Vec::new();

        for (dependency, result) in &latest_versions {
            match result {
                Ok(Some(latest)) if latest > &dependency.version => {
                    outdated.push(OutdatedDependency {
                        name: dependency.name.clone(),
                        current: dependency.version.clone(),
                        latest: latest.clone(),
                        location: dependency.location_relative_to(target),
                    });
                }
                Ok(_) => {}
                Err(error) => errors.push(format!("{}: {error}", dependency.name)),
            }
        }

        if outdated.is_empty() && errors.len() == direct_deps.len() && !direct_deps.is_empty() {
            return Err(FetchError::Failed(format!(
                "all registry lookups failed: {}",
                errors.join(", ")
            )));
        }

        Ok(outdated)
    }
}

fn fetch_latest_versions(
    dependencies: &[metadata::DirectDep],
) -> Vec<(
    &metadata::DirectDep,
    Result<Option<semver::Version>, FetchError>,
)> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = dependencies
            .iter()
            .map(|dependency| {
                scope.spawn(move || {
                    (
                        dependency,
                        crates_io::fetch_latest_version(&dependency.name),
                    )
                })
            })
            .collect();

        handles
            .into_iter()
            .zip(dependencies)
            .map(|(handle, dep)| match handle.join() {
                Ok(result) => result,
                Err(_) => (
                    dep,
                    Err(FetchError::Failed(format!(
                        "registry lookup failed for {}",
                        dep.name
                    ))),
                ),
            })
            .collect()
    })
}
