//! Fixed copy for post-moderation verdict notices (feed + mirrored thread commentary).
//!
//! Canonical user-facing strings live in `locales/en.json` → `ui.post_verdict.*` and are rendered
//! via `src/ui/post_card/feed.rs`. These constants are kept for tests/docs that need stable Rust literals.

pub const EXONERATED_HUMAN: &str =
    "A jury of human peers reviewed this thread against the community constitution and cleared it. The post is visible again.";
pub const EXONERATED_COUNCIL: &str =
    "A human jury upheld a report; the Council of 12 on appeal found no violation of the community constitution. The post is visible again.";
pub const REMOVED_HUMAN: &str =
    "A jury of human peers upheld a report against the community constitution. The thread stays hidden here as a stub — the original text is not shown.";
pub const REMOVED_COUNCIL: &str =
    "A human jury upheld a report; the Council of 12 on appeal confirmed it against the community constitution. The thread stays hidden here as a stub.";
