//! Stripe Nom pack purchase API (verified session).

use std::collections::HashMap;

use axum::extract::{Extension, Path, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::middleware::VerifiedSession;
use crate::nom_packs::{NomPack, PACK_128_SKU, PACK_32_SKU, PACK_64_SKU};
use crate::rate_limit;
use crate::routes::profile_gate;
use crate::social::human_onboarding;
use crate::social::noms::purchase::{self, PurchaseError, PurchaseStatus};
use crate::stripe;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/noms/purchase/config", get(purchase_config))
        .route(
            "/api/noms/purchase/intent",
            post(create_purchase_intent).layer(rate_limit::limit_layer(
                rate_limit::login_resend_config(),
            )),
        )
        .route("/api/noms/purchase/{id}", get(purchase_status))
        .route(
            "/api/noms/purchase/{id}/sync",
            post(sync_purchase_status).layer(rate_limit::limit_layer(
                rate_limit::login_resend_config(),
            )),
        )
        .route(
            "/api/noms/purchase/{id}/cancel",
            post(cancel_purchase_checkout).layer(rate_limit::limit_layer(
                rate_limit::login_resend_config(),
            )),
        )
}

#[derive(Debug, Serialize)]
struct PackJson {
    sku: &'static str,
    noms: u16,
    amount_cents: u32,
    currency: &'static str,
    display_price: String,
    label: String,
    max_balance_exclusive: u16,
    can_purchase: bool,
    wallet_full: bool,
    block_reason: Option<&'static str>,
}

fn block_reason_str(reason: crate::nom_packs::PurchaseBlockReason) -> &'static str {
    use crate::nom_packs::PurchaseBlockReason;
    match reason {
        PurchaseBlockReason::WalletCap => "wallet_cap",
        PurchaseBlockReason::StewardTier => "steward_tier",
    }
}

#[derive(Debug, Serialize)]
struct PurchaseConfigResponse {
    enabled: bool,
    publishable_key: Option<String>,
    packs: Vec<PackJson>,
    balance: u16,
    pending_checkout: bool,
    /// ISO 3166-1 alpha-2 pre-filled on Payment Element (postal code collected by Stripe when required).
    billing_country: String,
}

fn stripe_billing_country(raw: Option<String>) -> String {
    raw.and_then(|c| {
        let t = c.trim().to_uppercase();
        (t.len() == 2 && t.chars().all(|ch| ch.is_ascii_alphabetic())).then_some(t)
    })
    .unwrap_or_else(|| "AU".to_string())
}

fn pack_display_price(c: &i18n::Catalog, loc: &str, sku: &str) -> String {
    let key = match sku {
        PACK_32_SKU => "noms.purchase_pack_civic_price",
        PACK_64_SKU => "noms.purchase_pack_standard_price",
        PACK_128_SKU => "noms.purchase_pack_steward_price",
        _ => "noms.purchase_pack_standard_price",
    };
    c.t(loc, key)
}

fn pack_label(c: &i18n::Catalog, loc: &str, sku: &str) -> String {
    let key = match sku {
        PACK_32_SKU => "noms.purchase_pack_civic_label",
        PACK_64_SKU => "noms.purchase_pack_standard_label",
        PACK_128_SKU => "noms.purchase_pack_steward_label",
        _ => "noms.purchase_pack_standard_label",
    };
    c.t(loc, key)
}

fn pack_json(
    pack: NomPack,
    balance: u16,
    pending_checkout: bool,
    c: &i18n::Catalog,
    loc: &str,
) -> PackJson {
    let wallet_eligible = pack.can_purchase_at_balance(balance);
    let block_reason = pack
        .purchase_block_reason(balance)
        .map(block_reason_str);
    PackJson {
        sku: pack.sku,
        noms: pack.noms,
        amount_cents: pack.amount_cents,
        currency: pack.currency,
        display_price: pack_display_price(c, loc, pack.sku),
        label: pack_label(c, loc, pack.sku),
        max_balance_exclusive: pack.max_balance_exclusive,
        can_purchase: wallet_eligible,
        wallet_full: !wallet_eligible,
        block_reason,
    }
}

