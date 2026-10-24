# Council RAG

Practical AI tasks use a **thin prompt template** + **`{rag_context}`** loaded at runtime from `rag_documents` in each agent JSON (or shared fallbacks under `agents/council/shared/rag/`).

## Layout

- **`agents/council/shared/rag/`** — task guides shared across councillors (moderation appeal, thread reply, face validate, portrait colour card, etc.).
- **`agents/council/rag/<handle>/`** — councillor-specific guides (e.g. Epic `portrait_style_guide.txt` + `style-plate-bust-v6.png`).

## Business functions

| Key | Scope | Compose helper |
| --- | --- | --- |
| `avatar_creation` | **Epic only** — bust portrait raster | `compose_imagen_portrait_prompt` |
| `portrait_colour_card` | Epic onboarding / redraw vision step | `compose_portrait_colour_card_prompt` |
| `face_capture_validate` | System task (no agent JSON) | `compose_face_capture_validate_prompt` |
| `brand_logo_validate` | System task (no agent JSON) | `compose_brand_logo_validate_prompt` |
| `council_moderation_review` | Civic dispute council appeal | `compose_council_moderation_review_prompt` |
| `council_thread_reply` | Autonomous thread replies | `compose_council_thread_reply_prompt` |
| `council_content_share` | Daily link shares | `compose_council_content_share_prompt` |
| `council_portfolio_replace` | Portfolio host replacement | `compose_council_portfolio_replace_prompt` |
| `council_invite` | Platform invites | `compose_council_invite_prompt` |
| `council_godmother_*` | Godmother outreach | `compose_council_godmother_*_prompt` |

Imagen API parameters, style plate (`style-plate-bust-v6.png`), and multi-sample pick live on **`avatar_creation.portrait_synthesis`** in `epic.json` + `src/bin/ai_gateway/portrait.rs`. Portrait prompts are compact (no RAG injection). `portrait_style_guide.txt` is operator reference only.

Assembly: `src/agent_specs.rs`. Persona (AVC) stays in JSON — see `AVC_PATTERN.md`.
