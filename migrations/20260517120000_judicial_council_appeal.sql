-- Per-council-member votes on AI-escalated appeals + queue job type.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'judicial_council_appeal', 50, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE judicial_case_council_vote (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    judicial_case_id BIGINT UNSIGNED NOT NULL,
    ai_actor_id BIGINT UNSIGNED NOT NULL,
    vote_direction_id BIGINT UNSIGNED NOT NULL COMMENT 'for = uphold moderation; against = overturn',
    rationale TEXT NULL,
    model_ref VARCHAR(128) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_jccv_case_actor (judicial_case_id, ai_actor_id),
    KEY idx_jccv_case (judicial_case_id),
    CONSTRAINT fk_jccv_case FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jccv_actor FOREIGN KEY (ai_actor_id) REFERENCES ai_actor (id) ON DELETE CASCADE,
    CONSTRAINT fk_jccv_vote_direction FOREIGN KEY (vote_direction_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_jccv_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_jccv_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
