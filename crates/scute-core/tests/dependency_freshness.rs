use scute_core::dependency_freshness::{self, fetch_outdated};
use scute_test_utils::TestProject;
use test_case::test_case;

struct Context {
    project: fn() -> TestProject,
    outdated_package: &'static str,
    pinned_version: &'static str,
    manifest: &'static str,
}

const CARGO: Context = Context {
    project: TestProject::cargo,
    outdated_package: "rand",
    pinned_version: "=0.7.3",
    manifest: "Cargo.toml",
};

const NPM: Context = Context {
    project: TestProject::npm,
    outdated_package: "is-odd",
    pinned_version: "1.0.0",
    manifest: "package.json",
};

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_report_excludes_transitive_dependencies(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_eq!(
        dependencies.len(),
        1,
        "should only have direct deps, got: {dependencies:?}"
    );
    assert_eq!(dependencies[0].name, context.outdated_package);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_dep_reports_current_version(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_eq!(
        dependencies[0].current.to_string(),
        context.pinned_version.trim_start_matches('=')
    );
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_dep_reports_latest_available_version(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_ne!(dependencies[0].latest, dependencies[0].current);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn no_dependencies_returns_empty_report(context: &Context) {
    let dir = (context.project)().build();
    assert!(fetch_outdated(dir.path()).unwrap().is_empty());
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_report_includes_dev_dependencies(context: &Context) {
    let dir = (context.project)()
        .dev_dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, context.outdated_package);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_dep_location_points_to_manifest(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies[0].location.as_deref(), Some(context.manifest));
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
fn workspace_member_location_points_to_subcrate_manifest() {
    let dir = TestProject::cargo()
        .member("sub", |m| m.dependency("rand", "=0.7.3"))
        .build();
    let deps = fetch_outdated(dir.path()).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].location.as_deref(), Some("sub/Cargo.toml"));
}

#[test]
fn npm_workspace_member_location_points_to_member_manifest() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dependency("is-odd", "1.0.0"))
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, "is-odd");
    assert_eq!(
        dependencies[0].location.as_deref(),
        Some("apps/web/package.json")
    );
}

#[test]
fn npm_workspace_reports_outdated_from_multiple_members() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dependency("is-odd", "1.0.0"))
        .member("packages/utils", |m| m.dependency("is-number", "1.0.0"))
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    let names: Vec<&str> = dependencies.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"is-odd"), "missing is-odd, got: {names:?}");
    assert!(
        names.contains(&"is-number"),
        "missing is-number, got: {names:?}"
    );
}

#[test]
fn npm_workspace_root_deps_location_points_to_root_manifest() {
    let dir = TestProject::npm()
        .dependency("is-odd", "1.0.0")
        .member("apps/web", |m| m.dependency("is-even", "1.0.0"))
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    let root_dep = dependencies.iter().find(|d| d.name == "is-odd").unwrap();
    assert_eq!(root_dep.location.as_deref(), Some("package.json"));
}

#[test]
fn npm_workspace_shared_dep_reported_per_member() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dependency("is-odd", "1.0.0"))
        .member("packages/utils", |m| m.dependency("is-odd", "1.0.0"))
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

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
fn npm_workspace_member_dev_dep_location_points_to_member_manifest() {
    let dir = TestProject::npm()
        .member("apps/web", |m| m.dev_dependency("is-odd", "1.0.0"))
        .build();

    let dependencies = fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, "is-odd");
    assert_eq!(
        dependencies[0].location.as_deref(),
        Some("apps/web/package.json")
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
