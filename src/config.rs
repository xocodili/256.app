use std::net::IpAddr;

/// Canonical public site URL for links and OIDC redirect (no trailing slash).
/// When `SECURE_COOKIES` is on, bare hostnames and `http://` non-loopback URLs upgrade to `https://`.
pub fn normalize_public_url(raw: &str, secure_cookies: bool) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("https://") {
        return trimmed.to_string();
    }
    if lower.starts_with("http://") {
        let host = public_url_host(trimmed);
        if secure_cookies && !host_is_loopback(host) {
            return format!("https://{}", &trimmed[7..]);
        }
        return trimmed.to_string();
    }
    if secure_cookies {
        format!("https://{trimmed}")
    } else if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

fn public_url_host(url: &str) -> &str {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .split(':')
        .next()
        .unwrap_or(url)
}

fn host_is_loopback(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

#[derive(Clone, Debug)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Full `From` mailbox, e.g. `256.app <noreply@example.com>`.
    pub from: String,
    /// `starttls` (default), `tls` (SMTPS / implicit TLS), or `none` (e.g. local Mailpit).
    pub tls_mode: String,
}

#[derive(Clone, Debug)]
pub struct RecaptchaConfig {
    pub site_key: String,
    pub secret_key: String,
    pub min_score: f64,
    /// When true, skip vendor verification (local dev / CI only).
    pub bypass: bool,
}

#[derive(Clone, Debug)]
pub struct StripeConfig {
    pub secret_key: String,
    pub publishable_key: String,
    pub webhook_secret: String,
}

