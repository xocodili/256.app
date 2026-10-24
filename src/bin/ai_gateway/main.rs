use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

use base64::Engine as _;
use reqwest::Client;
use reqwest::StatusCode;
use serde_json::Value;
use tonic::transport::Server;
use tonic::{Code, Request, Response, Status};

pub mod pb {
    tonic::include_proto!("aiagents.v1");
}

use pb::ai_agent_gateway_server::{AiAgentGateway, AiAgentGatewayServer};
use pb::{
    ProcessingMode, RunPortraitAgentRequest, RunPortraitAgentResponse, RunTextAgentRequest,
    RunTextAgentResponse,
};

mod vertex_auth;
mod vertex_rate_limit;
mod portrait;

fn google_api_error_detail(json: &Value, raw: &[u8]) -> String {
    if let Some(msg) = json
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
    {
        return msg.to_string();
    }
    let s = String::from_utf8_lossy(raw);
    let t = s.chars().take(500).collect::<String>();
    if s.chars().count() > 500 {
        format!("{t}…")
    } else {
        t
    }
}

fn tonic_code_for_google_http(status: StatusCode) -> Code {
    match status.as_u16() {
        401 => Code::Unauthenticated,
        403 => Code::PermissionDenied,
        404 => Code::NotFound,
        429 => Code::ResourceExhausted,
        _ => Code::Unavailable,
    }
}

struct LocalGateway {
    google_api_key: Option<String>,
    google_model: String,
    http_timeout_ms: u64,
    service_account: Option<vertex_auth::ServiceAccountJson>,
    vertex_location: String,
    vertex_project_id: Option<String>,
    token_cache: tokio::sync::Mutex<Option<vertex_auth::CachedAccessToken>>,
}

impl Default for LocalGateway {
    fn default() -> Self {
        // Ensure `.env` is loaded before reading vars (same cwd as `cargo run`).
        // `main` also calls this; duplicate load is harmless.
        dotenvy::dotenv().ok();

        let cred_path = std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let service_account = cred_path.and_then(|p| {
            match vertex_auth::load_service_account(&p) {
                Ok(sa) => Some(sa),
                Err(e) => {
                    tracing::warn!(path = %p, error = %e, "failed to load GOOGLE_APPLICATION_CREDENTIALS");
                    None
                }
            }
        });

        let vertex_project_id = std::env::var("GOOGLE_CLOUD_PROJECT")
            .or_else(|_| std::env::var("VERTEX_PROJECT_ID"))
            .or_else(|_| std::env::var("GOOGLE_PROJECT_ID"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| service_account.as_ref().map(|s| s.project_id.clone()));

        let vertex_location = std::env::var("VERTEX_LOCATION")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "us-central1".to_string());

        Self {
            google_api_key: std::env::var("GOOGLE_AI_API_KEY")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            google_model: std::env::var("GOOGLE_AI_MODEL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "gemini-2.5-flash-lite".to_string()),
            http_timeout_ms: env_or("GATEWAY_HTTP_TIMEOUT_MS", 12000),
            service_account,
            vertex_location,
            vertex_project_id,
            token_cache: tokio::sync::Mutex::new(None),
        }
    }
}

impl LocalGateway {
    fn has_google_credentials(&self) -> bool {
        self.service_account.is_some() || self.google_api_key.is_some()
    }

    /// Safety / portrait agents must never return prose mocks — callers parse structured output.
    fn requires_google_credentials(agent: &str) -> bool {
        matches!(
            agent.trim(),
            "face_capture_validate_v1"
                | "brand_logo_validate_v1"
                | "portrait_colour_card_v1"
                | "portrait_raster_v1"
        )
    }

    fn has_ai_studio_key(&self) -> bool {
        self.google_api_key
            .as_deref()
            .map(|k| k.trim().starts_with("AIza"))
            .unwrap_or(false)
    }

    fn max_output_tokens(agent: &str) -> u32 {
        match agent.trim() {
            "council_moderation_review_v1"
            |             "council_thread_reply_v1"
            | "council_content_share_v1"
            | "council_content_shortlist_v1"
            | "council_portfolio_replace_v1"
            | "council_invite_v1"
            | "council_invite_article_shortlist_v1"
            | "council_invite_person_discover_v1"
            | "council_godmother_welcome_v1"
            | "face_capture_validate_v1"
            | "brand_logo_validate_v1"
            | "portrait_colour_card_v1" => 256,
            _ => 300,
        }
    }

