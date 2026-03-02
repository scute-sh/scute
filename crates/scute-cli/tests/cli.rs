use scute_test_utils::Scute;

#[test]
fn stdin_provides_commit_message_target() {
    Scute::cli_stdin()
        .check(&["commit-message", "fix: resolve crash on startup"])
        .expect_target("fix: resolve crash on startup");
}
