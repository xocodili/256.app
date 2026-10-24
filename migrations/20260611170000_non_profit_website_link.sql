-- Align non-profit website storage with profit (`social_profit.content_link_id`).
ALTER TABLE social_non_profit
    ADD COLUMN content_link_id BIGINT UNSIGNED NULL
        COMMENT 'Optional canonical website link (FK content_link.id)'
        AFTER founded_at,
    ADD KEY idx_social_non_profit_content_link_id (content_link_id),
    ADD CONSTRAINT fk_social_non_profit_content_link
        FOREIGN KEY (content_link_id) REFERENCES content_link (id) ON DELETE SET NULL;