    fn apply_json_generation_config(agent_id: &str, generation_config: &mut Value) {
        let schema = match agent_id.trim() {
            "face_capture_validate_v1" | "brand_logo_validate_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "valid": { "type": "BOOLEAN" },
                    "reason_code": { "type": "STRING" }
                },
                "required": ["valid"]
            }),
            "portrait_colour_card_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "skin_tone": { "type": "STRING" },
                    "head_hair": { "type": "STRING" },
                    "facial_hair": { "type": "STRING" },
                    "eyeglasses": { "type": "STRING" },
                    "clothing": { "type": "STRING" }
                },
                "required": ["skin_tone", "head_hair", "facial_hair", "eyeglasses", "clothing"]
            }),
            "council_thread_reply_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "action": { "type": "STRING", "enum": ["reply", "decline"] },
                    "parent_post_id": { "type": "INTEGER" },
                    "body_markup": { "type": "STRING" },
                    "reason": { "type": "STRING" }
                },
                "required": ["action"]
            }),
            "council_content_share_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "action": { "type": "STRING", "enum": ["share", "decline"] },
                    "content_link_id": { "type": "INTEGER" },
                    "body_markup": { "type": "STRING" },
                    "reason": { "type": "STRING" }
                },
                "required": ["action"]
            }),
            "council_content_shortlist_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "action": { "type": "STRING", "enum": ["shortlist", "decline"] },
                    "content_link_ids": {
                        "type": "ARRAY",
                        "items": { "type": "INTEGER" }
                    },
                    "reason": { "type": "STRING" }
                },
                "required": ["action"]
            }),
            "council_portfolio_replace_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "action": { "type": "STRING", "enum": ["replace", "decline"] },
                    "proof_url": { "type": "STRING" },
                    "body_markup": { "type": "STRING" },
                    "reason": { "type": "STRING" }
                },
                "required": ["action"]
            }),
            "council_invite_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "action": { "type": "STRING", "enum": ["invite", "decline"] },
                    "intended_email": { "type": "STRING" },
                    "intended_label": { "type": "STRING" },
                    "personal_note": { "type": "STRING" },
                    "reason": { "type": "STRING" }
                },
                "required": ["action"]
            }),
            "council_invite_article_shortlist_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "action": { "type": "STRING", "enum": ["shortlist", "decline"] },
                    "post_ids": {
                        "type": "ARRAY",
                        "items": { "type": "INTEGER" }
                    },
                    "reason": { "type": "STRING" }
                },
                "required": ["action"]
            }),
            "council_godmother_welcome_v1" => serde_json::json!({
                "type": "OBJECT",
                "properties": {
                    "action": { "type": "STRING", "enum": ["welcome", "decline"] },
                    "body_markup": { "type": "STRING" },
                    "reason": { "type": "STRING" }
                },
                "required": ["action"]
            }),
            _ => return,
        };
        generation_config["responseMimeType"] = serde_json::json!("application/json");
        generation_config["responseSchema"] = schema;
    }

    fn generate_content_parts(req: &RunTextAgentRequest) -> Result<Vec<Value>, Status> {
        let agent_id = req.agent.trim();
        let prompt = if let Some(p) = req
            .metadata
            .get("text_prompt")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            p.to_string()
        } else if agent_id == "face_capture_validate_v1"
            || agent_id == "brand_logo_validate_v1"
            || agent_id == "portrait_colour_card_v1"
            || agent_id == "council_moderation_review_v1"
            || agent_id == "council_thread_reply_v1"
            || agent_id == "council_content_share_v1"
            || agent_id == "council_content_shortlist_v1"
            || agent_id == "council_portfolio_replace_v1"
            || agent_id == "council_invite_v1"
            || agent_id == "council_invite_article_shortlist_v1"
            || agent_id == "council_invite_person_discover_v1"
            || agent_id == "council_godmother_welcome_v1"
        {
            return Err(Status::invalid_argument(format!(
                "{agent_id} requires metadata text_prompt"
            )));
        } else {
            "Respond briefly and clearly based on provided metadata and images.".to_string()
        };

        let mut parts = vec![serde_json::json!({ "text": prompt })];
        let label_photos = agent_id == "face_capture_validate_v1";
        for img in &req.images {
            if label_photos {
                let label = img.label.trim();
                if !label.is_empty() {
                    parts.push(serde_json::json!({
                        "text": format!("Photo — {label} expression:")
                    }));
                }
            }
            parts.push(serde_json::json!({
                "inline_data": {
                    "mime_type": image_mime_type(&img.jpeg),
                    "data": base64::engine::general_purpose::STANDARD.encode(&img.jpeg)
                }
            }));
        }
        Ok(parts)
    }

    fn parse_generate_content_text(json: &Value) -> Result<String, Status> {
        json.get("candidates")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|cand| cand.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .and_then(|parts| {
                let mut merged = String::new();
                for p in parts {
                    if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                        if !merged.is_empty() {
                            merged.push(' ');
                        }
                        merged.push_str(t.trim());
                    }
                }
                if merged.trim().is_empty() {
                    None
                } else {
                    Some(merged)
                }
            })
            .ok_or_else(|| Status::internal("google api returned empty text"))
    }

    fn usage_metadata_from_json(json: &Value) -> HashMap<String, String> {
        let mut usage = HashMap::new();
        if let Some(um) = json.get("usageMetadata") {
            for key in [
                "promptTokenCount",
                "candidatesTokenCount",
                "totalTokenCount",
            ] {
                if let Some(v) = um.get(key) {
                    usage.insert(key.to_string(), v.to_string());
                }
            }
        }
        usage
    }

    async fn get_vertex_access_token(&self) -> Result<String, Status> {
        let mut guard = self.token_cache.lock().await;
        if let Some(ref c) = *guard {
            if std::time::Instant::now() < c.valid_until {
                return Ok(c.token.clone());
            }
        }
        let sa = self
            .service_account
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("Vertex service account not loaded"))?;
        let (token, expires_in) = vertex_auth::fetch_oauth_access_token(sa)
            .await
            .map_err(|e| Status::internal(format!("google oauth token: {e}")))?;
        let cached = vertex_auth::cache_from_oauth(token.clone(), expires_in);
        *guard = Some(cached);
        Ok(token)
    }

    async fn generate_vertex(
        &self,
        req: &RunTextAgentRequest,
    ) -> Result<(String, String, HashMap<String, String>), Status> {
        let project = self.vertex_project_id.as_deref().ok_or_else(|| {
            Status::failed_precondition(
                "Vertex project id missing: set GOOGLE_CLOUD_PROJECT or use a key file with project_id",
            )
        })?;

        let endpoint = vertex_auth::vertex_generate_content_url(
            project,
            &self.vertex_location,
            &self.google_model,
        );

        let parts = Self::generate_content_parts(req)?;
        let max_tokens = Self::max_output_tokens(&req.agent);
        let mut generation_config = serde_json::json!({
            "temperature": 0.2,
            "maxOutputTokens": max_tokens
        });
        Self::apply_json_generation_config(req.agent.trim(), &mut generation_config);
        let body = serde_json::json!({
            "contents": [{ "role": "user", "parts": parts }],
            "generationConfig": generation_config
        });

        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(self.http_timeout_ms))
            .build()
            .map_err(|e| Status::internal(format!("gateway http client build failed: {e}")))?;

        let token = self.get_vertex_access_token().await?;

        const MAX_429_RETRIES: u32 = 3;
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let resp = client
                .post(&endpoint)
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    Status::new(Code::Unavailable, format!("vertex api request failed: {e}"))
                })?;

            let status = resp.status();
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| Status::internal(format!("vertex api read body: {e}")))?;
            let json: Value = if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap_or(Value::Null)
            };

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_429_RETRIES {
                let detail = google_api_error_detail(&json, &bytes);
                let backoff_ms = u64::from(attempt) * 2_000;
                tracing::warn!(
                    %status,
                    attempt,
                    backoff_ms,
                    error_body = %detail,
                    "vertex generateContent rate limited; retrying"
                );
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                continue;
            }

            if !status.is_success() {
                let detail = google_api_error_detail(&json, &bytes);
                tracing::warn!(%status, error_body = %detail, "vertex generateContent failed");
                let hint = if status == StatusCode::FORBIDDEN {
                    " Hint: enable Vertex AI API on the project, grant the service account roles/aiplatform.user (Vertex AI User), and set VERTEX_LOCATION to a region where the model is available."
                } else {
                    ""
                };
                return Err(Status::new(
                    tonic_code_for_google_http(status),
                    format!("vertex api {status}: {detail}{hint}"),
                ));
            }

            let text = Self::parse_generate_content_text(&json)?;
            let usage = Self::usage_metadata_from_json(&json);

            return Ok((text.trim().to_string(), self.google_model.clone(), usage));
        }
    }

    async fn generate_generativelanguage(
        &self,
        req: &RunTextAgentRequest,
    ) -> Result<(String, String, HashMap<String, String>), Status> {
        let key = self.google_api_key.as_deref().ok_or_else(|| {
            Status::new(
                Code::Unavailable,
                "GOOGLE_AI_API_KEY is not configured for gateway",
            )
        })?;

        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.google_model, key
        );

        let parts = Self::generate_content_parts(req)?;
        let max_tokens = Self::max_output_tokens(&req.agent);
        let mut generation_config = serde_json::json!({
            "temperature": 0.2,
            "maxOutputTokens": max_tokens
        });
        Self::apply_json_generation_config(req.agent.trim(), &mut generation_config);
        let body = serde_json::json!({
            "contents": [{ "role": "user", "parts": parts }],
            "generationConfig": generation_config
        });

        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(self.http_timeout_ms))
            .build()
            .map_err(|e| Status::internal(format!("gateway http client build failed: {e}")))?;

        let resp = client
            .post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                Status::new(Code::Unavailable, format!("google api request failed: {e}"))
            })?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Status::internal(format!("google api read body: {e}")))?;
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };

        if !status.is_success() {
            let detail = google_api_error_detail(&json, &bytes);
            tracing::warn!(%status, error_body = %detail, "google generateContent failed");
            let hint = if status == StatusCode::FORBIDDEN || status == StatusCode::UNAUTHORIZED {
                " Hint: this endpoint requires a Google AI Studio key (starts with AIza). For Vertex, set GOOGLE_APPLICATION_CREDENTIALS + GOOGLE_CLOUD_PROJECT and use OAuth bearer tokens."
            } else {
                ""
            };
            return Err(Status::new(
                tonic_code_for_google_http(status),
                format!("google api {status}: {detail}{hint}"),
            ));
        }

        let text = Self::parse_generate_content_text(&json)?;
        let usage = Self::usage_metadata_from_json(&json);

        Ok((text.trim().to_string(), self.google_model.clone(), usage))
    }

    async fn generate_with_google(
        &self,
        req: &RunTextAgentRequest,
    ) -> Result<(String, String, HashMap<String, String>), Status> {
        vertex_rate_limit::acquire_provider_slot().await;
        if self.service_account.is_some() {
            return self.generate_vertex(req).await;
        }
        if self.has_ai_studio_key() {
            return self.generate_generativelanguage(req).await;
        }
        if self.google_api_key.is_some() {
            return Err(Status::failed_precondition(
                "GOOGLE_AI_API_KEY is set but is not an AI Studio key (expected prefix AIza). Set GOOGLE_APPLICATION_CREDENTIALS for Vertex, or replace the key with an AI Studio key.",
            ));
        }
        Err(Status::new(
            Code::Unavailable,
            "Configure GOOGLE_APPLICATION_CREDENTIALS (Vertex) or GOOGLE_AI_API_KEY (Google AI Studio)",
        ))
    }
}

