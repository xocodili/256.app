//! Council AI profiles on disk: `agents/council/{handle}.json`.
//! Shared by the web app (portrait raster pipeline) and `ai_gateway` (appearance prompts).
//!
//! ## Configuration layout
//!
//! Each file may include `ai_configuration`:
//! - **`avc`** — Archetype–Voice–Constraint persona (repeatable template): functional
//!   title, archetype, core directive, **`voice_tone`**, **`constraint`**, and **`epistemic_stance`**.
//!   When any field is set, the block is injected into prompts (empty slots show as `(TBD)` until filled).
//! - **`business_functions`** — keyed by function id (e.g. `avatar_creation`, `council_moderation_review`):
//!   - **`text_prompt_template`** — multiline scaffold; placeholders depend on the function. Empty uses the
//!     shared default for that function (see `agents/council/shared/*.txt` compiled via `include_str!`).
//!   - **`rag_documents`** — repo-relative `.txt`/`.md` paths loaded into `{rag_context}` at prompt time.
//!   - **`portrait_synthesis`** (avatar_creation only) — Imagen parameters, style plate, optional direction lines.
//!
//! Shared task guides live under `agents/council/shared/rag/`; council-specific guides under
//! `agents/council/rag/{handle}/`. Epic alone owns `avatar_creation` (onboarding portraits).
//!
//! All twelve council AIs are modeled as one cooperative body aligned with 256.app; see the
//! collective line in [`format_avc_block`].
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::Value as JsonValue;

/// `business_functions` key for AI council review of a post-dispute appeal (escalation after human jury).
/// Not used for constitutional amendments (human jury only).
pub const COUNCIL_MODERATION_REVIEW_BF_KEY: &str = "council_moderation_review";
pub const COUNCIL_THREAD_REPLY_BF_KEY: &str = "council_thread_reply";
pub const COUNCIL_CONTENT_SHARE_BF_KEY: &str = "council_content_share";
pub const COUNCIL_CONTENT_SHORTLIST_BF_KEY: &str = "council_content_shortlist";
pub const COUNCIL_PORTFOLIO_REPLACE_BF_KEY: &str = "council_portfolio_replace";
pub const COUNCIL_INVITE_BF_KEY: &str = "council_invite";
pub const COUNCIL_INVITE_ARTICLE_SHORTLIST_BF_KEY: &str = "council_invite_article_shortlist";
pub const COUNCIL_INVITE_PERSON_DISCOVER_BF_KEY: &str = "council_invite_person_discover";
pub const COUNCIL_GODMOTHER_WELCOME_BF_KEY: &str = "council_godmother_welcome";
pub const COUNCIL_GODMOTHER_GLOBAL_FAILING_BF_KEY: &str = "council_godmother_global_failing";
pub const COUNCIL_GODMOTHER_GLOBAL_RECOVERY_BF_KEY: &str = "council_godmother_global_recovery";
/// Vision gate: three face photos → JSON accept/reject before portraiture.
pub const FACE_CAPTURE_VALIDATE_BF_KEY: &str = "face_capture_validate";
/// Vision gate: brand logo image → JSON accept/reject before profile display.
pub const BRAND_LOGO_VALIDATE_BF_KEY: &str = "brand_logo_validate";
/// Vision step: neutral face photo → JSON colour card before Imagen portraiture.
pub const PORTRAIT_COLOUR_CARD_BF_KEY: &str = "portrait_colour_card";
/// Raster step: Epic onboarding bust portrait (`avatar_creation` business function only).
pub const AVATAR_CREATION_BF_KEY: &str = "avatar_creation";

#[derive(Debug, Deserialize)]
pub struct CouncilAgentFile {
    pub agent_id: String,
    pub display_name: String,
    #[serde(default)]
    pub ai_configuration: AiConfiguration,
    /// Legacy root-level portrait config; prefer `business_functions.avatar_creation.portrait_synthesis`.
    #[serde(rename = "portrait_synthesis", alias = "vertex_image", default)]
    pub portrait_synthesis: Option<PortraitStyleSpec>,
}

/// Top-level council AI configuration: AVC persona layer, then per–business-function prompts and RAG refs.
#[derive(Debug, Deserialize, Default)]
pub struct AiConfiguration {
    /// Archetype–Voice–Constraint (AVC) repeatable persona template.
    #[serde(default)]
    pub avc: ArchetypeVoiceConstraint,
    #[serde(default)]
    pub business_functions: HashMap<String, BusinessFunctionSpec>,
}

