use scute_test_utils::Scute;

#[test]
fn malformed_config_produces_structured_error() {
    Scute::cli()
        .scute_config("not: valid: yaml: [")
        .check(&["commit-message", "feat: add login"])
        .expect_error("invalid_config");
}
