use scute_test_utils::{Scute, TestProject};

#[test]
fn passing_check_exits_with_code_0() {
    Scute::cli()
        .check(&["dependency-freshness"])
        .expect_check_pass();
}

#[test]
fn failing_check_exits_with_non_zero() {
    Scute::cli()
        .dependency("itoa", "=0.4.8")
        .check(&["dependency-freshness"])
        .expect_check_fail();
}

#[test]
fn uses_working_directory_as_target() {
    Scute::cli()
        .check(&["dependency-freshness"])
        .expect_target_matches_dir();
}

#[test]
fn outdated_deps_between_warn_and_fail_produces_warn() {
    Scute::cli()
        .dependency("itoa", "=0.4.8")
        .scute_config(
            r"
checks:
  dependency-freshness:
    thresholds:
      warn: 0
      fail: 5
",
        )
        .check(&["dependency-freshness"])
        .expect_check_warn();
}

#[test]
fn path_argument_resolves_to_provided_directory() {
    let project = TestProject::cargo().build();
    let canonical = project.path().canonicalize().unwrap();

    Scute::cli()
        .check(&["dependency-freshness", project.path().to_str().unwrap()])
        .expect_check_pass()
        .expect_target(canonical.to_str().unwrap());
}

#[test]
fn non_cargo_directory_produces_error_with_invalid_target() {
    let dir = TestProject::empty().build();

    Scute::cli()
        .check(&["dependency-freshness", dir.path().to_str().unwrap()])
        .expect_error("invalid_target");
}

#[test]
fn config_thresholds_override_default() {
    Scute::cli()
        .dependency("itoa", "=0.4.8")
        .scute_config(
            r"
checks:
  dependency-freshness:
    thresholds:
      fail: 5
",
        )
        .check(&["dependency-freshness"])
        .expect_check_pass();
}
