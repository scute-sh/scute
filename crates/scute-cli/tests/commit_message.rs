use scute_test_utils::Scute;

#[test]
fn passing_check_exits_with_code_0() {
    Scute::cli()
        .check(&["commit-message", "feat: add login"])
        .expect_pass();
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
        .expect_fail();
}

#[test]
fn json_output_nests_observed_and_thresholds_under_measurement() {
    Scute::cli()
        .check(&["commit-message", "feat: add login"])
        .expect_observed(0);
}

#[test]
fn serializes_provided_expected_in_evidence() {
    Scute::cli()
        .check(&["commit-message", "banana: do stuff"])
        .expect_evidence_has_expected(0);
}

#[test]
fn omits_absent_expected_from_evidence() {
    Scute::cli()
        .check(&["commit-message", "feat: add login\nnot separated"])
        .expect_evidence_rule(0, "body-separator")
        .expect_evidence_no_expected(0);
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
        .expect_pass();
}
