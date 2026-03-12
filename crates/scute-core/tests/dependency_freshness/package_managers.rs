use scute_core::dependency_freshness::PackageManager;
use scute_core::dependency_freshness::cargo::Cargo;
use scute_core::dependency_freshness::npm::Npm;
use scute_core::dependency_freshness::pnpm::Pnpm;
use scute_test_utils::TestProject;
use test_case::test_case;

struct Context {
    project: fn() -> TestProject,
    package_manager: fn() -> Box<dyn PackageManager>,
    outdated_package: &'static str,
    pinned_version: &'static str,
    manifest: &'static str,
}

const CARGO: Context = Context {
    project: TestProject::cargo,
    package_manager: || Box::new(Cargo),
    outdated_package: "rand",
    pinned_version: "=0.7.3",
    manifest: "Cargo.toml",
};

const NPM: Context = Context {
    project: TestProject::npm,
    package_manager: || Box::new(Npm),
    outdated_package: "is-odd",
    pinned_version: "1.0.0",
    manifest: "package.json",
};

const PNPM: Context = Context {
    project: TestProject::pnpm,
    package_manager: || Box::new(Pnpm),
    outdated_package: "is-odd",
    pinned_version: "1.0.0",
    manifest: "package.json",
};

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
#[test_case(&PNPM ; "pnpm")]
fn excludes_transitive_dependencies(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = (context.package_manager)()
        .fetch_outdated(dir.path())
        .unwrap();

    assert_eq!(
        dependencies.len(),
        1,
        "should only have direct deps, got: {dependencies:?}"
    );
    assert_eq!(dependencies[0].name, context.outdated_package);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
#[test_case(&PNPM ; "pnpm")]
fn reports_current_version_of_outdated_dependency(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = (context.package_manager)()
        .fetch_outdated(dir.path())
        .unwrap();

    assert_eq!(
        dependencies[0].current.to_string(),
        context.pinned_version.trim_start_matches('=')
    );
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
#[test_case(&PNPM ; "pnpm")]
fn reports_latest_version_of_outdated_dependency(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = (context.package_manager)()
        .fetch_outdated(dir.path())
        .unwrap();

    assert_ne!(dependencies[0].latest, dependencies[0].current);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
#[test_case(&PNPM ; "pnpm")]
fn no_dependencies_returns_empty_report(context: &Context) {
    let dir = (context.project)().build();

    assert!(
        (context.package_manager)()
            .fetch_outdated(dir.path())
            .unwrap()
            .is_empty()
    );
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
#[test_case(&PNPM ; "pnpm")]
fn includes_dev_dependencies(context: &Context) {
    let dir = (context.project)()
        .dev_dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = (context.package_manager)()
        .fetch_outdated(dir.path())
        .unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, context.outdated_package);
}

#[test_case(&CARGO ; "cargo")]
#[test_case(&NPM ; "npm")]
#[test_case(&PNPM ; "pnpm")]
fn location_points_to_manifest(context: &Context) {
    let dir = (context.project)()
        .dependency(context.outdated_package, context.pinned_version)
        .build();

    let dependencies = (context.package_manager)()
        .fetch_outdated(dir.path())
        .unwrap();

    assert_eq!(dependencies[0].location.as_deref(), Some(context.manifest));
}
