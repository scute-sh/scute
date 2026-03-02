use scute_test_utils::Scute;

#[test]
fn agent_discovers_available_checks() {
    Scute::mcp().list_checks().expect_contains("commit-message");
}

#[test]
fn valid_commit_message_passes() {
    Scute::mcp()
        .check(&["commit-message", "feat: add login"])
        .expect_pass();
}

#[test]
fn invalid_commit_message_fails() {
    Scute::mcp()
        .check(&["commit-message", "not conventional"])
        .expect_fail();
}
