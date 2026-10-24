use serde::{Deserialize, Serialize};

use crate::social::human_onboarding;

pub const BASE_PASSWORD: &str = "e2e-password-12";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaEntry {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    pub user_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixturesEntry {
    pub public_handle: String,
    pub sample_post_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2eManifest {
    pub base_password: String,
    pub terms_consent_version: String,
    pub personas: Personas,
    pub fixtures: FixturesEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personas {
    pub full: PersonaEntry,
    pub consent: PersonaEntry,
    pub partial: PersonaEntry,
}

impl E2eManifest {
    pub fn password_hint() -> &'static str {
        BASE_PASSWORD
    }

    pub fn terms_version() -> &'static str {
        human_onboarding::CURRENT_TERMS_CONSENT_VERSION
    }
}
