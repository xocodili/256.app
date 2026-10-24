-- Replace TEXT columns (row-size workaround) with JSON snapshots (app-enforced 4000-char cap).

SET @has_snapshot := (
    SELECT COUNT(*)
    FROM information_schema.COLUMNS
    WHERE TABLE_SCHEMA = DATABASE()
      AND TABLE_NAME = 'judicial_case_community_profile_amendment'
      AND COLUMN_NAME = 'current_snapshot'
);

SET @add_sql := IF(
    @has_snapshot = 0,
    'ALTER TABLE judicial_case_community_profile_amendment
        ADD COLUMN current_snapshot JSON NULL
            COMMENT ''Frozen current about/intention/join_policy at filing'' AFTER change_join_policy,
        ADD COLUMN proposed_snapshot JSON NULL
            COMMENT ''Proposed about/intention/join_policy at filing'' AFTER current_snapshot',
    'SELECT 1'
);

PREPARE add_stmt FROM @add_sql;
EXECUTE add_stmt;
DEALLOCATE PREPARE add_stmt;

SET @has_legacy := (
    SELECT COUNT(*)
    FROM information_schema.COLUMNS
    WHERE TABLE_SCHEMA = DATABASE()
      AND TABLE_NAME = 'judicial_case_community_profile_amendment'
      AND COLUMN_NAME = 'current_description_plain'
);

SET @update_sql := IF(
    @has_legacy > 0,
    'UPDATE judicial_case_community_profile_amendment
     SET
         current_snapshot = JSON_OBJECT(
             ''description_plain'', current_description_plain,
             ''intention_plain'', current_intention_plain,
             ''join_policy_key'', (
                 SELECT rk.`key`
                 FROM ref_key_values rk
                 WHERE rk.id = current_join_policy_id
                 LIMIT 1
             )
         ),
         proposed_snapshot = JSON_OBJECT(
             ''description_plain'', proposed_description_plain,
             ''description_html_sanitized'', proposed_description_html_sanitized,
             ''intention_plain'', proposed_intention_plain,
             ''intention_html_sanitized'', proposed_intention_html_sanitized,
             ''join_policy_key'', (
                 SELECT rk.`key`
                 FROM ref_key_values rk
                 WHERE rk.id = proposed_join_policy_id
                 LIMIT 1
             )
         )',
    'UPDATE judicial_case_community_profile_amendment
     SET current_snapshot = COALESCE(current_snapshot, JSON_OBJECT()),
         proposed_snapshot = COALESCE(proposed_snapshot, JSON_OBJECT())
     WHERE current_snapshot IS NULL OR proposed_snapshot IS NULL'
);

PREPARE update_stmt FROM @update_sql;
EXECUTE update_stmt;
DEALLOCATE PREPARE update_stmt;

SET @drop_sql := IF(
    @has_legacy > 0,
    'ALTER TABLE judicial_case_community_profile_amendment
         DROP FOREIGN KEY fk_jccpa_current_join_policy,
         DROP FOREIGN KEY fk_jccpa_proposed_join_policy,
         DROP COLUMN current_description_plain,
         DROP COLUMN current_intention_plain,
         DROP COLUMN current_join_policy_id,
         DROP COLUMN proposed_description_plain,
         DROP COLUMN proposed_description_markup,
         DROP COLUMN proposed_description_html_sanitized,
         DROP COLUMN proposed_intention_plain,
         DROP COLUMN proposed_intention_markup,
         DROP COLUMN proposed_intention_html_sanitized,
         DROP COLUMN proposed_join_policy_id',
    'SELECT 1'
);

PREPARE drop_stmt FROM @drop_sql;
EXECUTE drop_stmt;
DEALLOCATE PREPARE drop_stmt;

ALTER TABLE judicial_case_community_profile_amendment
    MODIFY current_snapshot JSON NOT NULL,
    MODIFY proposed_snapshot JSON NOT NULL;
