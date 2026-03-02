use scute_test_utils::Scute;
use scute_test_utils::mcp::McpConnection;
use test_case::test_case;

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
fn agent_discovers_dependency_freshness_check() {
    Scute::mcp()
        .list_checks()
        .expect_contains("dependency-freshness");
}

#[test]
fn fresh_project_passes_dependency_freshness() {
    Scute::mcp().check(&["dependency-freshness"]).expect_pass();
}

#[test]
fn uses_working_directory_as_target() {
    Scute::mcp()
        .check(&["dependency-freshness"])
        .expect_target_matches_dir();
}

#[test]
fn nonexistent_path_produces_invalid_target_error() {
    Scute::mcp()
        .check(&["dependency-freshness", "/nonexistent/path"])
        .expect_error("invalid_target");
}

#[test]
fn config_thresholds_override_default_for_dependency_freshness() {
    Scute::mcp()
        .dependency("itoa", "=0.4.8")
        .scute_config(
            r"
checks:
  dependency-freshness:
    thresholds:
      fail: 5
",
        )
        .check(&["dependency-freshness"])
        .expect_pass();
}

#[test]
fn config_types_override_default_for_commit_message() {
    Scute::mcp()
        .scute_config(
            r"
checks:
  commit-message:
    config:
      types: [hotfix]
",
        )
        .check(&["commit-message", "hotfix: urgent patch"])
        .expect_pass();
}

#[test]
fn malformed_config_produces_error() {
    Scute::mcp()
        .scute_config("not: valid: yaml: [")
        .check(&["commit-message", "feat: add login"])
        .expect_error("invalid_config");
}

#[test]
fn empty_config_uses_defaults() {
    Scute::mcp()
        .scute_config("")
        .check(&["commit-message", "feat: add login"])
        .expect_pass();
}

#[test]
fn warn_status_does_not_set_is_error() {
    Scute::mcp()
        .dependency("itoa", "=0.4.8")
        .scute_config(
            r"
checks:
  dependency-freshness:
    thresholds:
      warn: 0
      fail: 5
",
        )
        .check(&["dependency-freshness"])
        .expect_warn();
}

#[test]
fn tool_result_includes_content_text_fallback() {
    let mut conn = McpConnection::start_with_roots(
        std::env::temp_dir().as_path(),
        &[std::env::temp_dir().to_str().unwrap()],
    );
    conn.initialize();
    let response = conn.request(
        "tools/call",
        &serde_json::json!({
            "name": "check_commit_message",
            "arguments": { "message": "feat: add login" },
        }),
    );

    let result = &response["result"];
    let content = result["content"]
        .as_array()
        .expect("content array must be present");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");

    let text: serde_json::Value =
        serde_json::from_str(content[0]["text"].as_str().unwrap()).expect("content text is JSON");
    assert_eq!(text["evaluation"]["status"], "pass");

    assert!(
        result.get("structuredContent").is_some(),
        "structuredContent must also be present"
    );
}

#[test_case("check_commit_message")]
#[test_case("check_dependency_freshness")]
fn tool_declares_output_schema(tool_name: &str) {
    let tool = get_tool(tool_name);
    assert_check_outcome_schema(&tool);
}

fn get_tool(name: &str) -> serde_json::Value {
    let mut conn = McpConnection::start(std::env::temp_dir().as_path());
    conn.initialize();
    let response = conn.request("tools/list", &serde_json::json!({}));
    let tools = response["result"]["tools"].as_array().expect("tools array");
    tools
        .iter()
        .find(|t| t["name"] == name)
        .unwrap_or_else(|| panic!("{name} tool must exist"))
        .clone()
}

fn assert_check_outcome_schema(tool: &serde_json::Value) {
    let name = tool["name"].as_str().unwrap();
    let schema = tool
        .get("outputSchema")
        .unwrap_or_else(|| panic!("{name} must declare outputSchema"));
    assert_eq!(schema["type"], "object", "root schema type must be object");

    let props = schema["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{name}: schema must have properties"));
    for key in ["check", "target", "evaluation", "error"] {
        assert!(
            props.contains_key(key),
            "{name}: schema must define '{key}'"
        );
    }
}
