use scute_core::dependency_freshness::PackageManager;
use scute_core::dependency_freshness::cargo::Cargo;
use scute_test_utils::TestProject;

#[test]
fn workspace_member_location_points_to_member_manifest() {
    let dir = TestProject::cargo()
        .member("sub", |member| member.dependency("rand", "=0.7.3"))
        .build();

    let dependencies = Cargo.fetch_outdated(dir.path()).unwrap();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].location.as_deref(), Some("sub/Cargo.toml"));
}
