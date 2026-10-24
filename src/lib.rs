#![recursion_limit = "1024"]

mod agent_specs;
pub mod ai;
mod app_root;
pub mod app_state;
pub mod auth;
pub mod config;
pub mod content;
pub mod crisis_resources;
mod crypto;
pub mod db;
pub mod e2e_seed;
mod email;
pub mod error;
mod geocode;
mod google_maps;
mod html_escape;
mod i18n;
pub mod jobs;
mod locations;
mod middleware;
pub mod nom_packs;
pub mod pricing;
pub mod stripe;
mod rate_limit;
mod ref_key_values;
pub mod ref_keys;
mod routes;
pub mod social;
mod status;
mod ui;
mod verification;

use std::net::SocketAddr;
use std::time::Duration;

use axum::http::StatusCode;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Boot the HTTP server (used by `app` binary and integration tests).
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Load `.env` before tracing so `RUST_LOG` in the file applies (systemd
    // `EnvironmentFile` also works when set on the unit).
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let _ = i18n::catalog();
    crisis_resources::init_at_startup();
    let config = config::Config::from_env()?;
    if google_maps::maps_backend_configured(&config) {
        tracing::info!("GOOGLE_MAPS_API_KEY is set; Places/Geocode integration can call Google from the server.");
    }
    let pool = db::connect_and_migrate(&config.database_url).await?;

    jobs::spawn_scheduler(
        pool.clone(),
        config.clone(),
        Duration::from_secs(config.scheduler_tick_secs),
        config.scheduler_batch_size,
    );

    let state = app_state::AppState::new(pool, config.clone());

    let app = build_app(state);

    let addr = SocketAddr::new(config.host, config.port);
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

/// Assemble the fully-layered Axum application from an [`app_state::AppState`].
///
/// This is the single source of truth for router + middleware wiring, shared by
/// the production [`run`] path and the in-process integration-test harness
/// (`tests/common/api.rs`) so tests exercise the same stack real requests hit.
pub fn build_app(state: app_state::AppState) -> axum::Router {
    let cors = CorsLayer::new();

    let timeout = TimeoutLayer::with_status_code(
        StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(state.config.request_timeout_secs),
    );

    routes::router(state.clone())
        .layer(axum::middleware::from_fn(
            crate::middleware::locale::negotiate_locale,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::sliding_session::sliding_session,
        ))
        .layer(timeout)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
