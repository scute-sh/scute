use scute_core::dependency_freshness::PackageManager;
use scute_core::dependency_freshness::pnpm::Pnpm;
use scute_test_utils::TestProject;

#[test]
fn standalone_project_reports_outdated_dependency() {
    let dir = TestProject::pnpm().dependency("is-odd", "1.0.0").build();

    let dependencies = Pnpm.fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, "is-odd");
    assert_eq!(dependencies[0].location.as_deref(), Some("package.json"));
}
