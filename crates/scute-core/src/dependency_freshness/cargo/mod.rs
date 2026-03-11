mod crates_io;
mod metadata;

use std::path::Path;

use super::{FetchError, OutdatedDependency};

pub(super) fn is_cargo_project(target: &Path) -> bool {
    target.join("Cargo.toml").exists()
}

pub(super) fn fetch_outdated(target: &Path) -> Result<Vec<OutdatedDependency>, FetchError> {
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
            .filter_map(|handle| handle.join().ok())
            .collect()
    })
}
