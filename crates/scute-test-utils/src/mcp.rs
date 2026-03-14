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

use crate::{Backend, CheckResult, ExitStatus, ListChecksResult, target_bin};

pub(crate) struct McpBackend;

impl Backend for McpBackend {
    fn check(&self, dir: TempDir, working_dir: &Path, args: &[&str]) -> CheckResult {
        let check_name = args.get(1).expect("check name required");
        let tool_name = format!("check_{}", check_name.replace('-', "_"));
        let tool_args = build_tool_args(check_name, &args[2..]);
        let project_dir = working_dir.canonicalize().unwrap();

        let client = McpTestClient::connect(&project_dir);
        let result = client.call_tool(&tool_name, &tool_args);

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

fn build_tool_args(check_name: &str, args: &[&str]) -> serde_json::Value {
    match check_name {
        "commit-message" => {
            let message = args.first().copied().unwrap_or("");
            serde_json::json!({ "message": message })
        }
        "code-complexity" => positional_paths_args("paths", args),
        "code-similarity" => source_files_args(args),
        "dependency-freshness" => match args.first() {
            Some(path) => serde_json::json!({ "path": path }),
            None => serde_json::json!({}),
        },
        _ => serde_json::json!({}),
    }
}

fn positional_paths_args(key: &str, args: &[&str]) -> serde_json::Value {
    match args.first() {
        Some(_) => serde_json::json!({ key: args }),
        None => serde_json::json!({}),
    }
}

fn source_files_args(args: &[&str]) -> serde_json::Value {
    let mut json = serde_json::Map::new();
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--source-dir"
            && let Some(val) = args.get(i + 1)
        {
            json.insert("source_dir".into(), serde_json::json!(val));
            i += 2;
            continue;
        }
        files.push(args[i]);
        i += 1;
    }
    if !files.is_empty() {
        json.insert("files".into(), serde_json::json!(files));
    }
    serde_json::Value::Object(json)
}
