use scute_core::dependency_freshness::fetch_outdated;
use scute_test_utils::TestProject;

#[test]
fn outdated_report_excludes_transitive_dependencies() {
    let dir = TestProject::new().dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1, "should only have direct deps, got: {deps:?}");
    assert_eq!(deps[0].name, "rand");
}

#[test]
fn outdated_dep_reports_current_version() {
    let dir = TestProject::new().dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].current.to_string(), "0.7.3");
}

#[test]
fn outdated_dep_reports_latest_available_version() {
    let dir = TestProject::new().dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_ne!(deps[0].latest, deps[0].current);
}

#[test]
fn no_dependencies_returns_empty_report() {
    let dir = TestProject::new().build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert!(deps.is_empty());
}

#[test]
fn outdated_report_includes_dev_dependencies() {
    let dir = TestProject::new().dev_dependency("rand", "=0.7.3").build();

    let deps = fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "rand");
}
