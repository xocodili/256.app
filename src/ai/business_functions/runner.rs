//! gRPC runner for council text business functions.

use std::collections::HashMap;

use crate::agent_specs::load_council_agent;
use crate::ai::grpc::{self, AiClientError};
use crate::config::Config;

pub const COUNCIL_THREAD_REPLY_AGENT: &str = "council_thread_reply_v1";
pub const COUNCIL_CONTENT_SHARE_AGENT: &str = "council_content_share_v1";
pub const COUNCIL_CONTENT_SHORTLIST_AGENT: &str = "council_content_shortlist_v1";
pub const COUNCIL_PORTFOLIO_REPLACE_AGENT: &str = "council_portfolio_replace_v1";
pub const COUNCIL_INVITE_AGENT: &str = "council_invite_v1";
pub const COUNCIL_INVITE_ARTICLE_SHORTLIST_AGENT: &str = "council_invite_article_shortlist_v1";
pub const COUNCIL_INVITE_PERSON_DISCOVER_AGENT: &str = "council_invite_person_discover_v1";
pub const COUNCIL_GODMOTHER_WELCOME_AGENT: &str = "council_godmother_welcome_v1";

#[derive(Debug, Clone)]
pub struct CouncilBfRunResult {
    pub response_text: String,
    pub model_id: String,
}

pub async fn run_council_text_bf(
    config: &Config,
    council_slug: &str,
    agent_id: &str,
    system_user_id: u64,
    text_prompt: String,
    extra_metadata: HashMap<String, String>,
) -> Result<CouncilBfRunResult, AiClientError> {
    let _agent = load_council_agent(council_slug).map_err(|e| AiClientError::Rpc(e))?;

    let mut metadata = extra_metadata;
    metadata.insert("ai_actor_slug".to_string(), council_slug.to_string());
    metadata.insert("text_prompt".to_string(), text_prompt);
    metadata.insert(
        "business_function".to_string(),
        agent_id.trim_end_matches("_v1").to_string(),
    );

    let (response_text, model_id) = grpc::run_text_agent(
        &config.ai_grpc_endpoint,
        config.ai_grpc_timeout_ms,
        agent_id,
        system_user_id,
        vec![],
        metadata,
    )
    .await?;

    Ok(CouncilBfRunResult {
        response_text,
        model_id,
    })
}
