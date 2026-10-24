use axum::extract::{Extension, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Json;
use axum::Router;
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use crate::ai::grpc as ai_grpc;
use crate::app_state::AppState;
use crate::auth::password_reset::{self, ResetConsumeOutcome};
use crate::auth::post_auth;
use crate::auth::session;
use crate::auth::verification::{self, VerifyOutcome};
use crate::error::AppResult;
use crate::html_escape::escape_html;
use crate::i18n;
use crate::middleware::VerifiedSession;
use crate::rate_limit;
use crate::ref_keys;
use crate::social::community_constitution;
use crate::social::human_onboarding;
use crate::social::integrity as social_integrity;
use crate::ui::change_password_panel::{self, ChangePasswordPanelOpts};
use crate::ui::fragments;
use crate::ui::legal_card;
use crate::ui::snippets;
use super::profile_gate;

pub fn public_router(dev_auth_bypass: bool) -> Router<AppState> {
    let ui_cards = rate_limit::ui_card_surface_config(dev_auth_bypass);
    let login_and_cards = Router::new()
        .route("/login", get(login_page))
        .route("/login/", get(login_page))
        .route("/login/register-card", get(register_card_fragment))
        .route("/login/legal-card", get(legal_card_fragment))
        .route("/login/resend-card", get(resend_card_fragment))
        .route("/login/forgot-card", get(forgot_password_card_fragment))
        .route("/login/reactivate-card", get(reactivate_account_card_fragment))
        .layer(rate_limit::ui_card_limit_layer(ui_cards));

    Router::new()
        .merge(login_and_cards)
        .route("/", get(root_redirect_login))
        .route("/discovery", get(discovery_json))
        .route("/health/status", get(health_status_json))
        .route("/health/jobs", get(health_jobs_json))
        .route("/health/ai-gateway", get(ai_gateway_health))
        .route("/health/object-model", get(object_model_health))
        .route("/health", get(health_catalog_html))
        .route("/register", get(register_redirect))
        .route("/register/", get(register_redirect))
        .route("/terms", get(terms_page))
        .route("/conditions", get(conditions_page))
        .route("/privacy", get(privacy_page))
        .route("/safety", get(safety_page))
        .route("/crisis-help", get(crisis_help_page))
        .route("/crisis-help/", get(crisis_help_page))
        .route("/community-rules", get(community_rules_page))
        .route("/community-rules/", get(community_rules_page))
        .route(
            "/verify-email",
            get(verify_email).layer(rate_limit::verify_email_layer()),
        )
        .route(
            "/reset-password",
            get(reset_password).layer(rate_limit::verify_email_layer()),
        )
}

pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/account", get(account))
        .route("/account/change-password", get(change_password_page))
        .route("/account/change-password/", get(change_password_page))
}

async fn root_redirect_login() -> Redirect {
    Redirect::temporary("/login")
}

async fn discovery_json() -> Json<serde_json::Value> {
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    Json(serde_json::json!({
        "service": c.t(&loc, "discovery.service"),
        "root": c.t(&loc, "discovery.root"),
        "catalog_url": "/discovery",
        "health": c.t(&loc, "discovery.health"),
        "static": c.t(&loc, "discovery.static_path"),
        "login_page": c.t(&loc, "discovery.login_page"),
        "register_page": c.t(&loc, "discovery.register_page"),
        "verify_email": c.t(&loc, "discovery.verify_email"),
        "account_example": c.t(&loc, "discovery.account_example"),
        "home_page": c.t(&loc, "discovery.home_page"),
        "face_capture": c.t(&loc, "discovery.face_capture"),
        "locations": c.t(&loc, "discovery.locations"),
        "ops_invalidate": c.t(&loc, "discovery.ops_invalidate"),
        "ops_jobs_run": c.t(&loc, "discovery.ops_jobs_run"),
        "jobs": c.t(&loc, "discovery.jobs"),
        "ref_key_values": c.t(&loc, "discovery.ref_key_values"),
        "social_entities": c.t(&loc, "discovery.social_entities"),
        "social_entity_relationships": c.t(&loc, "discovery.social_entity_relationships"),
        "entity_public_profile": c.t(&loc, "discovery.entity_public_profile"),
        "auth": {
            "register": c.t(&loc, "discovery.auth.register"),
            "login": c.t(&loc, "discovery.auth.login"),
            "logout": c.t(&loc, "discovery.auth.logout"),
            "resend_verification": c.t(&loc, "discovery.auth.resend_verification"),
            "me_human_profile": c.t(&loc, "discovery.auth.me_human_profile")
        }
    }))
}

