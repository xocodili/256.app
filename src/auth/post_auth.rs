//! Post-authentication redirect targets (onboarding funnel).

use axum::response::{IntoResponse, Redirect};
use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::auth::session::LoginRecordOutcome;
use crate::social::human_onboarding;
use crate::status;

/// Next browser path after a successful session is established.
pub async fn next_path_after_auth(
    pool: &MySqlPool,
    user_id: u64,
    face_vision_mock: bool,
) -> Result<String, sqlx::Error> {
    if !human_onboarding::user_has_terms_consent(pool, user_id).await? {
        return Ok("/home?onboarding=consent".to_string());
    }

    if !crate::social::user_faces::user_passes_face_capture_gate(pool, user_id, face_vision_mock)
        .await?
    {
        return Ok("/onboarding".to_string());
    }

    if !human_onboarding::user_has_human_profile(pool, user_id).await? {
        return Ok("/home?onboarding=profile".to_string());
    }

    if !status::user_has_verified_status(pool, user_id).await? {
        return Ok("/onboarding".to_string());
    }

    Ok("/home".to_string())
}

fn append_query_param(path: &str, key: &str, value: &str) -> String {
    if path.contains('?') {
        format!("{path}&{key}={value}")
    } else {
        format!("{path}?{key}={value}")
    }
}

/// Appends one-shot home-shell notice query params when login refunded inactivity debits.
pub fn append_inactivity_refund_notice(path: &str, refunded: Decimal) -> String {
    if refunded <= Decimal::ZERO {
        return path.to_string();
    }
    let amount = format!("{refunded:.2}");
    let path = append_query_param(path, "notice", "inactivity_standing_restored");
    append_query_param(&path, "standing_restored", &amount)
}

/// Next browser path after sign-in, including inactivity-refund toast params when applicable.
pub async fn next_path_after_login(
    pool: &MySqlPool,
    user_id: u64,
    face_vision_mock: bool,
    login_outcome: &LoginRecordOutcome,
) -> Result<String, sqlx::Error> {
    let base = next_path_after_auth(pool, user_id, face_vision_mock).await?;
    Ok(append_inactivity_refund_notice(
        &base,
        login_outcome.inactivity_refunded,
    ))
}

/// When true, onboarding after consent must return to face capture first.
pub async fn missing_face_capture_redirect(
    pool: &MySqlPool,
    user_id: u64,
    face_vision_mock: bool,
) -> Result<Option<axum::response::Response>, sqlx::Error> {
    if crate::social::user_faces::user_passes_face_capture_gate(pool, user_id, face_vision_mock)
        .await?
    {
        Ok(None)
    } else {
        Ok(Some(axum::response::Redirect::temporary("/onboarding").into_response()))
    }
}

/// Whether the user must complete the consent step before other onboarding.
pub async fn needs_terms_consent(pool: &MySqlPool, user_id: u64) -> Result<bool, sqlx::Error> {
    Ok(!human_onboarding::user_has_terms_consent(pool, user_id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn append_inactivity_refund_notice_adds_params_to_home() {
        let path = append_inactivity_refund_notice("/home", Decimal::new(10, 2));
        assert_eq!(
            path,
            "/home?notice=inactivity_standing_restored&standing_restored=0.10"
        );
    }

    #[test]
    fn append_inactivity_refund_notice_merges_existing_query() {
        let path =
            append_inactivity_refund_notice("/home?onboarding=consent", Decimal::new(11, 2));
        assert_eq!(
            path,
            "/home?onboarding=consent&notice=inactivity_standing_restored&standing_restored=0.11"
        );
    }

    #[test]
    fn append_inactivity_refund_notice_zero_unchanged() {
        let path = append_inactivity_refund_notice("/home?onboarding=consent", Decimal::ZERO);
        assert_eq!(path, "/home?onboarding=consent");
    }
}
