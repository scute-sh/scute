use scute_test_utils::Scute;
use scute_test_utils::mcp::McpConnection;

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

#[test]
fn tools_declare_output_schema() {
    let mut conn = McpConnection::start(std::env::temp_dir().as_path());
    conn.initialize();
    let response = conn.request("tools/list", &serde_json::json!({}));
    let tools = response["result"]["tools"].as_array().expect("tools array");

    let tool = tools
        .iter()
        .find(|t| t["name"] == "check_commit_message")
        .expect("check_commit_message tool exists");

    let schema = tool
        .get("outputSchema")
        .expect("outputSchema is present on the tool definition");
    assert_eq!(schema["type"], "object", "root schema type must be object");

    let props = schema["properties"].as_object().expect("has properties");
    assert!(props.contains_key("check"), "schema defines 'check'");
    assert!(props.contains_key("target"), "schema defines 'target'");
    assert!(
        props.contains_key("evaluation"),
        "schema defines 'evaluation'"
    );
    assert!(props.contains_key("error"), "schema defines 'error'");
}
