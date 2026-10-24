//! Google Maps Platform integration (Places API New).
//!
//! [`places`] calls Google with a tight field mask, maps into [`crate::locations::CreateLocationRequest`], and does
//! not forward raw Google JSON to browsers (see `POST /me/google-place-location` in `routes/social.rs`).
//!
//! Persisted rows use `validation_source` `google.places` and `validation_ref` (Place id) per [`crate::verification`].
//!
//! Use the same GCP project as Vertex / Cloud SQL where possible; enable APIs in Google Cloud Console.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;

use crate::config::Config;

pub mod places;

fn http_client_lazy() -> &'static Client {
    static HTTP: OnceLock<Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest client for google maps")
    })
}

pub(crate) fn http_client() -> &'static Client {
    http_client_lazy()
}

/// True when a server-side Maps key is configured (Places / Geocode calls may run).
#[must_use]
pub fn maps_backend_configured(cfg: &Config) -> bool {
    cfg.google_maps_api_key
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}
