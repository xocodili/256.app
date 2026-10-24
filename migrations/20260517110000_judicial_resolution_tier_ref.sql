-- resolution_tier_key VARCHAR → FK ref_key_values (group judicial_resolution_tier).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('judicial_resolution_tier', 'jury', 10, 1, NULL, NULL),
    ('judicial_resolution_tier', 'ai_council', 20, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

ALTER TABLE judicial_case
    ADD COLUMN resolution_tier_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group judicial_resolution_tier'
        AFTER ai_votes_against;

UPDATE judicial_case jc
INNER JOIN ref_key_values rk
    ON rk.`group` = 'judicial_resolution_tier'
   AND rk.`key` = jc.resolution_tier_key
   AND rk.is_active = 1
SET jc.resolution_tier_id = rk.id
WHERE jc.resolution_tier_key IS NOT NULL;

ALTER TABLE judicial_case
    DROP COLUMN resolution_tier_key,
    ADD CONSTRAINT fk_judicial_case_resolution_tier
        FOREIGN KEY (resolution_tier_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;
