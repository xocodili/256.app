//! Stripe webhook (public, signature-verified).

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;

use crate::app_state::AppState;
use crate::social::noms::purchase;

pub fn router() -> Router<AppState> {
    Router::new().route("/webhooks/stripe", post(stripe_webhook))
}

async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let Some(stripe_cfg) = state.config.stripe.as_ref() else {
        return (StatusCode::NOT_FOUND, "not configured");
    };
    if stripe_cfg.webhook_secret.is_empty() {
        return (StatusCode::NOT_FOUND, "webhook not configured");
    }

    let signature = headers
        .get("Stripe-Signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let event = match crate::stripe::parse_webhook_event(
        &stripe_cfg.webhook_secret,
        signature,
        &body,
    ) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "stripe webhook signature failed");
            return (StatusCode::BAD_REQUEST, "invalid signature");
        }
    };

    match event.event_type.as_str() {
        "payment_intent.succeeded" => {
            let pi_id = event.data.object.id;
            match purchase::fulfill_purchase_by_payment_intent(&state.db, &pi_id).await {
                Ok(_) => (StatusCode::OK, "ok"),
                Err(e) => {
                    tracing::error!(error = %e, payment_intent_id = %pi_id, "nom purchase fulfill failed");
                    (StatusCode::INTERNAL_SERVER_ERROR, "fulfill failed")
                }
            }
        }
        "payment_intent.payment_failed" => {
            let pi_id = event.data.object.id;
            if let Err(e) =
                purchase::mark_purchase_failed_by_payment_intent(&state.db, &pi_id).await
            {
                tracing::warn!(error = %e, payment_intent_id = %pi_id, "mark purchase failed");
            }
            (StatusCode::OK, "ok")
        }
        _ => (StatusCode::OK, "ignored"),
    }
}