impl StripeConfig {
    #[must_use]
    pub fn enabled(&self) -> bool {
        !self.secret_key.is_empty() && !self.publishable_key.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct OidcGoogleConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: IpAddr,
    pub port: u16,
    /// Identifying `User-Agent` for Nominatim reverse-geocode (usage policy).
    pub nominatim_user_agent: String,
    /// `Secure` flag on session cookies (true in production behind HTTPS).
    pub secure_cookies: bool,
    /// Canonical site URL with no trailing slash (verification links, redirects).
    pub public_base_url: String,
    /// Enables local-only auth bypass endpoints for rapid secured-page testing.
    pub dev_auth_bypass: bool,
    /// When true, trust `X-Forwarded-For` / `X-Real-IP` for reCAPTCHA remoteip (reverse proxy).
    pub trusted_proxy: bool,
    /// When `None`, verification links are only logged at `info` (dev).
    pub smtp: Option<SmtpConfig>,
    /// When true, always use the mock appearance text instead of calling gRPC AI agents. Env: `FACE_VISION_MOCK=1`.
    pub face_vision_mock: bool,
    /// When true, skip the async Vertex vision accept/reject gate but still run real portraiture.
    /// Local dev only — Env: `SKIP_FACE_VISION_VALIDATE=1` (ignored unless bound to loopback).
    pub skip_face_vision_validate: bool,
    /// gRPC endpoint for AI agent gateway, e.g. `http://127.0.0.1:50051`.
    pub ai_grpc_endpoint: String,
    /// Per-call timeout for AI gRPC requests (milliseconds). Default 3000ms.
    pub ai_grpc_timeout_ms: u64,
    /// When true, face capture errors include technical details for debugging. Env: `VERBOSE_AI_ERRORS` (default true for prototyping; set `0` in production).
    pub verbose_ai_errors: bool,
    /// Publisher model id for raster portraits (passed to ai_gateway). Env: `PORTRAIT_RASTER_MODEL`.
    pub portrait_raster_model: String,
    /// Per-Imagen-predict HTTP budget (3 samples per call). Env: `PORTRAIT_RASTER_TIMEOUT_MS`
    /// (default 90000). Values below 90s clamp up; gRPC total ≈ 2× predict + 15s overhead.
    pub portrait_raster_timeout_ms: u64,
    /// Relative directory where generated face artwork is saved.
    pub generated_faces_dir: String,
    /// Council agent handle for appearance + portrait pipelines (`agents/council/{handle}.json`). Env: `COUNCIL_AGENT_HANDLE` (default `epic`).
    pub council_agent_handle: String,
    /// Inbox for platform recruitment during rollout (intended recipient named in body).
    /// Env: `COUNCIL_INVITE_REVIEW_EMAIL` (default `owner@example.com`).
    pub council_invite_review_email: String,
    /// When true (default), platform invites are delivered only to
    /// [`Self::council_invite_review_email`], not to intended recipients.
    /// Set `PLATFORM_RECRUITMENT_LIVE=1` to enable direct delivery (human friend invites).
    /// Council AI invites always route to the review inbox until live delivery is enabled in code.
    pub platform_recruitment_review_only: bool,
    /// Server-side Google Maps Platform key (Places / Geocode). Env: `GOOGLE_MAPS_API_KEY`. Restrict in GCP by API + IP / referrer.
    pub google_maps_api_key: Option<String>,
    /// HTTP timeout for Places Details (ms). Env: `GOOGLE_PLACES_TIMEOUT_MS` (default 12000).
    pub google_places_timeout_ms: u64,
    /// Bearer token for `POST /ops/locations/invalidate` (internal cadastral / policy ops). Env: `OPS_LOCATION_INVALIDATE_TOKEN`. If unset, route returns 404.
    pub ops_location_invalidate_token: Option<String>,
    /// Bearer token for `POST /ops/jobs/run` (job queue conductor). Env: `OPS_JOB_QUEUE_TOKEN`. If unset, route returns 404.
    pub ops_job_queue_token: Option<String>,
    /// Global per-request hard timeout (seconds). Env: `REQUEST_TIMEOUT_SECS` (default 60).
    /// Face capture POST returns 202 immediately; portrait work runs in the async job queue
    /// (gRPC budget up to ~195s per predict batch). This cap applies to synchronous HTTP handlers.
    pub request_timeout_secs: u64,
    /// Background scheduler tick interval (seconds). Env: `SCHEDULER_TICK_SECS` (default 1).
    /// On every tick the scheduler enqueues recurring system jobs (jury nomination,
    /// scope-recalc drain) — deduped by payload hash — and then runs one
    /// `run_conductor_tick` to drain the queue. Set to 0 to disable the in-process
    /// scheduler entirely (only the `/ops/jobs/run` endpoint will advance the queue).
    pub scheduler_tick_secs: u64,
    /// Batch size for each `run_conductor_tick` invoked by the scheduler. Env:
    /// `SCHEDULER_BATCH_SIZE` (default 16, clamped to [1, 256]).
    pub scheduler_batch_size: usize,
    /// Max jobs executing concurrently within one conductor tick. Env:
    /// `JOB_CONDUCTOR_PARALLELISM` (default 4, clamped to [1, 16]).
    pub job_conductor_parallelism: usize,
    /// Sub-cap for gateway/LLM-heavy job types within [`Self::job_conductor_parallelism`].
    /// Env: `JOB_CONDUCTOR_LLM_PARALLELISM` (default 2, clamped to [1, 8]).
    pub job_conductor_llm_parallelism: usize,
    /// Wall-clock budget for `council_portfolio_feed_poll` RSS/crawl work per handler run.
    /// Env: `FEED_POLL_WALL_BUDGET_SECS` (default 120, clamped to [30, 240]).
    pub feed_poll_wall_budget_secs: u64,
    /// When false, skip Panoply daily status and away-user engagement digest mail (local dev
    /// with real SMTP). Env: `OPS_DIGEST_LIVE` — default true; set `0` on local `.env`.
    pub ops_digest_live: bool,
    /// systemd journal retention window for the daily `system_journal_retention` job.
    /// Env: `JOURNAL_RETENTION_DAYS` (default 3).
    pub journal_retention_days: u32,
    /// Pepper for session-bound CSRF tokens. Env: `CSRF_PEPPER` (falls back to `DATABASE_URL` in dev).
    pub csrf_pepper: String,
    /// Lowercase SHA-256 hex digests blocked at media ingest (Class A/B stub). Env: `MEDIA_HASH_BLOCKLIST` (comma-separated).
    pub media_hash_blocklist: std::collections::HashSet<String>,
    pub recaptcha: Option<RecaptchaConfig>,
    pub oidc_google: Option<OidcGoogleConfig>,
    /// Stripe keys for Nom pack checkout. Enabled when secret + publishable are set.
    pub stripe: Option<StripeConfig>,
    /// Public platform safety contact (Safety legal card, closed-beta on-call). Env: `PLATFORM_SAFETY_CONTACT_EMAIL`.
    pub platform_safety_contact_email: String,
    /// Council invite person discovery — web search API key. Env: `COUNCIL_WEB_SEARCH_API_KEY`.
    pub council_web_search_api_key: Option<String>,
    /// `brave` or `google_cse`. Env: `COUNCIL_WEB_SEARCH_PROVIDER`.
    pub council_web_search_provider: WebSearchProvider,
    /// Google Custom Search engine id when provider is `google_cse`. Env: `COUNCIL_WEB_SEARCH_CX`.
    pub council_web_search_cx: Option<String>,
    /// Web search HTTP timeout (ms). Env: `COUNCIL_WEB_SEARCH_TIMEOUT_MS` (default 8000).
    pub council_web_search_timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchProvider {
    None,
    Brave,
    GoogleCse,
}

impl WebSearchProvider {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "brave" => Self::Brave,
            "google_cse" | "google" | "cse" => Self::GoogleCse,
            _ => Self::None,
        }
    }
}

