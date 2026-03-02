use std::path::PathBuf;

use rmcp::{
    ClientHandler, ErrorData, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, ClientCapabilities, ClientInfo, ListRootsResult,
        Root, Tool,
    },
    service::{RequestContext, RoleClient, RunningService},
    transport::TokioChildProcess,
};
use tempfile::TempDir;
use tokio::process::Command;

use crate::{Backend, CheckResult, ListChecksResult, target_bin};

pub(crate) struct McpBackend;

impl Backend for McpBackend {
    fn check(&self, dir: TempDir, args: &[&str]) -> Box<dyn CheckResult> {
        let check_name = args.get(1).expect("check name required");
        let tool_name = format!("check_{}", check_name.replace('-', "_"));
        let tool_args = build_tool_args(check_name, &args[2..]);
        let project_dir = dir.path().canonicalize().unwrap();

        let client = McpTestClient::connect(&project_dir);
        let result = client.call_tool(&tool_name, &tool_args);

        Box::new(McpCheckResult {
            _dir: dir,
            project_dir,
            result,
        })
    }

    fn list_checks(&self, dir: TempDir) -> Box<dyn ListChecksResult> {
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
        Box::new(McpListChecksResult { _dir: dir, checks })
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

        let root = Root {
            uri: format!("file://{}", project_root.display()),
            name: None,
        };
        let service = rt
            .block_on(async {
                let transport = TokioChildProcess::new({
                    let mut cmd = Command::new(target_bin("scute"));
                    cmd.arg("mcp");
                    cmd.current_dir(std::env::temp_dir());
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
            .block_on(self.service.call_tool(CallToolRequestParams {
                name: name.to_string().into(),
                arguments: Some(args.as_object().unwrap().clone()),
                meta: None,
                task: None,
            }))
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
        ClientInfo {
            capabilities: ClientCapabilities::builder().enable_roots().build(),
            ..Default::default()
        }
    }

    fn list_roots(
        &self,
        _: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<ListRootsResult, ErrorData>> + Send + '_ {
        std::future::ready(Ok(ListRootsResult {
            roots: self.0.clone(),
        }))
    }
}

fn build_tool_args(check_name: &str, args: &[&str]) -> serde_json::Value {
    match check_name {
        "commit-message" => {
            let message = args.first().copied().unwrap_or("");
            serde_json::json!({ "message": message })
        }
        "dependency-freshness" => match args.first() {
            Some(path) => serde_json::json!({ "path": path }),
            None => serde_json::json!({}),
        },
        _ => serde_json::json!({}),
    }
}

struct McpListChecksResult {
    _dir: TempDir,
    checks: Vec<String>,
}

impl ListChecksResult for McpListChecksResult {
    fn expect_contains(&self, name: &str) -> &dyn ListChecksResult {
        assert!(
            self.checks.iter().any(|c| c == name),
            "expected check '{name}' in {:?}",
            self.checks
        );
        self
    }
}

struct McpCheckResult {
    _dir: TempDir,
    project_dir: PathBuf,
    result: CallToolResult,
}

impl McpCheckResult {
    fn structured(&self) -> &serde_json::Value {
        self.result
            .structured_content
            .as_ref()
            .expect("structuredContent must be present")
    }

    fn is_error(&self) -> bool {
        self.result.is_error.unwrap_or(false)
    }
}

impl CheckResult for McpCheckResult {
    fn expect_pass(&self) -> &dyn CheckResult {
        assert_eq!(
            self.structured()["evaluation"]["status"],
            "pass",
            "got: {:?}",
            self.structured()
        );
        assert!(!self.is_error(), "pass should not set isError");
        self
    }

    fn expect_warn(&self) -> &dyn CheckResult {
        assert_eq!(
            self.structured()["evaluation"]["status"],
            "warn",
            "got: {:?}",
            self.structured()
        );
        assert!(!self.is_error(), "warn should not set isError");
        self
    }

    fn expect_fail(&self) -> &dyn CheckResult {
        assert_eq!(
            self.structured()["evaluation"]["status"],
            "fail",
            "got: {:?}",
            self.structured()
        );
        assert!(self.is_error(), "fail should set isError");
        self
    }

    fn expect_target(&self, expected: &str) -> &dyn CheckResult {
        assert_eq!(self.structured()["target"], expected);
        self
    }

    fn expect_target_matches_dir(&self) -> &dyn CheckResult {
        let target = self.structured()["target"]
            .as_str()
            .expect("target should be a string");
        assert_eq!(
            std::path::Path::new(target).canonicalize().unwrap(),
            self.project_dir
        );
        self
    }

    fn expect_observed(&self, expected: u64) -> &dyn CheckResult {
        assert_eq!(
            self.structured()["evaluation"]["measurement"]["observed"],
            expected
        );
        self
    }

    fn expect_evidence_rule(&self, index: usize, rule: &str) -> &dyn CheckResult {
        assert_eq!(
            self.structured()["evaluation"]["evidence"][index]["rule"],
            rule
        );
        self
    }

    fn expect_evidence_has_expected(&self, index: usize) -> &dyn CheckResult {
        assert!(
            !self.structured()["evaluation"]["evidence"][index]["expected"].is_null(),
            "expected evidence[{index}].expected to be present"
        );
        self
    }

    fn expect_evidence_no_expected(&self, index: usize) -> &dyn CheckResult {
        assert!(
            self.structured()["evaluation"]["evidence"][index]
                .get("expected")
                .is_none(),
            "expected evidence[{index}].expected to be absent"
        );
        self
    }

    fn expect_no_evidences(&self) -> &dyn CheckResult {
        assert!(
            self.structured()["evaluation"].get("evidence").is_none(),
            "expected evidence key to be absent, got: {}",
            self.structured()["evaluation"]
        );
        self
    }

    fn expect_error(&self, code: &str) -> &dyn CheckResult {
        let error = &self.structured()["error"];
        assert_eq!(error["code"], code, "got: {:?}", self.structured());
        assert!(
            error["message"].is_string(),
            "error.message should be present"
        );
        assert!(
            error["recovery"].is_string(),
            "error.recovery should be present"
        );
        assert!(self.is_error(), "execution error should set isError");
        self
    }

    fn debug(&self) -> &dyn CheckResult {
        eprintln!("result: {:?}", self.result);
        self
    }
}