/// Soul vs role: archetype, directive, voice & tone, operating constraint, and epistemic stance.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArchetypeVoiceConstraint {
    /// Line after display name, e.g. "Physical Health & Fitness".
    #[serde(default)]
    pub functional_title: String,
    #[serde(default)]
    pub archetype: String,
    /// One sentence: primary motivation in the demarchy / on 256.
    #[serde(default)]
    pub core_directive: String,
    /// Voice & tone (vocabulary, pacing, metaphors — editorial paragraph).
    #[serde(default)]
    pub voice_tone: String,
    /// Operating constraint (how this AI bounds its reasoning).
    #[serde(default)]
    pub constraint: String,
    #[serde(default)]
    pub epistemic_stance: String,
    /// Legacy nested bag from earlier schema; merged into rendering if flat `voice_tone` / `constraint` / `epistemic_stance` are empty.
    #[serde(default)]
    pub voice_and_tone: VoiceToneAttributes,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct VoiceToneAttributes {
    #[serde(default)]
    pub vocabulary: String,
    #[serde(default)]
    pub pacing: String,
    #[serde(default)]
    pub quirks: String,
    #[serde(default)]
    pub epistemic_stance: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct BusinessFunctionSpec {
    /// Multiline text prompt scaffold for this business function (placeholders are function-specific).
    /// Portrait generation uses `{avc_block}`, direction fields, `{rag_context}`.
    #[serde(default, alias = "imagen_prompt_template", alias = "prompt")]
    pub text_prompt_template: String,
    #[serde(default)]
    pub rag_documents: Vec<RagDocumentRef>,
    /// Imagen portrait task config (`avatar_creation` on `@epic` only).
    #[serde(default)]
    pub portrait_synthesis: Option<PortraitStyleSpec>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RagDocumentRef {
    #[serde(default)]
    pub id: String,
    /// Repo-relative path (e.g. markdown or manifest under `agents/council/rag/{handle}/`).
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct PortraitStyleSpec {
    #[serde(default)]
    pub perspective_direction: String,
    #[serde(default)]
    pub style_direction: String,
    #[serde(default)]
    pub color_direction: String,
    #[serde(default = "default_portrait_parameters")]
    pub parameters: JsonValue,
    /// Repo-relative path to a PNG style plate (Imagen `REFERENCE_TYPE_STYLE` `[3]`).
    #[serde(default)]
    pub style_reference_image: Option<String>,
}

fn default_portrait_parameters() -> JsonValue {
    JsonValue::Object(serde_json::Map::new())
}

/// Runtime strings for a moderation-appeal / council escalation prompt (no live URLs — use stored evidence text).
#[derive(Debug, Clone)]
pub struct CouncilModerationReviewContext<'a> {
    pub community_name: &'a str,
    pub target_content: &'a str,
    pub constitution_context: &'a str,
    pub jury_deliberation: &'a str,
    /// Summary of immutable captured URL evidence (`judicial_case_evidence`), or `"None"`.
    pub scraped_evidence_summary: &'a str,
}

/// Fallback bust-portrait scaffold when the portrait business function template is missing or blank.
///
/// Placeholders: `{avc_block}` (optional persona preamble), `{style_reference_block}`,
/// `{portrait_synthesis_block}` (non-empty JSON direction fields), `{rag_context}`.
///
/// Canonical copy for editors: `agents/council/shared/default_portrait_generation_prompt_template.txt`.
const DEFAULT_PORTRAIT_GENERATION_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_portrait_generation_prompt_template.txt");

/// Default council moderation-review mandate when per-agent JSON omits a custom template.
const DEFAULT_COUNCIL_MODERATION_REVIEW_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_moderation_review_prompt_template.txt");
const DEFAULT_COUNCIL_THREAD_REPLY_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_thread_reply_prompt_template.txt");
const DEFAULT_COUNCIL_CONTENT_SHARE_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_content_share_prompt_template.txt");
const DEFAULT_COUNCIL_CONTENT_SHORTLIST_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_content_shortlist_prompt_template.txt");
const DEFAULT_COUNCIL_PORTFOLIO_REPLACE_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_portfolio_replace_prompt_template.txt");
const DEFAULT_COUNCIL_INVITE_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_invite_prompt_template.txt");
const DEFAULT_COUNCIL_INVITE_ARTICLE_SHORTLIST_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_invite_article_shortlist_prompt_template.txt");
const DEFAULT_COUNCIL_INVITE_PERSON_DISCOVER_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_invite_person_discover_prompt_template.txt");
const DEFAULT_COUNCIL_GODMOTHER_WELCOME_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_godmother_welcome_prompt_template.txt");
const DEFAULT_COUNCIL_GODMOTHER_GLOBAL_FAILING_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_godmother_global_failing_prompt_template.txt");
const DEFAULT_COUNCIL_GODMOTHER_GLOBAL_RECOVERY_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_council_godmother_global_recovery_prompt_template.txt");

const DEFAULT_FACE_CAPTURE_VALIDATE_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_face_capture_validate_prompt_template.txt");
const DEFAULT_BRAND_LOGO_VALIDATE_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_brand_logo_validate_prompt_template.txt");
const DEFAULT_PORTRAIT_COLOUR_CARD_PROMPT_TEMPLATE: &str =
    include_str!("../agents/council/shared/default_portrait_colour_card_prompt_template.txt");

#[must_use]
pub fn council_agents_dir() -> PathBuf {
    crate::app_root::runtime_app_root().join("agents/council")
}

pub fn load_council_agent(handle: &str) -> Result<CouncilAgentFile, String> {
    let h = normalize_council_handle(handle)?;
    let path = council_agents_dir().join(format!("{h}.json"));
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("read council agent file {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn normalize_council_handle(handle: &str) -> Result<String, String> {
    let h = handle.trim().to_lowercase();
    if h.is_empty() {
        return Err("council agent handle is empty".to_string());
    }
    if !h
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!(
            "invalid council agent handle {h:?} (expected [a-z0-9_])"
        ));
    }
    Ok(h)
}

/// Imagen portrait task config for [`AVATAR_CREATION_BF_KEY`] (Epic only in production).
pub fn portrait_synthesis_spec(agent: &CouncilAgentFile) -> Result<&PortraitStyleSpec, String> {
    if let Some(bf) = agent
        .ai_configuration
        .business_functions
        .get(AVATAR_CREATION_BF_KEY)
    {
        if let Some(ref ps) = bf.portrait_synthesis {
            return Ok(ps);
        }
    }
    agent
        .portrait_synthesis
        .as_ref()
        .ok_or_else(|| {
            format!(
                "agent {} has no avatar_creation.portrait_synthesis",
                agent.agent_id
            )
        })
}

impl ArchetypeVoiceConstraint {
    fn any_field_set(&self) -> bool {
        let v = &self.voice_and_tone;
        !self.functional_title.trim().is_empty()
            || !self.archetype.trim().is_empty()
            || !self.core_directive.trim().is_empty()
            || !self.voice_tone.trim().is_empty()
            || !self.constraint.trim().is_empty()
            || !self.epistemic_stance.trim().is_empty()
            || !v.vocabulary.trim().is_empty()
            || !v.pacing.trim().is_empty()
            || !v.quirks.trim().is_empty()
            || !v.epistemic_stance.trim().is_empty()
    }

    fn voice_tone_resolved(&self) -> String {
        if !self.voice_tone.trim().is_empty() {
            return self.voice_tone.trim().to_string();
        }
        let v = &self.voice_and_tone;
        let mut parts: Vec<&str> = Vec::new();
        for s in [&v.vocabulary, &v.pacing, &v.quirks] {
            let t = s.trim();
            if !t.is_empty() {
                parts.push(t);
            }
        }
        parts.join(" ").trim().to_string()
    }

    fn constraint_resolved(&self) -> String {
        if !self.constraint.trim().is_empty() {
            return self.constraint.trim().to_string();
        }
        self.voice_and_tone.quirks.trim().to_string()
    }

    fn epistemic_resolved(&self) -> String {
        if !self.epistemic_stance.trim().is_empty() {
            return self.epistemic_stance.trim().to_string();
        }
        self.voice_and_tone.epistemic_stance.trim().to_string()
    }
}

fn tbd(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        "(TBD)".to_string()
    } else {
        t.to_string()
    }
}

