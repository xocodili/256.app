use std::collections::HashMap;
use std::time::Duration;

use tonic::transport::{Channel, Endpoint};
use tonic::Request;

use crate::ai::portraiture::PortraitureFacePhotos;

pub const MOCK_AI_MODEL_ID: &str = "mock-pending-agent";
/// Vision: three face photos → JSON accept/reject before portraiture (`face_capture_validate`).
pub const FACE_CAPTURE_VALIDATE_AGENT: &str = "face_capture_validate_v1";
/// Vision: brand logo image → JSON accept/reject before profile display (`brand_logo_validate`).
pub const BRAND_LOGO_VALIDATE_AGENT: &str = "brand_logo_validate_v1";
/// Council moderation appeal review (`compose_council_moderation_review_prompt` via metadata `text_prompt`).
pub const COUNCIL_MODERATION_REVIEW_AGENT: &str = "council_moderation_review_v1";
/// Vision: neutral face photo → JSON colour card before Imagen (`portrait_colour_card`).
pub const PORTRAIT_COLOUR_CARD_AGENT: &str = "portrait_colour_card_v1";
/// Imagen subject-customization portrait raster (Phase 1: routed through gateway rate limit).
pub const PORTRAIT_RASTER_AGENT: &str = "portrait_raster_v1";

/// Portrait requests carry multiple JPEGs + style plate; responses return a PNG raster.
const PORTRAIT_GRPC_MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

pub mod pb {
    tonic::include_proto!("aiagents.v1");
}

#[derive(Debug)]
pub enum AiClientError {
    Transport(String),
    Timeout,
    Rpc(String),
    EmptyResponse,
}

impl std::fmt::Display for AiClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiClientError::Transport(s) => write!(f, "{s}"),
            AiClientError::Timeout => write!(f, "grpc timeout"),
            AiClientError::Rpc(s) => write!(f, "{s}"),
            AiClientError::EmptyResponse => write!(f, "empty ai response"),
        }
    }
}

#[must_use]
pub fn is_transient_portrait_rpc_error(err: &AiClientError) -> bool {
    match err {
        AiClientError::Transport(_) | AiClientError::Timeout => true,
        AiClientError::Rpc(msg) => crate::social::face_capture_errors::is_portrait_infrastructure_error(msg),
        AiClientError::EmptyResponse => false,
    }
}

fn portrait_endpoint(endpoint: &str, grpc_timeout_ms: u64) -> Result<Endpoint, AiClientError> {
    let timeout = Duration::from_millis(grpc_timeout_ms);
    Ok(Endpoint::from_shared(endpoint.to_string())
        .map_err(|e| AiClientError::Transport(e.to_string()))?
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(15))
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .http2_keep_alive_interval(Duration::from_secs(30))
        .keep_alive_timeout(Duration::from_secs(20))
        .keep_alive_while_idle(true))
}

/// Smoke test helper: validates transport connectivity to the AI gateway.
pub async fn check_gateway(endpoint: &str, timeout_ms: u64) -> Result<(), AiClientError> {
    let timeout = Duration::from_millis(timeout_ms);
    let ep = Endpoint::from_shared(endpoint.to_string())
        .map_err(|e| AiClientError::Transport(e.to_string()))?
        .timeout(timeout);

    ep.connect()
        .await
        .map_err(|e| AiClientError::Transport(e.to_string()))?;
    Ok(())
}

pub async fn run_text_agent(
    endpoint: &str,
    timeout_ms: u64,
    agent: &str,
    user_id: u64,
    images: Vec<(String, Vec<u8>)>,
    metadata: HashMap<String, String>,
) -> Result<(String, String), AiClientError> {
    let timeout = Duration::from_millis(timeout_ms);
    let ep = Endpoint::from_shared(endpoint.to_string())
        .map_err(|e| AiClientError::Transport(e.to_string()))?
        .timeout(timeout);

    let channel: Channel = ep
        .connect()
        .await
        .map_err(|e| AiClientError::Transport(e.to_string()))?;
    let mut client = pb::ai_agent_gateway_client::AiAgentGatewayClient::new(channel);

    let req = pb::RunTextAgentRequest {
        agent: agent.to_string(),
        user_id,
        images: images
            .into_iter()
            .map(|(label, jpeg)| pb::ImageFrame { label, jpeg })
            .collect(),
        metadata,
        request_id: String::new(),
        mode: pb::ProcessingMode::ModeSync as i32,
        requested_timeout_ms: 0,
    };

    let resp = tokio::time::timeout(timeout, client.run_text_agent(Request::new(req)))
        .await
        .map_err(|_| AiClientError::Timeout)?
        .map_err(|e| AiClientError::Rpc(e.to_string()))?
        .into_inner();

    let txt = resp.output_text.trim().to_string();
    if txt.is_empty() {
        return Err(AiClientError::EmptyResponse);
    }
    let model_id = if resp.model_id.trim().is_empty() {
        "grpc-agent".to_string()
    } else {
        resp.model_id
    };
    Ok((txt, model_id))
}

