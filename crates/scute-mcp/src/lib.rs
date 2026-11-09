//! MCP server exposing Scute checks as tools.
//!
//! Runs on stdio and lets AI agents call checks like `check_commit_message`
//! or `check_code_complexity` through the Model Context Protocol.

mod schema;

use std::path::{Path, PathBuf};

use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::tool::schema_for_output,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
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
    /// Absolute path to the project root.
    project_root: String,
    /// The full commit message to validate against Conventional Commits.
    message: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckDependencyFreshnessInput {
    /// Absolute path to the project root. Relative paths resolve against it.
    project_root: String,
    /// Directory to check, for a project inside a monorepo. Defaults to the project root.
    path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckCodeComplexityInput {
    /// Absolute path to the project root. Relative paths resolve against it.
    project_root: String,
    /// Files or directories to check. Defaults to the project root.
    paths: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckSourceFilesInput {
    /// Absolute path to the project root. Relative paths resolve against it.
    project_root: String,
    /// Directory to scan for source files. Defaults to the project root.
    source_dir: Option<String>,
    /// Files to focus on. When empty, all discovered files are checked.
    files: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct ScuteMcp;

#[tool_router]
impl ScuteMcp {
    /// Validate a commit message before committing.
    ///
    /// Checks subject format (type, optional scope, description), body separation,
    /// footer syntax, and breaking change markers against the Conventional Commits spec.
    #[tool(
        name = "check_commit_message",
        output_schema = schema_for_output::<CheckReportSchema>(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    fn check_commit_message(
        Parameters(input): Parameters<CheckCommitMessageInput>,
    ) -> Result<CallToolResult, ErrorData> {
        run_check(
            Path::new(&input.project_root),
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
        output_schema = schema_for_output::<CheckReportSchema>(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    fn check_code_complexity(
        Parameters(input): Parameters<CheckCodeComplexityInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let project_root = Path::new(&input.project_root);
        let paths = rooted_paths(project_root, input.paths);
        run_check(
            project_root,
            code_complexity::CHECK_NAME,
            |def: &code_complexity::Definition| {
                code_complexity::check(project_root, paths.as_deref(), def)
            },
        )
    }

    /// Find code duplication in your project.
    ///
    /// Scans source files for duplicated token sequences. Optionally focus on
    /// specific files to only report clones involving them.
    #[tool(
        name = "check_code_similarity",
        output_schema = schema_for_output::<CheckReportSchema>(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    fn check_code_similarity(
        Parameters(input): Parameters<CheckSourceFilesInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let project_root = Path::new(&input.project_root);
        let source_dir = path_or_root(input.source_dir, project_root);
        let focus_paths = rooted_paths(project_root, input.files);
        run_check(
            project_root,
            code_similarity::CHECK_NAME,
            |def: &code_similarity::Definition| {
                code_similarity::check(project_root, &source_dir, focus_paths.as_deref(), def)
            },
        )
    }

    /// Find outdated dependencies in your project.
    ///
    /// Reports which packages are behind their latest version, how far behind
    /// (patch, minor, major), and what to update to.
    #[tool(
        name = "check_dependency_freshness",
        output_schema = schema_for_output::<CheckReportSchema>(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true,
        )
    )]
    fn check_dependency_freshness(
        Parameters(input): Parameters<CheckDependencyFreshnessInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let project_root = Path::new(&input.project_root);
        let target = path_or_root(input.path, project_root);
        run_check(
            project_root,
            dependency_freshness::CHECK_NAME,
            |def: &dependency_freshness::Definition| dependency_freshness::check(&target, def),
        )
    }
}

#[tool_handler]
impl ServerHandler for ScuteMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(INSTRUCTIONS)
    }
}

fn path_or_root(input: Option<String>, project_root: &Path) -> PathBuf {
    input.map_or(project_root.to_path_buf(), |path| project_root.join(path))
}

/// Joins the given paths onto the project root. `None` when no paths were
/// given, or an empty list.
fn rooted_paths(project_root: &Path, paths: Option<Vec<String>>) -> Option<Vec<PathBuf>> {
    paths
        .filter(|paths| !paths.is_empty())
        .map(|paths| paths.iter().map(|path| project_root.join(path)).collect())
}

fn run_check<D: Default + DeserializeOwned>(
    project_root: &Path,
    check_name: &str,
    execute: impl FnOnce(&D) -> Result<Vec<scute_core::Evaluation>, ExecutionError>,
) -> Result<CallToolResult, ErrorData> {
    let result = validate_project_root(project_root)
        .and_then(|()| load_definition(project_root, check_name))
        .and_then(|definition: D| execute(&definition));
    report_to_result(&CheckReport::new(check_name, result))
}

fn validate_project_root(project_root: &Path) -> Result<(), ExecutionError> {
    if project_root.is_absolute() && project_root.is_dir() {
        return Ok(());
    }
    Err(ExecutionError {
        code: "invalid_project_root".into(),
        message: format!(
            "project_root must be the absolute path of an existing directory, got {}",
            project_root.display()
        ),
        recovery: "pass the absolute path of the project you're working on".into(),
    })
}

fn load_definition<D: Default + DeserializeOwned>(
    project_root: &Path,
    check_name: &str,
) -> Result<D, ExecutionError> {
    ScuteConfig::load(project_root)
        .and_then(|config: ScuteConfig| config.definition(check_name))
        .map_err(|e| ExecutionError {
            code: "invalid_config".into(),
            message: format!("{e}"),
            recovery: "check your .scute.yml syntax".into(),
        })
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
        let service = ScuteMcp.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    })
}