async fn health_status_json(State(state): State<AppState>) -> Json<serde_json::Value> {
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    let oidc_redirect_uri = state
        .config
        .oidc_google
        .as_ref()
        .map(|o| o.redirect_uri.as_str());
    let queue = crate::jobs::job_queue_snapshot(&state.db).await.ok();
    let council_pipeline_ready =
        crate::jobs::council_pipeline_job_types_ready(&state.db)
            .await
            .unwrap_or(false);
    Json(serde_json::json!({
        "status": c.t(&loc, "health.status_ok"),
        "public_base_url": state.config.public_base_url,
        "secure_cookies": state.config.secure_cookies,
        "oidc_redirect_uri": oidc_redirect_uri,
        "smtp_configured": state.config.smtp.is_some(),
        "ops_digest_live": state.config.ops_digest_live,
        "stripe_configured": state
            .config
            .stripe
            .as_ref()
            .is_some_and(|s| s.enabled()),
        "scheduler_tick_secs": state.config.scheduler_tick_secs,
        "scheduler_enabled": state.config.scheduler_tick_secs > 0,
        "scheduler_batch_size": state.config.scheduler_batch_size,
        "job_conductor_mode": "parallel_capped",
        "job_conductor_parallelism": state.config.job_conductor_parallelism,
        "job_conductor_llm_parallelism": state.config.job_conductor_llm_parallelism,
        "feed_poll_wall_budget_secs": state.config.feed_poll_wall_budget_secs,
        "scheduler_tick_count": crate::jobs::scheduler_tick_count(),
        "scheduler_last_claimed": crate::jobs::scheduler_last_claimed(),
        "job_queue_pending": queue.as_ref().map(|q| q.pending),
        "job_queue_pending_due": queue.as_ref().map(|q| q.pending_due),
        "job_queue_leased": queue.as_ref().map(|q| q.leased),
        "journal_retention_days": state.config.journal_retention_days,
        "council_pipeline_job_types_ready": council_pipeline_ready,
    }))
}

async fn health_jobs_json(State(state): State<AppState>) -> Json<serde_json::Value> {
    let council_pipeline_ready = crate::jobs::council_pipeline_job_types_ready(&state.db)
        .await
        .unwrap_or(false);
    match crate::jobs::job_queue_snapshot(&state.db).await {
        Ok(snapshot) => Json(serde_json::json!({
            "ok": true,
            "scheduler_tick_count": crate::jobs::scheduler_tick_count(),
            "scheduler_last_claimed": crate::jobs::scheduler_last_claimed(),
            "council_pipeline_job_types_ready": council_pipeline_ready,
            "queue": snapshot,
        })),
        Err(e) => {
            tracing::warn!(error = %e, "job queue health snapshot failed");
            Json(serde_json::json!({
                "ok": false,
                "error": "job_queue_snapshot_failed",
                "council_pipeline_job_types_ready": council_pipeline_ready,
            }))
        }
    }
}

