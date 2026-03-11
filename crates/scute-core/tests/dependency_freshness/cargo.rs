use scute_core::dependency_freshness::PackageManager;
use scute_core::dependency_freshness::cargo::Cargo;
use scute_test_utils::TestProject;

#[test]
fn excludes_transitive_dependencies() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = Cargo.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1, "should only have direct deps, got: {deps:?}");
    assert_eq!(deps[0].name, "rand");
}

#[test]
fn reports_current_version() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = Cargo.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].current.to_string(), "0.7.3");
}

#[test]
fn reports_latest_available_version() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = Cargo.fetch_outdated(dir.path()).unwrap();

    assert_ne!(deps[0].latest, deps[0].current);
}

#[test]
fn no_dependencies_returns_empty_report() {
    let dir = TestProject::cargo().build();

    assert!(Cargo.fetch_outdated(dir.path()).unwrap().is_empty());
}

#[test]
fn includes_dev_dependencies() {
    let dir = TestProject::cargo()
        .dev_dependency("rand", "=0.7.3")
        .build();

    let deps = Cargo.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "rand");
}

#[test]
fn location_points_to_manifest() {
    let dir = TestProject::cargo().dependency("rand", "=0.7.3").build();

    let deps = Cargo.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].location.as_deref(), Some("Cargo.toml"));
}

#[test]
fn workspace_member_location_points_to_member_manifest() {
    let dir = TestProject::cargo()
        .member("sub", |m| m.dependency("rand", "=0.7.3"))
        .build();

    let deps = Cargo.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].location.as_deref(), Some("sub/Cargo.toml"));
}
