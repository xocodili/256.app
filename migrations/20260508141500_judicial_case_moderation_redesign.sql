-- Post moderation is a `judicial_case` with trigger `post_moderation`, not a separate `moderation_case`.
-- Drops legacy moderation_case tables; extends judicial_case / judicial_case_comment; constitution links move to judicial_case_constitution_item.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('judicial_case_trigger', 'post_moderation', 15, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

SET @fk_exists := (
    SELECT COUNT(*)
    FROM information_schema.TABLE_CONSTRAINTS
    WHERE CONSTRAINT_SCHEMA = DATABASE()
      AND TABLE_NAME = 'moderation_case'
      AND CONSTRAINT_NAME = 'fk_moderation_case_judicial_case'
);
SET @drop_fk := IF(
    @fk_exists > 0,
    'ALTER TABLE moderation_case DROP FOREIGN KEY fk_moderation_case_judicial_case',
    'SELECT 1'
);
PREPARE drop_fk_stmt FROM @drop_fk;
EXECUTE drop_fk_stmt;
DEALLOCATE PREPARE drop_fk_stmt;

DROP TABLE IF EXISTS moderation_case_clause;
DROP TABLE IF EXISTS moderation_case;

ALTER TABLE judicial_case
    ADD COLUMN subject_post_id BIGINT UNSIGNED NULL COMMENT 'FK post.id — primary post artifact when trigger is post_moderation' AFTER status_id,
    ADD COLUMN jury_votes_for TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER closed_at,
    ADD COLUMN jury_votes_against TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER jury_votes_for,
    ADD COLUMN ai_votes_for TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER jury_votes_against,
    ADD COLUMN ai_votes_against TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER ai_votes_for,
    ADD COLUMN resolution_tier_key VARCHAR(32) NULL COMMENT 'jury | ai_council (workflow hint)' AFTER ai_votes_against,
    ADD KEY idx_judicial_case_subject_post (subject_post_id),
    ADD CONSTRAINT fk_judicial_case_subject_post FOREIGN KEY (subject_post_id) REFERENCES post (id) ON DELETE SET NULL;

SET @chk_exists := (
    SELECT COUNT(*)
    FROM information_schema.TABLE_CONSTRAINTS
    WHERE CONSTRAINT_SCHEMA = DATABASE()
      AND TABLE_NAME = 'judicial_case'
      AND CONSTRAINT_NAME = 'chk_judicial_case_distinct_parties'
      AND CONSTRAINT_TYPE = 'CHECK'
);
SET @drop_chk := IF(
    @chk_exists > 0,
    'ALTER TABLE judicial_case DROP CHECK chk_judicial_case_distinct_parties',
    'SELECT 1'
);
PREPARE drop_chk_stmt FROM @drop_chk;
EXECUTE drop_chk_stmt;
DEALLOCATE PREPARE drop_chk_stmt;

ALTER TABLE judicial_case_comment
    ADD COLUMN linked_post_id BIGINT UNSIGNED NULL COMMENT 'FK post.id — e.g. post attached as evidence' AFTER in_reply_to_comment_id,
    ADD KEY idx_judicial_case_comment_linked_post (linked_post_id),
    ADD CONSTRAINT fk_jcc_linked_post FOREIGN KEY (linked_post_id) REFERENCES post (id) ON DELETE SET NULL;

CREATE TABLE judicial_case_constitution_item (
    judicial_case_id BIGINT UNSIGNED NOT NULL COMMENT 'FK judicial_case.id',
    constitution_item_id BIGINT UNSIGNED NOT NULL COMMENT 'FK community_constitution_item.id',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (judicial_case_id, constitution_item_id),
    KEY idx_jcc_constitution_item (constitution_item_id),
    CONSTRAINT fk_jcci_case FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jcci_constitution FOREIGN KEY (constitution_item_id) REFERENCES community_constitution_item (id) ON DELETE RESTRICT,
    CONSTRAINT fk_jcci_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
