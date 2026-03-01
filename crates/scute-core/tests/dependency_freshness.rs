use scute_core::dependency_freshness::fetch_outdated;
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
        err.to_string().contains("cargo outdated failed"),
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