type ApiRow = (&'static str, &'static str, &'static str);

fn health_api_rows_html(rows: &[ApiRow]) -> String {
    let mut out = String::new();
    for (method, path, note) in rows {
        out.push_str(&format!(
            "<tr><td class=\"hc-method\">{}</td><td><code>{}</code></td><td class=\"hc-note\">{}</td></tr>",
            escape_html(method),
            escape_html(path),
            escape_html(note)
        ));
    }
    out
}

/// Hand-maintained catalog: keep in sync with [`crate::routes`] modules.
const HEALTH_PUBLIC_ROUTES: &[ApiRow] = &[
    ("GET", "/", "Redirect to /login."),
    ("GET", "/discovery", "JSON service catalog (human-oriented hints)."),
    ("GET", "/health", "This HTML page."),
    ("GET", "/health/status", "JSON liveness: status, scheduler, SMTP, ops_digest_live, job queue depth."),
    ("GET", "/health/jobs", "JSON job queue snapshot: pending by type, sample last handler error."),
    ("GET", "/login", "Login / join marketing shell (optional ?login=1, ?join=1, ?resend=1, ?legal=…)."),
    ("GET", "/login/", "Same as /login."),
    ("GET", "/login/register-card", "HTML fragment: registration card."),
    ("GET", "/login/legal-card", "HTML fragment: legal doc card (?doc=terms|conditions|privacy|safety; ?legal=crisis aliases safety)."),
    ("GET", "/login/resend-card", "HTML fragment: resend verification."),
    ("GET", "/register", "Redirect to /login?join=1."),
    ("GET", "/register/", "Same as /register."),
    ("GET", "/terms", "Redirect to /login?legal=terms."),
    ("GET", "/conditions", "Redirect to /login?legal=terms (legacy alias)."),
    ("GET", "/privacy", "Redirect to /login?legal=privacy."),
    ("GET", "/safety", "Redirect to /login?legal=safety."),
    ("GET", "/crisis-help", "Crisis hotlines page (optional bookmark; profile country when signed in)."),
    ("GET", "/verify-email?token=", "Email verification: verify + session cookie → /home."),
    ("POST", "/auth/register", "Create account (rate limited)."),
    ("POST", "/auth/login", "Set session cookie (rate limited)."),
    ("POST", "/auth/logout", "Clear session; redirect /login."),
    ("POST", "/auth/resend-verification", "Anti-enumeration resend (rate limited)."),
    ("GET", "/ref-key-values?group=", "Active reference rows (optional group; rate limited)."),
    ("POST", "/ops/locations/invalidate", "Bearer OPS_LOCATION_INVALIDATE_TOKEN; location lifecycle (404 if token unset; rate limited)."),
    ("POST", "/ops/jobs/run", "Bearer OPS_JOB_QUEUE_TOKEN; job queue conductor tick (optional JSON batch_size; 404 if token unset; rate limited)."),
    ("GET", "/public/{handle}", "Public entity profile HTML shell (verified entities)."),
    ("GET", "/api/public/entity-profile/{handle}", "Public JSON card for profile shell (rate limited)."),
    ("GET", "/api/public/content-links/preview-og", "Public OG preview for a URL (query: url=, rate limited)."),
    ("GET", "/static/…", "Static assets (CSS, JS, images) via ServeDir."),
];

const HEALTH_CHECK_ROUTES: &[ApiRow] = &[
    ("GET", "/health/ai-gateway", "gRPC gateway reachability (or mock bypass)."),
    ("GET", "/health/object-model", "DB council / object-model integrity check."),
];

const HEALTH_VERIFIED_ROUTES: &[ApiRow] = &[
    ("GET", "/account", "JSON: session smoke (verified email)."),
    ("GET", "/home", "Signed-in home shell."),
    ("GET", "/home/profile/{handle}", "Private profile page (HTML)."),
    ("GET", "/home/private-profile-card/{handle}", "HTML fragment: private profile card."),
    (
        "POST",
        "/home/private-profile-connect/{handle}",
        "Send human→human connection invite (form connect_as or JSON).",
    ),
    (
        "POST",
        "/home/private-profile-connect-action/{handle}",
        "Accept / decline / block inbound connection invite.",
    ),
    (
        "POST",
        "/home/private-profile-cancel-invite/{handle}",
        "Cancel outbound pending invite (ends social_entity_relationship row).",
    ),
    (
        "POST",
        "/home/private-profile-disconnect/{handle}",
        "End verified or flagged friend/family/colleague connection.",
    ),
    (
        "POST",
        "/home/private-profile-leave-community/{handle}",
        "End viewer human member_of edge to a community (leave).",
    ),
    (
        "POST",
        "/home/private-profile-follow/{handle}",
        "Follow profit or non-profit (verified follow edge from viewer human).",
    ),
    (
        "POST",
        "/home/private-profile-unfollow/{handle}",
        "Remove follow edge to profit or non-profit.",
    ),
    (
        "POST",
        "/home/private-profile-gathering-rsvp/{handle}",
        "Set gathering RSVP (going / not_going / interested) from viewer human.",
    ),
    ("GET", "/onboarding", "Onboarding shell (face capture flow)."),
    ("GET", "/home/face-capture", "302 → /onboarding."),
    ("GET", "/home/profile-completion-card", "HTML fragment: profile completion."),
    ("GET", "/home/create-post-card", "HTML fragment: create-post composer."),
    ("POST", "/locations", "Create or get location (JSON body; rate limited)."),
    ("POST", "/me/reverse-geocode", "JSON lat/lon → country (rate limited)."),
    (
        "POST",
        "/me/google-place-location",
        "JSON { place_id, update_human_entity? } → Google Place Details → locations row; default updates human location_id; false records place only.",
    ),
    ("POST", "/me/human-profile", "Create human entity + profile (rate limited)."),
    ("POST", "/me/face-capture", "Multipart: three expression JPEGs (no biometrics stored); AI gateway / Vertex (rate limited)."),
    ("GET", "/me/godmother-options", "Council godmother candidates JSON."),
    ("POST", "/me/godmother", "Select godmother relationship (rate limited)."),
    (
        "POST",
        "/social-entities",
        "Create entity (rate limited). For type profit, JSON body includes required `profit`: { label, legal_name, founded_at, optional description, intention }; `location_id` from POST /me/google-place-location with update_human_entity false.",
    ),
    ("GET", "/social-entities/{id}", "Fetch entity by id."),
    ("POST", "/social-entity-relationships", "Create relationship (rate limited)."),
    ("GET", "/social-entity-relationships/{id}", "Fetch relationship by id."),
    ("POST", "/content-links/ingest", "Ingest URL → domain/link ids."),
    ("POST", "/content-links/preview-og", "Fetch OG preview for URL."),
    ("GET", "/me/posting-entities", "Entities current user may post as."),
    ("POST", "/jobs/enqueue", "Enqueue job (JSON job_type_key + payload; dedupe by canonical SHA-256 per type; rate limited)."),
    ("POST", "/jobs/{id}/cancel", "Cancel own pending job (not leased)."),
];

const HEALTH_DEV_ROUTES: &[ApiRow] = &[
    ("GET", "/dev/auth/login-as", "DEV_AUTH_BYPASS only: set session from ?email=&next=."),
    ("POST", "/dev/auth/login-as", "DEV_AUTH_BYPASS only: set session from JSON body."),
];

async fn health_catalog_html() -> Html<String> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let doc_title = c.t(&loc, "health.catalog_title");
    let h1 = c.t(&loc, "health.catalog_h1");
    let intro = c.t(&loc, "health.catalog_intro");
    let liveness_label = c.t(&loc, "health.liveness_label");
    let liveness_hint = c.t(&loc, "health.liveness_hint");
    let th_method = c.t(&loc, "health.th_method");
    let th_path = c.t(&loc, "health.th_path");
    let th_notes = c.t(&loc, "health.th_notes");

    let thead = format!(
        "<thead><tr><th scope=\"col\">{}</th><th scope=\"col\">{}</th><th scope=\"col\">{}</th></tr></thead>",
        escape_html(&th_method),
        escape_html(&th_path),
        escape_html(&th_notes)
    );

    let section = |title_key: &str, rows: &[ApiRow]| -> String {
        let title = c.t(&loc, title_key);
        format!(
            "<h2 class=\"hc-section\">{}</h2><table class=\"hc-table\">{}<tbody>{}</tbody></table>",
            escape_html(&title),
            thead,
            health_api_rows_html(rows)
        )
    };

    let body = format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{doc_title_esc}</title>
<link rel="stylesheet" href="/static/css/brand.css">
</head>
<body class="health-catalog-page">
<main class="health-catalog-inner">
<p class="health-catalog-kicker mono">256.app</p>
<h1>{h1_esc}</h1>
<p class="health-catalog-intro">{intro_esc}</p>
<p class="health-catalog-liveness"><strong>{liveness_label_esc}</strong> — {liveness_hint_esc} <a href="/health/status"><code>/health/status</code></a></p>
{checks}
{public}
{verified}
{dev}
<p class="health-catalog-footer">{footer_esc} <a href="/discovery"><code>/discovery</code></a></p>
</main>
</body>
</html>"#,
        lang = escape_html(&loc),
        doc_title_esc = escape_html(&doc_title),
        h1_esc = escape_html(&h1),
        intro_esc = escape_html(&intro),
        liveness_label_esc = escape_html(&liveness_label),
        liveness_hint_esc = escape_html(&liveness_hint),
        checks = section("health.section_health_checks", HEALTH_CHECK_ROUTES),
        public = section("health.section_public", HEALTH_PUBLIC_ROUTES),
        verified = section("health.section_verified", HEALTH_VERIFIED_ROUTES),
        dev = section("health.section_dev", HEALTH_DEV_ROUTES),
        footer_esc = escape_html(&c.t(&loc, "health.footer_discovery")),
    );

    Html(body)
}

