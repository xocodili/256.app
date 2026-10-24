-- Post engagement votes, moderation mirror posts, link share counts.

ALTER TABLE content_link
    ADD COLUMN share_count INT UNSIGNED NOT NULL DEFAULT 0 COMMENT 'Times this canonical link was shared to a new audience' AFTER url_canonical;

ALTER TABLE post
    ADD COLUMN is_moderation_mirror TINYINT(1) NOT NULL DEFAULT 0
        COMMENT '1 = judicial mirror comment; exempt from L1 binomial cap'
        AFTER country_code,
    ADD COLUMN source_judicial_case_comment_id BIGINT UNSIGNED NULL
        COMMENT 'FK judicial_case_comment.id when mirrored from a resolved case'
        AFTER is_moderation_mirror,
    ADD KEY idx_post_moderation_mirror (is_moderation_mirror, root_post_id),
    ADD UNIQUE KEY uq_post_source_jcc (source_judicial_case_comment_id);

CREATE TABLE post_engagement_vote (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    root_post_id BIGINT UNSIGNED NOT NULL COMMENT 'FK post.id — thread root',
    voter_user_id BIGINT UNSIGNED NOT NULL COMMENT 'FK users.id',
    direction ENUM('yes', 'no') NOT NULL,
    voter_weight DECIMAL(5, 4) NOT NULL COMMENT 'Frozen weight at vote time (typically rating/2.5)',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY uq_post_engagement_vote_voter (root_post_id, voter_user_id),
    KEY idx_post_engagement_vote_root (root_post_id),
    CONSTRAINT fk_pev_root_post FOREIGN KEY (root_post_id) REFERENCES post (id) ON DELETE CASCADE,
    CONSTRAINT fk_pev_voter FOREIGN KEY (voter_user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT chk_pev_weight_nonneg CHECK (voter_weight >= 0.0000)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
