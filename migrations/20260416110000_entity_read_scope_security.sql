-- DB-level scoped-read hardening foundation.
-- Maintains `social_entity.scope_version` for cache invalidation and enqueues
-- affected entities for ACL recomputation when graph edges change.

ALTER TABLE social_entity
    ADD COLUMN scope_version BIGINT UNSIGNED NOT NULL DEFAULT 1
        COMMENT 'Incremented when relationship graph affecting this entity changes; cache invalidation anchor'
        AFTER rating;

CREATE TABLE entity_read_scope (
    subject_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'Viewer perspective social_entity.id',
    visible_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'Entity visible to the subject',
    granted_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    source VARCHAR(32) NOT NULL DEFAULT 'materialized' COMMENT 'materialized|bootstrap|manual',
    PRIMARY KEY (subject_entity_id, visible_entity_id),
    KEY idx_entity_read_scope_visible_subject (visible_entity_id, subject_entity_id),
    CONSTRAINT fk_entity_read_scope_subject FOREIGN KEY (subject_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_entity_read_scope_visible FOREIGN KEY (visible_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT chk_entity_read_scope_source CHECK (source IN ('materialized', 'bootstrap', 'manual'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE entity_scope_recalc_queue (
    subject_entity_id BIGINT UNSIGNED NOT NULL,
    requested_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    reason VARCHAR(64) NOT NULL DEFAULT 'relationship_changed',
    PRIMARY KEY (subject_entity_id),
    KEY idx_entity_scope_recalc_requested_at (requested_at),
    CONSTRAINT fk_entity_scope_recalc_subject FOREIGN KEY (subject_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- DB-enforced live scope semantics (source of truth from relationships).
-- Use this view in read queries to guarantee scoped reads even before queue workers run.
DROP VIEW IF EXISTS vw_entity_read_scope_live;
CREATE VIEW vw_entity_read_scope_live AS
SELECT
    se.id AS subject_entity_id,
    se.id AS visible_entity_id
FROM social_entity se
UNION DISTINCT
SELECT
    ser.from_social_entity_id AS subject_entity_id,
    ser.to_social_entity_id AS visible_entity_id
FROM social_entity_relationship ser
INNER JOIN ref_key_values rk_st
    ON rk_st.id = ser.status_id
   AND rk_st.`group` = 'status'
   AND rk_st.is_active = 1
   AND rk_st.`key` IN ('pending_verification', 'verified', 'flagged')
WHERE ser.ended_at IS NULL
UNION DISTINCT
SELECT
    ser.to_social_entity_id AS subject_entity_id,
    ser.from_social_entity_id AS visible_entity_id
FROM social_entity_relationship ser
INNER JOIN ref_key_values rk_st
    ON rk_st.id = ser.status_id
   AND rk_st.`group` = 'status'
   AND rk_st.is_active = 1
   AND rk_st.`key` IN ('pending_verification', 'verified', 'flagged')
WHERE ser.ended_at IS NULL;

-- Bootstrap queue so workers can materialize `entity_read_scope` from current graph.
INSERT INTO entity_scope_recalc_queue (subject_entity_id, requested_at, reason)
SELECT se.id, UTC_TIMESTAMP(6), 'bootstrap'
FROM social_entity se
ON DUPLICATE KEY UPDATE
    requested_at = VALUES(requested_at),
    reason = VALUES(reason);

-- NOTE: triggers are intentionally not created in greenfield dev (some MySQL installs disallow CREATE TRIGGER
-- without elevated privileges when binary logging is enabled; error 1419).
-- In early builds, scope queueing and scope_version bumping are handled by application code / jobs.
