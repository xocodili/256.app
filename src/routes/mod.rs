mod api_public;
mod auth;
mod auth_oidc;
mod entity_public;
mod health;
mod home;
mod jobs;
mod locations;
mod noms_purchase;
mod ops;
mod stripe_webhook;
mod user_face_assets;
mod panoply;
mod profile_gate;
mod ref_key_values;
mod social;

use axum::response::IntoResponse;
use axum::middleware::{self, Next};
use axum::routing::get;
use axum::body::Body;
use axum::http::Request;
use axum::Router;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;

use crate::app_state::AppState;
use crate::middleware::require_verified;
use crate::rate_limit;

async fn block_raw_user_face_static(req: Request<Body>, next: Next) -> axum::response::Response {
    let path = req.uri().path().replace('\\', "/");
    if path.contains("/generated/user-faces/") {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }
    next.run(req).await
}

pub fn router(state: AppState) -> Router<AppState> {
    let static_dir = crate::app_root::runtime_app_root().join("static");
    let user_faces_web = user_face_assets::web_mount_prefix(&state.config.generated_faces_dir);
    let ui_cards = rate_limit::ui_card_surface_config(state.config.dev_auth_bypass);

    // Public profile HTML shell: same UI-card quota as login/home. JSON API keeps a separate scrape limit.
    let public_entity_surface = Router::new()
        .merge(
            entity_public::shell_router()
                .layer(rate_limit::ui_card_limit_layer(ui_cards.clone())),
        )
        .merge(api_public::router().layer(rate_limit::social_entity_public_profile_layer()));

    let static_service = ServiceBuilder::new()
        .layer(middleware::from_fn(block_raw_user_face_static))
        .service(ServeDir::new(static_dir));

    let public = Router::new()
        .route(
            &format!("{user_faces_web}/{{*asset_path}}"),
            get(user_face_assets::serve_generated_artwork),
        )
        .nest_service("/static", static_service)
        .merge(health::public_router(state.config.dev_auth_bypass))
        .merge(auth::router())
        .merge(auth_oidc::router())
        .merge(ref_key_values::router())
        .merge(ops::router())
        .merge(stripe_webhook::router())
        .merge(public_entity_surface);

    // Session + verified email required.
    // Posting permissions are further gated by composite verification status in handlers.
    let protected = Router::new()
        .merge(health::protected_router())
        .merge(home::router().layer(rate_limit::ui_card_limit_layer(ui_cards.clone())))
        .merge(panoply::router().layer(rate_limit::ui_card_limit_layer(ui_cards)))
        .merge(locations::router())
        .merge(jobs::router())
        .merge(social::router())
        .merge(noms_purchase::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::onboarding_gate::redirect_wrong_onboarding_step,
        ))
        .layer(middleware::from_fn(
            crate::middleware::password_reset_gate::restrict_password_reset_session,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_verified::require_verified_session,
        ));

    Router::new().merge(public).merge(protected)
}
