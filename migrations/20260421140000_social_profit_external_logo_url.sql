ALTER TABLE social_profit
    ADD COLUMN external_logo_url VARCHAR(2048) NULL COMMENT 'Optional HTTPS URL to a self-hosted logo image (square-ish recommended)' AFTER content_link_id;