async fn ai_gateway_health(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    if state.config.face_vision_mock {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "service": "ai_gateway",
                "mode": "mock",
                "note": "gateway check bypassed because FACE_VISION_MOCK=1"
            })),
        );
    }

    match ai_grpc::check_gateway(
        &state.config.ai_grpc_endpoint,
        state.config.ai_grpc_timeout_ms,
    )
    .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "service": "ai_gateway"
            })),
        ),
        Err(e) => {
            tracing::warn!(error = %e, "ai gateway health check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "ok": false,
                    "service": "ai_gateway",
                    "error": "unreachable"
                })),
            )
        }
    }
}

async fn object_model_health(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match social_integrity::council_integrity(&state.db).await {
        Ok(check) => {
            let status = if check.ok {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            (
                status,
                Json(serde_json::json!({
                    "ok": check.ok,
                    "ai_actor_count": check.ai_actor_count,
                    "council_membership_count": check.council_membership_count,
                    "domain_ownership_count": check.domain_ownership_count,
                    "profile_assignment_count": check.profile_assignment_count,
                    "judicial_scope_count": check.judicial_scope_count
                })),
            )
        }
        Err(e) => {
            tracing::warn!(error = %e, "object model health check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "ok": false,
                    "error": "object_model_check_failed"
                })),
            )
        }
    }
}

const LOGIN_TEMPLATE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/login.html"));
const CRISIS_HELP_TEMPLATE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/crisis_help.html"));
const REGISTER_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/register_card_fragment.html"
));
const RESEND_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/resend_card_fragment.html"
));
const FORGOT_PASSWORD_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/forgot_password_card_fragment.html"
));
const REACTIVATE_ACCOUNT_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/reactivate_account_card_fragment.html"
));
const CHANGE_PASSWORD_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/change_password.html"
));
const LEGAL_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/legal_card_fragment.html"
));

