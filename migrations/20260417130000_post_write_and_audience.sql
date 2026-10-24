-- Enable optional no-link posts + persist explicit audience targets.
ALTER TABLE post
    MODIFY COLUMN content_link_id BIGINT UNSIGNED NULL COMMENT 'FK content_link.id; optional canonical link for link-bearing posts';

CREATE TABLE post_audience_entity (
    post_id BIGINT UNSIGNED NOT NULL,
    audience_entity_id BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    created_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (post_id, audience_entity_id),
    KEY idx_post_audience_entity (audience_entity_id, post_id),
    CONSTRAINT fk_post_audience_post FOREIGN KEY (post_id) REFERENCES post (id) ON DELETE CASCADE,
    CONSTRAINT fk_post_audience_entity FOREIGN KEY (audience_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_post_audience_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
