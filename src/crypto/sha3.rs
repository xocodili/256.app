//! **SHA3-512** (Keccak) digests — standard one-way hash for this application.
//!
//! This is **not** encryption (there is no inverse). Do not use for password storage; use
//! [`crate::auth::password`] (Argon2id) instead.

use sha3::{Digest, Sha3_256, Sha3_512};

/// Raw 32-byte SHA3-256 digest (e.g. location fingerprints, token-sized ids).
#[must_use]
pub fn digest_256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha3_256::new();
    h.update(data);
    h.finalize().into()
}

/// Raw 64-byte SHA3-512 digest.
#[must_use]
pub fn digest(data: &[u8]) -> [u8; 64] {
    let mut h = Sha3_512::new();
    h.update(data);
    h.finalize().into()
}

/// Lowercase hexadecimal encoding of [`digest`].
#[must_use]
pub fn digest_hex(data: &[u8]) -> String {
    hex::encode(digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Must match `UNHEX(...)` in `migrations/20260411000002_seed_system_presence.sql` (v1 payload, no coordinates).
    #[test]
    fn seed_location_fingerprints_match_migrations() {
        fn push_opt(buf: &mut Vec<u8>, s: Option<&str>) {
            if let Some(t) = s {
                buf.extend_from_slice(t.as_bytes());
            }
        }
        fn fp(
            v: u8,
            country: &str,
            region: Option<&str>,
            locality: Option<&str>,
            postal: Option<&str>,
        ) -> String {
            let mut out = Vec::new();
            out.push(v);
            out.extend_from_slice(country.as_bytes());
            out.push(0x1f);
            push_opt(&mut out, region);
            out.push(0x1f);
            push_opt(&mut out, locality);
            out.push(0x1f);
            push_opt(&mut out, postal);
            out.push(0x1f);
            hex::encode(digest_256(&out))
        }
        assert_eq!(
            fp(1, "AU", Some("NSW"), Some("Katoomba"), None),
            "75c5f7cbf3453f0a58562764c9c4156f01d62e703a211090f16d422fff774a68"
        );
        assert_eq!(
            fp(1, "ZZ", None, None, None),
            "d668ba83cc35a3d46e3cf2544d70b4d5194ae479cd434d5556fa26635b0b50cf"
        );
    }

    /// NIST / Keccak empty-string vector (SHA3-512).
    #[test]
    fn sha3_512_empty_matches_nist() {
        const EXPECT: &str = "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26";
        assert_eq!(digest_hex(b""), EXPECT);
    }
}
