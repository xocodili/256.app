-- Minimal moderation workflow:
-- 1) Add `status.under_moderation` for post lifecycle.
-- 2) Add first-class moderation case records + selected clauses.
-- 3) Seed 256 realistic human test users with handles/entities.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('status', 'under_moderation', 70, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE moderation_case (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    post_id BIGINT UNSIGNED NOT NULL,
    filed_by_user_id BIGINT UNSIGNED NOT NULL,
    filing_rationale_html TEXT NULL,
    status_key VARCHAR(64) NOT NULL DEFAULT 'jury_selection_pending',
    resolution_tier_key VARCHAR(32) NULL COMMENT 'jury | ai_council',
    jury_votes_for TINYINT UNSIGNED NOT NULL DEFAULT 0,
    jury_votes_against TINYINT UNSIGNED NOT NULL DEFAULT 0,
    ai_votes_for TINYINT UNSIGNED NOT NULL DEFAULT 0,
    ai_votes_against TINYINT UNSIGNED NOT NULL DEFAULT 0,
    opened_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP NULL DEFAULT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_moderation_case_post (post_id),
    KEY idx_moderation_case_filed_by (filed_by_user_id),
    KEY idx_moderation_case_status (status_key),
    KEY idx_moderation_case_opened (opened_at),
    CONSTRAINT fk_moderation_case_post FOREIGN KEY (post_id) REFERENCES post (id) ON DELETE CASCADE,
    CONSTRAINT fk_moderation_case_filed_by FOREIGN KEY (filed_by_user_id) REFERENCES users (id) ON DELETE RESTRICT,
    CONSTRAINT fk_moderation_case_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_moderation_case_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SET @seed_location_id := (SELECT l.id FROM locations l ORDER BY l.id ASC LIMIT 1);
SET @active_location_lifecycle_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'location_lifecycle'
      AND rv.`key` = 'active'
    LIMIT 1
);
SET @verified_status_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'status'
      AND rv.`key` = 'verified'
    LIMIT 1
);
SET @verified_user_status_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'user_status'
      AND rv.`key` = 'verified'
    LIMIT 1
);
SET @verified_entity_status_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'social_entity_status'
      AND rv.`key` = 'verified'
    LIMIT 1
);
SET @human_user_type_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'user_type'
      AND rv.`key` = 'human'
    LIMIT 1
);
SET @basic_tier_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'verification_tier'
      AND rv.`key` = 'basic'
    LIMIT 1
);
SET @human_entity_type_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'social_entity_type'
      AND rv.`key` = 'human'
    LIMIT 1
);
SET @open_join_policy_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'social_join_policy'
      AND rv.`key` = 'open'
    LIMIT 1
);
SET @owner_role_id := (
    SELECT rv.id
    FROM ref_key_values rv
    WHERE rv.`group` = 'user_social_entity_role'
      AND rv.`key` = 'owner'
    LIMIT 1
);

INSERT INTO locations (
    country_code,
    region,
    locality,
    postal_code,
    latitude,
    longitude,
    display_label,
    validation_source,
    validation_ref,
    canonical_fingerprint,
    location_lifecycle_id,
    lifecycle_changed_at,
    lifecycle_note,
    superseded_by_location_id,
    created_by,
    modified_by
)
SELECT
    'US', NULL, 'Seed City', NULL, NULL, NULL,
    'Seed City, United States',
    'seed',
    'seed-256-users',
    UNHEX(SHA2('seed-256-users-location', 256)),
    @active_location_lifecycle_id,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL
FROM DUAL
WHERE @seed_location_id IS NULL;

SET @seed_location_id := (SELECT l.id FROM locations l ORDER BY l.id ASC LIMIT 1);

CREATE TEMPORARY TABLE tmp_seed_first_name (
    id INT UNSIGNED NOT NULL AUTO_INCREMENT,
    given_name VARCHAR(32) NOT NULL,
    PRIMARY KEY (id)
) ENGINE=InnoDB;

CREATE TEMPORARY TABLE tmp_seed_family_name (
    id INT UNSIGNED NOT NULL AUTO_INCREMENT,
    family_name VARCHAR(32) NOT NULL,
    PRIMARY KEY (id)
) ENGINE=InnoDB;

INSERT INTO tmp_seed_first_name (given_name) VALUES
('Ava'), ('Liam'), ('Mila'), ('Noah'),
('Zoe'), ('Ethan'), ('Nora'), ('Leo'),
('Ruby'), ('Mason'), ('Ivy'), ('Felix'),
('Luna'), ('Owen'), ('Aria'), ('Kai');

