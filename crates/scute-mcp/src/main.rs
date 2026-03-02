mod schema;

use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    handler::server::tool::schema_for_output,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use schema::CheckOutcomeSchema;
use scute_core::{CheckOutcome, commit_message};

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
    let schema = CheckOutcomeSchema::from_outcome(check_name, outcome);
    let value = serde_json::to_value(&schema)
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

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
