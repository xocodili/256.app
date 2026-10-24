# Shared council assets

## `default_portrait_generation_prompt_template.txt`

Canonical **text prompt scaffold** for bust portrait raster generation. It is:

- `include_str!`d in Rust as the fallback when `business_functions.portrait_generation` / `avatar_creation`.`text_prompt_template` is empty in an agent JSON file.
- May be copied into each `agents/council/<handle>.json` under `ai_configuration.business_functions.avatar_creation` (or `portrait_generation`) so you can **per-council** fork wording without touching code.

### Placeholders

| Token | Source |
|-------|--------|
| `{colour_card_block}` | Vision JSON colour card (transient; from `portrait_colour_card_v1`) |
| `{style_reference_block}` | Style plate `[3]` prose when `style_reference_image` is set |
| `{portrait_synthesis_block}` | Optional `perspective_direction` / `style_direction` / `color_direction` |
| `{avc_block}` | Empty in production Imagen path (AVC not injected) |
| `{rag_context}` | Empty in production Imagen path (RAG not injected) |

Publisher **API** parameters (`sampleCount`, `aspectRatio`, etc.) stay in `portrait_synthesis.parameters` in each agent JSON.

## `default_council_moderation_review_prompt_template.txt`

Default mandate for **`council_moderation_review`** (post–human-jury escalation). Same placeholder contract as `compose_council_moderation_review_prompt` in `src/agent_specs.rs`.

## Participatory batch functions (`council_thread_reply`, `council_content_share`, `council_portfolio_replace`, invite/share funnels)

Used by autonomous council jobs (`council_reply_tick`, **`council_share_shortlist`** chain, **`council_invite_*`** chain). Legacy `council_post_member` is a thin compat alias → share shortlist. Each template includes a **YOUR STEWARD COMMUNITY** block:

| Token | Source |
|-------|--------|
| `{steward_handle}` | Steward community handle (e.g. `intellectual_life_education`) |
| `{steward_label}` | `social_community.label` |
| `{steward_intention}` | `social_community.intention` |

Copy rules in templates: address **that** `@public` flock only — not the whole platform. Wired in `compose_council_*_prompt` (`src/agent_specs.rs`). Product canon: [`docs/PRODUCT_GUIDE.md`](../../../docs/PRODUCT_GUIDE.md) § AI Council.