fn login_page_i18n_json(loc: &str, signed_in: bool, state: &AppState) -> String {
    let c = i18n::catalog();
    let tagline = c.t(loc, "ui.site.tagline");
    let nav_title_marketing = c.tf(loc, "ui.site.nav_title_marketing", &[("tagline", &tagline)]);
    let recaptcha_site_key = state
        .config
        .recaptcha
        .as_ref()
        .map(|r| r.site_key.as_str())
        .unwrap_or("");
    serde_json::json!({
        "signed_in": signed_in,
        "recaptcha_site_key": recaptcha_site_key,
        "oidc_google_enabled": state.config.oidc_google.is_some(),
        "continue_with_google": c.t(loc, "auth.continue_with_google"),
    "sign_in_with_google": c.t(loc, "auth.sign_in_with_google"),
    "sign_up_with_google": c.t(loc, "auth.sign_up_with_google"),
        "sign_in_failed_generic": c.t(loc, "auth.sign_in_failed_generic"),
        "recaptcha_load_failed": c.t(loc, "auth.recaptcha_load_failed"),
        "use_google_sign_in": c.t(loc, "auth.use_google_sign_in"),
        "or_continue_with": c.t(loc, "auth.or_continue_with"),
        "flash_registered": c.t(loc, "ui.login.flash_registered"),
        "flash_verified": c.t(loc, "ui.login.flash_verified"),
        "flash_verify_error": c.t(loc, "ui.login.flash_verify_error"),
        "resend_maybe_sent": c.t(loc, "ui.login.resend_maybe_sent"),
        "request_failed": c.t(loc, "ui.shared.request_failed"),
        "network_error_short": c.t(loc, "ui.shared.network_error_short"),
        "sign_in_failed": c.t(loc, "ui.login.sign_in_failed"),
        "email_not_verified_fallback": c.t(loc, "ui.login.email_not_verified_fallback"),
        "network_error_detail": c.t(loc, "ui.shared.network_error_detail"),
        "register_load_failed": c.t(loc, "ui.login.register_load_failed"),
        "resend_load_failed": c.t(loc, "ui.login.resend_load_failed"),
        "card_rate_limited": c.t(loc, "ui.login.card_rate_limited"),
        "create_account_failed": c.t(loc, "ui.register.create_account_failed"),
        "legal_load_failed": c.t(loc, "ui.login.legal_load_failed"),
        "nav_title_auth": "256 - Log In or Create an Account",
        "nav_title_marketing": nav_title_marketing,
        "legal_nav_titles": {
            "terms": format!("256 - {}", c.t(loc, "ui.legal.terms_title")),
            "privacy": format!("256 - {}", c.t(loc, "ui.legal.privacy_title")),
            "safety": format!("256 - {}", c.t(loc, "ui.legal.safety_title")),
            "how": format!("256 - {}", c.t(loc, "ui.legal.how_title")),
            "constitution": format!("256 - {}", c.t(loc, "ui.legal.constitution_title")),
        },
        "nav_title_welcome": c.t(loc, "ui.login.nav_title_welcome"),
        "forgot_maybe_sent": c.t(loc, "ui.login.forgot_maybe_sent"),
        "forgot_load_failed": c.t(loc, "ui.login.forgot_load_failed"),
        "reactivate_maybe_sent": c.t(loc, "ui.login.reactivate_maybe_sent"),
        "reactivate_load_failed": c.t(loc, "ui.login.reactivate_load_failed"),
        "flash_reset_error": c.t(loc, "ui.login.flash_reset_error"),
        "flash_oidc_error": c.t(loc, "ui.login.flash_oidc_error"),
        "login_unsuccessful_caps": c.t(loc, "ui.login.login_unsuccessful_caps"),
    })
    .to_string()
}

async fn build_live_root_constitution_body(pool: &sqlx::MySqlPool, loc: &str) -> Option<String> {
    let entity_id: u64 = match sqlx::query_scalar::<_, u64>(
        "SELECT id FROM social_entity WHERE handle = ? LIMIT 1",
    )
    .bind(ref_keys::ROOT_OPERATING_CHARTER_HANDLE)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(id)) => id,
        _ => return None,
    };

    let clauses = community_constitution::fetch_owned_constitution_clauses(pool, entity_id)
        .await
        .ok()?;
    if clauses.is_empty() {
        return None;
    }

    let c = i18n::catalog();
    let intro = c.t(loc, "ui.login.constitution_intro");
    let aria = c.t(loc, "ui.login.constitution_list_aria");

    let mut body = String::new();
    body.push_str(&format!(
        r#"<p class="sub marketing-intro-line">{}</p>"#,
        escape_html(&intro)
    ));
    body.push_str(&format!(
        r#"<ol class="marketing-feature-list guest-constitution-live-list" aria-label="{}">"#,
        escape_html(&aria)
    ));
    for clause in &clauses {
        body.push_str(&format!(
            r#"<li><span class="profile-constitution-point mono">#{}</span> <span class="marketing-feature-text">{}</span></li>"#,
            clause.point,
            escape_html(clause.item.trim()),
        ));
    }
    body.push_str("</ol>");
    Some(body)
}

