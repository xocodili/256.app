-- Self-serve membership review applications (judicial_case trigger membership_review).

CREATE TABLE IF NOT EXISTS judicial_case_membership_review (
    judicial_case_id BIGINT UNSIGNED NOT NULL COMMENT 'FK judicial_case.id',
    application_statement_plain TEXT NOT NULL,
    application_statement_html TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (judicial_case_id),
    CONSTRAINT fk_jcmr_case
        FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS community_membership_reapply_barrier (
    human_social_entity_id BIGINT UNSIGNED NOT NULL,
    community_social_entity_id BIGINT UNSIGNED NOT NULL,
    barred_until DATETIME(6) NOT NULL,
    source_judicial_case_id BIGINT UNSIGNED NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (human_social_entity_id, community_social_entity_id),
    KEY idx_cmrb_barred_until (barred_until),
    CONSTRAINT fk_cmrb_human
        FOREIGN KEY (human_social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_cmrb_community
        FOREIGN KEY (community_social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_cmrb_case
        FOREIGN KEY (source_judicial_case_id) REFERENCES judicial_case (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
