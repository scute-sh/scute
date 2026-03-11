use scute_core::dependency_freshness::PackageManager;
use scute_core::dependency_freshness::npm::Npm;
use scute_test_utils::TestProject;

#[test]
fn excludes_transitive_dependencies() {
    let dir = TestProject::npm().dependency("is-odd", "1.0.0").build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1, "should only have direct deps, got: {deps:?}");
    assert_eq!(deps[0].name, "is-odd");
}

#[test]
fn reports_current_version() {
    let dir = TestProject::npm().dependency("is-odd", "1.0.0").build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].current.to_string(), "1.0.0");
}

#[test]
fn reports_latest_available_version() {
    let dir = TestProject::npm().dependency("is-odd", "1.0.0").build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    assert_ne!(deps[0].latest, deps[0].current);
}

#[test]
fn no_dependencies_returns_empty_report() {
    let dir = TestProject::npm().build();

    assert!(Npm.fetch_outdated(dir.path()).unwrap().is_empty());
}

#[test]
fn includes_dev_dependencies() {
    let dir = TestProject::npm().dev_dependency("is-odd", "1.0.0").build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "is-odd");
}

#[test]
fn location_points_to_manifest() {
    let dir = TestProject::npm().dependency("is-odd", "1.0.0").build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps[0].location.as_deref(), Some("package.json"));
}

#[test]
fn workspace_member_location_points_to_member_manifest() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dependency("is-odd", "1.0.0"))
        .build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "is-odd");
    assert_eq!(deps[0].location.as_deref(), Some("apps/web/package.json"));
}

#[test]
fn workspace_reports_outdated_from_multiple_members() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dependency("is-odd", "1.0.0"))
        .member("packages/utils", |m| m.dependency("is-number", "1.0.0"))
        .build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    let names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"is-odd"), "missing is-odd, got: {names:?}");
    assert!(
        names.contains(&"is-number"),
        "missing is-number, got: {names:?}"
    );
}

#[test]
fn workspace_root_deps_location_points_to_root_manifest() {
    let dir = TestProject::npm()
        .dependency("is-odd", "1.0.0")
        .member("apps/web", |m| m.dependency("is-even", "1.0.0"))
        .build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    let root_dep = deps.iter().find(|d| d.name == "is-odd").unwrap();
    assert_eq!(root_dep.location.as_deref(), Some("package.json"));
}

#[test]
fn workspace_shared_dep_reported_per_member() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dependency("is-odd", "1.0.0"))
        .member("packages/utils", |m| m.dependency("is-odd", "1.0.0"))
        .build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    let locations: Vec<_> = deps
        .iter()
        .filter(|d| d.name == "is-odd")
        .filter_map(|d| d.location.as_deref())
        .collect();
    assert_eq!(
        locations.len(),
        2,
        "shared dep should be reported per member, got: {locations:?}"
    );
    assert!(
        locations.contains(&"apps/web/package.json"),
        "missing web, got: {locations:?}"
    );
    assert!(
        locations.contains(&"packages/utils/package.json"),
        "missing utils, got: {locations:?}"
    );
}

#[test]
fn workspace_member_dev_dep_location_points_to_member_manifest() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dev_dependency("is-odd", "1.0.0"))
        .build();

    let deps = Npm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "is-odd");
    assert_eq!(deps[0].location.as_deref(), Some("apps/web/package.json"));
}