/// Hierarchical AVC block for council prompts. Returns `None` if no AVC field is configured yet.
#[must_use]
pub fn format_avc_block(agent: &CouncilAgentFile) -> Option<String> {
    let avc = &agent.ai_configuration.avc;
    if !avc.any_field_set() {
        return None;
    }
    let name = agent.display_name.trim();
    let voice_raw = avc.voice_tone_resolved();
    let con_raw = avc.constraint_resolved();
    let epist_raw = avc.epistemic_resolved();
    Some(format!(
        "{name} - {title}\n\
Archetype: {arch}\n\
Core Directive: {core}\n\
\n\
Voice & Tone:\n\
{voice}\n\
\n\
Constraint:\n\
{con}\n\
\n\
Epistemic Stance:\n\
{epist}\n\
\n\
Stewardship: You moderate exactly one steward community on 256.app (see mandate blocks below). When posting or replying, address that community's members—not \"everyone on 256.app\" or the platform at large.\n\
Collective context: The twelve council AIs collaborate on governance and trust, but each speaks publicly only as steward of their own flock.\n\
\n",
        name = if name.is_empty() { "(TBD)" } else { name },
        title = tbd(&avc.functional_title),
        arch = tbd(&avc.archetype),
        core = tbd(&avc.core_directive),
        voice = tbd(&voice_raw),
        con = tbd(&con_raw),
        epist = tbd(&epist_raw),
    ))
}

const MAX_RAG_DOC_CHARS: usize = 4_500;
const MAX_RAG_TOTAL_CHARS: usize = 9_000;

fn rag_text_extensions() -> &'static [&'static str] {
    &["txt", "md"]
}

/// Resolve a repo-relative path and ensure it stays under [`crate::app_root::runtime_app_root`].
fn resolve_path_under_app_root(rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Err("path is empty".to_string());
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(format!("path must be relative to app root: {rel:?}"));
    }

    let root = crate::app_root::runtime_app_root();
    let root_canon = root
        .canonicalize()
        .map_err(|e| format!("resolve app root {}: {e}", root.display()))?;

    let mut joined = root_canon.clone();
    for component in rel_path.components() {
        match component {
            Component::Normal(part) => joined.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !joined.pop() {
                    return Err(format!("path escapes app root: {rel:?}"));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("path must be relative to app root: {rel:?}"));
            }
        }
    }

    let resolved = if joined.exists() {
        joined
            .canonicalize()
            .map_err(|e| format!("resolve path {}: {e}", joined.display()))?
    } else {
        joined
    };

    if !resolved.starts_with(&root_canon) {
        return Err(format!("path escapes app root: {rel:?}"));
    }

    Ok(resolved)
}

fn load_rag_document_body(path: &str) -> Option<String> {
    let full = resolve_path_under_app_root(path).ok()?;
    let ext = full.extension()?.to_str()?.to_ascii_lowercase();
    if !rag_text_extensions().contains(&ext.as_str()) {
        return None;
    }
    let raw = std::fs::read_to_string(&full).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(if trimmed.len() > MAX_RAG_DOC_CHARS {
        format!("{}…", &trimmed[..MAX_RAG_DOC_CHARS])
    } else {
        trimmed.to_string()
    })
}

fn format_rag_context_block(docs: &[RagDocumentRef]) -> String {
    let mut sections: Vec<String> = Vec::new();
    let mut total = 0usize;

    for d in docs {
        if total >= MAX_RAG_TOTAL_CHARS {
            break;
        }
        let p = d.path.trim();
        if p.is_empty() {
            continue;
        }
        if let Some(body) = load_rag_document_body(p) {
            let id = d.id.trim();
            let header = if id.is_empty() {
                String::new()
            } else {
                format!("[{id}]\n")
            };
            let chunk = format!("{header}{body}");
            let take = chunk.len().min(MAX_RAG_TOTAL_CHARS.saturating_sub(total));
            if take == 0 {
                break;
            }
            sections.push(chunk[..take].to_string());
            total += take;
        }
    }

    if sections.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nReference material for this task:\n{}",
            sections.join("\n\n")
        )
    }
}

