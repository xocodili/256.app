-- Feed ad delivery tally for Nom accrual.
-- Every 13 ad cards sent in feed responses awards 1 Nom (`social_human.ai_tokens`), capped at 256.

ALTER TABLE social_human
    ADD COLUMN ad_delivery_tally TINYINT UNSIGNED NOT NULL DEFAULT 0
        COMMENT '0..12 ad cards toward the next Nom credit'
        AFTER ai_tokens,
    ADD CONSTRAINT chk_social_human_ad_delivery_tally CHECK (ad_delivery_tally < 13);
