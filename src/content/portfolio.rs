//! AI councillor outbound-host portfolios and entity profile link sizing.

/// Target outbound hosts per active councillor (`ai_actor_content_domain` rows).
pub const AI_PORTFOLIO_DOMAIN_TARGET: u32 = 32;

/// Catalogue window passed to council content-share prompts (matches portfolio target).
pub const AI_PORTFOLIO_CATALOGUE_LIMIT: u32 = AI_PORTFOLIO_DOMAIN_TARGET;

/// Daily published-share goal per councillor (`council_post_tick` Algorithm 2).
pub const COUNCIL_SHARE_GOAL: u32 = 3;

/// Replacement LLM retries per evicted host before giving up this tick.
pub const MAX_PORTFOLIO_REPLACE_ATTEMPTS: u32 = 3;

/// Steward-relevant `content_domain` rows offered as replacement catalogue context.
pub const PORTFOLIO_REPLACE_CATALOGUE_LIMIT: u32 = 40;

/// Minutes between per-councillor share shortlist jobs (12 × 5 min ≈ 1 hour spread).
pub const COUNCIL_POST_MEMBER_STAGGER_MINS: i64 = 5;

/// Shortlist pool size (portfolio slice-of-life candidates).
pub const COUNCIL_SHORTLIST_POOL: usize = 32;

/// Target shortlist size after LLM pass.
pub const COUNCIL_SHORTLIST_TARGET: usize = 8;

/// Max shortlist re-attempts per correlation per UTC day.
pub const COUNCIL_SHORTLIST_MAX_ATTEMPTS: u32 = 3;
