-- Jury nomination lottery: system job type plus invite rows with rating snapshots.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'judicial_jury_nomination', 40, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE judicial_case_juror_invite (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    judicial_case_id BIGINT UNSIGNED NOT NULL COMMENT 'FK judicial_case.id',
    juror_social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id; human selected by lottery',
    juror_user_id BIGINT UNSIGNED NULL COMMENT 'FK users.id when the selected human has an account owner',
    juror_rating_at_invite DECIMAL(3, 2) NOT NULL DEFAULT 0.00 COMMENT 'Snapshot of social_entity.rating when selected',
    invite_status VARCHAR(16) NOT NULL DEFAULT 'pending' COMMENT 'pending | accepted | declined | expired | voided',
    selected_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    responded_at DATETIME(6) NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_judicial_case_juror (judicial_case_id, juror_social_entity_id),
    KEY idx_jcji_case_status (judicial_case_id, invite_status),
    KEY idx_jcji_juror (juror_social_entity_id),
    KEY idx_jcji_user (juror_user_id),
    CONSTRAINT chk_jcji_rating_snapshot CHECK (juror_rating_at_invite >= 0.00 AND juror_rating_at_invite <= 5.00),
    CONSTRAINT chk_jcji_status CHECK (invite_status IN ('pending', 'accepted', 'declined', 'expired', 'voided')),
    CONSTRAINT fk_jcji_case FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jcji_juror_entity FOREIGN KEY (juror_social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_jcji_juror_user FOREIGN KEY (juror_user_id) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_jcji_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_jcji_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