#[derive(Debug, Default, Deserialize)]
struct LoginPageQuery {
    legal: Option<String>,
    join: Option<String>,
    login: Option<String>,
    resend: Option<String>,
}

fn login_page_allows_signed_in_guest_shell(q: &LoginPageQuery) -> bool {
    q.legal.is_some()
        || q.join.as_deref() == Some("1")
        || q.login.as_deref() == Some("1")
        || q.resend.as_deref() == Some("1")
}

async fn login_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<LoginPageQuery>,
) -> Response {
    let signed_in = session::optional_user_id_from_jar(&state.db, &jar)
        .await
        .is_some();
    if signed_in && !login_page_allows_signed_in_guest_shell(&q) {
        return Redirect::temporary("/home").into_response();
    }
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let chrome = snippets::guest_topbar_chrome(&loc, signed_in);
    let recaptcha_site_key = state
        .config
        .recaptcha
        .as_ref()
        .map(|r| r.site_key.as_str())
        .unwrap_or("");
    let recaptcha_script = if recaptcha_site_key.is_empty() {
        String::new()
    } else {
        format!(
            r#"<script src="https://www.google.com/recaptcha/api.js?render={}"></script>"#,
            escape_html(recaptcha_site_key)
        )
    };
    let topbar_html = chrome.public_topbar(&format!(
        r#"<span class="app-topbar-title mono">{}</span>"#,
        escape_html(&c.t(&loc, "ui.login.nav_title_welcome"))
    ));
    let html = LOGIN_TEMPLATE
        .replace("__PUBLIC_TOPBAR__", &topbar_html)
        .replace(
            "__GUEST_MARKETING_SPRITE__",
            &snippets::guest_marketing_sprite_html(),
        )
        .replace(
            "__GUEST_WELCOME_MARKETING__",
            &snippets::guest_welcome_marketing_html(&loc),
        )
        .replace("__LOGIN_PAGE_I18N_JSON__", &login_page_i18n_json(&loc, signed_in, &state))
        .replace("__RECAPTCHA_SCRIPT__", &recaptcha_script)
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__CARD_LOADER_REGISTER__",
            &snippets::card_loader("login-register"),
        )
        .replace(
            "__CARD_LOADER_LEGAL__",
            &snippets::card_loader("login-legal"),
        )
        .replace(
            "__CARD_LOADER_RESEND__",
            &snippets::card_loader("login-resend"),
        )
        .replace(
            "__CARD_LOADER_FORGOT__",
            &snippets::card_loader("login-forgot"),
        )
        .replace(
            "__CARD_LOADER_REACTIVATE__",
            &snippets::card_loader("login-reactivate"),
        );
    let base = state.config.public_base_url.trim_end_matches('/');
    let canonical = format!("{base}/login");
    let seo_title = c.t(&loc, "ui.site.seo_og_title");
    let seo_desc = c.t(&loc, "ui.site.seo_description");
    let seo_html = snippets::site_seo_meta_html(&base, &canonical, &seo_title, &seo_desc, "website");
    let html = html.replace(snippets::SITE_SEO_PLACEHOLDER, &seo_html);
    let html = snippets::inject_public_footer(&html);
    Html(i18n::subst_markers(&html, &loc)).into_response()
}

async fn register_redirect() -> Redirect {
    Redirect::temporary("/login?join=1")
}

#[derive(Debug, Deserialize)]
struct LegalQuery {
    doc: Option<String>,
}

fn normalize_legal_doc(doc: &str) -> &str {
    match doc {
        "terms" | "conditions" => "terms",
        "privacy" => "privacy",
        "safety" | "crisis" => "safety",
        "how" | "how-it-works" => "how",
        "constitution" | "community-rules" => "constitution",
        _ => "privacy",
    }
}

fn legal_doc_content(loc: &str, doc: &str, safety_contact_email: Option<&str>) -> (String, String) {
    let c = i18n::catalog();
    match normalize_legal_doc(doc) {
        "terms" => (
            c.t(loc, "ui.legal.terms_title"),
            c.t(loc, "ui.legal.terms_body"),
        ),
        "safety" => {
            let email = safety_contact_email.unwrap_or("safety@example.com");
            (
                c.t(loc, "ui.legal.safety_title"),
                c.tf(
                    loc,
                    "ui.legal.safety_body",
                    &[("safety_contact_email", email)],
                ),
            )
        }
        "how" => (
            c.t(loc, "ui.legal.how_title"),
            c.t(loc, "ui.legal.how_body"),
        ),
        "constitution" => (
            c.t(loc, "ui.legal.constitution_title"),
            String::new(),
        ),
        _ => (
            c.t(loc, "ui.legal.privacy_title"),
            c.t(loc, "ui.legal.privacy_body"),
        ),
    }
}