#[tonic::async_trait]
impl AiAgentGateway for LocalGateway {
    async fn run_text_agent(
        &self,
        request: Request<RunTextAgentRequest>,
    ) -> Result<Response<RunTextAgentResponse>, Status> {
        let started = Instant::now();
        let req = request.into_inner();

        if req.agent.trim().is_empty() {
            return Err(Status::invalid_argument("agent is required"));
        }

        let mode = match ProcessingMode::try_from(req.mode).unwrap_or(ProcessingMode::ModeSync) {
            ProcessingMode::ModeAsync => "async",
            ProcessingMode::ModeSync | ProcessingMode::Unspecified => "sync",
        };

        let labels: Vec<String> = req
            .images
            .iter()
            .map(|img| img.label.trim())
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();

        let label_summary = if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join(", ")
        };

        let request_id = if req.request_id.trim().is_empty() {
            format!("req-{}", chrono::Utc::now().timestamp_millis())
        } else {
            req.request_id.trim().to_string()
        };

        let mut usage = HashMap::new();
        usage.insert("images_received".to_string(), req.images.len().to_string());
        usage.insert("metadata_keys".to_string(), req.metadata.len().to_string());
        usage.insert("mode".to_string(), mode.to_string());
        usage.insert("labels".to_string(), label_summary);