fn default_shared_rag_filename(bf_key: &str) -> Option<&'static str> {
    match bf_key {
        COUNCIL_MODERATION_REVIEW_BF_KEY => Some("moderation_appeal_guide.txt"),
        COUNCIL_THREAD_REPLY_BF_KEY => Some("council_thread_reply_guide.txt"),
        COUNCIL_CONTENT_SHARE_BF_KEY => Some("council_content_share_guide.txt"),
        COUNCIL_CONTENT_SHORTLIST_BF_KEY => Some("council_content_shortlist_guide.txt"),
        COUNCIL_PORTFOLIO_REPLACE_BF_KEY => Some("council_portfolio_replace_guide.txt"),
        COUNCIL_INVITE_BF_KEY => Some("council_invite_guide.txt"),
        COUNCIL_INVITE_ARTICLE_SHORTLIST_BF_KEY => Some("council_invite_article_shortlist_guide.txt"),
        COUNCIL_INVITE_PERSON_DISCOVER_BF_KEY => Some("council_invite_person_discover_guide.txt"),
        COUNCIL_GODMOTHER_WELCOME_BF_KEY => Some("godmother_welcome_guide.txt"),
        COUNCIL_GODMOTHER_GLOBAL_FAILING_BF_KEY => Some("godmother_global_failing_guide.txt"),
        COUNCIL_GODMOTHER_GLOBAL_RECOVERY_BF_KEY => Some("godmother_global_recovery_guide.txt"),
        FACE_CAPTURE_VALIDATE_BF_KEY => Some("face_capture_validate_guide.txt"),
        BRAND_LOGO_VALIDATE_BF_KEY => Some("brand_logo_validate_guide.txt"),
        PORTRAIT_COLOUR_CARD_BF_KEY => Some("portrait_colour_card_guide.txt"),
        AVATAR_CREATION_BF_KEY => None,
        _ => None,
    }
}

fn rag_block_for_bf(ai: &AiConfiguration, bf_key: &str) -> String {
    if let Some(bf) = ai.business_functions.get(bf_key) {
        if !bf.rag_documents.is_empty() {
            return format_rag_context_block(&bf.rag_documents);
        }
    }
    if let Some(file) = default_shared_rag_filename(bf_key) {
        let path = format!("agents/council/shared/rag/{file}");
        if let Some(body) = load_rag_document_body(&path) {
            return format!("\n\nReference material for this task:\n{body}");
        }
    }
    String::new()
}

fn compose_system_task_prompt(bf_key: &str, template: &str) -> String {
    let rag = rag_block_for_bf_key_only(bf_key);
    template.replace("{rag_context}", &rag)
}

fn rag_block_for_bf_key_only(bf_key: &str) -> String {
    if let Some(file) = default_shared_rag_filename(bf_key) {
        let path = format!("agents/council/shared/rag/{file}");
        if let Some(body) = load_rag_document_body(&path) {
            return format!("\n\nReference material for this task:\n{body}");
        }
    }
    String::new()
}

fn portrait_business_function(ai: &AiConfiguration) -> Option<&BusinessFunctionSpec> {
    ai.business_functions.get(AVATAR_CREATION_BF_KEY)
}

fn style_reference_prompt_block(v: &PortraitStyleSpec) -> String {
    let Some(path) = v.style_reference_image.as_deref() else {
        return String::new();
    };
    if path.trim().is_empty() {
        return String::new();
    }
    "\nStyle reference [3]: grey faceless editorial bust — medium and framing only. Colours and likeness from [1]. Ground: #000000.\n"
        .to_string()
}

fn portrait_synthesis_direction_block(v: &PortraitStyleSpec) -> String {
    let perspective = v.perspective_direction.trim();
    let style = v.style_direction.trim();
    let color = v.color_direction.trim();
    if perspective.is_empty() && style.is_empty() && color.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    if !perspective.is_empty() {
        lines.push(format!("Perspective direction: {perspective}"));
    }
    if !style.is_empty() {
        lines.push(format!("Style direction: {style}"));
    }
    if !color.is_empty() {
        lines.push(format!("Color direction: {color}"));
    }
    format!("\n{}\n", lines.join("\n"))
}

fn substitute_portrait_generation_placeholders(
    template: &str,
    v: &PortraitStyleSpec,
    avc_block: &str,
    rag_context: &str,
    colour_card_block: &str,
) -> String {
    template
        .replace("{portrait_synthesis_block}", &portrait_synthesis_direction_block(v))
        .replace("{style_reference_block}", &style_reference_prompt_block(v))
        .replace("{canonical_appearance_block}", "")
        .replace("{colour_card_block}", colour_card_block)
        .replace("{avc_block}", avc_block)
        .replace("{rag_context}", rag_context)
}

/// Load optional council style-plate bytes for Imagen style customization (`[3]`).
pub fn load_portrait_style_reference_bytes(
    spec: &PortraitStyleSpec,
) -> Result<Option<Vec<u8>>, String> {
    let Some(rel) = spec.style_reference_image.as_deref() else {
        return Ok(None);
    };
    let rel = rel.trim();
    if rel.is_empty() {
        return Ok(None);
    }
    let path = resolve_path_under_app_root(rel)?;
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("read portrait style reference {}: {e}", path.display()))?;
    if bytes.is_empty() {
        return Err(format!("portrait style reference {} is empty", path.display()));
    }
    Ok(Some(bytes))
}

/// Full text prompt for council-assisted bust portrait raster generation.
/// Imagen refs: subject `[1]`, style plate `[3]`.
#[must_use]
pub fn compose_portrait_generation_prompt(agent: &CouncilAgentFile) -> String {
    compose_imagen_portrait_prompt(agent, "this person from the reference photo", "", false)
}

/// Portrait detail block — compact Imagen instructions (no AVC persona, no RAG injection).
fn compose_portrait_detail_prompt(agent: &CouncilAgentFile, colour_card_block: &str) -> String {
    let default_style = PortraitStyleSpec::default();
    let style_spec = portrait_synthesis_spec(agent).unwrap_or(&default_style);

    let template = match portrait_business_function(&agent.ai_configuration) {
        Some(bf) => {
            let t = bf.text_prompt_template.trim();
            if t.is_empty() {
                DEFAULT_PORTRAIT_GENERATION_PROMPT_TEMPLATE
            } else {
                t
            }
        }
        None => DEFAULT_PORTRAIT_GENERATION_PROMPT_TEMPLATE,
    };

    substitute_portrait_generation_placeholders(template, style_spec, "", "", colour_card_block)
}