async fn purchase_config(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
) -> AppResult<Json<PurchaseConfigResponse>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;

    let Some(stripe_cfg) = state.config.stripe.as_ref().filter(|s| s.enabled()) else {
        return Ok(Json(PurchaseConfigResponse {
            enabled: false,
            publishable_key: None,
            packs: Vec::new(),
            balance,
            pending_checkout: false,
            billing_country: "AU".to_string(),
        }));
    };

    let (balance, pending_checkout) =
        purchase::purchase_config_for_user(&state.db, user_id).await.map_err(map_purchase_err)?;

    let profile_country = human_onboarding::country_code_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let billing_country = stripe_billing_country(profile_country);

    let packs: Vec<PackJson> = NomPack::all()
        .iter()
        .copied()
        .map(|pack| pack_json(pack, balance, pending_checkout, &c, &loc))
        .collect();

    Ok(Json(PurchaseConfigResponse {
        enabled: true,
        publishable_key: Some(stripe_cfg.publishable_key.clone()),
        packs,
        balance,
        pending_checkout,
        billing_country,
    }))
}

#[derive(Debug, Deserialize)]
struct CreateIntentBody {
    sku: String,
}

#[derive(Debug, Serialize)]
struct CreateIntentResponse {
    purchase_id: u64,
    client_secret: String,
}

async fn create_purchase_intent(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Json(body): Json<CreateIntentBody>,
) -> AppResult<Json<CreateIntentResponse>> {
    profile_gate::require_csrf(&headers, &state.config)?;

    let stripe_cfg = state
        .config
        .stripe
        .as_ref()
        .filter(|s| s.enabled())
        .ok_or_else(|| AppError::BadRequest("Nom purchases are not enabled".to_string()))?;

    let pack = NomPack::find(body.sku.trim()).ok_or_else(|| {
        AppError::BadRequest(format!("Unknown Nom pack: {}", body.sku.trim()))
    })?;

    let entity_id = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| AppError::BadRequest("Human profile required".to_string()))?;

    let balance = crate::social::noms::balance_for_human_entity(&state.db, entity_id)
        .await
        .map_err(AppError::Db)?;

    let stale_pi_ids =
        purchase::abandon_all_pending_purchases_for_user(&state.db, user_id)
            .await
            .map_err(map_purchase_err)?;
    for pi_id in &stale_pi_ids {
        if let Err(e) = stripe::cancel_payment_intent(&stripe_cfg.secret_key, pi_id).await {
            tracing::warn!(
                error = %e,
                payment_intent_id = %pi_id,
                user_id,
                "failed to cancel superseded payment intent"
            );
        }
    }

    if !pack.can_purchase_at_balance(balance) {
        let loc = i18n::request_locale();
        let c = i18n::catalog();
        return Err(match pack.purchase_block_reason(balance) {
            Some(crate::nom_packs::PurchaseBlockReason::StewardTier) => AppError::BadRequest(c.tf(
                &loc,
                "noms.purchase_wallet_full",
                &[
                    ("balance", &balance.to_string()),
                    ("max", &pack.max_balance_exclusive.to_string()),
                ],
            )),
            _ => AppError::BadRequest(c.t(&loc, "noms.purchase_wallet_cap")),
        });
    }

    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), user_id.to_string());
    metadata.insert("social_entity_id".to_string(), entity_id.to_string());
    metadata.insert("sku".to_string(), pack.sku.to_string());
    metadata.insert("noms".to_string(), pack.noms.to_string());

    let pi = stripe::create_payment_intent(
        &stripe_cfg.secret_key,
        pack.amount_cents,
        pack.currency,
        &metadata,
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = %e, user_id, "stripe payment_intent create failed");
        AppError::BadRequest("Payment setup failed. Try again.".to_string())
    })?;

    let client_secret = pi.client_secret.ok_or_else(|| {
        AppError::BadRequest("Payment setup incomplete".to_string())
    })?;

    let purchase_id = match purchase::insert_pending_purchase(
        &state.db,
        user_id,
        entity_id,
        pack,
        &pi.id,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            if let Err(cancel_err) =
                stripe::cancel_payment_intent(&stripe_cfg.secret_key, &pi.id).await
            {
                tracing::warn!(
                    error = %cancel_err,
                    payment_intent_id = %pi.id,
                    user_id,
                    "failed to cancel orphan payment intent after nom_purchase insert error"
                );
            }
            return Err(map_purchase_err(e));
        }
    };

    Ok(Json(CreateIntentResponse {
        purchase_id,
        client_secret,
    }))
}

#[derive(Debug, Serialize)]
struct PurchaseStatusResponse {
    id: u64,
    status: String,
    balance_after: Option<u16>,
    noms_granted: u16,
}