        let agent = req.agent.trim();
        let (output_text, model_id, provider, provider_usage) = if self.has_google_credentials() {
            let (text, model, p_usage) = self.generate_with_google(&req).await?;
            (text, model, "google".to_string(), p_usage)
        } else if Self::requires_google_credentials(agent) {
            return Err(Status::failed_precondition(
                "Configure GOOGLE_APPLICATION_CREDENTIALS (Vertex) or GOOGLE_AI_API_KEY (Google AI Studio) on ai_gateway — face validation cannot run without provider credentials",
            ));
        } else {
            tracing::warn!(
                agent = %req.agent,
                user_id = req.user_id,
                "No Google credentials; returning mock text (set GOOGLE_APPLICATION_CREDENTIALS for Vertex or GOOGLE_AI_API_KEY for AI Studio)"
            );
            (
                format!(
                    "Local gateway mock response for agent '{}' (user {}, images {}). Set GOOGLE_APPLICATION_CREDENTIALS (Vertex) or GOOGLE_AI_API_KEY (AI Studio) to enable real provider calls.",
                    req.agent, req.user_id, req.images.len()
                ),
                std::env::var("GATEWAY_MODEL_ID")
                    .unwrap_or_else(|_| "local-gateway-mock-v1".to_string()),
                std::env::var("GATEWAY_PROVIDER_ID")
                    .unwrap_or_else(|_| "local-mock".to_string()),
                HashMap::new(),
            )
        };
        usage.extend(provider_usage);

