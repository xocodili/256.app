//! Godmother outreach moments (v1).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GodmotherMoment {
    Welcome,
    GlobalFailing,
    GlobalRecovery,
}

impl GodmotherMoment {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Welcome => "welcome",
            Self::GlobalFailing => "global_failing",
            Self::GlobalRecovery => "global_recovery",
        }
    }

    #[must_use]
    pub fn from_key(raw: &str) -> Option<Self> {
        match raw.trim() {
            "welcome" => Some(Self::Welcome),
            "global_failing" => Some(Self::GlobalFailing),
            "global_recovery" => Some(Self::GlobalRecovery),
            _ => None,
        }
    }
}
