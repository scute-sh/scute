use scute_test_utils::mcp::McpTestClient;
use test_case::test_case;

#[test]
fn tool_result_includes_content_text_fallback() {
    let client = McpTestClient::connect(&std::env::temp_dir());
    let result = client.call_tool(
        "check_commit_message",
        &serde_json::json!({ "message": "feat: add login" }),
    );

    assert!(
        !result.content.is_empty(),
        "content must have text fallback"
    );
    assert!(
        result.structured_content.is_some(),
        "structuredContent must also be present"
    );
}

#[test_case("check_commit_message")]
#[test_case("check_dependency_freshness")]
fn tool_declares_output_schema(tool_name: &str) {
    let client = McpTestClient::connect(&std::env::temp_dir());
    let tools = client.list_tools();

    let tool = tools
        .iter()
        .find(|t| t.name == tool_name)
        .unwrap_or_else(|| panic!("{tool_name} tool must exist"));

    let schema = tool
        .output_schema
        .as_ref()
        .unwrap_or_else(|| panic!("{tool_name} must declare outputSchema"));
    assert_eq!(schema["type"], "object", "root schema type must be object");

    let props = schema["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{tool_name}: schema must have properties"));
    for key in ["check", "summary", "findings", "error"] {
        assert!(
            props.contains_key(key),
            "{tool_name}: schema must define '{key}'"
        );
    }
}
