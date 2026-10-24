-- Moderation severity lives on community_constitution_item (ref_key_values), not judicial_case.
-- Case severity is derived transitively from filer-selected violated clauses.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('judicial_moderation_tier', 'regular', 10, 1, NULL, NULL),
    ('judicial_moderation_tier', 'serious', 20, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

UPDATE ref_key_values
SET is_active = 0
WHERE `group` = 'judicial_moderation_tier'
  AND `key` IN ('standard', 'emergency');

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

SET @tier_regular_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'judicial_moderation_tier' AND `key` = 'regular' AND is_active = 1
    LIMIT 1
);

SET @tier_serious_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'judicial_moderation_tier' AND `key` = 'serious' AND is_active = 1
    LIMIT 1
);

ALTER TABLE community_constitution_item
    ADD COLUMN moderation_tier_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group judicial_moderation_tier — severity when cited in post dispute'
        AFTER judicial_trigger_chk,
    ADD KEY idx_community_constitution_moderation_tier (moderation_tier_id),
    ADD CONSTRAINT fk_community_constitution_moderation_tier
        FOREIGN KEY (moderation_tier_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;

-- Universal Safety Floor (point 4) ⇒ serious; other dispute triggers ⇒ regular.
UPDATE community_constitution_item
SET moderation_tier_id = @tier_serious_id
WHERE community_id = @root_community_id
  AND point = 4
  AND status = 'active';

UPDATE community_constitution_item
SET moderation_tier_id = @tier_regular_id
WHERE judicial_trigger_chk = 1
  AND status = 'active'
  AND (moderation_tier_id IS NULL OR moderation_tier_id <> @tier_serious_id);

SET @fk_exists := (
    SELECT COUNT(*)
    FROM information_schema.TABLE_CONSTRAINTS
    WHERE CONSTRAINT_SCHEMA = DATABASE()
      AND TABLE_NAME = 'judicial_case'
      AND CONSTRAINT_NAME = 'fk_judicial_case_moderation_tier'
);
SET @drop_fk := IF(
    @fk_exists > 0,
    'ALTER TABLE judicial_case DROP FOREIGN KEY fk_judicial_case_moderation_tier',
    'SELECT 1'
);
PREPARE drop_fk_stmt FROM @drop_fk;
EXECUTE drop_fk_stmt;
DEALLOCATE PREPARE drop_fk_stmt;

SET @col_exists := (
    SELECT COUNT(*)
    FROM information_schema.COLUMNS
    WHERE TABLE_SCHEMA = DATABASE()
      AND TABLE_NAME = 'judicial_case'
      AND COLUMN_NAME = 'moderation_tier_id'
);
SET @drop_col := IF(
    @col_exists > 0,
    'ALTER TABLE judicial_case DROP COLUMN moderation_tier_id',
    'SELECT 1'
);
PREPARE drop_col_stmt FROM @drop_col;
EXECUTE drop_col_stmt;
DEALLOCATE PREPARE drop_col_stmt;
