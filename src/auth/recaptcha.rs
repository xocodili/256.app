//! Google reCAPTCHA v3 server verification.

use serde::Deserialize;

use crate::config::RecaptchaConfig;

#[derive(Debug)]
pub enum RecaptchaError {
    MissingToken,
    VerifyFailed,
    LowScore,
    ActionMismatch,
    NotConfigured,
}

impl RecaptchaError {
    #[must_use]
    pub fn is_bypass_eligible(&self) -> bool {
        matches!(self, Self::NotConfigured)
    }
}

#[derive(Debug, Deserialize)]
struct SiteVerifyResponse {
    success: bool,
    score: Option<f64>,
    action: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    error_codes: Vec<String>,
}

/// Verify a client token for the expected v3 action. Returns the score on success.
pub async fn verify_recaptcha_v3(
    config: &RecaptchaConfig,
    token: &str,
    remote_ip: Option<&str>,
    expected_action: &str,
) -> Result<f64, RecaptchaError> {
    if config.bypass {
        return Ok(1.0);
    }

    let token = token.trim();
    if token.is_empty() {
        return Err(RecaptchaError::MissingToken);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| RecaptchaError::VerifyFailed)?;

    let mut form = vec![
        ("secret", config.secret_key.as_str()),
        ("response", token),
    ];
    if let Some(ip) = remote_ip.filter(|s| !s.is_empty()) {
        form.push(("remoteip", ip));
    }

    let resp = client
        .post("https://www.google.com/recaptcha/api/siteverify")
        .form(&form)
        .send()
        .await
        .map_err(|_| RecaptchaError::VerifyFailed)?;

    let body: SiteVerifyResponse = resp
        .json()
        .await
        .map_err(|_| RecaptchaError::VerifyFailed)?;

    if !body.success {
        tracing::warn!(
            error_codes = ?body.error_codes,
            expected_action = expected_action,
            "recaptcha siteverify returned success=false"
        );
        return Err(RecaptchaError::VerifyFailed);
    }

    let action = body.action.as_deref().unwrap_or("");
    if !action.is_empty() && action != expected_action {
        tracing::warn!(
            expected = expected_action,
            actual = action,
            score = ?body.score,
            "recaptcha action mismatch"
        );
        return Err(RecaptchaError::ActionMismatch);
    }

    let score = body.score.unwrap_or(0.0);
    if score < config.min_score {
        tracing::warn!(
            score,
            min = config.min_score,
            action,
            "recaptcha score below threshold"
        );
        return Err(RecaptchaError::LowScore);
    }

    Ok(score)
}
