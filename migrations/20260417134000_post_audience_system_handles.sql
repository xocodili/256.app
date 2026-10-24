-- System-scoped post audience handles (not user/entity-owned): @friends, @family, @private, @public.
CREATE TABLE post_audience_scope (
    post_id BIGINT UNSIGNED NOT NULL,
    scope_key VARCHAR(32) NOT NULL COMMENT 'friends|family|private|public',
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    created_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (post_id, scope_key),
    KEY idx_post_audience_scope_key (scope_key, post_id),
    CONSTRAINT chk_post_audience_scope_key CHECK (scope_key IN ('friends', 'family', 'private', 'public')),
    CONSTRAINT fk_post_audience_scope_post FOREIGN KEY (post_id) REFERENCES post (id) ON DELETE CASCADE,
    CONSTRAINT fk_post_audience_scope_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
