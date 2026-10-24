-- Acute harm sanitation audit log (Root point 15 due diligence).

CREATE TABLE IF NOT EXISTS safety_sanitation_event (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    action_class VARCHAR(32) NOT NULL COMMENT 'hash_gate_block, domain_deny, class_c_contain',
    harm_class VARCHAR(8) NULL COMMENT 'A, B, C when applicable',
    source VARCHAR(64) NOT NULL COMMENT 'face_capture, content_link_ingest, create_post, ...',
    actor_user_id BIGINT UNSIGNED NULL COMMENT 'FK users.id — uploader when known',
    target_user_id BIGINT UNSIGNED NULL COMMENT 'FK users.id — subject when known',
    target_post_id BIGINT UNSIGNED NULL COMMENT 'FK post.id when post-scoped',
    content_sha256 CHAR(64) NULL COMMENT 'SHA-256 hex of blocked media bytes',
    registrable_domain VARCHAR(253) NULL COMMENT 'blocked outbound domain',
    detail_json JSON NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    KEY idx_safety_sanitation_created (created_at),
    KEY idx_safety_sanitation_class (action_class, created_at),
    KEY idx_safety_sanitation_actor (actor_user_id),
    KEY idx_safety_sanitation_domain (registrable_domain),
    CONSTRAINT fk_safety_sanitation_actor FOREIGN KEY (actor_user_id) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_safety_sanitation_target_user FOREIGN KEY (target_user_id) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_safety_sanitation_post FOREIGN KEY (target_post_id) REFERENCES post (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
