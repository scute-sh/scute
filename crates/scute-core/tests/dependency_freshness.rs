#[path = "dependency_freshness/cargo.rs"]
mod cargo;
#[path = "dependency_freshness/npm.rs"]
mod npm;

use scute_core::dependency_freshness::{self, OutdatedDependency, fetch_outdated};
use scute_test_utils::TestProject;

fn assert_single_dep(deps: &[OutdatedDependency], name: &str, expected_location: &str) {
    let matching: Vec<_> = deps.iter().filter(|d| d.name == name).collect();
    assert_eq!(
        matching.len(),
        1,
        "{name} should appear exactly once, got: {matching:?}"
    );
    assert_eq!(matching[0].location.as_deref(), Some(expected_location));
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

    let evals = dependency_freshness::check(dir.path(), &definition).unwrap();

    assert_eq!(
        evals[0].target,
        dir.path().canonicalize().unwrap().display().to_string()
    );
}

#[test]
fn polyglot_monorepo_reports_each_root_once_with_relative_locations() {
    let dir = TestProject::empty()
        .nested(
            "backend",
            TestProject::cargo().member("crates/api", |m| m.dependency("rand", "=0.7.3")),
        )
        .nested(
            "frontend",
            TestProject::npm().member("apps/web", |m| m.dependency("is-odd", "1.0.0")),
        )
        .build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_single_dep(&deps, "rand", "backend/crates/api/Cargo.toml");
    assert_single_dep(&deps, "is-odd", "frontend/apps/web/package.json");
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
