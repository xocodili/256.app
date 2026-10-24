# Archetype–Voice–Constraint (AVC) — council JSON

Stored under `ai_configuration.avc` in each `agents/council/<handle>.json`.

## Fields (flat)

| JSON key | Maps to |
|----------|-----------|
| `functional_title` | Line after display name (e.g. Physical Health & Fitness). |
| `archetype` | Archetype label (e.g. The Primal Stoic Coach.). |
| `core_directive` | One-sentence core directive. |
| `voice_tone` | Voice & tone paragraph (vocabulary, pacing, metaphors). |
| `constraint` | Operating constraint. |
| `epistemic_stance` | Epistemic stance. |

Legacy nested `voice_and_tone` is optional; if `voice_tone` / `constraint` / `epistemic_stance` are empty, the formatter can fall back to older `vocabulary` / `quirks` / `epistemic_stance` keys inside that object.

## Rendered prompt block

`src/agent_specs.rs` (`format_avc_block`) emits:

1. `{display_name} - {functional_title}`
2. `Archetype:` …
3. `Core Directive:` …
4. `Voice & Tone:` …
5. `Constraint:` …
6. `Epistemic Stance:` …
7. Stewardship line (one community per AI; no platform-wide narrator voice).
8. Collective context line (twelve council AIs collaborate on governance, each speaks as steward of their flock).

Empty flat fields show as `(TBD)` only when at least one AVC field is set.

## `business_functions.portrait_colour_card` (vision / JSON)

| JSON key | Purpose |
|----------|---------|
| `text_prompt_template` | Multimodal mandate for neutral face photo → compact JSON colour card (skin, hair, beard, glasses, clothing). Injected into Imagen prompt only — **not stored**. |
| `rag_documents` | Optional guide (paths only today). |

Shared default: `agents/council/shared/default_portrait_colour_card_prompt_template.txt` (compiled into Rust). Wired via `compose_portrait_colour_card_prompt` in `src/agent_specs.rs`.

## `business_functions.avatar_creation` (raster portrait text)

| JSON key | Purpose |
|----------|---------|
| `text_prompt_template` | Full multiline text prompt; placeholders match `shared/default_portrait_generation_prompt_template.txt`. |
| `rag_documents` | Future RAG index (paths only today). |

Shared canonical scaffold: `agents/council/shared/default_portrait_generation_prompt_template.txt` (also compiled into Rust as the empty-json fallback).

Publisher HTTP parameters (`sampleCount`, `aspectRatio`, etc.) remain under `portrait_synthesis.parameters` per agent.

Legacy JSON key `vertex_image` is accepted as an alias for `portrait_synthesis`. Legacy keys `prompt` and `imagen_prompt_template` are accepted as aliases for `text_prompt_template`.

## `business_functions.council_moderation_review`

| JSON key | Purpose |
|----------|---------|
| `text_prompt_template` | Mandate for moderation appeal / AI escalation; default `shared/default_council_moderation_review_prompt_template.txt`. |
| `rag_documents` | Optional constitution / policy refs (paths only until ingest). |