impl Config {
    #[must_use]
    pub fn web_search_configured(&self) -> bool {
        self.council_web_search_api_key
            .as_deref()
            .is_some_and(|k| !k.trim().is_empty())
            && !matches!(self.council_web_search_provider, WebSearchProvider::None)
            && (self.council_web_search_provider != WebSearchProvider::GoogleCse
                || self
                    .council_web_search_cx
                    .as_deref()
                    .is_some_and(|cx| !cx.trim().is_empty()))
    }

    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            "DATABASE_URL must be set (e.g. copy .env.example to .env and set a MySQL URL)"
        })?;

        let host: IpAddr = std::env::var("HOST")
            .unwrap_or_else(|_| "127.0.0.1".into())
            .parse()
            .map_err(|e| format!("invalid HOST: {e}"))?;

        let port: u16 = std::env::var("PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse()
            .map_err(|e| format!("invalid PORT: {e}"))?;

        let secure_cookies = std::env::var("SECURE_COOKIES")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let public_base_url = normalize_public_url(
            &std::env::var("PUBLIC_APP_URL")
                .or_else(|_| std::env::var("PUBLIC_BASE_URL"))
                .unwrap_or_else(|_| format!("http://127.0.0.1:{port}")),
            secure_cookies,
        );
        let dev_auth_bypass = std::env::var("DEV_AUTH_BYPASS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let trusted_proxy = std::env::var("TRUSTED_PROXY")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let nominatim_user_agent = std::env::var("NOMINATIM_USER_AGENT")
            .unwrap_or_else(|_| "256.app/0.1 (https://256.app)".to_string())
            .trim()
            .to_string();
        if nominatim_user_agent.is_empty() {
            return Err("NOMINATIM_USER_AGENT must not be empty when set".to_string());
        }

        let smtp = match std::env::var("SMTP_HOST") {
            Ok(h) if !h.trim().is_empty() => {
                let smtp_port: u16 = std::env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "587".into())
                    .parse()
                    .map_err(|e| format!("invalid SMTP_PORT: {e}"))?;
                let from = std::env::var("SMTP_FROM").map_err(|_| {
                    "SMTP_FROM is required when SMTP_HOST is set (e.g. \"256.app <noreply@example.com>\")"
                })?;
                let tls_mode = std::env::var("SMTP_TLS_MODE").unwrap_or_else(|_| "starttls".into());
                Some(SmtpConfig {
                    host: h,
                    port: smtp_port,
                    username: std::env::var("SMTP_USERNAME")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    password: std::env::var("SMTP_PASSWORD").ok(),
                    from,
                    tls_mode,
                })
            }
            _ => None,
        };

        let face_vision_mock = std::env::var("FACE_VISION_MOCK")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let skip_face_vision_validate = std::env::var("SKIP_FACE_VISION_VALIDATE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let ai_grpc_endpoint = std::env::var("AI_GRPC_ENDPOINT")
            .or_else(|_| std::env::var("GOOGLE_GRPC_ENDPOINT"))
            .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string())
            .trim()
            .to_string();
        if ai_grpc_endpoint.is_empty() {
            return Err(
                "AI_GRPC_ENDPOINT (or GOOGLE_GRPC_ENDPOINT) must not be empty when set".to_string(),
            );
        }

        let ai_grpc_timeout_ms: u64 = std::env::var("AI_GRPC_TIMEOUT_MS")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .map_err(|e| format!("invalid AI_GRPC_TIMEOUT_MS: {e}"))?;

        let verbose_ai_errors = match std::env::var("VERBOSE_AI_ERRORS") {
            Ok(v) => match v.trim() {
                "1" | "true" | "yes" => true,
                "0" | "false" | "no" => false,
                _ => true,
            },
            Err(_) => true,
        };

        let portrait_raster_model = std::env::var("PORTRAIT_RASTER_MODEL")
            .or_else(|_| std::env::var("VERTEX_IMAGE_MODEL"))
            .unwrap_or_else(|_| "imagen-3.0-capability-001".to_string())
            .trim()
            .to_string();

        let portrait_raster_timeout_ms: u64 = std::env::var("PORTRAIT_RASTER_TIMEOUT_MS")
            .or_else(|_| std::env::var("VERTEX_IMAGE_TIMEOUT_MS"))
            .unwrap_or_else(|_| "90000".to_string())
            .parse()
            .map_err(|e| format!("invalid PORTRAIT_RASTER_TIMEOUT_MS / VERTEX_IMAGE_TIMEOUT_MS: {e}"))?;

        let generated_faces_dir = std::env::var("GENERATED_FACE_ART_DIR")
            .unwrap_or_else(|_| "static/generated/user-faces".to_string())
            .trim()
            .to_string();

        let council_agent_handle = std::env::var("COUNCIL_AGENT_HANDLE")
            .unwrap_or_else(|_| "epic".to_string())
            .trim()
            .to_lowercase();
        if council_agent_handle.is_empty() {
            return Err("COUNCIL_AGENT_HANDLE must not be empty when set".to_string());
        }

        let council_invite_review_email = std::env::var("COUNCIL_INVITE_REVIEW_EMAIL")
            .unwrap_or_else(|_| "owner@example.com".to_string())
            .trim()
            .to_string();
        if council_invite_review_email.is_empty() {
            return Err(
                "COUNCIL_INVITE_REVIEW_EMAIL must not be empty when set".to_string(),
            );
        }

        let platform_recruitment_review_only = !std::env::var("PLATFORM_RECRUITMENT_LIVE")
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false);

        let google_maps_api_key = std::env::var("GOOGLE_MAPS_API_KEY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let council_web_search_api_key = std::env::var("COUNCIL_WEB_SEARCH_API_KEY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let council_web_search_provider = std::env::var("COUNCIL_WEB_SEARCH_PROVIDER")
            .map(|v| WebSearchProvider::parse(&v))
            .unwrap_or(WebSearchProvider::None);
        let council_web_search_cx = std::env::var("COUNCIL_WEB_SEARCH_CX")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let council_web_search_timeout_ms: u64 =
            std::env::var("COUNCIL_WEB_SEARCH_TIMEOUT_MS")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .map_err(|e| format!("invalid COUNCIL_WEB_SEARCH_TIMEOUT_MS: {e}"))?;

        let google_places_timeout_ms: u64 = std::env::var("GOOGLE_PLACES_TIMEOUT_MS")
            .unwrap_or_else(|_| "12000".to_string())
            .parse()
            .map_err(|e| format!("invalid GOOGLE_PLACES_TIMEOUT_MS: {e}"))?;

        let ops_location_invalidate_token = std::env::var("OPS_LOCATION_INVALIDATE_TOKEN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let ops_job_queue_token = std::env::var("OPS_JOB_QUEUE_TOKEN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let request_timeout_secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .map_err(|e| format!("invalid REQUEST_TIMEOUT_SECS: {e}"))?;
        if request_timeout_secs == 0 {
            return Err("REQUEST_TIMEOUT_SECS must be greater than zero".to_string());
        }

        let scheduler_tick_secs: u64 = std::env::var("SCHEDULER_TICK_SECS")
            .unwrap_or_else(|_| "1".to_string())
            .parse()
            .map_err(|e| format!("invalid SCHEDULER_TICK_SECS: {e}"))?;

        let scheduler_batch_size: usize = std::env::var("SCHEDULER_BATCH_SIZE")
            .unwrap_or_else(|_| "16".to_string())
            .parse::<usize>()
            .map_err(|e| format!("invalid SCHEDULER_BATCH_SIZE: {e}"))?
            .clamp(1, 256);

        let job_conductor_parallelism: usize = std::env::var("JOB_CONDUCTOR_PARALLELISM")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<usize>()
            .map_err(|e| format!("invalid JOB_CONDUCTOR_PARALLELISM: {e}"))?
            .clamp(1, 16);

        let job_conductor_llm_parallelism: usize = std::env::var("JOB_CONDUCTOR_LLM_PARALLELISM")
            .unwrap_or_else(|_| "2".to_string())
            .parse::<usize>()
            .map_err(|e| format!("invalid JOB_CONDUCTOR_LLM_PARALLELISM: {e}"))?
            .clamp(1, 8)
            .min(job_conductor_parallelism);

        let feed_poll_wall_budget_secs: u64 = std::env::var("FEED_POLL_WALL_BUDGET_SECS")
            .unwrap_or_else(|_| "120".to_string())
            .parse::<u64>()
            .map_err(|e| format!("invalid FEED_POLL_WALL_BUDGET_SECS: {e}"))?
            .clamp(30, 240);

        let ops_digest_live = std::env::var("OPS_DIGEST_LIVE")
            .map(|v| v.trim() != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);

        let journal_retention_days: u32 = std::env::var("JOURNAL_RETENTION_DAYS")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<u32>()
            .map_err(|e| format!("invalid JOURNAL_RETENTION_DAYS: {e}"))?
            .clamp(1, 365);

        let csrf_pepper = std::env::var("CSRF_PEPPER")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| database_url.clone());

        let media_hash_blocklist = std::env::var("MEDIA_HASH_BLOCKLIST")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|h| h.trim().to_ascii_lowercase())
                    .filter(|h| !h.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let recaptcha_bypass = std::env::var("RECAPTCHA_BYPASS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let recaptcha = match std::env::var("RECAPTCHA_SECRET_KEY") {
            Ok(secret) if !secret.trim().is_empty() => {
                let site_key = std::env::var("RECAPTCHA_SITE_KEY")
                    .map_err(|_| "RECAPTCHA_SITE_KEY is required when RECAPTCHA_SECRET_KEY is set")?
                    .trim()
                    .to_string();
                if site_key.is_empty() {
                    return Err("RECAPTCHA_SITE_KEY must not be empty".to_string());
                }
                let min_score: f64 = std::env::var("RECAPTCHA_MIN_SCORE")
                    .unwrap_or_else(|_| "0.5".to_string())
                    .parse()
                    .map_err(|e| format!("invalid RECAPTCHA_MIN_SCORE: {e}"))?;
                Some(RecaptchaConfig {
                    site_key,
                    secret_key: secret.trim().to_string(),
                    min_score,
                    bypass: recaptcha_bypass,
                })
            }
            _ => {
                if recaptcha_bypass {
                    if !host.is_loopback() {
                        return Err(
                            "RECAPTCHA_BYPASS=1 requires HOST=127.0.0.1 or ::1 (local dev only)"
                                .to_string(),
                        );
                    }
                    Some(RecaptchaConfig {
                        site_key: String::new(),
                        secret_key: String::new(),
                        min_score: 0.0,
                        bypass: true,
                    })
                } else {
                    None
                }
            }
        };

        let oidc_google = match std::env::var("OIDC_CLIENT_ID") {
            Ok(client_id) if !client_id.trim().is_empty() => {
                let client_secret = std::env::var("OIDC_CLIENT_SECRET")
                    .map_err(|_| "OIDC_CLIENT_SECRET is required when OIDC_CLIENT_ID is set")?
                    .trim()
                    .to_string();
                if client_secret.is_empty() {
                    return Err("OIDC_CLIENT_SECRET must not be empty".to_string());
                }
                let issuer_url = std::env::var("OIDC_ISSUER_URL")
                    .unwrap_or_else(|_| "https://accounts.google.com".to_string())
                    .trim()
                    .trim_end_matches('/')
                    .to_string();
                let redirect_uri = normalize_public_url(
                    &std::env::var("OIDC_REDIRECT_URI")
                        .unwrap_or_else(|_| format!("{public_base_url}/auth/oidc/callback")),
                    secure_cookies,
                );
                Some(OidcGoogleConfig {
                    issuer_url,
                    client_id: client_id.trim().to_string(),
                    client_secret,
                    redirect_uri,
                })
            }
            _ => None,
        };

        if dev_auth_bypass && !host.is_loopback() {
            return Err(
                "DEV_AUTH_BYPASS=1 requires HOST=127.0.0.1 or ::1 (local dev only)".to_string(),
            );
        }
        if skip_face_vision_validate && !host.is_loopback() {
            return Err(
                "SKIP_FACE_VISION_VALIDATE=1 requires HOST=127.0.0.1 or ::1 (local dev only)"
                    .to_string(),
            );
        }

        let stripe_secret = std::env::var("STRIPE_SECRET_KEY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let stripe_publishable = std::env::var("STRIPE_PUBLISHABLE_KEY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let stripe = match (stripe_secret, stripe_publishable) {
            (Some(secret_key), Some(publishable_key)) => {
                let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET")
                    .ok()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                Some(StripeConfig {
                    secret_key,
                    publishable_key,
                    webhook_secret,
                })
            }
            (None, None) => None,
            _ => {
                return Err(
                    "STRIPE_SECRET_KEY and STRIPE_PUBLISHABLE_KEY must both be set or both unset"
                        .to_string(),
                );
            }
        };

        let platform_safety_contact_email = std::env::var("PLATFORM_SAFETY_CONTACT_EMAIL")
            .unwrap_or_else(|_| "safety@example.com".to_string())
            .trim()
            .to_string();
        if platform_safety_contact_email.is_empty() {
            return Err(
                "PLATFORM_SAFETY_CONTACT_EMAIL must not be empty when set".to_string(),
            );
        }

        Ok(Self {
            database_url,
            csrf_pepper,
            host,
            port,
            nominatim_user_agent,
            secure_cookies,
            public_base_url,
            dev_auth_bypass,
            trusted_proxy,
            smtp,
            face_vision_mock,
            skip_face_vision_validate,
            ai_grpc_endpoint,
            ai_grpc_timeout_ms,
            verbose_ai_errors,
            portrait_raster_model,
            portrait_raster_timeout_ms,
            generated_faces_dir,
            council_agent_handle,
            council_invite_review_email,
            platform_recruitment_review_only,
            google_maps_api_key,
            google_places_timeout_ms,
            ops_location_invalidate_token,
            ops_job_queue_token,
            request_timeout_secs,
            scheduler_tick_secs,
            scheduler_batch_size,
            job_conductor_parallelism,
            job_conductor_llm_parallelism,
            feed_poll_wall_budget_secs,
            ops_digest_live,
            journal_retention_days,
            media_hash_blocklist,
            recaptcha,
            oidc_google,
            stripe,
            platform_safety_contact_email,
            council_web_search_api_key,
            council_web_search_provider,
            council_web_search_cx,
            council_web_search_timeout_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_public_url_upgrades_http_when_secure_cookies() {
        assert_eq!(
            normalize_public_url("http://beta.twofivesix.app", true),
            "https://beta.twofivesix.app"
        );
        assert_eq!(
            normalize_public_url(
                "http://beta.twofivesix.app/auth/oidc/callback",
                true
            ),
            "https://beta.twofivesix.app/auth/oidc/callback"
        );
    }

    #[test]
    fn normalize_public_url_adds_https_to_bare_host_when_secure() {
        assert_eq!(
            normalize_public_url("beta.twofivesix.app", true),
            "https://beta.twofivesix.app"
        );
    }

    #[test]
    fn normalize_public_url_keeps_localhost_http() {
        assert_eq!(
            normalize_public_url("http://127.0.0.1:8080", true),
            "http://127.0.0.1:8080"
        );
    }
}
