use scute_test_utils::Scute;
use scute_test_utils::mcp::McpTestClient;
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
    for key in ["check", "target", "evaluation", "error"] {
        assert!(
            props.contains_key(key),
            "{tool_name}: schema must define '{key}'"
        );
    }
}