pub async fn run_portrait_agent(
    endpoint: &str,
    grpc_timeout_ms: u64,
    predict_timeout_ms: u64,
    user_id: u64,
    council_handle: &str,
    photos: &PortraitureFacePhotos,
    style_reference_png: Option<Vec<u8>>,
    prompt: &str,
    parameters_json: &str,
    model_id: &str,
    subject_label: &str,
) -> Result<Vec<u8>, AiClientError> {
    let style_reference_png = style_reference_png.clone();
    match run_portrait_agent_once(
        endpoint,
        grpc_timeout_ms,
        predict_timeout_ms,
        user_id,
        council_handle,
        photos,
        style_reference_png.clone(),
        prompt,
        parameters_json,
        model_id,
        subject_label,
    )
    .await
    {
        Ok(png) => Ok(png),
        Err(first) if is_transient_portrait_rpc_error(&first) => {
            tracing::warn!(error = %first, "portrait gRPC transient failure; retrying once");
            tokio::time::sleep(Duration::from_secs(2)).await;
            run_portrait_agent_once(
                endpoint,
                grpc_timeout_ms,
                predict_timeout_ms,
                user_id,
                council_handle,
                photos,
                style_reference_png,
                prompt,
                parameters_json,
                model_id,
                subject_label,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

async fn run_portrait_agent_once(
    endpoint: &str,
    grpc_timeout_ms: u64,
    predict_timeout_ms: u64,
    user_id: u64,
    council_handle: &str,
    photos: &PortraitureFacePhotos,
    style_reference_png: Option<Vec<u8>>,
    prompt: &str,
    parameters_json: &str,
    model_id: &str,
    subject_label: &str,
) -> Result<Vec<u8>, AiClientError> {
    let timeout = Duration::from_millis(grpc_timeout_ms);
    let ep = portrait_endpoint(endpoint, grpc_timeout_ms)?;

    let channel: Channel = ep
        .connect()
        .await
        .map_err(|e| AiClientError::Transport(e.to_string()))?;
    let mut client = pb::ai_agent_gateway_client::AiAgentGatewayClient::new(channel)
        .max_decoding_message_size(PORTRAIT_GRPC_MAX_MESSAGE_BYTES)
        .max_encoding_message_size(PORTRAIT_GRPC_MAX_MESSAGE_BYTES);

    let mut images = vec![
        ("neutral".to_string(), photos.neutral.clone()),
        ("smile".to_string(), photos.smile.clone()),
        ("frown".to_string(), photos.frown.clone()),
    ];
    if let Some(style_png) = style_reference_png {
        images.push(("style".to_string(), style_png));
    }

    let req = pb::RunPortraitAgentRequest {
        user_id,
        council_handle: council_handle.to_string(),
        images: images
            .into_iter()
            .map(|(label, jpeg)| pb::ImageFrame { label, jpeg })
            .collect(),
        model_id: model_id.to_string(),
        prompt: prompt.to_string(),
        parameters_json: parameters_json.to_string(),
        request_id: String::new(),
        requested_timeout_ms: predict_timeout_ms.min(u32::MAX as u64) as u32,
        subject_description_suffix: subject_label.to_string(),
    };

    let resp = tokio::time::timeout(timeout, client.run_portrait_agent(Request::new(req)))
        .await
        .map_err(|_| AiClientError::Timeout)?
        .map_err(|e| AiClientError::Rpc(e.to_string()))?
        .into_inner();

    if resp.png.is_empty() {
        return Err(AiClientError::EmptyResponse);
    }
    Ok(resp.png)
}
