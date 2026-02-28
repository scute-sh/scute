use scute_test_utils::Scute;

#[test]
fn passing_check_exits_with_code_0() {
    Scute::cli().check(&["dependency-freshness"]).expect_pass();
}

#[test]
fn failing_check_exits_with_non_zero() {
    Scute::cli()
        .dependency("itoa", "=0.4.8")
        .check(&["dependency-freshness"])
        .expect_fail();
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
        .expect_warn();
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
        .expect_pass();
}
