use scute_test_utils::Scute;

#[test]
fn unknown_check_name_exits_with_error() {
    Scute::cli()
        .check(&["does-not-exist"])
        .expect_error_containing("does-not-exist");
}

#[test]
fn invalid_config_exits_with_error() {
    Scute::cli()
        .scute_config("not: valid: yaml: [")
        .check(&["commit-message", "feat: add login"])
        .expect_error_containing("failed to parse .scute.yml");
}