/// Imagen person-stylization scaffold + Epic detail prompt.
#[must_use]
pub fn compose_imagen_portrait_prompt(
    agent: &CouncilAgentFile,
    subject_label: &str,
    colour_card_block: &str,
    has_style_plate: bool,
) -> String {
    let label = subject_label.trim();
    let label = if label.is_empty() {
        "this person from the reference photo"
    } else {
        label
    };
    let detail = compose_portrait_detail_prompt(agent, colour_card_block);
    if has_style_plate {
        format!(
            "Create a filled humanist editorial illustration [3] image about {label} [1] to match the description: a portrait of {label} [1] {detail}"
        )
    } else {
        format!(
            "Create an image about {label} [1] to match the description: a portrait of {label} [1] {detail}"
        )
    }
}

/// Portrait Imagen prompt with optional vision colour-card block injected.
#[must_use]
pub fn compose_portrait_generation_prompt_with_colour(
    agent: &CouncilAgentFile,
    colour_card_block: &str,
) -> String {
    compose_portrait_detail_prompt(agent, colour_card_block)
}

fn substitute_council_moderation_review_placeholders(
    template: &str,
    avc_block: &str,
    ctx: &CouncilModerationReviewContext<'_>,
    rag_context: &str,
) -> String {
    template
        .replace("{avc_block}", avc_block)
        .replace("{community_name}", ctx.community_name)
        .replace("{target_content}", ctx.target_content)
        .replace("{constitution_context}", ctx.constitution_context)
        .replace("{jury_deliberation}", ctx.jury_deliberation)
        .replace("{scraped_evidence_summary}", ctx.scraped_evidence_summary)
        .replace("{rag_context}", rag_context)
}

/// Text prompt for council moderation appeal / escalation (send stored evidence, not live URLs).
#[must_use]
pub fn compose_council_moderation_review_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilModerationReviewContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag_block = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_MODERATION_REVIEW_BF_KEY,
    );

    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_MODERATION_REVIEW_BF_KEY,
        DEFAULT_COUNCIL_MODERATION_REVIEW_PROMPT_TEMPLATE,
    );

    substitute_council_moderation_review_placeholders(
        template,
        &avc_block,
        ctx,
        &rag_block,
    )
}

#[derive(Debug, Clone)]
pub struct CouncilThreadReplyContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub thread_document: &'a str,
    pub candidate_parent_ids: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilContentShareContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub link_url: &'a str,
    pub link_title: &'a str,
    pub link_description: &'a str,
    pub link_site_name: &'a str,
    pub link_catalogue: &'a str,
    pub candidate_link_ids: &'a str,
    /// Transient read summaries for publish-stage selection (never persisted).
    pub candidate_summaries: &'a str,
    pub excluded_content_link_ids: &'a str,
    pub excluded_url_canonical: &'a str,
    pub excluded_domains: &'a str,
    pub excluded_domains_evicted: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilContentShortlistContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub link_catalogue: &'a str,
    pub candidate_link_ids: &'a str,
    pub excluded_content_link_ids: &'a str,
    pub excluded_url_canonical: &'a str,
    pub excluded_domains: &'a str,
    pub excluded_domains_evicted: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilPortfolioReplaceContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub evicted_domain: &'a str,
    pub replacement_catalogue: &'a str,
    pub excluded_content_link_ids: &'a str,
    pub excluded_url_canonical: &'a str,
    pub excluded_domains: &'a str,
    pub excluded_domains_evicted: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilInviteContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub verified_email: &'a str,
    pub intended_label: Option<&'a str>,
    pub article_context: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilInviteArticleShortlistContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub article_catalogue: &'a str,
    pub candidate_post_ids: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilInvitePersonDiscoverContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub shortlisted_articles: &'a str,
    pub search_hits: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilGodmotherWelcomeContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub human_handle: &'a str,
    pub how_it_works_url: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilGodmotherGlobalFailingContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub human_handle: &'a str,
    pub how_it_works_url: &'a str,
    pub civic_standing_url: &'a str,
}

#[derive(Debug, Clone)]
pub struct CouncilGodmotherGlobalRecoveryContext<'a> {
    pub display_name: &'a str,
    pub steward_handle: &'a str,
    pub steward_label: &'a str,
    pub steward_intention: &'a str,
    pub human_handle: &'a str,
    pub how_it_works_url: &'a str,
    pub civic_standing_url: &'a str,
}

fn bf_template<'a>(
    ai: &'a AiConfiguration,
    key: &str,
    default: &'static str,
) -> &'a str {
    match ai.business_functions.get(key) {
        Some(bf) => {
            let t = bf.text_prompt_template.trim();
            if t.is_empty() {
                default
            } else {
                t
            }
        }
        None => default,
    }
}

/// Face capture vision gate prompt (system task — shared RAG).
#[must_use]
pub fn compose_face_capture_validate_prompt() -> String {
    compose_system_task_prompt(
        FACE_CAPTURE_VALIDATE_BF_KEY,
        DEFAULT_FACE_CAPTURE_VALIDATE_PROMPT_TEMPLATE,
    )
}

/// Brand logo vision gate prompt (system task — shared RAG).
#[must_use]
pub fn compose_brand_logo_validate_prompt() -> String {
    compose_system_task_prompt(
        BRAND_LOGO_VALIDATE_BF_KEY,
        DEFAULT_BRAND_LOGO_VALIDATE_PROMPT_TEMPLATE,
    )
}

