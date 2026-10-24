//! Shared graph lifecycle types.

use crate::ref_keys;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathSource {
    RatingZero,
    Voluntary,
    ManualOps,
}

impl DeathSource {
    pub(crate) const fn ref_key(self) -> &'static str {
        match self {
            Self::RatingZero => ref_keys::KEY_GRAPH_DEATH_SOURCE_RATING_ZERO,
            Self::Voluntary => ref_keys::KEY_GRAPH_DEATH_SOURCE_USER_REQUESTED,
            Self::ManualOps => ref_keys::KEY_GRAPH_DEATH_SOURCE_MANUAL_OPS,
        }
    }
}
