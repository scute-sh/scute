use std::path::Path;

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, CallToolResult, Tool},
    service::{RoleClient, RunningService},
    transport::TokioChildProcess,
};
use tempfile::TempDir;
use tokio::process::Command;

use crate::{Backend, CheckInput, CheckResult, ExitStatus, ListChecksResult, target_bin};

pub(crate) struct McpBackend;

impl McpBackend {
    fn call(
        dir: TempDir,
        working_dir: &Path,
        tool_name: &str,
        tool_args: &serde_json::Value,
    ) -> CheckResult {
        let project_dir = working_dir.canonicalize().unwrap();
        let client = McpTestClient::connect();
        let mut args = tool_args.clone();
        args["project_root"] = serde_json::json!(project_dir);
        let result = client.call_tool(tool_name, &args);

        let json = result
            .structured_content
            .clone()
            .expect("structuredContent must be present");
        let is_error = result.is_error.unwrap_or(false);
        let exit_status = if !is_error {
            ExitStatus::Success
        } else if json.get("error").is_some() {
            ExitStatus::Error
        } else {
            ExitStatus::Failure
        };
        let debug_info = format!("{result:?}");

        CheckResult {
            _dir: dir,
            json,
            project_dir,
            exit_status,
            debug_info,
        }
    }
}

impl Backend for McpBackend {
    fn run_check(&self, dir: TempDir, working_dir: &Path, input: &CheckInput) -> CheckResult {
        let (tool_name, tool_args) = mcp_tool_call(input);
        Self::call(dir, working_dir, tool_name, &tool_args)
    }

    fn list_checks(&self, dir: TempDir) -> ListChecksResult {
        let client = McpTestClient::connect();
        let checks = client
            .list_tools()
            .iter()
            .map(|t| {
                t.name
                    .strip_prefix("check_")
                    .expect("tool name starts with check_")
                    .replace('_', "-")
            })
            .collect();
        ListChecksResult { _dir: dir, checks }
    }
}

/// An MCP client connected to a running Scute MCP server.
///
/// Wraps rmcp's client with its own tokio runtime so callers don't need async.
/// Use for protocol-level tests that need direct access beyond the `Scute` harness.
pub struct McpTestClient {
    service: RunningService<RoleClient, ()>,
    rt: tokio::runtime::Runtime,
}

impl McpTestClient {
    /// Starts the server outside any project: it only knows about a project
    /// through the `project_root` of each tool call.
    pub fn connect() -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let service = rt
            .block_on(async {
                let transport = TokioChildProcess::new({
                    let mut cmd = Command::new(target_bin("scute"));
                    cmd.arg("mcp");
                    cmd.current_dir(std::env::temp_dir());
                    cmd
                })
                .expect("failed to spawn scute mcp");

                ().serve(transport).await
            })
            .expect("failed to connect to scute mcp");

        Self { service, rt }
    }

    pub fn call_tool(&self, name: &str, args: &serde_json::Value) -> CallToolResult {
        self.rt
            .block_on(
                self.service.call_tool(
                    CallToolRequestParams::new(name.to_string())
                        .with_arguments(args.as_object().unwrap().clone()),
                ),
            )
            .expect("call_tool failed")
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        self.rt
            .block_on(self.service.list_all_tools())
            .expect("list_all_tools failed")
    }
}

/// Builds a JSON object, leaving out fields whose value is `null`.
fn json_object(entries: &[(&str, serde_json::Value)]) -> serde_json::Value {
    serde_json::Value::Object(
        entries
            .iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| ((*k).into(), v.clone()))
            .collect(),
    )
}

fn mcp_tool_call(input: &CheckInput) -> (&'static str, serde_json::Value) {
    match input {
        CheckInput::CommitMessage { message } => (
            "check_commit_message",
            serde_json::json!({ "message": message }),
        ),
        CheckInput::DependencyFreshness { path } => (
            "check_dependency_freshness",
            json_object(&[("path", serde_json::json!(path))]),
        ),
        CheckInput::CodeComplexity { paths } => (
            "check_code_complexity",
            json_object(&[("paths", serde_json::json!(paths))]),
        ),
        CheckInput::CodeSimilarity { source_dir, files } => (
            "check_code_similarity",
            json_object(&[
                ("source_dir", serde_json::json!(source_dir)),
                ("files", serde_json::json!(files)),
            ]),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_empty_path_list_as_empty_array() {
        let input = CheckInput::CodeComplexity {
            paths: Some(vec![]),
        };

        let (_, args) = mcp_tool_call(&input);

        assert_eq!(args, serde_json::json!({ "paths": [] }));
    }

    #[test]
    fn sends_empty_focus_file_list_as_empty_array() {
        let input = CheckInput::CodeSimilarity {
            source_dir: None,
            files: Some(vec![]),
        };

        let (_, args) = mcp_tool_call(&input);

        assert_eq!(args, serde_json::json!({ "files": [] }));
    }

    #[test]
    fn leaves_out_path_list_without_input() {
        let input = CheckInput::CodeComplexity { paths: None };

        let (_, args) = mcp_tool_call(&input);

        assert_eq!(args, serde_json::json!({}));
    }
}
