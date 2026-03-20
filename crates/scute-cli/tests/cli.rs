use scute_test_utils::Scute;

#[test]
fn stdin_provides_commit_message_as_target() {
    Scute::cli_stdin()
        .commit_message("not conventional")
        .run()
        .expect_target("not conventional");
}
