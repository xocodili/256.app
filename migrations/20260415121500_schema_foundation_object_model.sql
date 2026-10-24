-- Domain foundation: explicit ownership, scoped lifecycle refs, AI actor/domain model, and structured safety classification.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('user_status', 'pending_verification', 10, 1, NULL, NULL),
('user_status', 'verified', 20, 1, NULL, NULL),
('user_status', 'retired', 30, 1, NULL, NULL),
('user_status', 'to_be_deleted', 40, 1, NULL, NULL),
('user_status', 'blocked', 50, 1, NULL, NULL),
('user_status', 'flagged', 60, 1, NULL, NULL),
('social_entity_status', 'pending_verification', 10, 1, NULL, NULL),
('social_entity_status', 'verified', 20, 1, NULL, NULL),
('social_entity_status', 'retired', 30, 1, NULL, NULL),
('social_entity_status', 'to_be_deleted', 40, 1, NULL, NULL),
('social_entity_status', 'blocked', 50, 1, NULL, NULL),
('social_entity_status', 'flagged', 60, 1, NULL, NULL),
('social_relationship_status', 'pending_verification', 10, 1, NULL, NULL),
('social_relationship_status', 'verified', 20, 1, NULL, NULL),
('social_relationship_status', 'retired', 30, 1, NULL, NULL),
('social_relationship_status', 'to_be_deleted', 40, 1, NULL, NULL),
('social_relationship_status', 'blocked', 50, 1, NULL, NULL),
('social_relationship_status', 'flagged', 60, 1, NULL, NULL),
('user_social_entity_role', 'owner', 10, 1, NULL, NULL),
('user_social_entity_role', 'admin', 20, 1, NULL, NULL),
('user_social_entity_role', 'member', 30, 1, NULL, NULL),
('user_social_entity_role', 'moderator', 40, 1, NULL, NULL),
('ai_council_role', 'member', 10, 1, NULL, NULL),
('ai_council_role', 'lead', 20, 1, NULL, NULL),
('ai_domain_ownership_role', 'primary', 10, 1, NULL, NULL),
('ai_domain_ownership_role', 'secondary', 20, 1, NULL, NULL),
('safety_source', 'human_review', 10, 1, NULL, NULL),
('safety_source', 'automated', 20, 1, NULL, NULL),
('safety_source', 'seed', 30, 1, NULL, NULL);

ALTER TABLE users
    ADD COLUMN user_status_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group user_status' AFTER status_id,
    ADD KEY idx_users_user_status_id (user_status_id),
    ADD CONSTRAINT fk_users_user_status_id FOREIGN KEY (user_status_id) REFERENCES ref_key_values (id) ON DELETE SET NULL;

ALTER TABLE social_entity
    ADD COLUMN entity_status_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group social_entity_status' AFTER status_id,
    ADD KEY idx_social_entity_entity_status_id (entity_status_id),
    ADD CONSTRAINT fk_social_entity_entity_status_id FOREIGN KEY (entity_status_id) REFERENCES ref_key_values (id) ON DELETE SET NULL;

ALTER TABLE social_entity_relationship
    ADD COLUMN relationship_status_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group social_relationship_status' AFTER status_id,
    ADD KEY idx_ser_relationship_status_id (relationship_status_id),
    ADD CONSTRAINT fk_ser_relationship_status_id FOREIGN KEY (relationship_status_id) REFERENCES ref_key_values (id) ON DELETE SET NULL;

UPDATE users u
JOIN ref_key_values old_status ON old_status.id = u.status_id
JOIN ref_key_values scoped ON scoped.`group` = 'user_status' AND scoped.`key` = old_status.`key`
SET u.user_status_id = scoped.id
WHERE u.status_id IS NOT NULL AND u.user_status_id IS NULL;

UPDATE social_entity se
JOIN ref_key_values old_status ON old_status.id = se.status_id
JOIN ref_key_values scoped ON scoped.`group` = 'social_entity_status' AND scoped.`key` = old_status.`key`
SET se.entity_status_id = scoped.id
WHERE se.status_id IS NOT NULL AND se.entity_status_id IS NULL;

UPDATE social_entity_relationship ser
JOIN ref_key_values old_status ON old_status.id = ser.status_id
JOIN ref_key_values scoped ON scoped.`group` = 'social_relationship_status' AND scoped.`key` = old_status.`key`
SET ser.relationship_status_id = scoped.id
WHERE ser.status_id IS NOT NULL AND ser.relationship_status_id IS NULL;

