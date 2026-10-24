-- Parent "ticket" for judicial / legislative workflows (membership review, moderation appeal,
-- constitutional amendment). `moderation_case` remains the post-scoped workflow table; optional
-- `judicial_case_id` links a post moderation filing to the umbrella case when populated from app code.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('judicial_case_trigger', 'membership_review', 10, 1, NULL, NULL),
('judicial_case_trigger', 'moderation_appeal', 20, 1, NULL, NULL),
('judicial_case_trigger', 'constitutional_amendment', 30, 1, NULL, NULL),
('judicial_case_status', 'open', 10, 1, NULL, NULL),
('judicial_case_status', 'jury_active', 20, 1, NULL, NULL),
('judicial_case_status', 'escalated_to_ai', 30, 1, NULL, NULL),
('judicial_case_status', 'resolved', 40, 1, NULL, NULL),
('judicial_case_status', 'voided', 50, 1, NULL, NULL),
('judicial_case_comment_type', 'originator', 10, 1, NULL, NULL),
('judicial_case_comment_type', 'evidence', 20, 1, NULL, NULL),
('judicial_case_comment_type', 'rebuttal', 30, 1, NULL, NULL),
('judicial_case_comment_type', 'ai_brief', 40, 1, NULL, NULL),
('judicial_case_comment_type', 'verdict_justification', 50, 1, NULL, NULL),
('judicial_case_comment_type', 'closing_statement', 60, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE judicial_case (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    originator_social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — party opening the case',
    target_social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — subject of the case',
    trigger_type_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group judicial_case_trigger',
    status_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group judicial_case_status',
    opened_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP NULL DEFAULT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_judicial_case_originator (originator_social_entity_id),
    KEY idx_judicial_case_target (target_social_entity_id),
    KEY idx_judicial_case_trigger (trigger_type_id),
    KEY idx_judicial_case_status (status_id),
    KEY idx_judicial_case_opened (opened_at),
    CONSTRAINT chk_judicial_case_distinct_parties CHECK (originator_social_entity_id <> target_social_entity_id),
    CONSTRAINT fk_judicial_case_originator FOREIGN KEY (originator_social_entity_id) REFERENCES social_entity (id) ON DELETE RESTRICT,
    CONSTRAINT fk_judicial_case_target FOREIGN KEY (target_social_entity_id) REFERENCES social_entity (id) ON DELETE RESTRICT,
    CONSTRAINT fk_judicial_case_trigger FOREIGN KEY (trigger_type_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_judicial_case_status FOREIGN KEY (status_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_judicial_case_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_judicial_case_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE judicial_case_comment (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    judicial_case_id BIGINT UNSIGNED NOT NULL COMMENT 'FK judicial_case.id',
    type_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group judicial_case_comment_type',
    body_plain NVARCHAR(4000) NOT NULL COMMENT 'Plain-text extraction (same contract as post.body_plain)',
    body_markup NVARCHAR(4000) NOT NULL COMMENT 'Restricted rich-text source (same contract as post.body_markup)',
    body_html_sanitized NVARCHAR(4000) NOT NULL COMMENT 'Safe render target (same contract as post.body_html_sanitized)',
    author_user_id BIGINT UNSIGNED NOT NULL COMMENT 'FK users.id — account that authored the comment',
    author_social_entity_id BIGINT UNSIGNED NULL COMMENT 'FK social_entity.id — entity the author spoke as, if applicable',
    author_entity_rating_at_create DECIMAL(3, 2) NOT NULL DEFAULT 0.00 COMMENT 'Snapshot of social_entity.rating for author_social_entity_id at insert',
    in_reply_to_comment_id BIGINT UNSIGNED NULL COMMENT 'FK judicial_case_comment.id — prior row for rebuttal threading',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_judicial_case_comment_case (judicial_case_id),
    KEY idx_judicial_case_comment_type (type_id),
    KEY idx_judicial_case_comment_author_user (author_user_id),
    KEY idx_judicial_case_comment_reply (in_reply_to_comment_id),
    CONSTRAINT chk_jcc_rating_snapshot CHECK (author_entity_rating_at_create >= 0.00 AND author_entity_rating_at_create <= 5.00),
    CONSTRAINT chk_jcc_plain_len CHECK (CHAR_LENGTH(body_plain) <= 4000),
    CONSTRAINT chk_jcc_markup_len CHECK (CHAR_LENGTH(body_markup) <= 4000),
    CONSTRAINT chk_jcc_html_len CHECK (CHAR_LENGTH(body_html_sanitized) <= 4000),
    CONSTRAINT fk_jcc_case FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jcc_type FOREIGN KEY (type_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_jcc_author_user FOREIGN KEY (author_user_id) REFERENCES users (id) ON DELETE RESTRICT,
    CONSTRAINT fk_jcc_author_entity FOREIGN KEY (author_social_entity_id) REFERENCES social_entity (id) ON DELETE SET NULL,
    CONSTRAINT fk_jcc_reply FOREIGN KEY (in_reply_to_comment_id) REFERENCES judicial_case_comment (id) ON DELETE SET NULL,
    CONSTRAINT fk_jcc_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_jcc_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

ALTER TABLE moderation_case
    ADD COLUMN judicial_case_id BIGINT UNSIGNED NULL COMMENT 'FK judicial_case.id — umbrella ticket when bridged' AFTER id,
    ADD KEY idx_moderation_case_judicial_case (judicial_case_id),
    ADD CONSTRAINT fk_moderation_case_judicial_case FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE SET NULL;
