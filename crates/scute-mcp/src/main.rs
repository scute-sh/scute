use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use scute_core::{CheckOutcome, commit_message, output::to_check_json};

const INSTRUCTIONS: &str = "\
Scute runs deterministic fitness checks and returns structured results.

Every tool returns a CheckOutcome JSON object with this shape:

- On success: { check, target, evaluation: { status, measurement: { observed, thresholds }, evidence? } }
- On error:   { check, target, error: { code, message, recovery } }

`status` is \"pass\", \"warn\", or \"fail\". `evidence` lists individual violations \
with `rule`, `found`, and optionally `expected`. When status is \"fail\", use the \
evidence to understand what went wrong and how to fix it.";

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckCommitMessageInput {
    /// The full commit message to validate against Conventional Commits.
    message: String,
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
    /// Validate a commit message against the Conventional Commits specification.
    ///
    /// Checks subject format (type, optional scope, description), body separation,
    /// footer syntax, and breaking change markers. Returns a structured `CheckOutcome`
    /// with evidence for each violation found.
    #[tool(
        name = "check_commit_message",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    async fn check_commit_message(
        &self,
        Parameters(input): Parameters<CheckCommitMessageInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let definition = commit_message::Definition::default();
        let outcome = commit_message::check(&input.message, &definition);
        outcome_to_result(commit_message::CHECK_NAME, &outcome)
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

fn outcome_to_result(
    check_name: &str,
    outcome: &CheckOutcome,
) -> Result<CallToolResult, ErrorData> {
    let json = to_check_json(check_name, outcome);
    let value =
        serde_json::to_value(&json).map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

    if outcome.is_fail() || outcome.is_error() {
        Ok(CallToolResult::structured_error(value))
    } else {
        Ok(CallToolResult::structured(value))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ScuteMcp::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
