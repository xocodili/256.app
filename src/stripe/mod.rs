//! Stripe REST client (PaymentIntent create + webhook signature verify).

use std::collections::HashMap;
use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize)]
pub struct PaymentIntentResponse {
    pub id: String,
    pub client_secret: Option<String>,
    pub status: String,
}

#[derive(Debug)]
pub enum StripeError {
    Http(String),
    Api { status: u16, body: String },
    Parse(String),
    WebhookSignature(String),
}

impl std::fmt::Display for StripeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StripeError::Http(s) => write!(f, "stripe http: {s}"),
            StripeError::Api { status, body } => write!(f, "stripe api {status}: {body}"),
            StripeError::Parse(s) => write!(f, "stripe parse: {s}"),
            StripeError::WebhookSignature(s) => write!(f, "stripe webhook: {s}"),
        }
    }
}

fn stripe_client() -> Result<Client, StripeError> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| StripeError::Http(e.to_string()))
}

pub async fn create_payment_intent(
    secret_key: &str,
    amount_cents: u32,
    currency: &str,
    metadata: &HashMap<String, String>,
) -> Result<PaymentIntentResponse, StripeError> {
    let mut form: Vec<(String, String)> = vec![
        ("amount".to_string(), amount_cents.to_string()),
        ("currency".to_string(), currency.to_string()),
        ("payment_method_types[]".to_string(), "card".to_string()),
    ];
    for (k, v) in metadata {
        form.push((format!("metadata[{k}]"), v.clone()));
    }

    let client = stripe_client()?;
    let resp = client
        .post("https://api.stripe.com/v1/payment_intents")
        .bearer_auth(secret_key)
        .form(&form)
        .send()
        .await
        .map_err(|e| StripeError::Http(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| StripeError::Http(e.to_string()))?;
    if !status.is_success() {
        return Err(StripeError::Api {
            status: status.as_u16(),
            body,
        });
    }
    serde_json::from_str(&body).map_err(|e| StripeError::Parse(e.to_string()))
}

pub async fn cancel_payment_intent(
    secret_key: &str,
    payment_intent_id: &str,
) -> Result<(), StripeError> {
    let client = stripe_client()?;
    let url = format!(
        "https://api.stripe.com/v1/payment_intents/{payment_intent_id}/cancel"
    );
    let resp = client
        .post(url)
        .bearer_auth(secret_key)
        .send()
        .await
        .map_err(|e| StripeError::Http(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| StripeError::Http(e.to_string()))?;
    if !status.is_success() {
        return Err(StripeError::Api {
            status: status.as_u16(),
            body,
        });
    }
    Ok(())
}

pub async fn retrieve_payment_intent(
    secret_key: &str,
    payment_intent_id: &str,
) -> Result<PaymentIntentResponse, StripeError> {
    let client = stripe_client()?;
    let url = format!(
        "https://api.stripe.com/v1/payment_intents/{payment_intent_id}"
    );
    let resp = client
        .get(url)
        .bearer_auth(secret_key)
        .send()
        .await
        .map_err(|e| StripeError::Http(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| StripeError::Http(e.to_string()))?;
    if !status.is_success() {
        return Err(StripeError::Api {
            status: status.as_u16(),
            body,
        });
    }
    serde_json::from_str(&body).map_err(|e| StripeError::Parse(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: WebhookEventData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookEventData {
    pub object: WebhookPaymentIntent,
}

#[derive(Debug, Deserialize)]
pub struct WebhookPaymentIntent {
    pub id: String,
    pub status: String,
    pub metadata: HashMap<String, String>,
}

/// Verify `Stripe-Signature` and parse the event JSON.
pub fn parse_webhook_event(
    webhook_secret: &str,
    signature_header: &str,
    body: &[u8],
) -> Result<WebhookEvent, StripeError> {
    verify_webhook_signature(webhook_secret, signature_header, body)?;
    serde_json::from_slice(body).map_err(|e| StripeError::Parse(e.to_string()))
}

fn verify_webhook_signature(
    webhook_secret: &str,
    signature_header: &str,
    body: &[u8],
) -> Result<(), StripeError> {
    let mut timestamp: Option<&str> = None;
    let mut signatures: Vec<&str> = Vec::new();
    for part in signature_header.split(',') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            match k {
                "t" => timestamp = Some(v),
                "v1" => signatures.push(v),
                _ => {}
            }
        }
    }
    let timestamp = timestamp.ok_or_else(|| {
        StripeError::WebhookSignature("missing timestamp".to_string())
    })?;
    if signatures.is_empty() {
        return Err(StripeError::WebhookSignature(
            "missing v1 signature".to_string(),
        ));
    }

    let signed_payload = format!("{timestamp}.{}", String::from_utf8_lossy(body));
    let mut mac = HmacSha256::new_from_slice(webhook_secret.as_bytes())
        .map_err(|e| StripeError::WebhookSignature(e.to_string()))?;
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    let valid = signatures.iter().any(|sig| constant_time_eq(sig, &expected));
    if !valid {
        return Err(StripeError::WebhookSignature(
            "signature mismatch".to_string(),
        ));
    }

    let ts: i64 = timestamp
        .parse()
        .map_err(|_| StripeError::WebhookSignature("invalid timestamp".to_string()))?;
    let now = chrono::Utc::now().timestamp();
    if (now - ts).abs() > 300 {
        return Err(StripeError::WebhookSignature(
            "timestamp outside tolerance".to_string(),
        ));
    }
    Ok(())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes()
        .iter()
        .zip(b.as_bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event_body() -> &'static [u8] {
        br#"{"type":"payment_intent.succeeded","data":{"object":{"id":"pi_test_123","status":"succeeded","metadata":{}}}}"#
    }

    fn sign_payload(secret: &str, timestamp: i64, body: &[u8]) -> String {
        let signed_payload = format!("{timestamp}.{}", String::from_utf8_lossy(body));
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
        mac.update(signed_payload.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        format!("t={timestamp},v1={sig}")
    }

    #[test]
    fn webhook_signature_accepts_valid_payload() {
        let secret = "whsec_test_secret";
        let body = sample_event_body();
        let timestamp = chrono::Utc::now().timestamp();
        let header = sign_payload(secret, timestamp, body);
        let event = parse_webhook_event(secret, &header, body).expect("valid signature");
        assert_eq!(event.event_type, "payment_intent.succeeded");
        assert_eq!(event.data.object.id, "pi_test_123");
    }

    #[test]
    fn webhook_signature_rejects_tampered_body() {
        let secret = "whsec_test_secret";
        let body = sample_event_body();
        let timestamp = chrono::Utc::now().timestamp();
        let header = sign_payload(secret, timestamp, body);
        let tampered = br#"{"type":"payment_intent.succeeded","data":{"object":{"id":"pi_hacked","status":"succeeded","metadata":{}}}}"#;
        let err = parse_webhook_event(secret, &header, tampered).expect_err("tampered body");
        assert!(matches!(err, StripeError::WebhookSignature(_)));
    }

    #[test]
    fn webhook_signature_rejects_stale_timestamp() {
        let secret = "whsec_test_secret";
        let body = sample_event_body();
        let timestamp = chrono::Utc::now().timestamp() - 600;
        let header = sign_payload(secret, timestamp, body);
        let err = parse_webhook_event(secret, &header, body).expect_err("stale timestamp");
        assert!(matches!(err, StripeError::WebhookSignature(_)));
    }
}
