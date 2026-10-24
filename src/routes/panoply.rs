//! Panoply operator dashboard (`/home/ops`).

use axum::extract::{Extension, Form, Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::html_escape::escape_html;
use crate::i18n;
use crate::middleware::VerifiedSession;
use crate::social::human_onboarding;
use crate::social::panoply::{
    self, block_domain, eject_human_by_handle, file_civic_dispute, file_emergency_safety,
    kill_sessions_for_handle, list_contained_emergencies, load_post_target, recent_actions,
    vacate_emergency_case, ActionError, FilingError,
};
use crate::social::user_faces;
use crate::ui::panoply::{self as panoply_ui, EmergencyClauseOption};
use crate::ui::snippets;

use super::home::{csrf_meta_html, secured_shell_topbar_html, topbar_title_with_suffix_html};

static PANOPLY_TEMPLATE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/panoply.html"));

#[derive(Debug, Deserialize)]
struct PanoplyFlashQuery {
    #[serde(default)]
    ok: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PanoplyActionForm {
    action: String,
    #[serde(default)]
    post_id: Option<String>,
    #[serde(default)]
    clause_id: Option<String>,
    #[serde(default)]
    case_id: Option<String>,
    #[serde(default)]
    handle: Option<String>,
    #[serde(default)]
    domain: Option<String>,
    note: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/home/ops", get(panoply_page).post(panoply_action_submit))
}

async fn require_panoply_operator(state: &AppState, user_id: u64) -> AppResult<()> {
    if panoply::require_operator(&state.db, user_id).await.is_err() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn load_clause_options(pool: &sqlx::MySqlPool) -> Result<Vec<EmergencyClauseOption>, sqlx::Error> {
    let rows: Vec<(u64, u16, String)> = sqlx::query_as(
        r#"
        SELECT cci.id, cci.point, cci.item
        FROM community_constitution_item cci
        INNER JOIN social_entity se ON se.id = cci.community_id
        WHERE se.handle = 'operating_charter_256'
          AND cci.status = 'active'
          AND cci.judicial_trigger_chk = 1
          AND cci.point NOT IN (4, 14)
        ORDER BY cci.point ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, point, item)| EmergencyClauseOption {
            id,
            point,
            label: item,
        })
        .collect())
}

async fn render_panoply_page(
    state: &AppState,
    user_id: u64,
    headers: &HeaderMap,
    flash: Option<String>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    require_panoply_operator(state, user_id).await?;

    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let connections = crate::social::connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.panoply.topbar_title"),
    );
    let topbar_html = secured_shell_topbar_html(
        state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;

    let emergency_clauses = load_clause_options(&state.db)
        .await
        .map_err(AppError::Db)?;
    let queue = list_contained_emergencies(&state.db, 20)
        .await
        .map_err(AppError::Db)?;
    let recent = recent_actions(&state.db, 20)
        .await
        .map_err(AppError::Db)?;

    let card = panoply_ui::render_card(
        &loc,
        &emergency_clauses,
        &queue,
        &recent,
        flash.as_deref(),
    );

    let title_esc = escape_html(&c.t(&loc, "ui.panoply.document_title"));
    let csrf_meta = csrf_meta_html(headers, &state.config);
    let html = PANOPLY_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace("__CSRF_META__", &csrf_meta)
        .replace("__PANOPLY_DOCUMENT_TITLE__", &title_esc)
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__PANOPLY_CARD__", &card);

    Ok(Html(i18n::subst_markers(
        &crate::ui::snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn panoply_page(
    State(state): State<AppState>,
    Extension(VerifiedSession {
        user_id,
        password_reset_only: _,
    }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Query(query): Query<PanoplyFlashQuery>,
) -> AppResult<Response> {
    let flash = query.ok.filter(|s| !s.is_empty());
    render_panoply_page(&state, user_id, &headers, flash).await
}

fn parse_u64_field(raw: Option<&String>, field: &str) -> AppResult<u64> {
    let Some(s) = raw.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Err(AppError::BadRequest(format!("{field} required")));
    };
    s.parse::<u64>()
        .map_err(|_| AppError::BadRequest(format!("invalid {field}")))
}

async fn panoply_action_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession {
        user_id,
        password_reset_only: _,
    }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Form(form): Form<PanoplyActionForm>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    require_panoply_operator(&state, user_id).await?;

    let note = form.note.trim();
    if note.is_empty() {
        return render_panoply_page(
            &state,
            user_id,
            &headers,
            Some(c.t(&loc, "ui.panoply.err_note_required").to_string()),
        )
        .await;
    }

    let operator_entity_id = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.panoply.err_no_human").to_string()))?;

    let action = form.action.trim();
    let result_msg = match action {
        "emergency_file" => {
            let post_id = parse_u64_field(form.post_id.as_ref(), "post_id")?;
            let clause_id = parse_u64_field(form.clause_id.as_ref(), "clause_id")?;
            match file_emergency_safety(
                &state.db,
                user_id,
                operator_entity_id,
                post_id,
                clause_id,
                note,
            )
            .await
            {
                Ok(case_id) => {
                    if let Ok(Some(target)) = load_post_target(&state.db, post_id).await {
                        if let Some(author_user_id) = target.author_user_id {
                            crate::email::notify_emergency_filed_to_author(
                                &state.db,
                                &state.config,
                                author_user_id,
                                target.root_post_id,
                            )
                            .await;
                        }
                        crate::email::notify_emergency_filed_to_filer(
                            &state.db,
                            &state.config,
                            user_id,
                            target.root_post_id,
                            case_id,
                        )
                        .await;
                    }
                    c.tf(
                        &loc,
                        "ui.panoply.ok_emergency",
                        &[("case_id", &case_id.to_string())],
                    )
                }
                Err(FilingError::PostNotFound) => c.t(&loc, "ui.panoply.err_post_not_found").to_string(),
                Err(FilingError::DuplicateEmergency) => {
                    c.t(&loc, "ui.panoply.err_duplicate_emergency").to_string()
                }
                Err(FilingError::MixedClauseKinds) | Err(FilingError::InvalidClause) => {
                    c.t(&loc, "ui.panoply.err_invalid_clause").to_string()
                }
                Err(FilingError::SanitationClause) => {
                    c.t(&loc, "ui.panoply.err_sanitation_clause").to_string()
                }
                Err(FilingError::Db) => c.t(&loc, "ui.shared.request_failed").to_string(),
            }
        }
        "civic_file" => {
            let post_id = parse_u64_field(form.post_id.as_ref(), "post_id")?;
            let clause_id = parse_u64_field(form.clause_id.as_ref(), "clause_id")?;
            match file_civic_dispute(
                &state.db,
                user_id,
                operator_entity_id,
                post_id,
                &[clause_id],
                note,
            )
            .await
            {
                Ok(case_id) => {
                    if let Err(err) = crate::jobs::enqueue_job(
                        &state.db,
                        None,
                        "judicial_jury_nomination",
                        &serde_json::json!({ "limit": 32 }),
                        Some(&format!("judicial-case:{case_id}")),
                        30,
                    )
                    .await
                    {
                        tracing::warn!(
                            error = %err,
                            case_id,
                            "failed to enqueue judicial_jury_nomination after panoply civic filing"
                        );
                    }
                    c.tf(
                        &loc,
                        "ui.panoply.ok_civic",
                        &[("case_id", &case_id.to_string())],
                    )
                }
                Err(FilingError::PostNotFound) => c.t(&loc, "ui.panoply.err_post_not_found").to_string(),
                Err(FilingError::MixedClauseKinds) | Err(FilingError::InvalidClause) => {
                    c.t(&loc, "ui.panoply.err_invalid_clause").to_string()
                }
                Err(FilingError::SanitationClause) => {
                    c.t(&loc, "ui.panoply.err_sanitation_clause").to_string()
                }
                Err(FilingError::DuplicateEmergency) | Err(FilingError::Db) => {
                    c.t(&loc, "ui.shared.request_failed").to_string()
                }
            }
        }
        "vacate_emergency" => {
            let case_id = parse_u64_field(form.case_id.as_ref(), "case_id")?;
            match vacate_emergency_case(&state.db, user_id, case_id, note).await {
                Ok(()) => c.t(&loc, "ui.panoply.ok_vacate").to_string(),
                Err(ActionError::CaseNotFound) => {
                    c.t(&loc, "ui.panoply.err_case_not_found").to_string()
                }
                Err(ActionError::CaseNotVacatable) => {
                    c.t(&loc, "ui.panoply.err_case_not_vacatable").to_string()
                }
                Err(_) => c.t(&loc, "ui.shared.request_failed").to_string(),
            }
        }
        "eject" => {
            let handle = form
                .handle
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("handle required".to_string()))?;
            match eject_human_by_handle(&state.db, user_id, handle, note).await {
                Ok(()) => c.t(&loc, "ui.panoply.ok_eject").to_string(),
                Err(ActionError::HandleNotFound) => {
                    c.t(&loc, "ui.panoply.err_handle_not_found").to_string()
                }
                Err(ActionError::DeathImmune) => c.t(&loc, "ui.panoply.err_death_immune").to_string(),
                Err(ActionError::AlreadyDead) => c.t(&loc, "ui.panoply.err_already_dead").to_string(),
                Err(_) => c.t(&loc, "ui.shared.request_failed").to_string(),
            }
        }
        "session_kill" => {
            let handle = form
                .handle
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("handle required".to_string()))?;
            match kill_sessions_for_handle(&state.db, user_id, handle, note).await {
                Ok(()) => c.t(&loc, "ui.panoply.ok_session_kill").to_string(),
                Err(ActionError::HandleNotFound) => {
                    c.t(&loc, "ui.panoply.err_handle_not_found").to_string()
                }
                Err(_) => c.t(&loc, "ui.shared.request_failed").to_string(),
            }
        }
        "domain_block" => {
            let domain = form
                .domain
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("domain required".to_string()))?;
            match block_domain(&state.db, user_id, domain, note).await {
                Ok(()) => c.t(&loc, "ui.panoply.ok_domain_block").to_string(),
                Err(ActionError::DomainInvalid) => {
                    c.t(&loc, "ui.panoply.err_domain_invalid").to_string()
                }
                Err(_) => c.t(&loc, "ui.shared.request_failed").to_string(),
            }
        }
        _ => c.t(&loc, "ui.panoply.err_unknown_action").to_string(),
    };

    Ok(render_panoply_page(&state, user_id, &headers, Some(result_msg)).await?)
}
