use scute_test_utils::mcp::McpTestClient;
use test_case::test_case;

#[test]
fn tool_result_includes_content_text_fallback() {
    let client = McpTestClient::connect();
    let result = client.call_tool(
        "check_commit_message",
        &serde_json::json!({
            "project_root": std::env::temp_dir(),
            "message": "feat: add login",
        }),
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
#[test_case("check_code_complexity")]
#[test_case("check_code_similarity")]
#[test_case("check_dependency_freshness")]
fn tool_requires_project_root(tool_name: &str) {
    let client = McpTestClient::connect();

    let tools = client.list_tools();

    let tool = tools.iter().find(|t| t.name == tool_name).unwrap();
    let required = tool.input_schema["required"].as_array();
    assert!(
        required.is_some_and(|r| r.contains(&serde_json::json!("project_root"))),
        "{tool_name} must require project_root, got {required:?}"
    );
}

#[test_case("check_commit_message")]
#[test_case("check_dependency_freshness")]
fn tool_declares_output_schema(tool_name: &str) {
    let client = McpTestClient::connect();
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

#[test_case("relative/project"; "relative path")]
#[test_case("/scute/does/not/exist"; "missing directory")]
fn rejects_unusable_project_root(project_root: &str) {
    let client = McpTestClient::connect();

    let result = client.call_tool(
        "check_commit_message",
        &serde_json::json!({
            "project_root": project_root,
            "message": "feat: add login",
        }),
    );

    let json = result.structured_content.unwrap();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(json["error"]["code"], "invalid_project_root");
}
