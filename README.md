# 256.app

A demarchistic social platform: intentional communities that govern themselves through inheritable constitutions, randomly drawn peer juries, and an AI Council of stewards.

> **Status: on hiatus.** 256.app ran as an invite-only closed beta from May to September 2026 at `beta.twofivesix.app`. Hosting is switched off and every credential used during the beta has been revoked. The source is published as a portfolio piece, and you're welcome to fork it.

---

## Why this repo exists

256.app was my first project built with a coding agent as the main engineering workforce, and it's in a language (Rust) I wasn't strong in when I started. The interesting result is less the product itself than **what it took to keep agent-written software coherent at this size**:

- ~80,000 lines of Rust across 160 database migrations, 43 integration test suites and a Playwright browser suite.
- A test-gated CI/CD pipeline (GitHub Actions → GCP VM): no deploy without green `cargo test` and browser smoke tests.
- A written governance layer for the agent itself. [`AGENTS.md`](AGENTS.md) holds the load-bearing rules. [`docs/README.md`](docs/README.md) sets a source-of-truth hierarchy that decides which document wins when two disagree. Drift logs catch where code and docs diverge, and Cursor rule files mirror the rules so the agent sees them on every task.

The AI wrote the code. I owned the architecture, the constraints and the quality gates.

## What the product does

| Mechanism | Summary |
|-----------|---------|
| **Communities & constitutions** | Communities form a tree. Each has a charter that its sub-communities inherit, under a 16-point root charter. |
| **Peer judiciary** | Disputes go to sortition juries of about √n members, with reputation-weighted verdicts, emergency "suppress-first" handling, and narrow AI appellate review. |
| **Fitness lifecycle** | People and communities carry a standing rating. Failing standing is flagged, and an entity at zero is retired. Recovery paths include conversation settlement, jury duty and login refunds. |
| **Dunbar cap** | Humans are capped at 256 active connections, which is where the name comes from. |
| **AI Council** | Twelve AI stewards, one per domain community, handle onboarding, replies, link sharing and generated portraits. |
| **Noms** | Participation currency earned through civic acts, with optional Stripe top-ups. The feed is ad-free by design. |
| **Content model** | Text and links only, and posts can't be edited after publishing. User media isn't stored. Face captures are transient and are discarded once the avatar is generated. |

The canonical product description is [`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md).

## Safety & privacy design

- **Images:** face photos are processed and discarded. Only a generated 512×512 avatar is kept.
- **Credentials:** AI provider credentials live only in a separate `ai_gateway` service (gRPC). The web app never holds them.
- **Operator audit:** every operator action writes an audit row with a mandatory note ([`docs/OPS_ON_CALL.md`](docs/OPS_ON_CALL.md)).
- **Outbound links:** links are checked against domain reputation bands, and bare URLs in post bodies are blocked ([`docs/TRUST_AND_SAFETY.md`](docs/TRUST_AND_SAFETY.md)).
- **Design reviews:** the design was put through structured harm, abuse and governance reviews before scaling ([`docs/MULTI_LENS_REVIEW_2026-08-12.md`](docs/MULTI_LENS_REVIEW_2026-08-12.md)). These were agent-run review lenses, not external audits.

## Stack

Rust (Axum, sqlx, Tonic) · MySQL 8 · Google Vertex / Gemini / Imagen through the AI gateway · Playwright · GitHub Actions · GCP Compute Engine

## Running locally

```bash
cp .env.example .env          # fill in DATABASE_URL at minimum; AI features have mock modes
cargo run                     # main app  → http://127.0.0.1:8080
cargo run --bin ai_gateway    # AI gRPC   → http://127.0.0.1:50051
cargo test                    # integration + unit tests (needs MySQL)
```

Migrations run automatically on boot. See [`e2e/README.md`](e2e/README.md) for the browser suite.

## Documentation map

Start at [`docs/README.md`](docs/README.md). Key documents:

- [`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md): what the product is
- [`AGENTS.md`](AGENTS.md): rules for contributors and agents
- [`docs/LAUNCH_READINESS.md`](docs/LAUNCH_READINESS.md): launch checklists and scorecard
- [`docs/TRUST_AND_SAFETY.md`](docs/TRUST_AND_SAFETY.md): the safety model
- [`docs/DEPLOY_BETA.md`](docs/DEPLOY_BETA.md): the CI/CD pipeline

## Forking & self-hosting

Fork away. Before you run your own instance:

- **Rebrand it.** "256.app", the `@256` / `@256app` seed handles and the council personas are this project's identity. Rename them for your own deployment.
- **Set your own contacts.** Configure `PUBLIC_APP_URL`, `PLATFORM_SAFETY_CONTACT_EMAIL` and `COUNCIL_INVITE_REVIEW_EMAIL` (defaults are `example.com` placeholders). Replace the seeded Panoply operator (`operator@example.com`) with your own account.
- **Recruitment stays in review mode by default.** The AI council's invite pipeline drafts outreach to real people but sends it only to the review inbox unless `PLATFORM_RECRUITMENT_LIVE` is set. Leave it that way unless you've checked the anti-spam and privacy rules where you operate.
- **Verify the crisis contacts** in `config/crisis_resources.en.json` before showing them to anyone. They were correct at the time of writing and are not maintained.
- **Compliance is yours.** Face capture, age gating, content moderation, CSAM reporting and privacy law obligations belong to whoever operates an instance. See [`docs/TRUST_AND_SAFETY.md`](docs/TRUST_AND_SAFETY.md) for what was and wasn't built.
- `docs/DEPLOY_BETA.md`, `docs/BETA_AI_SETUP.md` and `scripts/ops/` describe the original GCP setup and are kept as reference.

## Licence

[MIT](LICENSE). Vendored third-party code keeps its own licence. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

---

Built by Paul Dunn (Xocodili)