CREATE TABLE user_social_entity_role (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    social_entity_id BIGINT UNSIGNED NOT NULL,
    role_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group user_social_entity_role',
    is_primary TINYINT(1) NOT NULL DEFAULT 0 COMMENT 'At most one primary per user',
    starts_at DATE NULL,
    ends_at DATE NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_user_entity_role (user_id, social_entity_id, role_id),
    KEY idx_user_entity_role_user_primary (user_id, is_primary),
    KEY idx_user_entity_role_entity (social_entity_id),
    KEY idx_user_entity_role_role (role_id),
    KEY idx_user_entity_role_created_by (created_by),
    KEY idx_user_entity_role_modified_by (modified_by),
    CONSTRAINT chk_user_entity_role_dates CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at >= starts_at),
    CONSTRAINT fk_user_entity_role_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT fk_user_entity_role_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_user_entity_role_role FOREIGN KEY (role_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_user_entity_role_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_user_entity_role_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_actor (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    slug VARCHAR(64) NOT NULL COMMENT 'Stable AI actor slug; aligns with council file names',
    user_id BIGINT UNSIGNED NULL COMMENT 'Optional auth/account identity for the actor',
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'Public-facing social entity for this AI actor',
    provider_ref VARCHAR(128) NULL COMMENT 'Optional provider reference (vertex/openai/etc)',
    model_ref VARCHAR(128) NULL COMMENT 'Optional default model reference',
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ai_actor_slug (slug),
    UNIQUE KEY uq_ai_actor_user (user_id),
    UNIQUE KEY uq_ai_actor_entity (social_entity_id),
    KEY idx_ai_actor_active (is_active),
    CONSTRAINT fk_ai_actor_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_actor_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_actor_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_actor_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_council_membership (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    ai_actor_id BIGINT UNSIGNED NOT NULL,
    role_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group ai_council_role',
    starts_at DATE NULL,
    ends_at DATE NULL,
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ai_council_membership (ai_actor_id, role_id),
    KEY idx_ai_council_membership_active (is_active),
    CONSTRAINT chk_ai_council_membership_dates CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at >= starts_at),
    CONSTRAINT fk_ai_council_membership_actor FOREIGN KEY (ai_actor_id) REFERENCES ai_actor (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_council_membership_role FOREIGN KEY (role_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_ai_council_membership_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_council_membership_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_domain (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `key` VARCHAR(128) NOT NULL,
    label VARCHAR(255) NOT NULL,
    description TEXT NULL,
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ai_domain_key (`key`),
    KEY idx_ai_domain_active (is_active),
    CONSTRAINT fk_ai_domain_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_domain_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_domain_ownership (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    ai_actor_id BIGINT UNSIGNED NOT NULL,
    ai_domain_id BIGINT UNSIGNED NOT NULL,
    role_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group ai_domain_ownership_role',
    starts_at DATE NULL,
    ends_at DATE NULL,
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ai_domain_actor_role (ai_actor_id, ai_domain_id, role_id),
    KEY idx_ai_domain_ownership_domain (ai_domain_id),
    KEY idx_ai_domain_ownership_active (is_active),
    CONSTRAINT chk_ai_domain_ownership_dates CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at >= starts_at),
    CONSTRAINT fk_ai_domain_ownership_actor FOREIGN KEY (ai_actor_id) REFERENCES ai_actor (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_domain_ownership_domain FOREIGN KEY (ai_domain_id) REFERENCES ai_domain (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_domain_ownership_role FOREIGN KEY (role_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_ai_domain_ownership_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_domain_ownership_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE social_entity_safety_classification (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    social_entity_id BIGINT UNSIGNED NOT NULL,
    is_adult TINYINT(1) NOT NULL DEFAULT 0,
    is_sensitive TINYINT(1) NOT NULL DEFAULT 0,
    source_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group safety_source',
    reason TEXT NULL,
    confidence DECIMAL(5,4) NULL,
    reviewed_by BIGINT UNSIGNED NULL,
    reviewed_at TIMESTAMP NULL DEFAULT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_social_entity_safety_entity (social_entity_id),
    KEY idx_social_entity_safety_flags (is_adult, is_sensitive),
    KEY idx_social_entity_safety_source (source_id),
    CONSTRAINT fk_social_entity_safety_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_social_entity_safety_source FOREIGN KEY (source_id) REFERENCES ref_key_values (id) ON DELETE SET NULL,
    CONSTRAINT fk_social_entity_safety_reviewed_by FOREIGN KEY (reviewed_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_social_entity_safety_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_social_entity_safety_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT INTO social_entity_safety_classification (
    social_entity_id,
    is_adult,
    is_sensitive,
    source_id,
    reason
)
SELECT
    se.id,
    CASE WHEN se.is_adult_or_sensitive = 1 THEN 1 ELSE 0 END,
    CASE WHEN se.is_adult_or_sensitive = 1 THEN 1 ELSE 0 END,
    src.id,
    'Initial compatibility backfill from social_entity.is_adult_or_sensitive'
FROM social_entity se
LEFT JOIN ref_key_values src ON src.`group` = 'safety_source' AND src.`key` = 'seed'
LEFT JOIN social_entity_safety_classification esc ON esc.social_entity_id = se.id
WHERE esc.id IS NULL;
