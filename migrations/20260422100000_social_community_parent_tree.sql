ALTER TABLE social_community
    ADD COLUMN parent_community_id BIGINT UNSIGNED NULL
        COMMENT 'Parent community (social_community.social_entity_id). Null for root council communities.',
    ADD KEY idx_social_community_parent_community_id (parent_community_id),
    ADD CONSTRAINT fk_social_community_parent_community
        FOREIGN KEY (parent_community_id) REFERENCES social_community (social_entity_id) ON DELETE SET NULL;
