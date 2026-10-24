use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response;
use axum_extra::extract::cookie::Cookie;

use crate::app_state::AppState;
use crate::auth::session::{self, SESSION_COOKIE_NAME};
use crate::db::TimestampUtc;
use crate::ref_keys;

use super::auth_gate::{self, AuthGateOutcome};

/// Set on the request by [`require_verified_session`] for routes under the protected [`crate::routes`] subtree.
#[derive(Clone, Copy, Debug)]
pub struct VerifiedSession {
    pub user_id: u64,
    pub password_reset_only: bool,
}

fn session_token_from_cookies(req: &Request<Body>) -> Option<String> {
    let raw = req
        .headers()
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Ok(c) = Cookie::parse(part) {
            if c.name() == SESSION_COOKIE_NAME {
                let v = c.value();
                if v.len() == session::SESSION_TOKEN_HEX_LEN
                    && v.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
                {
                    return Some(v.to_string());
                }
                return None;
            }
        }
    }
    None
}

/// Requires a valid session whose user has verified email. Inserts [`VerifiedSession`] for handlers.
///
/// Note: Posting rights are composite and enforced downstream (`status=verified`),
/// so users can still complete onboarding steps (human profile + face capture).
pub async fn require_verified_session(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let Some(token) = session_token_from_cookies(&request) else {
        return auth_gate::respond(AuthGateOutcome::MissingSession, &request);
    };

    let res = sqlx::query_as::<_, (u64, Option<TimestampUtc>, Option<String>, bool)>(
        r#"
        SELECT s.user_id, u.email_verified_at, rk.`key`, s.password_reset_only
        FROM sessions s
        INNER JOIN users u ON u.id = s.user_id
        LEFT JOIN ref_key_values rk ON rk.id = COALESCE(u.user_status_id, u.status_id)
        WHERE s.session_token = ? AND s.expires_at > UTC_TIMESTAMP()
        "#,
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await;

    let row = match res {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "verified-session gate query failed");
            return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let Some((user_id, verified_at, user_status_key, password_reset_only)) = row else {
        return auth_gate::respond(AuthGateOutcome::InvalidSession, &request);
    };

    if matches!(
        user_status_key.as_deref(),
        Some(ref_keys::KEY_TO_BE_DELETED) | Some(ref_keys::KEY_BLOCKED)
    ) {
        if let Err(e) = sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&state.db)
            .await
        {
            tracing::warn!(error = %e, user_id, "failed to purge sessions for retired user");
        }
        return auth_gate::respond(AuthGateOutcome::AccountRemoved, &request);
    }

    if verified_at.is_none() {
        return auth_gate::respond(AuthGateOutcome::EmailNotVerified, &request);
    }

    request.extensions_mut().insert(VerifiedSession {
        user_id,
        password_reset_only,
    });
    next.run(request).await
}
