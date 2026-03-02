mod schema;

use std::path::PathBuf;

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
use schema::CheckOutcomeSchema;
use scute_core::{CheckOutcome, ExecutionError, commit_message, dependency_freshness};

const INSTRUCTIONS: &str = "\
Scute gives you a feedback loop to catch problems as you work, not after. \
Run checks proactively: if you're about to take an action and a relevant check \
exists, use it first. Don't wait to be asked.

When a check fails, use the `evidence` entries to understand exactly what went wrong \
and self-correct before proceeding.";

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
        output_schema = schema_for_output::<CheckOutcomeSchema>().unwrap(),
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
        let definition = match scute_config::load_commit_message_definition(&project_root) {
            Ok(def) => def,
            Err(e) => return config_error(commit_message::CHECK_NAME, &e),
        };
        let outcome = commit_message::check(&input.message, &definition);
        outcome_to_result(commit_message::CHECK_NAME, &outcome)
    }

    /// Find outdated dependencies in your project.
    ///
    /// Reports which packages are behind their latest version, how far behind
    /// (patch, minor, major), and what to update to.
    #[tool(
        name = "check_dependency_freshness",
        output_schema = schema_for_output::<CheckOutcomeSchema>().unwrap(),
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
        let path = match input.path {
            Some(p) => PathBuf::from(p),
            None => project_root.clone(),
        };
        let definition = match scute_config::load_freshness_definition(&project_root) {
            Ok(def) => def,
            Err(e) => return config_error(dependency_freshness::CHECK_NAME, &e),
        };
        let outcome = dependency_freshness::check(&path, &definition);
        outcome_to_result(dependency_freshness::CHECK_NAME, &outcome)
    }
}

#[tool_handler]
impl ServerHandler for ScuteMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(INSTRUCTIONS.into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
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

fn config_error(
    check_name: &str,
    err: &scute_config::ConfigError,
) -> Result<CallToolResult, ErrorData> {
    let outcome = CheckOutcome {
        target: String::new(),
        result: Err(ExecutionError {
            code: "invalid_config".into(),
            message: format!("{err}"),
            recovery: "check your .scute.yml syntax".into(),
        }),
    };
    outcome_to_result(check_name, &outcome)
}

fn outcome_to_result(
    check_name: &str,
    outcome: &CheckOutcome,
) -> Result<CallToolResult, ErrorData> {
    let schema = CheckOutcomeSchema::from_outcome(check_name, outcome);
    let value = serde_json::to_value(&schema)
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

    if outcome.is_fail() || outcome.is_error() {
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
