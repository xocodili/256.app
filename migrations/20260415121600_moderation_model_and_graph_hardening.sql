-- Graph hardening + first-class AI moderation scope/decision logging.

ALTER TABLE social_entity_relationship
    ADD CONSTRAINT chk_ser_no_self_loop CHECK (from_social_entity_id <> to_social_entity_id);

CREATE TABLE ai_policy_bundle (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `key` VARCHAR(128) NOT NULL,
    label VARCHAR(255) NOT NULL,
    description TEXT NULL,
    version VARCHAR(64) NOT NULL DEFAULT 'v1',
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ai_policy_bundle_key_version (`key`, version),
    KEY idx_ai_policy_bundle_active (is_active),
    CONSTRAINT fk_ai_policy_bundle_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_policy_bundle_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_moderation_scope (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    ai_actor_id BIGINT UNSIGNED NOT NULL,
    social_entity_id BIGINT UNSIGNED NULL COMMENT 'Optional explicit entity scope (e.g. one community)',
    ai_domain_id BIGINT UNSIGNED NULL COMMENT 'Optional domain scope',
    policy_bundle_id BIGINT UNSIGNED NULL,
    allow_remove_content TINYINT(1) NOT NULL DEFAULT 1,
    allow_suspend_author TINYINT(1) NOT NULL DEFAULT 0,
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    starts_at DATE NULL,
    ends_at DATE NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_ai_mod_scope_actor (ai_actor_id),
    KEY idx_ai_mod_scope_entity (social_entity_id),
    KEY idx_ai_mod_scope_domain (ai_domain_id),
    KEY idx_ai_mod_scope_policy (policy_bundle_id),
    KEY idx_ai_mod_scope_active (is_active),
    CONSTRAINT chk_ai_mod_scope_dates CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at >= starts_at),
    CONSTRAINT fk_ai_mod_scope_actor FOREIGN KEY (ai_actor_id) REFERENCES ai_actor (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_mod_scope_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_mod_scope_domain FOREIGN KEY (ai_domain_id) REFERENCES ai_domain (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_mod_scope_policy FOREIGN KEY (policy_bundle_id) REFERENCES ai_policy_bundle (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_mod_scope_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_mod_scope_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_moderation_decision_log (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    ai_actor_id BIGINT UNSIGNED NOT NULL,
    moderation_scope_id BIGINT UNSIGNED NULL,
    policy_bundle_id BIGINT UNSIGNED NULL,
    subject_ref VARCHAR(255) NOT NULL COMMENT 'Opaque subject id (post/comment/etc)',
    decision VARCHAR(64) NOT NULL COMMENT 'e.g. allow, remove_content, escalate',
    rationale TEXT NULL,
    provider_ref VARCHAR(128) NULL,
    model_ref VARCHAR(128) NULL,
    trace_id VARCHAR(128) NULL,
    decided_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    KEY idx_ai_mod_log_actor_time (ai_actor_id, decided_at),
    KEY idx_ai_mod_log_scope (moderation_scope_id),
    KEY idx_ai_mod_log_policy (policy_bundle_id),
    KEY idx_ai_mod_log_subject (subject_ref),
    PRIMARY KEY (id),
    CONSTRAINT fk_ai_mod_log_actor FOREIGN KEY (ai_actor_id) REFERENCES ai_actor (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_mod_log_scope FOREIGN KEY (moderation_scope_id) REFERENCES ai_moderation_scope (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_mod_log_policy FOREIGN KEY (policy_bundle_id) REFERENCES ai_policy_bundle (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- NOTE: trigger creation is intentionally avoided in greenfield dev because some MySQL installs disallow
-- CREATE TRIGGER without elevated privileges when binary logging is enabled (error 1419).
-- Uniqueness for active edges is enforced in app code for now.
