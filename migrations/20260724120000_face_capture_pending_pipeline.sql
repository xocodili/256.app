-- Async face portrait pipeline: ephemeral JPEG staging + job queue handlers.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('ops_job_type', 'face_portrait_pipeline', 45, 1, NULL, NULL),
('ops_job_type', 'face_capture_pending_cleanup', 46, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE IF NOT EXISTS face_capture_pending (
    id CHAR(36) NOT NULL COMMENT 'UUID v4 pending capture id',
    user_id BIGINT UNSIGNED NOT NULL COMMENT 'FK users.id',
    human_entity_id BIGINT UNSIGNED NULL COMMENT 'FK social_entity.id (human) for billing',
    billing_mode ENUM('onboarding', 'avatar_redraw') NOT NULL,
    status ENUM('queued', 'processing', 'complete', 'failed') NOT NULL DEFAULT 'queued',
    quality_score FLOAT NULL COMMENT 'Client-reported capture quality average',
    user_face_id BIGINT UNSIGNED NULL COMMENT 'FK user_faces.id when complete (onboarding insert)',
    generated_artwork_path VARCHAR(512) NULL,
    portrait_model_id VARCHAR(128) NULL,
    noms_spent SMALLINT UNSIGNED NULL,
    noms_balance_after SMALLINT UNSIGNED NULL,
    error_code VARCHAR(64) NULL,
    error_message VARCHAR(512) NULL,
    expires_at TIMESTAMP(6) NOT NULL COMMENT 'Poll window + disk TTL',
    created_at TIMESTAMP(6) NOT NULL DEFAULT (UTC_TIMESTAMP(6)),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT (UTC_TIMESTAMP(6)) ON UPDATE CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    KEY idx_face_capture_pending_user_status (user_id, status, expires_at),
    KEY idx_face_capture_pending_expires (expires_at),
    CONSTRAINT fk_face_capture_pending_user
        FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
