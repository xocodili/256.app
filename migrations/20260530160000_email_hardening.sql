-- Registration hardening:
--   1. `users.email_canonical` — canonical form for uniqueness (Gmail dot-insensitivity + `+tag`
--      subaddressing collapsed). UNIQUE so common dup vectors map to a single account. The
--      deliverable address stays in `users.email`.
--   2. `blocked_email_domain` — DB-driven disposable/temp-mail denylist checked at registration,
--      mirroring the `content_domain` block pattern.

-- 1. Canonical email column + backfill -------------------------------------------------------
ALTER TABLE users
    ADD COLUMN email_canonical VARCHAR(255) NULL
        COMMENT 'Canonical email for uniqueness (gmail dots + +tag collapsed); NOT a delivery target'
        AFTER email;

-- Backfill: lowercase, strip from '+', and (for gmail/googlemail) strip dots + alias to gmail.com.
UPDATE users
SET email_canonical = CONCAT(
    CASE
        WHEN SUBSTRING_INDEX(LOWER(email), '@', -1) IN ('gmail.com', 'googlemail.com')
            THEN REPLACE(SUBSTRING_INDEX(SUBSTRING_INDEX(LOWER(email), '@', 1), '+', 1), '.', '')
        ELSE SUBSTRING_INDEX(SUBSTRING_INDEX(LOWER(email), '@', 1), '+', 1)
    END,
    '@',
    CASE
        WHEN SUBSTRING_INDEX(LOWER(email), '@', -1) = 'googlemail.com' THEN 'gmail.com'
        ELSE SUBSTRING_INDEX(LOWER(email), '@', -1)
    END
)
WHERE email_canonical IS NULL
  AND email LIKE '%@%';

ALTER TABLE users
    ADD UNIQUE KEY uq_users_email_canonical (email_canonical);

-- 2. Disposable / temp-mail domain denylist --------------------------------------------------
CREATE TABLE blocked_email_domain (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    domain VARCHAR(255) NOT NULL,
    reason VARCHAR(255) NOT NULL DEFAULT 'Disposable / temporary email provider.',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_blocked_email_domain (domain),
    KEY idx_blocked_email_domain_created_by (created_by),
    KEY idx_blocked_email_domain_modified_by (modified_by),
    CONSTRAINT fk_blocked_email_domain_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_blocked_email_domain_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SET @seed_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

INSERT INTO blocked_email_domain (domain, reason, created_by, modified_by)
VALUES
    ('mailinator.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('guerrillamail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('guerrillamailblock.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('sharklasers.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('grr.la', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('10minutemail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('temp-mail.org', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('tempmail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('tempmailo.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('yopmail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('throwawaymail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('getnada.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('trashmail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('maildrop.cc', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('dispostable.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('fakeinbox.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('mintemail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('mohmal.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('emailondeck.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('moakt.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('tempr.email', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('mailnesia.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('discard.email', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('spam4.me', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('inboxkitten.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('mailcatch.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('mytemp.email', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('tutanota-temp.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('mailinator.net', 'Disposable / temporary email provider.', @seed_uid, @seed_uid),
    ('1secmail.com', 'Disposable / temporary email provider.', @seed_uid, @seed_uid)
ON DUPLICATE KEY UPDATE domain = VALUES(domain);
