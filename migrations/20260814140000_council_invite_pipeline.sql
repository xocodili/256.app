-- Council invite pipeline: four-stage job chain + scratch state.

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('ops_job_type', 'council_invite_article_pick', 55, 1, NULL, NULL),
    ('ops_job_type', 'council_invite_person_discover', 55, 1, NULL, NULL),
    ('ops_job_type', 'council_invite_contact_resolve', 55, 1, NULL, NULL),
    ('ops_job_type', 'council_invite_compose_send', 55, 1, NULL, NULL);

CREATE TABLE council_invite_pipeline_state (
    correlation_id CHAR(36) NOT NULL,
    ai_actor_slug VARCHAR(32) NOT NULL,
    `utc_date` DATE NOT NULL,
    stage VARCHAR(32) NOT NULL,
    payload JSON NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    expires_at TIMESTAMP(6) NOT NULL,
    PRIMARY KEY (correlation_id),
    KEY idx_invite_pipeline_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