INSERT INTO tmp_seed_family_name (family_name) VALUES
('Parker'), ('Nguyen'), ('Patel'), ('Rivera'),
('Brooks'), ('Kim'), ('Ali'), ('Morris'),
('Santos'), ('Reed'), ('Bennett'), ('Khan'),
('Shaw'), ('Wright'), ('Coleman'), ('Dawson');

CREATE TEMPORARY TABLE tmp_seed_people (
    seq INT UNSIGNED NOT NULL,
    given_name VARCHAR(32) NOT NULL,
    family_name VARCHAR(32) NOT NULL,
    handle VARCHAR(64) NOT NULL,
    email VARCHAR(128) NOT NULL,
    birth_date DATE NOT NULL,
    PRIMARY KEY (seq),
    UNIQUE KEY uq_tmp_seed_people_handle (handle),
    UNIQUE KEY uq_tmp_seed_people_email (email)
) ENGINE=InnoDB;

SET @seed_seq := 0;
INSERT INTO tmp_seed_people (seq, given_name, family_name, handle, email, birth_date)
SELECT
    combo.seq,
    combo.given_name,
    combo.family_name,
    CONCAT('seed_', LPAD(combo.seq, 3, '0'), '_', LOWER(combo.given_name), '_', LOWER(combo.family_name)) AS handle,
    CONCAT('seed.', LPAD(combo.seq, 3, '0'), '.', LOWER(combo.given_name), '.', LOWER(combo.family_name), '@256.app') AS email,
    DATE_ADD('1980-01-01', INTERVAL combo.seq DAY) AS birth_date
FROM (
    SELECT
        (@seed_seq := @seed_seq + 1) AS seq,
        f.given_name,
        l.family_name
    FROM tmp_seed_first_name f
    CROSS JOIN tmp_seed_family_name l
    ORDER BY f.id ASC, l.id ASC
) AS combo
LIMIT 256;

INSERT INTO users (
    email,
    password_hash,
    email_verified_at,
    status_id,
    user_status_id,
    user_type_id,
    verification_tier_id,
    created_by,
    modified_by
)
SELECT
    tsp.email,
    NULL,
    UTC_TIMESTAMP(),
    @verified_status_id,
    @verified_user_status_id,
    @human_user_type_id,
    @basic_tier_id,
    NULL,
    NULL
FROM tmp_seed_people tsp
LEFT JOIN users u ON u.email = tsp.email
WHERE u.id IS NULL;

INSERT INTO social_entity (
    handle,
    type_id,
    status_id,
    entity_status_id,
    location_id,
    rating,
    created_by,
    modified_by
)
SELECT
    tsp.handle,
    @human_entity_type_id,
    @verified_status_id,
    @verified_entity_status_id,
    @seed_location_id,
    2.50,
    u.id,
    u.id
FROM tmp_seed_people tsp
INNER JOIN users u ON u.email = tsp.email
LEFT JOIN social_entity se ON se.created_by = u.id AND se.type_id = @human_entity_type_id
WHERE se.id IS NULL;

INSERT INTO social_human (
    social_entity_id,
    family_name,
    given_name,
    additional_name,
    birth_date,
    birth_location_id,
    join_policy_id
)
SELECT
    se.id,
    tsp.family_name,
    tsp.given_name,
    NULL,
    tsp.birth_date,
    @seed_location_id,
    @open_join_policy_id
FROM tmp_seed_people tsp
INNER JOIN users u ON u.email = tsp.email
INNER JOIN social_entity se ON se.created_by = u.id AND se.type_id = @human_entity_type_id
LEFT JOIN social_human sh ON sh.social_entity_id = se.id
WHERE sh.social_entity_id IS NULL;

INSERT INTO user_social_entity_role (
    user_id,
    social_entity_id,
    role_id,
    is_primary,
    starts_at,
    ends_at,
    created_by,
    modified_by
)
SELECT
    u.id,
    se.id,
    @owner_role_id,
    1,
    CURRENT_DATE(),
    NULL,
    u.id,
    u.id
FROM tmp_seed_people tsp
INNER JOIN users u ON u.email = tsp.email
INNER JOIN social_entity se ON se.created_by = u.id AND se.type_id = @human_entity_type_id
LEFT JOIN user_social_entity_role ur
    ON ur.user_id = u.id
   AND ur.social_entity_id = se.id
   AND ur.role_id = @owner_role_id
WHERE ur.id IS NULL;

DROP TEMPORARY TABLE IF EXISTS tmp_seed_people;
DROP TEMPORARY TABLE IF EXISTS tmp_seed_first_name;
DROP TEMPORARY TABLE IF EXISTS tmp_seed_family_name;
