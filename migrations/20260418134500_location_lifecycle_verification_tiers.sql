-- Location lifecycle (administrative / cadastral drift) and user verification tiers (Basic → Enhanced → Professional).
-- Invalidating a location rotates `canonical_fingerprint` so a fresh validated row can reuse the same geography.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('location_lifecycle', 'active', 10, 1, NULL, NULL),
('location_lifecycle', 'deprecated', 20, 1, NULL, NULL),
('location_lifecycle', 'obsolete', 30, 1, NULL, NULL),
('verification_tier', 'basic', 10, 1, NULL, NULL),
('verification_tier', 'enhanced', 20, 1, NULL, NULL),
('verification_tier', 'professional', 30, 1, NULL, NULL);

ALTER TABLE locations
    ADD COLUMN location_lifecycle_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group location_lifecycle'
        AFTER modified_by,
    ADD COLUMN lifecycle_changed_at TIMESTAMP NULL DEFAULT NULL
        COMMENT 'UTC: when lifecycle last moved to or from non-active'
        AFTER location_lifecycle_id,
    ADD COLUMN lifecycle_note VARCHAR(256) NULL
        COMMENT 'Audit: cadastral change, provider retraction, manual review, etc.'
        AFTER lifecycle_changed_at,
    ADD COLUMN superseded_by_location_id BIGINT UNSIGNED NULL
        COMMENT 'FK locations.id — preferred replacement when known'
        AFTER lifecycle_note;

UPDATE locations l
CROSS JOIN (
    SELECT id FROM ref_key_values
    WHERE `group` = 'location_lifecycle' AND `key` = 'active' AND is_active = 1
    LIMIT 1
) rk
SET l.location_lifecycle_id = rk.id
WHERE l.location_lifecycle_id IS NULL;

ALTER TABLE locations
    MODIFY COLUMN location_lifecycle_id BIGINT UNSIGNED NOT NULL,
    ADD KEY idx_locations_lifecycle (location_lifecycle_id),
    ADD CONSTRAINT fk_locations_location_lifecycle_id FOREIGN KEY (location_lifecycle_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    ADD KEY idx_locations_superseded_by (superseded_by_location_id),
    ADD CONSTRAINT fk_locations_superseded_by FOREIGN KEY (superseded_by_location_id) REFERENCES locations (id) ON DELETE SET NULL;

ALTER TABLE users
    ADD COLUMN verification_tier_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group verification_tier — product entitlements'
        AFTER user_type_id;

UPDATE users u
CROSS JOIN (
    SELECT id FROM ref_key_values
    WHERE `group` = 'verification_tier' AND `key` = 'basic' AND is_active = 1
    LIMIT 1
) rk
SET u.verification_tier_id = rk.id
WHERE u.verification_tier_id IS NULL;

ALTER TABLE users
    MODIFY COLUMN verification_tier_id BIGINT UNSIGNED NOT NULL,
    ADD KEY idx_users_verification_tier (verification_tier_id),
    ADD CONSTRAINT fk_users_verification_tier FOREIGN KEY (verification_tier_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;
