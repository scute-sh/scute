use scute_test_utils::Scute;

#[test]
fn agent_discovers_available_checks() {
    Scute::mcp().list_checks().expect_contains("commit-message");
}
