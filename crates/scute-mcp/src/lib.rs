mod schema;

use std::path::{Path, PathBuf};

use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    handler::server::tool::schema_for_output,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    schemars,
    service::Peer,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schema::CheckReportSchema;
use scute_config::ScuteConfig;
use scute_core::report::CheckReport;
use scute_core::{
    ExecutionError, code_complexity, code_similarity, commit_message, dependency_freshness,
};
use serde::de::DeserializeOwned;

const INSTRUCTIONS: &str = "\
Scute gives you a feedback loop to catch problems as you work, not after. \
Run checks proactively: if you're about to take an action and a relevant check \
exists, use it first. Don't wait to be asked.

When a check returns warnings or failures, use the `evidence` entries to understand \
exactly what went wrong and self-correct before proceeding. \
Warnings are not noise. They flag real issues that should be addressed now, \
not deferred. Do not skip over them.";

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckCommitMessageInput {
    /// The full commit message to validate against Conventional Commits.
    message: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckDependencyFreshnessInput {
    /// Path to the project directory. Defaults to the current working directory.
    path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckCodeComplexityInput {
    /// Files or directories to check. Defaults to the project root.
    paths: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckSourceFilesInput {
    /// Directory to scan for source files. Defaults to the project root.
    source_dir: Option<String>,
    /// Files to focus on. When empty, all discovered files are checked.
    files: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct ScuteMcp {
    tool_router: ToolRouter<Self>,
}

impl ScuteMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ScuteMcp {
    /// Validate a commit message before committing.
    ///
    /// Checks subject format (type, optional scope, description), body separation,
    /// footer syntax, and breaking change markers against the Conventional Commits spec.
    #[tool(
        name = "check_commit_message",
        output_schema = schema_for_output::<CheckReportSchema>().unwrap(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    async fn check_commit_message(
        &self,
        peer: Peer<RoleServer>,
        Parameters(input): Parameters<CheckCommitMessageInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let project_root = resolve_project_root(&peer).await?;
        run_check(
            &project_root,
            commit_message::CHECK_NAME,
            |def: &commit_message::Definition| commit_message::check(&input.message, def),
        )
    }

    /// Measure code complexity of functions in your project.
    ///
    /// Scores each function based on how hard it is to understand: nesting,
    /// control flow, logical operators, recursion. Flags functions that
    /// exceed the configured threshold.
    #[tool(
        name = "check_code_complexity",
        output_schema = schema_for_output::<CheckReportSchema>().unwrap(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    async fn check_code_complexity(
        &self,
        peer: Peer<RoleServer>,
        Parameters(input): Parameters<CheckCodeComplexityInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let project_root = resolve_project_root(&peer).await?;
        let paths: Vec<PathBuf> = input
            .paths
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();
        let paths = scute_core::files::paths_or_default(paths, &project_root);
        run_check(
            &project_root,
            code_complexity::CHECK_NAME,
            |def: &code_complexity::Definition| code_complexity::check(&paths, def),
        )
    }

    /// Find code duplication in your project.
    ///
    /// Scans source files for duplicated token sequences. Optionally focus on
    /// specific files to only report clones involving them.
    #[tool(
        name = "check_code_similarity",
        output_schema = schema_for_output::<CheckReportSchema>().unwrap(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    async fn check_code_similarity(
        &self,
        peer: Peer<RoleServer>,
        Parameters(input): Parameters<CheckSourceFilesInput>,
    ) -> Result<CallToolResult, ErrorData> {
        run_source_check(
            &peer,
            input,
            code_similarity::CHECK_NAME,
            code_similarity::check,
        )
        .await
    }

    /// Find outdated dependencies in your project.
    ///
    /// Reports which packages are behind their latest version, how far behind
    /// (patch, minor, major), and what to update to.
    #[tool(
        name = "check_dependency_freshness",
        output_schema = schema_for_output::<CheckReportSchema>().unwrap(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true,
        )
    )]
    async fn check_dependency_freshness(
        &self,
        peer: Peer<RoleServer>,
        Parameters(input): Parameters<CheckDependencyFreshnessInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let project_root = resolve_project_root(&peer).await?;
        let target = path_or_root(input.path, &project_root);
        run_check(
            &project_root,
            dependency_freshness::CHECK_NAME,
            |def: &dependency_freshness::Definition| dependency_freshness::check(&target, def),
        )
    }
}

#[tool_handler]
impl ServerHandler for ScuteMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(INSTRUCTIONS)
    }
}

async fn run_source_check<D: Default + DeserializeOwned>(
    peer: &Peer<RoleServer>,
    input: CheckSourceFilesInput,
    check_name: &str,
    execute: impl FnOnce(&Path, &[PathBuf], &D) -> Result<Vec<scute_core::Evaluation>, ExecutionError>,
) -> Result<CallToolResult, ErrorData> {
    let project_root = resolve_project_root(peer).await?;
    let source_dir = input
        .source_dir
        .map(PathBuf::from)
        .unwrap_or(project_root.clone());
    let focus_files: Vec<PathBuf> = input
        .files
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    run_check(&project_root, check_name, |def: &D| {
        execute(&source_dir, &focus_files, def)
    })
}

fn path_or_root(input: Option<String>, project_root: &Path) -> PathBuf {
    input
        .map(PathBuf::from)
        .unwrap_or(project_root.to_path_buf())
}

async fn resolve_project_root(peer: &Peer<RoleServer>) -> Result<PathBuf, ErrorData> {
    let roots = peer
        .list_roots()
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

    match roots.roots.first() {
        Some(root) => root
            .uri
            .strip_prefix("file://")
            .map(PathBuf::from)
            .ok_or_else(|| ErrorData::invalid_params("root URI must use file:// scheme", None)),
        None => std::env::current_dir().map_err(|e| ErrorData::internal_error(e.to_string(), None)),
    }
}

fn run_check<D: Default + DeserializeOwned>(
    project_root: &Path,
    check_name: &str,
    execute: impl FnOnce(&D) -> Result<Vec<scute_core::Evaluation>, ExecutionError>,
) -> Result<CallToolResult, ErrorData> {
    let definition =
        match ScuteConfig::load(project_root).and_then(|c: ScuteConfig| c.definition(check_name)) {
            Ok(def) => def,
            Err(e) => {
                let report = CheckReport::new(
                    check_name,
                    Err(ExecutionError {
                        code: "invalid_config".into(),
                        message: format!("{e}"),
                        recovery: "check your .scute.yml syntax".into(),
                    }),
                );
                return report_to_result(&report);
            }
        };
    let result = execute(&definition);
    report_to_result(&CheckReport::new(check_name, result))
}

fn report_to_result(report: &CheckReport) -> Result<CallToolResult, ErrorData> {
    let schema = CheckReportSchema::from(report);
    let value = serde_json::to_value(&schema)
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

    if report.has_failures() || report.has_errors() {
        Ok(CallToolResult::structured_error(value))
    } else {
        Ok(CallToolResult::structured(value))
    }
}

/// Start the MCP server on stdio.
///
/// Blocks until the client disconnects. Handles its own async runtime
/// so callers don't need tokio.
///
/// # Errors
///
/// Returns an error if the tokio runtime fails to start, the MCP
/// handshake fails, or the server exits abnormally.
pub fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let service = ScuteMcp::new().serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    })
}
