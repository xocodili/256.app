-- Human graph lifecycle: death audit, social_human death metadata.

ALTER TABLE social_human
    ADD COLUMN died_at TIMESTAMP(6) NULL COMMENT 'When the human entity entered dead status' AFTER join_policy_id,
    ADD COLUMN death_source ENUM('rating_zero', 'manual_ops') NULL COMMENT 'Why the human was removed' AFTER died_at;

CREATE TABLE IF NOT EXISTS human_graph_event (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    event_type ENUM('death') NOT NULL,
    human_social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id',
    detail_json JSON NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT (UTC_TIMESTAMP(6)),
    created_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_human_graph_event_human (human_social_entity_id, created_at),
    CONSTRAINT fk_human_graph_event_human
        FOREIGN KEY (human_social_entity_id) REFERENCES social_entity (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