async fn purchase_status(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(id): Path<u64>,
) -> AppResult<Json<PurchaseStatusResponse>> {
    let row = purchase::load_purchase_by_id(&state.db, id, user_id)
        .await
        .map_err(map_purchase_err)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(PurchaseStatusResponse {
        id: row.id,
        status: status_str(row.status),
        balance_after: row.balance_after,
        noms_granted: row.noms_granted,
    }))
}

async fn cancel_purchase_checkout(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    profile_gate::require_csrf(&headers, &state.config)?;

    let pi_id = purchase::cancel_pending_purchase_by_id(&state.db, id, user_id)
        .await
        .map_err(map_purchase_err)?;

    if let (Some(pi_id), Some(stripe_cfg)) = (
        pi_id.as_deref(),
        state.config.stripe.as_ref().filter(|s| s.enabled()),
    ) {
        if let Err(e) = stripe::cancel_payment_intent(&stripe_cfg.secret_key, pi_id).await {
            tracing::warn!(error = %e, purchase_id = id, "stripe payment_intent cancel failed");
        }
    }

    Ok(Json(serde_json::json!({ "cancelled": pi_id.is_some() })))
}

async fn sync_purchase_status(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Path(id): Path<u64>,
) -> AppResult<Json<PurchaseStatusResponse>> {
    profile_gate::require_csrf(&headers, &state.config)?;

    let stripe_cfg = state
        .config
        .stripe
        .as_ref()
        .filter(|s| s.enabled())
        .ok_or_else(|| AppError::BadRequest("Nom purchases are not enabled".to_string()))?;

    let row = purchase::load_purchase_by_id(&state.db, id, user_id)
        .await
        .map_err(map_purchase_err)?
        .ok_or(AppError::NotFound)?;

    if row.status == PurchaseStatus::Pending {
        let pi = stripe::retrieve_payment_intent(&stripe_cfg.secret_key, &row.stripe_payment_intent_id)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, purchase_id = id, "stripe payment_intent retrieve failed");
                AppError::BadRequest("Could not verify payment".to_string())
            })?;
        if pi.status == "succeeded" {
            purchase::fulfill_purchase_by_payment_intent(&state.db, &row.stripe_payment_intent_id)
                .await
                .map_err(map_purchase_err)?;
        } else if pi.status == "canceled" || pi.status == "requires_payment_method" {
            let _ = purchase::mark_purchase_failed_by_payment_intent(
                &state.db,
                &row.stripe_payment_intent_id,
            )
            .await;
        }
    }

    let updated = purchase::load_purchase_by_id(&state.db, id, user_id)
        .await
        .map_err(map_purchase_err)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(PurchaseStatusResponse {
        id: updated.id,
        status: status_str(updated.status),
        balance_after: updated.balance_after,
        noms_granted: updated.noms_granted,
    }))
}

fn status_str(status: PurchaseStatus) -> String {
    match status {
        PurchaseStatus::Pending => "pending".to_string(),
        PurchaseStatus::Completed => "completed".to_string(),
        PurchaseStatus::Failed => "failed".to_string(),
    }
}

fn map_purchase_err(err: PurchaseError) -> AppError {
    match err {
        PurchaseError::WalletFull { balance, max_exclusive } => {
            let loc = i18n::request_locale();
            let c = i18n::catalog();
            AppError::BadRequest(c.tf(
                &loc,
                "noms.purchase_wallet_full",
                &[
                    ("balance", &balance.to_string()),
                    ("max", &max_exclusive.to_string()),
                ],
            ))
        }
        PurchaseError::PackNotFound => AppError::BadRequest("Unknown Nom pack".to_string()),
        PurchaseError::NoHumanEntity => {
            AppError::BadRequest("Human profile required".to_string())
        }
        PurchaseError::Db(e) => AppError::Db(e),
        PurchaseError::AlreadyCompleted | PurchaseError::NotPending => {
            AppError::Conflict("Purchase already settled".to_string())
        }
        PurchaseError::CapExceeded { .. } => {
            AppError::BadRequest("Wallet cap would be exceeded".to_string())
        }
        PurchaseError::PendingPurchaseExists => {
            let loc = i18n::request_locale();
            let c = i18n::catalog();
            AppError::BadRequest(c.t(&loc, "noms.purchase_checkout_pending"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PACK_128_SKU;
    use crate::nom_packs::NomPack;

    #[test]
    fn default_pack_is_128_aud() {
        let pack = NomPack::pack_128_aud();
        assert_eq!(pack.sku, PACK_128_SKU);
        assert_eq!(pack.noms, 128);
    }

    #[test]
    fn all_packs_exposed_in_catalog() {
        assert_eq!(NomPack::all().len(), 3);
    }
}
