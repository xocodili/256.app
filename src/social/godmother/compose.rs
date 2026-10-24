//! Shared compose helpers for godmother outreach moments.

use std::collections::HashMap;

use sqlx::MySqlPool;

use crate::ai::business_functions::{
    parse_godmother_outreach_response, run_council_text_bf, GodmotherOutreachDecision,
};
use crate::ai::business_functions::runner::COUNCIL_GODMOTHER_WELCOME_AGENT;
use crate::config::Config;
use crate::i18n;
use crate::social::council_members::load_council_member_by_entity_id;
use crate::social::council_portfolio::load_steward_context;

pub fn how_it_works_url(config: &Config) -> String {
    format!(
        "{}/login?legal=how",
        config.public_base_url.trim_end_matches('/')
    )
}

pub fn civic_standing_url(config: &Config) -> String {
    format!(
        "{}/login?legal=how#civic-standing",
        config.public_base_url.trim_end_matches('/')
    )
}

pub fn fallback_with_link(
    loc: &str,
    body_locale_key: &str,
    body_vars: &[(&str, &str)],
    link_url: &str,
    link_label_key: &str,
) -> String {
    let c = i18n::catalog();
    let body = c.tf(loc, body_locale_key, body_vars);
    let link_label = c.t(loc, link_label_key);
    format!(
        "<p>{}</p><p><a href=\"{}\" rel=\"noopener noreferrer nofollow\">{}</a></p>",
        crate::html_escape::escape_html(&body),
        crate::html_escape::escape_html(link_url),
        crate::html_escape::escape_html(&link_label),
    )
}

pub struct StewardComposeContext {
    pub member_slug: String,
    pub display_name: String,
    pub steward_handle: String,
    pub steward_label: String,
    pub steward_intention: String,
}

pub async fn load_steward_compose_context(
    pool: &MySqlPool,
    godmother_entity_id: u64,
) -> Result<Option<StewardComposeContext>, String> {
    let member = load_council_member_by_entity_id(pool, godmother_entity_id)
        .await
        .map_err(|e| e.to_string())?;
    let Some(member) = member else {
        return Ok(None);
    };
    let steward = load_steward_context(pool, member.community_entity_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(StewardComposeContext {
        member_slug: member.slug,
        display_name: member.display_name,
        steward_handle: member.community_handle,
        steward_label: steward.label,
        steward_intention: steward.intention,
    }))
}

pub async fn compose_llm_markup(
    config: &Config,
    slug: &str,
    prompt: String,
    system_user: u64,
    human_entity_id: u64,
    fallback_markup: String,
) -> Result<(String, bool), String> {
    let run = run_council_text_bf(
        config,
        slug,
        COUNCIL_GODMOTHER_WELCOME_AGENT,
        system_user,
        prompt,
        HashMap::new(),
    )
    .await;

    match run {
        Ok(run) => {
            if let Some(GodmotherOutreachDecision::Outreach { body_markup }) =
                parse_godmother_outreach_response(&run.response_text)
            {
                return Ok((body_markup, true));
            }
            tracing::warn!(
                slug = %slug,
                human_entity_id,
                "godmother outreach LLM response unparsable — using fallback"
            );
        }
        Err(e) => {
            tracing::warn!(
                slug = %slug,
                human_entity_id,
                error = %e,
                "godmother outreach LLM failed — using fallback"
            );
        }
    }

    Ok((fallback_markup, false))
}
