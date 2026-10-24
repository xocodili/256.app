-- Greenfield schema: drop app tables (dependency order), then create structure only.
-- Reference row data lives in `20260411000001_seed_reference_data.sql`.

SET FOREIGN_KEY_CHECKS = 0;
DROP TABLE IF EXISTS social_entity_relationship;
DROP TABLE IF EXISTS social_gathering;
DROP TABLE IF EXISTS social_human;
DROP TABLE IF EXISTS social_community;
DROP TABLE IF EXISTS social_digital;
DROP TABLE IF EXISTS social_profit;
DROP TABLE IF EXISTS social_non_profit;
DROP TABLE IF EXISTS social_entity;
DROP TABLE IF EXISTS ref_key_values;
DROP TABLE IF EXISTS locations;
DROP TABLE IF EXISTS email_verification_tokens;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS users;
SET FOREIGN_KEY_CHECKS = 1;

-- ---------------------------------------------------------------------------
-- Reference keys (before users so users.status_id can FK here).
-- FKs to users for created_by/modified_by are added after users exists (cycle break).
-- ---------------------------------------------------------------------------
CREATE TABLE ref_key_values (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `group` VARCHAR(128) NOT NULL COMMENT 'Logical namespace for the row (e.g. country, status)',
    `key` VARCHAR(128) NOT NULL COMMENT 'Stable code; paired with group for locale lookup',
    sort_order INT NOT NULL DEFAULT 0 COMMENT 'Ascending order within group for UI lists',
    is_active TINYINT(1) NOT NULL DEFAULT 1 COMMENT '0 = hidden from normal selection',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ref_key_values_group_key (`group`, `key`),
    KEY idx_ref_key_values_group_active (`group`, is_active, sort_order),
    KEY idx_ref_key_values_created_by (created_by),
    KEY idx_ref_key_values_modified_by (modified_by)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- Identity
-- ---------------------------------------------------------------------------
CREATE TABLE users (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    email VARCHAR(255) NOT NULL,
    password_hash VARBINARY(255) NULL,
    email_verified_at TIMESTAMP NULL DEFAULT NULL,
    status_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group status',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_users_email (email),
    KEY idx_users_status_id (status_id),
    KEY idx_users_created_by (created_by),
    KEY idx_users_modified_by (modified_by),
    CONSTRAINT fk_users_status_id FOREIGN KEY (status_id) REFERENCES ref_key_values (id) ON DELETE SET NULL,
    CONSTRAINT fk_users_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_users_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

ALTER TABLE ref_key_values
    ADD CONSTRAINT fk_ref_key_values_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    ADD CONSTRAINT fk_ref_key_values_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL;

-- Opaque session token in the cookie (not the internal BIGINT id).
CREATE TABLE sessions (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    session_token VARCHAR(64) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_sessions_session_token (session_token),
    KEY idx_sessions_user (user_id),
    KEY idx_sessions_expires (expires_at),
    KEY idx_sessions_created_by (created_by),
    KEY idx_sessions_modified_by (modified_by),
    CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT fk_sessions_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_sessions_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE email_verification_tokens (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    token_hash VARBINARY(64) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_email_verification_tokens_hash (token_hash),
    KEY idx_evt_user (user_id),
    CONSTRAINT fk_evt_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- Locations (dedup by canonical fingerprint; see app layer)
-- ---------------------------------------------------------------------------
CREATE TABLE locations (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    country_code CHAR(2) NOT NULL COMMENT 'ISO 3166-1 alpha-2',
    region VARCHAR(128) NULL COMMENT 'admin1: state, province, oblast, etc.',
    locality VARCHAR(128) NULL COMMENT 'city, town, or recognized neighborhood',
    postal_code VARCHAR(32) NULL,
    latitude DECIMAL(9, 6) NULL COMMENT 'WGS84; NULL if not collected or redacted',
    longitude DECIMAL(9, 6) NULL,
    display_label VARCHAR(512) NOT NULL COMMENT 'Human-readable label after validation',
    validation_source VARCHAR(64) NOT NULL COMMENT 'e.g. internal, nominatim, pelias — audit only',
    validation_ref VARCHAR(255) NULL COMMENT 'Opaque id from validator when applicable',
    validated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    canonical_fingerprint VARBINARY(32) NOT NULL COMMENT 'SHA3-256 (Keccak) digest for deduplication',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_locations_fingerprint (canonical_fingerprint),
    KEY idx_locations_country_locality (country_code, locality(64)),
    KEY idx_locations_created_by (created_by),
    KEY idx_locations_modified_by (modified_by),
    CONSTRAINT fk_locations_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_locations_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ---------------------------------------------------------------------------
-- Social: minimal on-site presence (type/status from ref_key_values; canonical place from locations)
-- ---------------------------------------------------------------------------
CREATE TABLE social_entity (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    handle VARCHAR(32) NOT NULL COMMENT 'Unique public handle (lowercase [a-z0-9_]); shown as @handle',
    type_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id — use a dedicated `group` (e.g. social_entity_type)',
    status_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group status',
    location_id BIGINT UNSIGNED NOT NULL COMMENT 'FK locations.id',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_social_entity_handle (handle),
    KEY idx_social_entity_type_id (type_id),
    KEY idx_social_entity_status_id (status_id),
    KEY idx_social_entity_location_id (location_id),
    KEY idx_social_entity_created_by (created_by),
    KEY idx_social_entity_modified_by (modified_by),
    CONSTRAINT fk_social_entity_type_id FOREIGN KEY (type_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_social_entity_status_id FOREIGN KEY (status_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_social_entity_location FOREIGN KEY (location_id) REFERENCES locations (id) ON DELETE RESTRICT,
    CONSTRAINT fk_social_entity_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_social_entity_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 1:1 extension for `social_entity` rows with type `social_entity_type.gathering`.
-- Identity, lifecycle (`status_id`), and `location_id` live on `social_entity`; this table holds schedule + display copy + `join_policy_id`.
-- Recurrence: `recurrence_frequency_id` NULL = one-off. `recurrence_interval` = every N periods (e.g. weekly + 2, or frequency `fortnightly`).
-- Anchor for patterns is `starts_at` / `ends_at` (duration preserved each occurrence); `recurrence_until` caps the series (UTC date).
CREATE TABLE social_gathering (
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — PK',
    label VARCHAR(512) NOT NULL COMMENT 'Human-facing event name',
    description TEXT NULL,
    starts_at DATETIME(6) NOT NULL COMMENT 'UTC instant; series anchor for recurrence',
    ends_at DATETIME(6) NULL COMMENT 'UTC instant; NULL = open-ended / unknown',
    timezone VARCHAR(64) NOT NULL DEFAULT 'UTC' COMMENT 'IANA name for local labels (e.g. America/New_York)',
    recurrence_frequency_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group social_gathering_recurrence; NULL = does not repeat',
    recurrence_interval SMALLINT UNSIGNED NOT NULL DEFAULT 1 COMMENT 'Every N frequency steps (>=1; e.g. 2 with weekly = fortnightly)',
    recurrence_until DATE NULL COMMENT 'Optional UTC calendar last day an occurrence may start; NULL = unbounded',
    join_policy_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group social_join_policy',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (social_entity_id),
    KEY idx_social_gathering_starts_at (starts_at),
    KEY idx_social_gathering_recurrence_frequency (recurrence_frequency_id),
    KEY idx_social_gathering_join_policy (join_policy_id),
    CONSTRAINT chk_social_gathering_ends_after_start CHECK (ends_at IS NULL OR ends_at >= starts_at),
    CONSTRAINT chk_social_gathering_recurrence_interval CHECK (recurrence_interval >= 1 AND recurrence_interval <= 366),
    CONSTRAINT chk_social_gathering_recurrence_coherent CHECK (
        recurrence_frequency_id IS NOT NULL
        OR (recurrence_interval = 1 AND recurrence_until IS NULL)
    ),
    CONSTRAINT fk_social_gathering_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_social_gathering_recurrence_frequency FOREIGN KEY (recurrence_frequency_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_social_gathering_join_policy FOREIGN KEY (join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 1:1 extension for `social_entity` rows with type `social_entity_type.human`.
-- Public display name is `social_entity.handle` (@handle). Structured names are optional.
-- NOTE: we intentionally avoid triggers here (some MySQL installs disallow trigger creation without elevated privileges
-- when binary logging is enabled). The app should maintain `name_sort_key` on write.
-- `birth_date` is a DATE (starsign / birthday features). `birth_location_id` is optional.
-- `join_policy_id` applies to how others may engage with this profile (follow, join edges, etc. — enforced in app).
CREATE TABLE social_human (
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — PK',
    family_name VARCHAR(128) NULL,
    given_name VARCHAR(128) NULL,
    additional_name VARCHAR(256) NULL,
    name_sort_key VARCHAR(512) NOT NULL DEFAULT '' COMMENT 'Write-maintained by app from (family, given, additional) for sort/search',
    birth_date DATE NOT NULL COMMENT 'Calendar date',
    birth_location_id BIGINT UNSIGNED NULL COMMENT 'FK locations.id; optional place of birth',
    join_policy_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group social_join_policy',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (social_entity_id),
    KEY idx_social_human_name_sort_key (name_sort_key),
    KEY idx_social_human_birth_date (birth_date),
    KEY idx_social_human_birth_location (birth_location_id),
    KEY idx_social_human_join_policy (join_policy_id),
    CONSTRAINT fk_social_human_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_social_human_birth_location FOREIGN KEY (birth_location_id) REFERENCES locations (id) ON DELETE SET NULL,
    CONSTRAINT fk_social_human_join_policy FOREIGN KEY (join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- name_sort_key maintenance is handled in application code (see note above).

-- 1:1 extension for `social_entity` rows with type `social_entity_type.community`.
-- Public slug remains `social_entity.handle`; `label` is the human-facing name. `intention` and `rules` are distinct from `description`.
CREATE TABLE social_community (
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — PK',
    label VARCHAR(512) NOT NULL COMMENT 'Human-facing community name',
    description TEXT NULL,
    intention TEXT NULL COMMENT 'Clear statement of purpose / what the community is for',
    rules TEXT NULL COMMENT 'Community rules / code of conduct (plain text)',
    founded_at DATE NULL COMMENT 'Optional founding date',
    join_policy_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group social_join_policy',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (social_entity_id),
    KEY idx_social_community_join_policy (join_policy_id),
    KEY idx_social_community_founded_at (founded_at),
    CONSTRAINT fk_social_community_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_social_community_join_policy FOREIGN KEY (join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 1:1 extension for `social_entity` rows with type `social_entity_type.digital` (AI agents, bots, service accounts, etc.).
-- Public slug remains `social_entity.handle`. `digital_type_id` classifies the machine account (`social_digital_type` ref group).
CREATE TABLE social_digital (
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — PK',
    digital_type_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group social_digital_type',
    join_policy_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group social_join_policy',
    label VARCHAR(512) NULL COMMENT 'Optional friendly name alongside @handle',
    description TEXT NULL,
    intention TEXT NULL COMMENT 'Stated purpose / scope of operation',
    operator_disclosure TEXT NULL COMMENT 'Who operates it; transparency / disclosure copy',
    activated_at DATE NULL COMMENT 'When the account went live',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (social_entity_id),
    KEY idx_social_digital_type (digital_type_id),
    KEY idx_social_digital_join_policy (join_policy_id),
    KEY idx_social_digital_activated_at (activated_at),
    CONSTRAINT fk_social_digital_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_social_digital_type FOREIGN KEY (digital_type_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_social_digital_join_policy FOREIGN KEY (join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 1:1 extension for `social_entity` rows with type `social_entity_type.profit`.
-- Public slug remains `social_entity.handle`. `label` is optional friendly name; `legal_name` is registered name when distinct.
CREATE TABLE social_profit (
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — PK',
    join_policy_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group social_join_policy',
    label VARCHAR(512) NULL COMMENT 'Optional friendly name alongside @handle',
    description TEXT NULL,
    intention TEXT NULL COMMENT 'Stated purpose / positioning (short)',
    legal_name VARCHAR(512) NULL COMMENT 'Registered legal name where applicable',
    lei CHAR(20) NULL COMMENT 'ISO 17442 Legal Entity Identifier when assigned',
    incorporation_country CHAR(2) NULL COMMENT 'ISO 3166-1 alpha-2; primary registration jurisdiction',
    registration_subdivision VARCHAR(128) NULL COMMENT 'State, province, canton, etc. when applicable',
    registration_number VARCHAR(64) NULL COMMENT 'Jurisdiction-local company / registry number',
    founded_at DATE NULL COMMENT 'Optional incorporation or founding date',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (social_entity_id),
    KEY idx_social_profit_join_policy (join_policy_id),
    KEY idx_social_profit_lei (lei),
    KEY idx_social_profit_incorporation_country (incorporation_country),
    KEY idx_social_profit_founded_at (founded_at),
    CONSTRAINT fk_social_profit_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_social_profit_join_policy FOREIGN KEY (join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 1:1 extension for `social_entity` rows with type `social_entity_type.non_profit`.
-- Same display/registry shape as `social_profit`; `registration_number` holds charity / tax-exempt / society ids per jurisdiction.
CREATE TABLE social_non_profit (
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — PK',
    join_policy_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group social_join_policy',
    label VARCHAR(512) NULL COMMENT 'Optional friendly name alongside @handle',
    description TEXT NULL,
    intention TEXT NULL COMMENT 'Mission / public benefit statement (short)',
    legal_name VARCHAR(512) NULL COMMENT 'Registered legal name where applicable',
    lei CHAR(20) NULL COMMENT 'ISO 17442 Legal Entity Identifier when assigned',
    incorporation_country CHAR(2) NULL COMMENT 'ISO 3166-1 alpha-2; primary registration jurisdiction',
    registration_subdivision VARCHAR(128) NULL COMMENT 'State, province, canton, etc. when applicable',
    registration_number VARCHAR(64) NULL COMMENT 'Charity / society / tax-exempt registry id as defined locally',
    founded_at DATE NULL COMMENT 'Optional founding or recognition date',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (social_entity_id),
    KEY idx_social_non_profit_join_policy (join_policy_id),
    KEY idx_social_non_profit_lei (lei),
    KEY idx_social_non_profit_incorporation_country (incorporation_country),
    KEY idx_social_non_profit_founded_at (founded_at),
    CONSTRAINT fk_social_non_profit_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_social_non_profit_join_policy FOREIGN KEY (join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Connector between two social entities; type/status via ref_key_values.
-- Optional DATE window: employment tenure, contract/planned end, etc. (`ended_at` NULL = ongoing / unknown end).
CREATE TABLE social_entity_relationship (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    from_social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — origin side',
    to_social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_entity.id — target side',
    type_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id — group social_entity_relationship_type',
    status_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group status',
    started_at DATE NULL COMMENT 'Relationship began (e.g. hire date)',
    ended_at DATE NULL COMMENT 'Actual or expected end; NULL if open-ended',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_ser_from_entity (from_social_entity_id),
    KEY idx_ser_to_entity (to_social_entity_id),
    KEY idx_ser_type_id (type_id),
    KEY idx_ser_status_id (status_id),
    KEY idx_ser_created_by (created_by),
    KEY idx_ser_modified_by (modified_by),
    CONSTRAINT fk_ser_from_entity FOREIGN KEY (from_social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_ser_to_entity FOREIGN KEY (to_social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_ser_type_id FOREIGN KEY (type_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_ser_status_id FOREIGN KEY (status_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT chk_ser_dates CHECK (ended_at IS NULL OR started_at IS NULL OR ended_at >= started_at),
    CONSTRAINT fk_ser_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ser_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
