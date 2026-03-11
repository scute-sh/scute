use scute_core::dependency_freshness::{self, OutdatedDependency, fetch_outdated};
use scute_test_utils::TestProject;
use test_case::test_case;

struct Ecosystem {
    project: fn() -> TestProject,
    dep_name: &'static str,
    dep_version: &'static str,
    manifest: &'static str,
}

const CARGO: Ecosystem = Ecosystem {
    project: TestProject::cargo,
    dep_name: "rand",
    dep_version: "=0.7.3",
    manifest: "Cargo.toml",
};

const NPM: Ecosystem = Ecosystem {
    project: TestProject::npm,
    dep_name: "is-odd",
    dep_version: "1.0.0",
    manifest: "package.json",
};

fn fetch_single_dep(eco: &Ecosystem) -> OutdatedDependency {
    let dir = (eco.project)()
        .dependency(eco.dep_name, eco.dep_version)
        .build();
    let mut deps = fetch_outdated(dir.path()).unwrap();
    assert_eq!(deps.len(), 1, "expected exactly one dep, got: {deps:?}");
    deps.remove(0)
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_report_includes_direct_dependencies(eco: &Ecosystem) {
    let dep = fetch_single_dep(eco);
    assert_eq!(dep.name, eco.dep_name);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_dep_reports_current_version(eco: &Ecosystem) {
    let dep = fetch_single_dep(eco);
    assert_eq!(
        dep.current.to_string(),
        eco.dep_version.trim_start_matches('=')
    );
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_dep_reports_latest_available_version(eco: &Ecosystem) {
    let dep = fetch_single_dep(eco);
    assert_ne!(dep.latest, dep.current);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn no_dependencies_returns_empty_report(eco: &Ecosystem) {
    let dir = (eco.project)().build();
    assert!(fetch_outdated(dir.path()).unwrap().is_empty());
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_report_includes_dev_dependencies(eco: &Ecosystem) {
    let dir = (eco.project)()
        .dev_dependency(eco.dep_name, eco.dep_version)
        .build();
    let deps = fetch_outdated(dir.path()).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, eco.dep_name);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
fn outdated_dep_location_points_to_manifest(eco: &Ecosystem) {
    let dep = fetch_single_dep(eco);
    assert_eq!(dep.location.as_deref(), Some(eco.manifest));
}

#[test]
fn fetch_from_unsupported_directory_reports_error() {
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
fn check_sets_target_to_canonicalized_path() {
    let dir = TestProject::cargo().build();
    let definition = dependency_freshness::Definition::default();
    let evals = dependency_freshness::check(dir.path(), &definition).unwrap();
    assert_eq!(
        evals[0].target,
        dir.path().canonicalize().unwrap().display().to_string()
    );
}