        let res = RunTextAgentResponse {
            output_text,
            model_id,
            request_id,
            agent: req.agent,
            provider,
            latency_ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
            usage,
        };

        Ok(Response::new(res))
    }

    async fn run_portrait_agent(
        &self,
        request: Request<RunPortraitAgentRequest>,
    ) -> Result<Response<RunPortraitAgentResponse>, Status> {
        let started = Instant::now();
        let req = request.into_inner();

        if req.user_id == 0 {
            return Err(Status::invalid_argument("user_id is required"));
        }

        let request_id = if req.request_id.trim().is_empty() {
            format!("portrait-{}", chrono::Utc::now().timestamp_millis())
        } else {
            req.request_id.trim().to_string()
        };

        let timeout_ms = if req.requested_timeout_ms == 0 {
            self.http_timeout_ms
        } else {
            u64::from(req.requested_timeout_ms)
        };

        vertex_rate_limit::acquire_provider_slot().await;

        let (png, model_id, provider) = if self.service_account.is_some() {
            let project = self.vertex_project_id.as_deref().ok_or_else(|| {
                Status::failed_precondition(
                    "Vertex project id missing: set GOOGLE_CLOUD_PROJECT or use a key file with project_id",
                )
            })?;
            let token = self.get_vertex_access_token().await?;
            let (png, model) = portrait::generate_portrait(
                &req,
                project,
                &self.vertex_location,
                &token,
                timeout_ms,
            )
            .await?;
            (png, model, "vertex".to_string())
        } else {
            return Err(Status::failed_precondition(
                "Portrait raster requires GOOGLE_APPLICATION_CREDENTIALS (Vertex Imagen)",
            ));
        };

        let res = RunPortraitAgentResponse {
            png,
            model_id,
            request_id,
            provider,
            latency_ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
        };

        Ok(Response::new(res))
    }
}

fn image_mime_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes[8..12] == *b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let host: IpAddr = env_or("GATEWAY_HOST", "127.0.0.1".parse()?);
    let port: u16 = env_or("GATEWAY_PORT", 50051);
    let addr = SocketAddr::new(host, port);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let has_google_key = std::env::var("GOOGLE_AI_API_KEY")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if has_google_key {
        let k = std::env::var("GOOGLE_AI_API_KEY").unwrap_or_default();
        let t = k.trim();
        if !t.starts_with("AIza") {
            tracing::warn!(
                "GOOGLE_AI_API_KEY does not start with \"AIza\" — generativelanguage.googleapis.com expects a Google AI Studio key; Vertex-only keys often return HTTP 403."
            );
        }
    }
    let has_vertex_creds = std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let (vertex_max_rpm, vertex_burst) = vertex_rate_limit::limit_summary();
    tracing::info!(
        %addr,
        google_api_key_configured = has_google_key,
        vertex_credentials_path_set = has_vertex_creds,
        vertex_gateway_max_rpm = vertex_max_rpm,
        vertex_gateway_burst = vertex_burst,
        "ai gateway listening (Vertex: GOOGLE_APPLICATION_CREDENTIALS + VERTEX_LOCATION; or AI Studio: GOOGLE_AI_API_KEY)"
    );

    Server::builder()
        .add_service(
            AiAgentGatewayServer::new(LocalGateway::default())
                .max_decoding_message_size(16 * 1024 * 1024)
                .max_encoding_message_size(16 * 1024 * 1024),
        )
        .serve(addr)
        .await?;

    Ok(())
}
