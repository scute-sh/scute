use scute_core::dependency_freshness::PackageManager;
use scute_core::dependency_freshness::pnpm::Pnpm;
use scute_test_utils::TestProject;

#[test]
fn workspace_member_location_points_to_member_manifest() {
    let dir = TestProject::pnpm()
        .member("apps/web", |member| member.dependency("is-odd", "1.0.0"))
        .build();

    let dependencies = Pnpm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, "is-odd");
    assert_eq!(
        dependencies[0].location.as_deref(),
        Some("apps/web/package.json")
    );
}

#[test]
fn workspace_reports_outdated_from_multiple_members() {
    let dir = TestProject::pnpm()
        .member("apps/web", |member| member.dependency("is-odd", "1.0.0"))
        .member("packages/utils", |member| {
            member.dependency("is-number", "1.0.0")
        })
        .build();

    let dependencies = Pnpm.fetch_outdated(dir.path()).unwrap();

    let names: Vec<&str> = dependencies.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"is-odd"), "missing is-odd, got: {names:?}");
    assert!(
        names.contains(&"is-number"),
        "missing is-number, got: {names:?}"
    );
}

#[test]
fn workspace_root_deps_location_points_to_root_manifest() {
    let dir = TestProject::pnpm()
        .dependency("is-odd", "1.0.0")
        .member("apps/web", |member| member.dependency("is-even", "1.0.0"))
        .build();

    let dependencies = Pnpm.fetch_outdated(dir.path()).unwrap();

    let root_dep = dependencies.iter().find(|d| d.name == "is-odd").unwrap();
    assert_eq!(root_dep.location.as_deref(), Some("package.json"));
}

#[test]
fn workspace_shared_dep_reported_per_member() {
    let dir = TestProject::pnpm()
        .member("apps/web", |member| member.dependency("is-odd", "1.0.0"))
        .member("packages/utils", |member| {
            member.dependency("is-odd", "1.0.0")
        })
        .build();

    let dependencies = Pnpm.fetch_outdated(dir.path()).unwrap();

    let locations: Vec<_> = dependencies
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
    let dir = TestProject::pnpm()
        .member("apps/web", |member| {
            member.dev_dependency("is-odd", "1.0.0")
        })
        .build();

    let dependencies = Pnpm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, "is-odd");
    assert_eq!(
        dependencies[0].location.as_deref(),
        Some("apps/web/package.json")
    );
}
