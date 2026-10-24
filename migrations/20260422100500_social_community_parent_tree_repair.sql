SET @col_exists := (
    SELECT COUNT(*)
    FROM information_schema.COLUMNS
    WHERE TABLE_SCHEMA = DATABASE()
      AND TABLE_NAME = 'social_community'
      AND COLUMN_NAME = 'parent_community_id'
);

SET @sql := IF(
    @col_exists = 0,
    'ALTER TABLE social_community\n    ADD COLUMN parent_community_id BIGINT UNSIGNED NULL COMMENT ''Parent community (social_community.social_entity_id). Null for root council communities.'',\n    ADD KEY idx_social_community_parent_community_id (parent_community_id),\n    ADD CONSTRAINT fk_social_community_parent_community FOREIGN KEY (parent_community_id) REFERENCES social_community (social_entity_id) ON DELETE SET NULL',
    'SELECT 1'
);
PREPARE stmt FROM @sql;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;