async fn constitution_body_html(pool: &sqlx::MySqlPool, loc: &str) -> String {
    if let Some(body) = build_live_root_constitution_body(pool, loc).await {
        return body;
    }
    fragments::inject_root_constitution(
        r#"<div class="legal-constitution-body">__ROOT_CONSTITUTION_CONTENT__</div>"#,
    )
}

async fn legal_card_fragment(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<LegalQuery>,
) -> Html<String> {
    let loc = i18n::request_locale();
    let doc_raw = q
        .doc
        .as_deref()
        .unwrap_or("terms")
        .trim()
        .to_ascii_lowercase();
    let doc_key = normalize_legal_doc(&doc_raw);
    let (title, body) = legal_doc_content(
        &loc,
        doc_key,
        Some(state.config.platform_safety_contact_email.as_str()),
    );
    let profile_country = match session::optional_user_id_from_jar(&state.db, &jar).await {
        Some(user_id) => human_onboarding::country_code_for_user(&state.db, user_id)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let crisis_panel = if doc_key == "safety" {
        crate::crisis_resources::panel_for_everyone(
            &loc,
            profile_country.as_deref(),
            true,
        )
    } else {
        String::new()
    };
    let body_html = if doc_key == "constitution" {
        constitution_body_html(&state.db, &loc).await
    } else {
        legal_card::legal_body_html(&body)
    };
    let html = LEGAL_CARD_FRAGMENT
        .replace("__LEGAL_TITLE__", &escape_html(&title))
        .replace("__LEGAL_BODY__", &body_html)
        .replace("__CRISIS_RESOURCES_PANEL__", &crisis_panel);
    Html(i18n::subst_markers(&html, &loc))
}

async fn terms_page() -> Redirect {
    Redirect::temporary("/login?legal=terms")
}

async fn conditions_page() -> Redirect {
    Redirect::temporary("/login?legal=terms")
}

async fn privacy_page() -> Redirect {
    Redirect::temporary("/login?legal=privacy")
}

async fn safety_page() -> Redirect {
    Redirect::temporary("/login?legal=safety")
}

async fn community_rules_page() -> Redirect {
    Redirect::temporary("/login?legal=constitution")
}

async fn crisis_help_page(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Html<String> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let session_user_id = session::optional_user_id_from_jar(&state.db, &jar).await;
    let signed_in = session_user_id.is_some();
    let chrome = snippets::guest_topbar_chrome(&loc, signed_in);
    let profile_country = match session_user_id {
        Some(user_id) => human_onboarding::country_code_for_user(&state.db, user_id)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let crisis_panel = crate::crisis_resources::panel_for_everyone(
        &loc,
        profile_country.as_deref(),
        true,
    );
    let back_html = if signed_in {
        format!(
            r#"<a class="auth-text-btn" href="/home">{}</a>"#,
            escape_html(&c.t(&loc, "ui.crisis_help.back_to_home"))
        )
    } else {
        format!(
            r#"<a class="auth-text-btn" href="/login">{}</a>"#,
            escape_html(&c.t(&loc, "ui.crisis_help.back_to_login"))
        )
    };
    let topbar_html = chrome.public_topbar(&format!(
        r#"<span class="app-topbar-title mono">{}</span>"#,
        escape_html(&c.t(&loc, "ui.crisis_help.topbar_title"))
    ));
    let html = CRISIS_HELP_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__CRISIS_HELP_TITLE__",
            &escape_html(&c.t(&loc, "ui.crisis_help.document_title")),
        )
        .replace(
            "__CRISIS_HELP_HEADING__",
            &escape_html(&c.t(&loc, "ui.crisis_help.heading")),
        )
        .replace(
            "__CRISIS_HELP_LEAD__",
            &escape_html(&c.t(&loc, "ui.crisis_help.lead")),
        )
        .replace("__CRISIS_RESOURCES_PANEL__", &crisis_panel)
        .replace("__CRISIS_HELP_BACK_HTML__", &back_html)
        .replace("__PUBLIC_TOPBAR__", &topbar_html);
    Html(i18n::subst_markers(
        &snippets::inject_public_footer(&html),
        &loc,
    ))
}

async fn register_card_fragment() -> Html<String> {
    let loc = i18n::request_locale();
    Html(i18n::subst_markers(REGISTER_CARD_FRAGMENT, &loc))
}

async fn resend_card_fragment() -> Html<String> {
    let loc = i18n::request_locale();
    Html(i18n::subst_markers(RESEND_CARD_FRAGMENT, &loc))
}

async fn forgot_password_card_fragment() -> Html<String> {
    let loc = i18n::request_locale();
    Html(i18n::subst_markers(FORGOT_PASSWORD_CARD_FRAGMENT, &loc))
}

async fn reactivate_account_card_fragment() -> Html<String> {
    let loc = i18n::request_locale();
    Html(i18n::subst_markers(REACTIVATE_ACCOUNT_CARD_FRAGMENT, &loc))
}

fn change_password_page_i18n_json(loc: &str) -> String {
    let c = i18n::catalog();
    serde_json::json!({
        "change_password_success": c.t(loc, "ui.entity_private.change_password_success"),
        "change_password_failed": c.t(loc, "ui.entity_private.change_password_failed"),
        "change_password_mismatch": c.t(loc, "ui.entity_private.change_password_mismatch"),
        "account_password_required": c.t(loc, "ui.entity_private.account_password_required"),
        "account_password_same_as_current": c.t(loc, "ui.entity_private.account_password_same_as_current"),
        "password_too_short": c.t(loc, "auth.password_too_short"),
        "password_min_len": crate::auth::password::MIN_PASSWORD_LEN,
    })
    .to_string()
}

fn csrf_meta_html(headers: &HeaderMap, config: &crate::config::Config) -> String {
    profile_gate::csrf_token_for_request(headers, config)
        .map(|token| {
            format!(
                r#"<meta name="csrf-token" content="{}">"#,
                escape_html(&token)
            )
        })
        .unwrap_or_default()
}

async fn change_password_page(
    State(state): State<AppState>,
    Extension(session): Extension<VerifiedSession>,
    headers: HeaderMap,
) -> Html<String> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let reset_only = session.password_reset_only;
    let panel = change_password_panel::change_password_panel_html(
        &loc,
        &c,
        ChangePasswordPanelOpts {
            standalone: true,
            skip_verify_step: reset_only,
        },
    );
    let chrome = snippets::guest_topbar_chrome(&loc, true);
    let topbar_html = chrome.public_topbar(&format!(
        r#"<span class="app-topbar-title mono">{}</span>"#,
        escape_html(&c.t(&loc, "ui.login.reset_password_page_title"))
    ));
    let html = CHANGE_PASSWORD_TEMPLATE
        .replace("__PUBLIC_TOPBAR__", &topbar_html)
        .replace("__CHANGE_PASSWORD_PANEL__", &panel)
        .replace(
            "__CHANGE_PASSWORD_I18N_JSON__",
            &change_password_page_i18n_json(&loc),
        )
        .replace("__CSRF_META__", &csrf_meta_html(&headers, &state.config))
        .replace("__HTML_LANG__", &loc);
    Html(i18n::subst_markers(
        &snippets::inject_site_favicon(&html),
        &loc,
    ))
}

