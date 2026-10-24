-- Community graph lifecycle: dead status, rating backfill, reparent judicial trigger, audit, death job.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('social_entity_status', 'dead', 70, 1, NULL, NULL),
('judicial_case_trigger', 'community_reparent', 40, 1, NULL, NULL),
('ops_job_type', 'community_graph_lifecycle', 40, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

-- Neutral default for communities still at zero (do not lower positive ratings).
UPDATE social_entity se
INNER JOIN ref_key_values rk_type
    ON rk_type.id = se.type_id
   AND rk_type.`group` = 'social_entity_type'
   AND rk_type.`key` = 'community'
SET se.rating = 2.50
WHERE se.rating = 0.00;

ALTER TABLE social_community
    ADD COLUMN died_at TIMESTAMP(6) NULL COMMENT 'When the community entered dead status' AFTER parent_community_id,
    ADD COLUMN death_source ENUM('rating_zero', 'manual_ops') NULL COMMENT 'Why the community died' AFTER died_at;

CREATE TABLE IF NOT EXISTS community_graph_event (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    event_type ENUM('death', 'reparent', 'reparent_judicial') NOT NULL,
    community_social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id',
    old_parent_id BIGINT UNSIGNED NULL COMMENT 'FK social_community.social_entity_id before change',
    new_parent_id BIGINT UNSIGNED NULL COMMENT 'FK social_community.social_entity_id after change',
    judicial_case_id BIGINT UNSIGNED NULL,
    detail_json JSON NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT (UTC_TIMESTAMP(6)),
    created_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_community_graph_event_community (community_social_entity_id, created_at),
    KEY idx_community_graph_event_case (judicial_case_id),
    CONSTRAINT fk_community_graph_event_community
        FOREIGN KEY (community_social_entity_id) REFERENCES social_entity (id) ON DELETE RESTRICT,
    CONSTRAINT fk_community_graph_event_judicial_case
        FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS judicial_case_community_reparent (
    judicial_case_id BIGINT UNSIGNED NOT NULL,
    proposed_parent_community_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_community.social_entity_id',
    phase ENUM('origin_vote', 'impacted_vote', 'completed') NOT NULL DEFAULT 'origin_vote',
    origin_phase_resolved_at TIMESTAMP(6) NULL,
    impacted_pool_size_frozen INT UNSIGNED NULL COMMENT 'Distinct verified humans in subtree(M) at phase-2 freeze',
    impacted_jury_size_frozen TINYINT UNSIGNED NULL COMMENT 'ceil(sqrt(impacted_pool_size_frozen))',
    PRIMARY KEY (judicial_case_id),
    CONSTRAINT fk_jccr_judicial_case
        FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jccr_proposed_parent
        FOREIGN KEY (proposed_parent_community_id) REFERENCES social_community (social_entity_id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
