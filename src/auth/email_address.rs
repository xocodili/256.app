//! Email address handling: deliverable normalization, canonicalization for uniqueness,
//! and a disposable-domain denylist check (DB-driven, mirroring the `content_domain` pattern).
//!
//! Two distinct forms are used deliberately:
//! - **normalized** (`normalize`): trimmed + lowercased. This is the *deliverable* address we
//!   store in `users.email` and send mail to. Safe for every provider.
//! - **canonical** (`canonical`): the normalized form with provider-specific aliasing collapsed
//!   (Gmail dot-insensitivity + `+tag` subaddressing). Stored in `users.email_canonical` with a
//!   UNIQUE index so common dup vectors (`j.doe+spam@gmail.com` vs `jdoe@gmail.com`) collapse to a
//!   single account. Never used as a delivery target.

use sqlx::MySqlPool;

/// Gmail-family domains where dots in the local part are ignored and `googlemail.com` aliases
/// `gmail.com`.
const GMAIL_DOMAINS: [&str; 2] = ["gmail.com", "googlemail.com"];

/// Deliverable form: trimmed + lowercased. Use this for storage in `users.email` and for sending.
pub fn normalize(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

/// Domain (host) portion of an already-normalized address, if present and non-empty.
pub fn domain_of(normalized: &str) -> Option<&str> {
    normalized
        .rsplit_once('@')
        .map(|(_, domain)| domain)
        .filter(|d| !d.is_empty())
}

/// Canonical form for uniqueness checks. Lowercases, strips `+tag` subaddressing from the local
/// part, removes Gmail dot-insensitivity, and collapses `googlemail.com` to `gmail.com`.
///
/// For addresses without a single `@`, falls back to the normalized form so callers still get a
/// stable key (validation rejects malformed addresses separately).
pub fn canonical(raw: &str) -> String {
    let normalized = normalize(raw);
    let Some((local, domain)) = normalized.rsplit_once('@') else {
        return normalized;
    };
    if local.is_empty() || domain.is_empty() {
        return normalized;
    }

    // Strip everything from the first '+' (subaddressing), supported by most providers.
    let local = local.split('+').next().unwrap_or(local);

    let is_gmail = GMAIL_DOMAINS.contains(&domain);
    let local = if is_gmail {
        // Gmail ignores dots in the local part.
        local.replace('.', "")
    } else {
        local.to_string()
    };
    let domain = if domain == "googlemail.com" {
        "gmail.com"
    } else {
        domain
    };

    if local.is_empty() {
        // Degenerate local part (e.g. "+tag@gmail.com" or "...@gmail.com"): keep normalized.
        return normalized;
    }

    format!("{local}@{domain}")
}

/// Whether the (already-normalized, lowercase) registrable email domain is on the disposable
/// denylist. Mirrors `content::links::registrable_domain_is_blocked` (DB-driven, current rows).
pub async fn domain_is_disposable(pool: &MySqlPool, domain: &str) -> Result<bool, sqlx::Error> {
    let row: Option<(u64,)> =
        sqlx::query_as(r#"SELECT id FROM blocked_email_domain WHERE domain = ? LIMIT 1"#)
            .bind(domain)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_dots_and_plus_collapse() {
        assert_eq!(canonical("J.Doe+spam@Gmail.com"), "jdoe@gmail.com");
        assert_eq!(canonical("jdoe@gmail.com"), "jdoe@gmail.com");
        assert_eq!(canonical("j.d.o.e@googlemail.com"), "jdoe@gmail.com");
    }

    #[test]
    fn non_gmail_keeps_dots_but_strips_plus() {
        assert_eq!(canonical("First.Last+news@outlook.com"), "first.last@outlook.com");
        assert_eq!(canonical("user@example.com"), "user@example.com");
    }

    #[test]
    fn degenerate_inputs_fall_back_to_normalized() {
        assert_eq!(canonical("  Bad@@ "), "bad@@");
        assert_eq!(canonical("+tag@gmail.com"), "+tag@gmail.com");
        assert_eq!(canonical("noatsign"), "noatsign");
    }

    #[test]
    fn domain_extraction() {
        assert_eq!(domain_of("user@example.com"), Some("example.com"));
        assert_eq!(domain_of("nodomain@"), None);
        assert_eq!(domain_of("noat"), None);
    }
}