#[derive(Debug, Deserialize)]
struct ResetPasswordQuery {
    token: String,
}

async fn reset_password(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<ResetPasswordQuery>,
) -> AppResult<(CookieJar, Redirect)> {
    let outcome = password_reset::consume_password_reset_token(&state.db, q.token.trim())
        .await
        .map_err(crate::error::AppError::Db)?;
    match outcome {
        ResetConsumeOutcome::Valid(user_id) => {
            let cookie = session::start_password_reset_session(
                &state.db,
                user_id,
                state.config.secure_cookies,
            )
            .await
            .map_err(crate::error::AppError::Db)?;
            let jar = jar.add(cookie);
            Ok((jar, Redirect::temporary("/account/change-password")))
        }
        ResetConsumeOutcome::Invalid => Ok((jar, Redirect::temporary("/login?reset_error=1"))),
    }
}

#[derive(Debug, Deserialize)]
struct VerifyQuery {
    token: String,
}

async fn verify_email(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<VerifyQuery>,
) -> AppResult<(CookieJar, Redirect)> {
    let outcome = verification::consume_verification_token(&state.db, q.token.trim())
        .await
        .map_err(crate::error::AppError::Db)?;
    match outcome {
        VerifyOutcome::Verified(user_id) | VerifyOutcome::AlreadyVerified(user_id) => {
            let (cookie, login_outcome) = session::start_user_session(
                &state.db,
                user_id,
                state.config.secure_cookies,
            )
            .await
            .map_err(crate::error::AppError::Db)?;
            let jar = jar.add(cookie);
            let next = post_auth::next_path_after_login(
                &state.db,
                user_id,
                state.config.face_vision_mock,
                &login_outcome,
            )
                .await
                .map_err(crate::error::AppError::Db)?;
            Ok((jar, Redirect::temporary(&next)))
        }
        VerifyOutcome::Invalid => Ok((jar, Redirect::temporary("/login?verify_error=1"))),
    }
}

async fn account(Extension(VerifiedSession { user_id: _, password_reset_only: _ }): Extension<VerifiedSession>) -> Json<serde_json::Value> {
    let loc = i18n::request_locale();
    Json(serde_json::json!({
        "ok": true,
        "message": i18n::catalog().t(&loc, "api.account_ok")
    }))
}
