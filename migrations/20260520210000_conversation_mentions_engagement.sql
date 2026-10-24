-- Conversation settlement timestamps, @mentions, per-post engagement votes (incl. replies).

ALTER TABLE post
    ADD COLUMN conv_opened_at TIMESTAMP(6) NULL
        COMMENT 'When this post/reply conversation window opened'
        AFTER source_judicial_case_comment_id,
    ADD COLUMN conv_last_activity_at TIMESTAMP(6) NULL
        COMMENT 'Last vote or reply activity affecting this conversation'
        AFTER conv_opened_at,
    ADD COLUMN conv_rating_applied_at TIMESTAMP(6) NULL
        COMMENT 'When civic rating feedback was applied once for this post'
        AFTER conv_last_activity_at,
    ADD KEY idx_post_conv_settlement (conv_rating_applied_at, conv_opened_at, conv_last_activity_at);

UPDATE post
SET conv_opened_at = COALESCE(published_at, created_at),
    conv_last_activity_at = COALESCE(published_at, created_at)
WHERE conv_opened_at IS NULL;

ALTER TABLE post_engagement_vote
    ADD COLUMN target_post_id BIGINT UNSIGNED NULL
        COMMENT 'Post row receiving the vote (root or reply); defaults to root_post_id'
        AFTER root_post_id;

UPDATE post_engagement_vote SET target_post_id = root_post_id WHERE target_post_id IS NULL;

ALTER TABLE post_engagement_vote
    MODIFY target_post_id BIGINT UNSIGNED NOT NULL,
    DROP INDEX uq_post_engagement_vote_voter,
    ADD UNIQUE KEY uq_pev_target_voter (target_post_id, voter_user_id),
    ADD KEY idx_pev_target (target_post_id);

CREATE TABLE post_mention (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    post_id BIGINT UNSIGNED NOT NULL COMMENT 'FK post.id',
    mentioned_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id',
    seen_at TIMESTAMP(6) NULL COMMENT 'When the mentioned party acknowledged on home load',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY uq_post_mention (post_id, mentioned_entity_id),
    KEY idx_post_mention_entity_unseen (mentioned_entity_id, seen_at, created_at),
    CONSTRAINT fk_post_mention_post FOREIGN KEY (post_id) REFERENCES post (id) ON DELETE CASCADE,
    CONSTRAINT fk_post_mention_entity FOREIGN KEY (mentioned_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'conversation_rating_settlement', 45, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE is_active = 1;
