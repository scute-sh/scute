use googletest::prelude::*;
use scute_test_utils::Scute;

#[gtest]
fn agent_discovers_available_checks() {
    let checks = Scute::mcp().list_checks();

    expect_that!(checks, contains(eq("commit-message")));
}