/// Portrait colour-card vision prompt (system task — shared RAG).
#[must_use]
pub fn compose_portrait_colour_card_prompt() -> String {
    compose_system_task_prompt(
        PORTRAIT_COLOUR_CARD_BF_KEY,
        DEFAULT_PORTRAIT_COLOUR_CARD_PROMPT_TEMPLATE,
    )
}

/// Thread reply prompt for [`COUNCIL_THREAD_REPLY_BF_KEY`].
#[must_use]
pub fn compose_council_thread_reply_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilThreadReplyContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(&agent.ai_configuration, COUNCIL_THREAD_REPLY_BF_KEY);
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_THREAD_REPLY_BF_KEY,
        DEFAULT_COUNCIL_THREAD_REPLY_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{thread_document}", ctx.thread_document)
        .replace("{candidate_parent_ids}", ctx.candidate_parent_ids)
}

/// Content share prompt for [`COUNCIL_CONTENT_SHARE_BF_KEY`].
#[must_use]
pub fn compose_council_content_share_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilContentShareContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(&agent.ai_configuration, COUNCIL_CONTENT_SHARE_BF_KEY);
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_CONTENT_SHARE_BF_KEY,
        DEFAULT_COUNCIL_CONTENT_SHARE_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{link_url}", ctx.link_url)
        .replace("{link_title}", ctx.link_title)
        .replace("{link_description}", ctx.link_description)
        .replace("{link_site_name}", ctx.link_site_name)
        .replace("{link_catalogue}", ctx.link_catalogue)
        .replace("{candidate_link_ids}", ctx.candidate_link_ids)
        .replace("{candidate_summaries}", ctx.candidate_summaries)
        .replace(
            "{excluded_content_link_ids}",
            ctx.excluded_content_link_ids,
        )
        .replace("{excluded_url_canonical}", ctx.excluded_url_canonical)
        .replace("{excluded_domains}", ctx.excluded_domains)
        .replace(
            "{excluded_domains_evicted}",
            ctx.excluded_domains_evicted,
        )
}

/// Shortlist prompt for [`COUNCIL_CONTENT_SHORTLIST_BF_KEY`].
#[must_use]
pub fn compose_council_content_shortlist_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilContentShortlistContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_CONTENT_SHORTLIST_BF_KEY,
    );
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_CONTENT_SHORTLIST_BF_KEY,
        DEFAULT_COUNCIL_CONTENT_SHORTLIST_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{link_catalogue}", ctx.link_catalogue)
        .replace("{candidate_link_ids}", ctx.candidate_link_ids)
        .replace(
            "{excluded_content_link_ids}",
            ctx.excluded_content_link_ids,
        )
        .replace("{excluded_url_canonical}", ctx.excluded_url_canonical)
        .replace("{excluded_domains}", ctx.excluded_domains)
        .replace(
            "{excluded_domains_evicted}",
            ctx.excluded_domains_evicted,
        )
}

/// Portfolio replacement prompt for [`COUNCIL_PORTFOLIO_REPLACE_BF_KEY`].
#[must_use]
pub fn compose_council_portfolio_replace_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilPortfolioReplaceContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_PORTFOLIO_REPLACE_BF_KEY,
    );
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_PORTFOLIO_REPLACE_BF_KEY,
        DEFAULT_COUNCIL_PORTFOLIO_REPLACE_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{evicted_domain}", ctx.evicted_domain)
        .replace("{replacement_catalogue}", ctx.replacement_catalogue)
        .replace(
            "{excluded_content_link_ids}",
            ctx.excluded_content_link_ids,
        )
        .replace("{excluded_url_canonical}", ctx.excluded_url_canonical)
        .replace("{excluded_domains}", ctx.excluded_domains)
        .replace(
            "{excluded_domains_evicted}",
            ctx.excluded_domains_evicted,
        )
}

/// Platform invite prompt for [`COUNCIL_INVITE_BF_KEY`].
#[must_use]
pub fn compose_council_invite_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilInviteContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(&agent.ai_configuration, COUNCIL_INVITE_BF_KEY);
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_INVITE_BF_KEY,
        DEFAULT_COUNCIL_INVITE_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{verified_email}", ctx.verified_email)
        .replace(
            "{intended_label}",
            ctx.intended_label.unwrap_or("(name unknown)"),
        )
        .replace("{article_context}", ctx.article_context)
}

/// Article shortlist prompt for [`COUNCIL_INVITE_ARTICLE_SHORTLIST_BF_KEY`].
#[must_use]
pub fn compose_council_invite_article_shortlist_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilInviteArticleShortlistContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_INVITE_ARTICLE_SHORTLIST_BF_KEY,
    );
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_INVITE_ARTICLE_SHORTLIST_BF_KEY,
        DEFAULT_COUNCIL_INVITE_ARTICLE_SHORTLIST_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{article_catalogue}", ctx.article_catalogue)
        .replace("{candidate_post_ids}", ctx.candidate_post_ids)
}

/// Person discover prompt for [`COUNCIL_INVITE_PERSON_DISCOVER_BF_KEY`].
#[must_use]
pub fn compose_council_invite_person_discover_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilInvitePersonDiscoverContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_INVITE_PERSON_DISCOVER_BF_KEY,
    );
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_INVITE_PERSON_DISCOVER_BF_KEY,
        DEFAULT_COUNCIL_INVITE_PERSON_DISCOVER_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{shortlisted_articles}", ctx.shortlisted_articles)
        .replace("{search_hits}", ctx.search_hits)
}

/// Godmother welcome prompt for [`COUNCIL_GODMOTHER_WELCOME_BF_KEY`].
#[must_use]
pub fn compose_council_godmother_welcome_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilGodmotherWelcomeContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_GODMOTHER_WELCOME_BF_KEY,
    );
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_GODMOTHER_WELCOME_BF_KEY,
        DEFAULT_COUNCIL_GODMOTHER_WELCOME_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{human_handle}", ctx.human_handle)
        .replace("{how_it_works_url}", ctx.how_it_works_url)
}

