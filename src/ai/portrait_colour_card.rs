//! Vision step: read neutral face capture → structured colour card for Imagen prompt injection.

use std::collections::HashMap;

use serde::Deserialize;

use crate::ai::grpc::{self, PORTRAIT_COLOUR_CARD_AGENT};
use crate::config::Config;

const PORTRAIT_COLOUR_CARD_TIMEOUT_MS: u64 = 15_000;

/// Structured appearance anchors extracted from the neutral capture before Imagen.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PortraitColourCard {
    pub skin_tone: String,
    pub head_hair: String,
    pub facial_hair: String,
    pub eyeglasses: String,
    #[serde(default)]
    pub clothing: String,
}

impl PortraitColourCard {
    /// Short label for Imagen `[1]` subjectDescription.
    #[must_use]
    pub fn imagen_subject_label(&self) -> String {
        format!(
            "person with {skin} skin, {head} head hair, {face} facial hair, {glasses}",
            skin = self.skin_tone.trim(),
            head = self.head_hair.trim(),
            face = self.facial_hair.trim(),
            glasses = self.eyeglasses.trim(),
        )
    }

    /// Compact colour-card block injected into the portrait generation prompt.
    #[must_use]
    pub fn prompt_block(&self) -> String {
        let clothing_line = if self.clothing.trim().is_empty()
            || self.clothing.eq_ignore_ascii_case("not visible")
            || self.clothing.eq_ignore_ascii_case("none visible")
        {
            String::new()
        } else {
            format!("\n- Clothing: {}", self.clothing.trim())
        };

        format!(
            "\nIDENTITY COLOUR CARD (from [1]):\n\
             - Skin tone: {skin}\n\
             - Head hair: {head}\n\
             - Facial hair: {face}\n\
             - Eyeglasses: {glasses}{clothing}\n\
             Reproduce exactly; head hair and facial hair are independent colours.",
            skin = self.skin_tone.trim(),
            head = self.head_hair.trim(),
            face = self.facial_hair.trim(),
            glasses = self.eyeglasses.trim(),
            clothing = clothing_line,
        )
    }
}

/// Extract a colour card from the neutral JPEG. Returns `None` when mock mode is on or vision fails.
pub async fn extract_portrait_colour_card(
    config: &Config,
    user_id: u64,
    neutral_jpeg: &[u8],
) -> Option<PortraitColourCard> {
    if config.face_vision_mock {
        return None;
    }

    let mut metadata = HashMap::new();
    metadata.insert(
        "text_prompt".to_string(),
        crate::agent_specs::compose_portrait_colour_card_prompt(),
    );

    let timeout_ms = config
        .ai_grpc_timeout_ms
        .max(PORTRAIT_COLOUR_CARD_TIMEOUT_MS);

    let images = vec![("neutral".to_string(), neutral_jpeg.to_vec())];

    let (text, _) = match grpc::run_text_agent(
        &config.ai_grpc_endpoint,
        timeout_ms,
        PORTRAIT_COLOUR_CARD_AGENT,
        user_id,
        images,
        metadata,
    )
    .await
    {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(error = %err, "portrait colour card vision call failed; continuing without card");
            return None;
        }
    };

    match parse_colour_card_response(&text) {
        Ok(card) => {
            tracing::info!(
                skin = %card.skin_tone,
                head_hair = %card.head_hair,
                facial_hair = %card.facial_hair,
                eyeglasses = %card.eyeglasses,
                "portrait colour card extracted"
            );
            Some(card)
        }
        Err(err) => {
            tracing::warn!(error = %err, raw = %text, "portrait colour card parse failed; continuing without card");
            None
        }
    }
}

fn parse_colour_card_response(raw: &str) -> Result<PortraitColourCard, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty colour card response".into());
    }

    if let Ok(card) = serde_json::from_str::<PortraitColourCard>(trimmed) {
        return validate_colour_card(card);
    }

    if let Some(json) = extract_json_object(trimmed) {
        let card: PortraitColourCard = serde_json::from_str(json)
            .map_err(|e| format!("colour card json parse: {e}"))?;
        return validate_colour_card(card);
    }

    Err("colour card response missing json object".into())
}

fn validate_colour_card(card: PortraitColourCard) -> Result<PortraitColourCard, String> {
    if card.skin_tone.trim().is_empty() {
        return Err("colour card missing skin_tone".into());
    }
    if card.head_hair.trim().is_empty() {
        return Err("colour card missing head_hair".into());
    }
    if card.facial_hair.trim().is_empty() {
        return Err("colour card missing facial_hair".into());
    }
    if card.eyeglasses.trim().is_empty() {
        return Err("colour card missing eyeglasses".into());
    }
    Ok(card)
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&raw[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colour_card_json() {
        let card = parse_colour_card_response(
            r#"{"skin_tone":"fair pink-beige","head_hair":"strawberry blonde swept back","facial_hair":"sandy grey-white full beard","eyeglasses":"rectangular dark frames","clothing":"grey v-neck collar"}"#,
        )
        .expect("parse");
        assert_eq!(card.skin_tone, "fair pink-beige");
        assert!(card.prompt_block().contains("strawberry blonde"));
        assert!(card.prompt_block().contains("grey-white"));
        assert!(!card.prompt_block().contains("GLASSES (mandatory"));
    }

    #[test]
    fn imagen_subject_label_is_concise() {
        let card = parse_colour_card_response(
            r#"{"skin_tone":"fair pink-beige","head_hair":"strawberry blonde","facial_hair":"grey-white beard","eyeglasses":"rectangular dark frames","clothing":""}"#,
        )
        .expect("parse");
        let label = card.imagen_subject_label();
        assert!(label.contains("strawberry blonde"));
        assert!(label.contains("rectangular dark frames"));
        assert!(label.len() < 200);
    }

    #[test]
    fn rejects_missing_skin_tone() {
        assert!(parse_colour_card_response(
            r#"{"head_hair":"brown","facial_hair":"none","eyeglasses":"none visible"}"#
        )
        .is_err());
    }

    #[test]
    fn parses_json_in_fenced_prose() {
        let card = parse_colour_card_response(
            "Result:\n```json\n{\"skin_tone\":\"light\",\"head_hair\":\"brown\",\"facial_hair\":\"grey stubble\",\"eyeglasses\":\"none visible\",\"clothing\":\"not visible\"}\n```",
        )
        .expect("parse fenced");
        assert_eq!(card.eyeglasses, "none visible");
    }

    #[test]
    fn prompt_block_omits_empty_clothing() {
        let card = PortraitColourCard {
            skin_tone: "fair".into(),
            head_hair: "blonde".into(),
            facial_hair: "none".into(),
            eyeglasses: "none visible".into(),
            clothing: "not visible".into(),
        };
        assert!(!card.prompt_block().contains("Clothing:"));
    }
}
