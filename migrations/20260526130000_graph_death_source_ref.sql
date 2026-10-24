-- Graph death source: FK to ref_key_values (group graph_death_source) instead of ENUM columns.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('graph_death_source', 'rating_zero', 10, 1, NULL, NULL),
('graph_death_source', 'manual_ops', 20, 1, NULL, NULL),
('graph_death_source', 'user_requested', 30, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

ALTER TABLE social_human
    ADD COLUMN death_source_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group graph_death_source'
        AFTER died_at;

UPDATE social_human sh
INNER JOIN ref_key_values rk
    ON rk.`group` = 'graph_death_source'
   AND rk.`key` = sh.death_source
   AND rk.is_active = 1
SET sh.death_source_id = rk.id
WHERE sh.death_source IS NOT NULL;

ALTER TABLE social_human
    DROP COLUMN death_source;

ALTER TABLE social_human
    ADD CONSTRAINT fk_social_human_death_source
        FOREIGN KEY (death_source_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;

ALTER TABLE social_community
    ADD COLUMN death_source_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group graph_death_source'
        AFTER died_at;

UPDATE social_community sc
INNER JOIN ref_key_values rk
    ON rk.`group` = 'graph_death_source'
   AND rk.`key` = sc.death_source
   AND rk.is_active = 1
SET sc.death_source_id = rk.id
WHERE sc.death_source IS NOT NULL;

ALTER TABLE social_community
    DROP COLUMN death_source;

ALTER TABLE social_community
    ADD CONSTRAINT fk_social_community_death_source
        FOREIGN KEY (death_source_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;
