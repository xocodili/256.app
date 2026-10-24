-- Post foundation: one-link-per-post, bounded text, status lifecycle, and dialogue tree support.

CREATE TABLE post (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id (authoring entity)',
    content_link_id BIGINT UNSIGNED NOT NULL COMMENT 'FK content_link.id; one canonical link per post',
    parent_post_id BIGINT UNSIGNED NULL COMMENT 'Self-FK for dialogue tree; NULL for root posts',
    status_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group status; defaults to verified via trigger',
    rating DECIMAL(3,2) NOT NULL DEFAULT 0.00 COMMENT 'Internal qualitative score, 0.00..5.00',
    body_plain NVARCHAR(4000) NOT NULL COMMENT 'Plain-text extraction for search/moderation',
    body_markup NVARCHAR(4000) NOT NULL COMMENT 'Restricted rich-text source (no inline links)',
    body_html_sanitized NVARCHAR(4000) NOT NULL COMMENT 'Safe render target generated server-side',
    published_at DATETIME(6) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_post_content_link (content_link_id),
    KEY idx_post_entity_id (entity_id),
    KEY idx_post_parent_post_id (parent_post_id),
    KEY idx_post_status_id (status_id),
    KEY idx_post_published_at (published_at),
    KEY idx_post_created_by (created_by),
    KEY idx_post_modified_by (modified_by),
    CONSTRAINT chk_post_rating_range CHECK (rating >= 0.00 AND rating <= 5.00),
    CONSTRAINT chk_post_body_plain_len CHECK (CHAR_LENGTH(body_plain) <= 4000),
    CONSTRAINT chk_post_body_markup_len CHECK (CHAR_LENGTH(body_markup) <= 4000),
    CONSTRAINT chk_post_body_html_len CHECK (CHAR_LENGTH(body_html_sanitized) <= 4000),
    CONSTRAINT fk_post_entity FOREIGN KEY (entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_post_parent_post FOREIGN KEY (parent_post_id) REFERENCES post (id) ON DELETE SET NULL,
    CONSTRAINT fk_post_status FOREIGN KEY (status_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_post_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_post_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Add `content_link` FK when the table is present in this environment.
SET @has_content_link := (
    SELECT COUNT(*)
    FROM information_schema.tables
    WHERE table_schema = DATABASE()
      AND table_name = 'content_link'
);
SET @post_fk_sql := IF(
    @has_content_link = 1,
    'ALTER TABLE post ADD CONSTRAINT fk_post_content_link FOREIGN KEY (content_link_id) REFERENCES content_link (id) ON DELETE RESTRICT',
    'SELECT 1'
);
PREPARE post_fk_stmt FROM @post_fk_sql;
EXECUTE post_fk_stmt;
DEALLOCATE PREPARE post_fk_stmt;

-- Default new posts to lifecycle status `verified` until the app sets statuses explicitly.
-- NOTE: we avoid CREATE TRIGGER in greenfield dev (some MySQL installs disallow it without elevated privileges
-- when binary logging is enabled). The app should populate post.status_id on insert.
