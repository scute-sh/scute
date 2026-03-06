use scute_core::dependency_freshness::{self, fetch_outdated};
use scute_test_utils::TestProject;

#[test]
fn outdated_report_excludes_transitive_dependencies() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1, "should only have direct deps, got: {deps:?}");
    assert_eq!(deps[0].name, "rand");
}

#[test]
fn outdated_dep_reports_current_version() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].current.to_string(), "0.7.3");
}

#[test]
fn outdated_dep_reports_latest_available_version() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_ne!(deps[0].latest, deps[0].current);
}

#[test]
fn no_dependencies_returns_empty_report() {
    let dir = TestProject::cargo().build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert!(deps.is_empty());
}

#[test]
fn fetch_from_non_cargo_directory_reports_error() {
    let dir = TestProject::empty().build();

    let err = fetch_outdated(dir.path()).unwrap_err();

    assert!(
        err.to_string().contains("invalid target"),
        "expected helpful error, got: {err}"
    );
}

#[test]
fn outdated_report_includes_dev_dependencies() {
    let dir = TestProject::cargo()
        .dev_dependency("rand", "=0.7.3")
        .build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "rand");
}

#[test]
fn outdated_dep_location_points_to_manifest() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].location.as_deref(), Some("Cargo.toml"));
}

#[test]
fn workspace_member_location_points_to_subcrate_manifest() {
    let dir = TestProject::cargo()
        .member("sub", |m| m.dependency("rand", "=0.7.3"))
        .build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].location.as_deref(), Some("sub/Cargo.toml"));
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
