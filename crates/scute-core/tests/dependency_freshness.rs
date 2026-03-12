#[path = "dependency_freshness/package_managers.rs"]
mod package_managers;
#[path = "dependency_freshness/pnpm.rs"]
mod pnpm;

use scute_core::dependency_freshness::{self, OutdatedDependency, fetch_outdated};
use scute_test_utils::TestProject;

fn assert_dependency_at(dependencies: &[OutdatedDependency], name: &str, location: &str) {
    assert!(
        dependencies
            .iter()
            .any(|d| d.name == name && d.location.as_deref() == Some(location)),
        "{name} at {location} not found in {dependencies:?}"
    );
}

#[test]
fn non_project_directory_reports_error() {
    let dir = TestProject::empty().build();

    let err = fetch_outdated(dir.path()).unwrap_err();

    assert!(
        err.to_string().contains("invalid target"),
        "expected helpful error, got: {err}"
    );
}

#[test]
fn check_sets_target_to_canonicalized_path() {
    let dir = TestProject::cargo().build();
    let definition = dependency_freshness::Definition::default();

    let evaluations = dependency_freshness::check(dir.path(), &definition).unwrap();

    assert_eq!(
        evaluations[0].target,
        dir.path().canonicalize().unwrap().display().to_string()
    );
}

#[test]
fn polyglot_monorepo_reports_each_root_once_with_relative_locations() {
    let dir = TestProject::empty()
        .nested(
            "backend",
            TestProject::cargo().member("crates/api", |member| member.dependency("rand", "=0.7.3")),
        )
        .nested(
            "frontend",
            TestProject::npm().member("apps/web", |member| member.dependency("is-odd", "1.0.0")),
        )
        .nested(
            "tools",
            TestProject::pnpm().member("packages/cli", |member| {
                member.dependency("is-odd", "1.0.0")
            }),
        )
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 3);
    assert_dependency_at(&dependencies, "rand", "backend/crates/api/Cargo.toml");
    assert_dependency_at(&dependencies, "is-odd", "frontend/apps/web/package.json");
    assert_dependency_at(&dependencies, "is-odd", "tools/packages/cli/package.json");
}

#[test]
fn polyglot_monorepo_fails_fast_when_one_root_errors() {
    let dir = TestProject::empty()
        .nested(
            "backend",
            TestProject::cargo()
                .dependency("this-crate-definitely-does-not-exist-scute-test", "=1.0.0"),
        )
        .nested("frontend", TestProject::npm().dependency("is-odd", "1.0.0"))
        .build();

    let result = fetch_outdated(dir.path());

    assert!(
        result.is_err(),
        "should fail entirely when one root errors, not return partial results"
    );
}
