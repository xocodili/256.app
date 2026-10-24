ALTER TABLE social_profit
    ADD COLUMN content_link_id BIGINT UNSIGNED NULL COMMENT 'Optional canonical business website link (FK content_link.id)' AFTER founded_at;

ALTER TABLE social_profit
    ADD KEY idx_social_profit_content_link_id (content_link_id),
    ADD CONSTRAINT fk_social_profit_content_link
        FOREIGN KEY (content_link_id) REFERENCES content_link (id) ON DELETE SET NULL;
