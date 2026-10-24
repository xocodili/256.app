//! Per-moment godmother outreach copy (LLM + locale fallback).

use sqlx::MySqlPool;

use crate::agent_specs::{
    compose_council_godmother_global_failing_prompt, compose_council_godmother_global_recovery_prompt,
    compose_council_godmother_welcome_prompt, CouncilGodmotherGlobalFailingContext,
    CouncilGodmotherGlobalRecoveryContext, CouncilGodmotherWelcomeContext,
};
use crate::config::Config;
use crate::i18n;

use super::compose::{
    civic_standing_url, compose_llm_markup, fallback_with_link, how_it_works_url,
    load_steward_compose_context,
};
use super::moment::GodmotherMoment;

struct MomentSpec {
    body_locale_key: &'static str,
    link_url: fn(&Config) -> String,
    link_label_key: &'static str,
}

fn moment_spec(moment: GodmotherMoment) -> MomentSpec {
    match moment {
        GodmotherMoment::Welcome => MomentSpec {
            body_locale_key: "council.godmother_welcome_body_fallback",
            link_url: how_it_works_url,
            link_label_key: "email.layout.footer_how",
        },
        GodmotherMoment::GlobalFailing => MomentSpec {
            body_locale_key: "council.godmother_global_failing_body_fallback",
            link_url: civic_standing_url,
            link_label_key: "ui.entity_private.civic_standing_learn_more",
        },
        GodmotherMoment::GlobalRecovery => MomentSpec {
            body_locale_key: "council.godmother_global_recovery_body_fallback",
            link_url: how_it_works_url,
            link_label_key: "email.layout.footer_how",
        },
    }
}

fn compose_prompt(
    moment: GodmotherMoment,
    agent: &crate::agent_specs::CouncilAgentFile,
    steward: &super::compose::StewardComposeContext,
    human_handle: &str,
    how_url: &str,
    civic_url: &str,
) -> String {
    match moment {
        GodmotherMoment::Welcome => compose_council_godmother_welcome_prompt(
            agent,
            &CouncilGodmotherWelcomeContext {
                display_name: &steward.display_name,
                steward_handle: &steward.steward_handle,
                steward_label: &steward.steward_label,
                steward_intention: &steward.steward_intention,
                human_handle,
                how_it_works_url: how_url,
            },
        ),
        GodmotherMoment::GlobalFailing => compose_council_godmother_global_failing_prompt(
            agent,
            &CouncilGodmotherGlobalFailingContext {
                display_name: &steward.display_name,
                steward_handle: &steward.steward_handle,
                steward_label: &steward.steward_label,
                steward_intention: &steward.steward_intention,
                human_handle,
                how_it_works_url: how_url,
                civic_standing_url: civic_url,
            },
        ),
        GodmotherMoment::GlobalRecovery => compose_council_godmother_global_recovery_prompt(
            agent,
            &CouncilGodmotherGlobalRecoveryContext {
                display_name: &steward.display_name,
                steward_handle: &steward.steward_handle,
                steward_label: &steward.steward_label,
                steward_intention: &steward.steward_intention,
                human_handle,
                how_it_works_url: how_url,
                civic_standing_url: civic_url,
            },
        ),
    }
}

pub async fn compose_moment_markup(
    pool: &MySqlPool,
    config: &Config,
    moment: GodmotherMoment,
    godmother_entity_id: u64,
    human_entity_id: u64,
    human_handle: &str,
    system_user: u64,
) -> Result<(String, bool), String> {
    let loc = i18n::request_locale();
    let spec = moment_spec(moment);
    let link_url = (spec.link_url)(config);
    let how_url = how_it_works_url(config);
    let civic_url = civic_standing_url(config);
    let steward = load_steward_compose_context(pool, godmother_entity_id).await?;
    let Some(steward) = steward else {
        return Ok((
            fallback_with_link(
                &loc,
                spec.body_locale_key,
                &[
                    ("human_handle", human_handle),
                    ("steward_handle", "your community"),
                ],
                &link_url,
                spec.link_label_key,
            ),
            false,
        ));
    };

    let fallback = fallback_with_link(
        &loc,
        spec.body_locale_key,
        &[
            ("human_handle", human_handle),
            ("steward_handle", &steward.steward_handle),
        ],
        &link_url,
        spec.link_label_key,
    );

    let agent = crate::agent_specs::load_council_agent(&steward.member_slug)?;
    let prompt = compose_prompt(
        moment,
        &agent,
        &steward,
        human_handle,
        &how_url,
        &civic_url,
    );

    compose_llm_markup(
        config,
        &steward.member_slug,
        prompt,
        system_user,
        human_entity_id,
        fallback,
    )
    .await
}
