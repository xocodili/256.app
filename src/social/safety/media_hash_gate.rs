//! Pre-persist SHA-256 media gate (Class A/B stub — vendor hash lists plug in here).

use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmClass {
    A,
    B,
}

impl HarmClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaScanOutcome {
    Allowed,
    Blocked { harm_class: HarmClass, sha256_hex: String },
}

/// Scan JPEG bytes against configured blocklist (`Config::media_hash_blocklist`).
#[must_use]
pub fn scan_jpeg_bytes(bytes: &[u8], blocklist: &HashSet<String>) -> MediaScanOutcome {
    if blocklist.is_empty() {
        return MediaScanOutcome::Allowed;
    }
    let sha256_hex = sha256_hex(bytes);
    if blocklist.contains(&sha256_hex) {
        return MediaScanOutcome::Blocked {
            harm_class: HarmClass::A,
            sha256_hex,
        };
    }
    MediaScanOutcome::Allowed
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocklist_match_blocks() {
        let bytes = b"test jpeg payload";
        let hash = sha256_hex(bytes);
        let mut blocklist = HashSet::new();
        blocklist.insert(hash.clone());
        assert_eq!(
            scan_jpeg_bytes(bytes, &blocklist),
            MediaScanOutcome::Blocked {
                harm_class: HarmClass::A,
                sha256_hex: hash,
            }
        );
    }

    #[test]
    fn empty_blocklist_allows() {
        assert_eq!(
            scan_jpeg_bytes(b"anything", &HashSet::new()),
            MediaScanOutcome::Allowed
        );
    }
}
