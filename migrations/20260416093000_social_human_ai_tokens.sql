-- Human-only profile credits for downstream AI council usage accounting.
-- Internal field (not user-facing): capped integer range 0..256.

ALTER TABLE social_human
    ADD COLUMN ai_tokens SMALLINT UNSIGNED NOT NULL DEFAULT 0
        COMMENT 'Internal AI council service credits (0..256)'
        AFTER join_policy_id,
    ADD CONSTRAINT chk_social_human_ai_tokens_range CHECK (ai_tokens <= 256);
