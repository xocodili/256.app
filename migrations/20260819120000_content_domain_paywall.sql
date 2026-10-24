-- Council share: domain paywall profile (does not affect domain rating).
-- Default `free` — assume readable until a paywall observation says otherwise.

ALTER TABLE content_domain
    ADD COLUMN paywall VARCHAR(16) NOT NULL DEFAULT 'free'
        COMMENT 'free|partial|whole — council domain selection; not used for rating'
        AFTER safety_state,
    ADD CONSTRAINT chk_content_domain_paywall
        CHECK (paywall IN ('free', 'partial', 'whole'));

CREATE INDEX idx_content_domain_paywall ON content_domain (paywall);
