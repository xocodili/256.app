-- Profit / non-profit logos: canonical URL via `content_link` (same domain + dedupe path as website links).

ALTER TABLE social_profit
    ADD COLUMN logo_content_link_id BIGINT UNSIGNED NULL COMMENT 'Optional logo image URL via content_link.id (FK)' AFTER content_link_id,
    ADD KEY idx_social_profit_logo_content_link_id (logo_content_link_id),
    ADD CONSTRAINT fk_social_profit_logo_content_link
        FOREIGN KEY (logo_content_link_id) REFERENCES content_link (id) ON DELETE SET NULL;

-- Optional: remove legacy raw URL column after backfill (not required for new installs):
-- ALTER TABLE social_profit DROP COLUMN external_logo_url;

ALTER TABLE social_non_profit
    ADD COLUMN logo_content_link_id BIGINT UNSIGNED NULL COMMENT 'Optional logo image URL via content_link.id (FK)' AFTER founded_at,
    ADD KEY idx_social_non_profit_logo_content_link_id (logo_content_link_id),
    ADD CONSTRAINT fk_social_non_profit_logo_content_link
        FOREIGN KEY (logo_content_link_id) REFERENCES content_link (id) ON DELETE SET NULL;
