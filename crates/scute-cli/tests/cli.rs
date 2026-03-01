use scute_test_utils::Scute;

#[test]
fn unknown_check_name_produces_structured_error() {
    Scute::cli()
        .check(&["does-not-exist"])
        .expect_error("unknown_check");
}

#[test]
fn missing_check_subcommand_produces_structured_error() {
    Scute::cli().check(&[]).expect_error("invalid_usage");
}

#[test]
fn top_level_invalid_subcommand_produces_invalid_usage() {
    Scute::cli()
        .run(&["commit-message", "feat: test"])
        .expect_error("invalid_usage");
}

#[test]
fn malformed_config_produces_structured_error() {
    Scute::cli()
        .scute_config("not: valid: yaml: [")
        .check(&["commit-message", "feat: add login"])
        .expect_error("invalid_config");
}
