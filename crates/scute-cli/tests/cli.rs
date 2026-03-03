use scute_test_utils::Scute;

#[test]
fn stdin_provides_commit_message_as_target() {
    Scute::cli_stdin()
        .check(&["commit-message", "not conventional"])
        .expect_target("not conventional");
}
