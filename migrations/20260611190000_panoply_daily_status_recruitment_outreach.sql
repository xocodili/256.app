-- Panoply operator daily status email + global platform recruitment dedupe (one invite per email).
INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'panoply_daily_status', 58, 1, NULL, NULL);

CREATE TABLE IF NOT EXISTS platform_recruitment_outreach (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    email_canonical VARCHAR(254) NOT NULL COMMENT 'Normalized recipient — global once-only gate',
    invited_by_social_entity_id BIGINT UNSIGNED NOT NULL,
    invited_by_ai_actor_id BIGINT UNSIGNED NULL,
    source_post_id BIGINT UNSIGNED NULL,
    source_content_link_id BIGINT UNSIGNED NULL,
    intended_label VARCHAR(255) NULL,
    delivery_route VARCHAR(32) NOT NULL DEFAULT 'review_routed' COMMENT 'review_routed | direct',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY uq_platform_recruitment_outreach_email (email_canonical),
    KEY idx_platform_recruitment_outreach_created (created_at),
    KEY idx_platform_recruitment_outreach_inviter (invited_by_social_entity_id),
    CONSTRAINT fk_pro_inviter_entity FOREIGN KEY (invited_by_social_entity_id) REFERENCES social_entity (id) ON DELETE RESTRICT,
    CONSTRAINT fk_pro_ai_actor FOREIGN KEY (invited_by_ai_actor_id) REFERENCES ai_actor (id) ON DELETE SET NULL,
    CONSTRAINT fk_pro_source_post FOREIGN KEY (source_post_id) REFERENCES post (id) ON DELETE SET NULL,
    CONSTRAINT fk_pro_source_link FOREIGN KEY (source_content_link_id) REFERENCES content_link (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
