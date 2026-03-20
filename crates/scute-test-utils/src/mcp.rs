use std::path::Path;

use rmcp::{
    ClientHandler, ErrorData, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, ClientCapabilities, ClientInfo, Implementation,
        InitializeRequestParams, ListRootsResult, Root, Tool,
    },
    service::{RequestContext, RoleClient, RunningService},
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
        let client = McpTestClient::connect(&project_dir);
        let result = client.call_tool(tool_name, tool_args);

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
        let project_dir = dir.path().canonicalize().unwrap_or(dir.path().into());
        let client = McpTestClient::connect(&project_dir);
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
    service: RunningService<RoleClient, RootsProvider>,
    rt: tokio::runtime::Runtime,
}

impl McpTestClient {
    pub fn connect(project_root: &std::path::Path) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let root: Root = serde_json::from_value(serde_json::json!({
            "uri": format!("file://{}", project_root.display())
        }))
        .expect("valid root");
        let service = rt
            .block_on(async {
                let transport = TokioChildProcess::new({
                    let mut cmd = Command::new(target_bin("scute"));
                    cmd.arg("mcp");
                    cmd.current_dir(project_root);
                    cmd
                })
                .expect("failed to spawn scute mcp");

                RootsProvider(vec![root]).serve(transport).await
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

/// A [`ClientHandler`] that provides project roots to the server.
struct RootsProvider(Vec<Root>);

impl ClientHandler for RootsProvider {
    fn get_info(&self) -> ClientInfo {
        InitializeRequestParams::new(
            ClientCapabilities::builder().enable_roots().build(),
            Implementation::new("scute-test", env!("CARGO_PKG_VERSION")),
        )
    }

    fn list_roots(
        &self,
        _: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<ListRootsResult, ErrorData>> + Send + '_ {
        let result: ListRootsResult =
            serde_json::from_value(serde_json::json!({ "roots": self.0 })).expect("valid roots");
        std::future::ready(Ok(result))
    }
}

fn json_object(entries: &[(&str, Option<serde_json::Value>)]) -> serde_json::Value {
    serde_json::Value::Object(
        entries
            .iter()
            .filter_map(|(k, v)| v.as_ref().map(|v| ((*k).into(), v.clone())))
            .collect(),
    )
}

fn non_empty(slice: &[String]) -> Option<serde_json::Value> {
    (!slice.is_empty()).then(|| serde_json::json!(slice))
}

fn mcp_tool_call(input: &CheckInput) -> (&'static str, serde_json::Value) {
    match input {
        CheckInput::CommitMessage { message } => (
            "check_commit_message",
            serde_json::json!({ "message": message }),
        ),
        CheckInput::DependencyFreshness { path } => (
            "check_dependency_freshness",
            json_object(&[("path", path.as_ref().map(|p| serde_json::json!(p)))]),
        ),
        CheckInput::CodeComplexity { paths } => (
            "check_code_complexity",
            json_object(&[("paths", non_empty(paths))]),
        ),
        CheckInput::CodeSimilarity { source_dir, files } => (
            "check_code_similarity",
            json_object(&[
                (
                    "source_dir",
                    source_dir.as_ref().map(|d| serde_json::json!(d)),
                ),
                ("files", non_empty(files)),
            ]),
        ),
    }
}
