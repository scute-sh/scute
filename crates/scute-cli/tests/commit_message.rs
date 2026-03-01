use scute_test_utils::Scute;

#[test]
fn passing_check_exits_with_code_0() {
    Scute::cli()
        .check(&["commit-message", "feat: add login"])
        .expect_check_pass();
}

#[test]
fn no_argument_reads_message_from_stdin() {
    Scute::cli_stdin()
        .check(&["commit-message", "fix: resolve crash on startup"])
        .expect_target("fix: resolve crash on startup");
}

#[test]
fn cli_argument_becomes_target() {
    Scute::cli()
        .check(&["commit-message", "feat: from argument"])
        .expect_target("feat: from argument");
}

#[test]
fn failing_check_exits_with_code_1() {
    Scute::cli()
        .check(&["commit-message", "not a conventional commit"])
        .expect_check_fail();
}

#[test]
fn config_types_override_defaults() {
    Scute::cli()
        .scute_config(
            r"
checks:
  commit-message:
    config:
      types: [hotfix]
",
        )
        .check(&["commit-message", "hotfix: urgent patch"])
        .expect_check_pass();
}