/// Global failing outreach prompt for [`COUNCIL_GODMOTHER_GLOBAL_FAILING_BF_KEY`].
#[must_use]
pub fn compose_council_godmother_global_failing_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilGodmotherGlobalFailingContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_GODMOTHER_GLOBAL_FAILING_BF_KEY,
    );
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_GODMOTHER_GLOBAL_FAILING_BF_KEY,
        DEFAULT_COUNCIL_GODMOTHER_GLOBAL_FAILING_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{human_handle}", ctx.human_handle)
        .replace("{how_it_works_url}", ctx.how_it_works_url)
        .replace("{civic_standing_url}", ctx.civic_standing_url)
}

/// Global recovery outreach prompt for [`COUNCIL_GODMOTHER_GLOBAL_RECOVERY_BF_KEY`].
#[must_use]
pub fn compose_council_godmother_global_recovery_prompt(
    agent: &CouncilAgentFile,
    ctx: &CouncilGodmotherGlobalRecoveryContext<'_>,
) -> String {
    let avc_block = format_avc_block(agent).unwrap_or_default();
    let rag = rag_block_for_bf(
        &agent.ai_configuration,
        COUNCIL_GODMOTHER_GLOBAL_RECOVERY_BF_KEY,
    );
    let template = bf_template(
        &agent.ai_configuration,
        COUNCIL_GODMOTHER_GLOBAL_RECOVERY_BF_KEY,
        DEFAULT_COUNCIL_GODMOTHER_GLOBAL_RECOVERY_PROMPT_TEMPLATE,
    );
    template
        .replace("{avc_block}", &avc_block)
        .replace("{rag_context}", &rag)
        .replace("{display_name}", ctx.display_name)
        .replace("{steward_handle}", ctx.steward_handle)
        .replace("{steward_label}", ctx.steward_label)
        .replace("{steward_intention}", ctx.steward_intention)
        .replace("{human_handle}", ctx.human_handle)
        .replace("{how_it_works_url}", ctx.how_it_works_url)
        .replace("{civic_standing_url}", ctx.civic_standing_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_share_prompt_includes_link_og_fields() {
        let agent = load_council_agent("dogo").expect("dogo agent json");
        let prompt = compose_council_content_share_prompt(
            &agent,
            &CouncilContentShareContext {
                display_name: "Dogo",
                steward_handle: "spiritual_life_meaning",
                steward_label: "Spiritual Life",
                steward_intention: "Meaning",
                link_url: "https://example.com/story",
                link_title: "Example Headline",
                link_description: "A specific article angle.",
                link_site_name: "Example News",
                link_catalogue: "id=1",
                candidate_link_ids: "1",
                candidate_summaries: "1. id=1 url=https://example.com/story title=Example Headline site=Example News\nsummary: A specific article angle.",
                excluded_content_link_ids: "(none)",
                excluded_url_canonical: "(none)",
                excluded_domains: "(none)",
                excluded_domains_evicted: "(none)",
            },
        );
        assert!(prompt.contains("Example Headline"));
        assert!(prompt.contains("A specific article angle."));
        assert!(prompt.contains("https://example.com/story"));
        assert!(prompt.contains("Name one specific idea"));
    }

    #[test]
    fn epic_portrait_prompt_is_compact_without_rag_or_avc() {
        let agent = load_council_agent("epic").expect("epic agent json");
        let prompt = compose_imagen_portrait_prompt(
            &agent,
            "person with fair skin and rectangular glasses",
            "",
            true,
        );
        assert!(prompt.contains("[1]"));
        assert!(prompt.contains("[3]"));
        assert!(!prompt.contains("[2]"));
        assert!(!prompt.contains("control image"));
        assert!(prompt.contains("Create a filled humanist editorial illustration"));
        assert!(prompt.contains("pure black #000000"));
        assert!(!prompt.contains("Epistemic Stance"));
        assert!(!prompt.contains("Identity fidelity (highest priority)"));
        assert!(prompt.len() < 3_500, "prompt should stay compact: {} chars", prompt.len());
    }

    #[test]
    fn epic_portrait_prompt_references_subject_and_style_plate() {
        let agent = load_council_agent("epic").expect("epic agent json");
        let prompt = compose_imagen_portrait_prompt(
            &agent,
            "this person from the reference photo",
            "",
            true,
        );
        assert!(prompt.contains("[1]"));
        assert!(prompt.contains("[3]"));
        assert!(!prompt.contains("control image"));
        assert!(!prompt.contains("Identity record from reference photos"));
    }

    #[test]
    fn face_capture_validate_prompt_includes_rag_guide() {
        let prompt = compose_face_capture_validate_prompt();
        assert!(prompt.contains("Reference material for this task"));
        assert!(prompt.contains("reason_code"));
        assert!(
            prompt.contains("not_same_person ONLY"),
            "validate prompt must constrain not_same_person to clearly different people"
        );
        assert!(
            prompt.contains("Expression change alone is expected"),
            "validate prompt must accept same person across expressions"
        );
    }

    #[test]
    fn dogo_agent_has_no_portrait_synthesis() {
        let agent = load_council_agent("dogo").expect("dogo agent json");
        assert!(
            portrait_synthesis_spec(&agent).is_err(),
            "non-Epic councillors must not define portrait synthesis"
        );
    }

    #[test]
    fn all_council_agent_json_files_load() {
        for handle in [
            "aspa", "atol", "augi", "ciro", "dogo", "epic", "hypa", "marc", "milo", "pele",
            "soka", "theo",
        ] {
            load_council_agent(handle).unwrap_or_else(|e| panic!("load {handle}: {e}"));
        }
    }

    #[test]
    fn judicial_moderation_review_prompt_includes_rag_and_case_fields() {
        let agent = load_council_agent("theo").expect("theo agent json");
        let prompt = compose_council_moderation_review_prompt(
            &agent,
            &CouncilModerationReviewContext {
                community_name: "Test Community",
                target_content: "Disputed post body",
                constitution_context: "Rule 4.2",
                jury_deliberation: "Jury voted 8-4 to uphold",
                scraped_evidence_summary: "Captured URL excerpt",
            },
        );
        assert!(prompt.contains("Test Community"));
        assert!(prompt.contains("Disputed post body"));
        assert!(prompt.contains("CONSTITUTIONAL ALIGNMENT"));
        assert!(prompt.contains(r#""vote":"uphold""#));
    }

    #[test]
    fn council_thread_reply_prompt_includes_rag() {
        let agent = load_council_agent("milo").expect("milo agent json");
        let prompt = compose_council_thread_reply_prompt(
            &agent,
            &CouncilThreadReplyContext {
                display_name: "Milo",
                steward_handle: "health_wellness",
                steward_label: "Health",
                steward_intention: "Wellness",
                thread_document: "post_id=1 audience=For: @health_wellness",
                candidate_parent_ids: "1",
            },
        );
        assert!(prompt.contains("parent_post_id"));
        assert!(prompt.contains("post_id=1"));
    }

    #[test]
    fn council_portfolio_replace_prompt_includes_rag() {
        let agent = load_council_agent("soka").expect("soka agent json");
        let prompt = compose_council_portfolio_replace_prompt(
            &agent,
            &CouncilPortfolioReplaceContext {
                display_name: "Soka",
                steward_handle: "education_learning",
                steward_label: "Education",
                steward_intention: "Learning",
                evicted_domain: "bad.example",
                replacement_catalogue: "id=99",
                excluded_content_link_ids: "(none)",
                excluded_url_canonical: "(none)",
                excluded_domains: "(none)",
                excluded_domains_evicted: "bad.example",
            },
        );
        assert!(prompt.contains("proof_url"));
        assert!(prompt.contains("bad.example"));
    }

    #[test]
    fn council_invite_prompt_includes_rag() {
        let agent = load_council_agent("pele").expect("pele agent json");
        let prompt = compose_council_invite_prompt(
            &agent,
            &CouncilInviteContext {
                display_name: "Pele",
                steward_handle: "love_relationships",
                steward_label: "Love",
                steward_intention: "Partnership",
                verified_email: "jane@example.org",
                intended_label: Some("Jane Doe"),
                article_context: "post_id=5 url=https://example.com/story",
            },
        );
        assert!(prompt.contains("intended_email"));
        assert!(prompt.contains("jane@example.org"));
        assert!(prompt.contains("post_id=5"));
    }

    #[test]
    fn godmother_prompts_include_rag() {
        let agent = load_council_agent("hypa").expect("hypa agent json");
        let welcome = compose_council_godmother_welcome_prompt(
            &agent,
            &CouncilGodmotherWelcomeContext {
                display_name: "Hypa",
                steward_handle: "family_relationships",
                steward_label: "Family",
                steward_intention: "Relationships",
                human_handle: "newbie",
                how_it_works_url: "https://256.app/how",
            },
        );
        assert!(welcome.contains(r#""action":"welcome""#));

        let failing = compose_council_godmother_global_failing_prompt(
            &agent,
            &CouncilGodmotherGlobalFailingContext {
                display_name: "Hypa",
                steward_handle: "family_relationships",
                steward_label: "Family",
                steward_intention: "Relationships",
                human_handle: "struggling",
                how_it_works_url: "https://256.app/how",
                civic_standing_url: "https://256.app/standing",
            },
        );
        assert!(failing.contains("below 0.66"));

        let recovery = compose_council_godmother_global_recovery_prompt(
            &agent,
            &CouncilGodmotherGlobalRecoveryContext {
                display_name: "Hypa",
                steward_handle: "family_relationships",
                steward_label: "Family",
                steward_intention: "Relationships",
                human_handle: "returning",
                how_it_works_url: "https://256.app/how",
                civic_standing_url: "https://256.app/standing",
            },
        );
        assert!(recovery.contains("healthy band"));
    }

    #[test]
    fn brand_logo_validate_prompt_includes_rag() {
        let prompt = compose_brand_logo_validate_prompt();
        assert!(prompt.contains("reason_code"));
        assert!(prompt.contains("Reference material for this task"));
    }

    #[test]
    fn rag_path_rejects_traversal() {
        assert!(load_rag_document_body("../../../Cargo.toml").is_none());
        assert!(load_rag_document_body("/etc/passwd").is_none());
        assert!(load_rag_document_body("agents/council/../../Cargo.toml").is_none());
    }

    #[test]
    fn rag_path_loads_valid_guide() {
        assert!(load_rag_document_body(
            "agents/council/shared/rag/face_capture_validate_guide.txt"
        )
        .is_some());
    }

    #[test]
    fn portrait_style_path_rejects_traversal() {
        let spec = PortraitStyleSpec {
            style_reference_image: Some("../../../Cargo.toml".to_string()),
            ..PortraitStyleSpec::default()
        };
        assert!(load_portrait_style_reference_bytes(&spec).is_err());
    }

    #[test]
    fn epic_portrait_style_plate_v6_loads() {
        let agent = load_council_agent("epic").expect("epic agent json");
        let style = portrait_synthesis_spec(&agent).expect("epic portrait synthesis");
        assert!(
            load_portrait_style_reference_bytes(style)
                .expect("style plate read")
                .is_some()
        );
        assert!(
            style
                .style_reference_image
                .as_deref()
                .is_some_and(|p| p.contains("style-plate-bust-v6"))
        );
    }
}
