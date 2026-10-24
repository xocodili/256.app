-- Platform bootstrap: system user, locations (Katoomba AU + global), entities, moderator edges.
-- @256 — digital platform presence (Katoomba). @examplestudio — sample for-profit entity (Katoomba).
-- @256app — 256.app community (global). @256 moderates @examplestudio and @256app via `social_entity_relationship_type.moderator`.
-- Location fingerprints: SHA3-256 over v1 `locations` payload (see `crypto::sha3::tests::seed_location_fingerprints_match_migrations`).
-- Runs after `20260411000001_seed_reference_data.sql`. Greenfield-oriented (unique email, handles, fingerprints).

SET @seed_ts = TIMESTAMP('2026-04-15 00:00:00');
SET @seed_date = DATE('2026-04-15');

INSERT INTO users (email, password_hash, email_verified_at, status_id, created_by, modified_by)
VALUES (
    'system@256.app',
    NULL,
    @seed_ts,
    (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1),
    NULL,
    NULL
);

SET @seed_uid = LAST_INSERT_ID();

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
    validated_at,
    canonical_fingerprint,
    created_by,
    modified_by
) VALUES (
    'AU',
    'NSW',
    'Katoomba',
    NULL,
    NULL,
    NULL,
    'Katoomba, NSW, Australia',
    'seed',
    NULL,
    @seed_ts,
    UNHEX('75c5f7cbf3453f0a58562764c9c4156f01d62e703a211090f16d422fff774a68'),
    @seed_uid,
    @seed_uid
);

SET @katoomba_lid = LAST_INSERT_ID();

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
    validated_at,
    canonical_fingerprint,
    created_by,
    modified_by
) VALUES (
    'ZZ',
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    'Global',
    'seed',
    NULL,
    @seed_ts,
    UNHEX('d668ba83cc35a3d46e3cf2544d70b4d5194ae479cd434d5556fa26635b0b50cf'),
    @seed_uid,
    @seed_uid
);

SET @global_lid = LAST_INSERT_ID();

-- @256 — official digital presence
INSERT INTO social_entity (
    handle,
    type_id,
    status_id,
    location_id,
    created_by,
    modified_by
) VALUES (
    '256',
    (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'digital' LIMIT 1),
    (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1),
    @katoomba_lid,
    @seed_uid,
    @seed_uid
);

SET @seed_eid = LAST_INSERT_ID();

INSERT INTO social_digital (
    social_entity_id,
    digital_type_id,
    join_policy_id,
    label,
    description,
    intention,
    operator_disclosure,
    activated_at
) VALUES (
    @seed_eid,
    (SELECT id FROM ref_key_values WHERE `group` = 'social_digital_type' AND `key` = 'service_account' LIMIT 1),
    (SELECT id FROM ref_key_values WHERE `group` = 'social_join_policy' AND `key` = 'open' LIMIT 1),
    '256.app',
    'Official 256.app presence on this network.',
    'Product updates, announcements, and support entry points.',
    'Operated by 256.app.',
    @seed_date
);

-- @examplestudio — sample for-profit entity
INSERT INTO social_entity (
    handle,
    type_id,
    status_id,
    location_id,
    created_by,
    modified_by
) VALUES (
    'examplestudio',
    (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'profit' LIMIT 1),
    (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1),
    @katoomba_lid,
    @seed_uid,
    @seed_uid
);

SET @profit_eid = LAST_INSERT_ID();

INSERT INTO social_profit (
    social_entity_id,
    join_policy_id,
    label,
    description,
    intention,
    legal_name,
    lei,
    incorporation_country,
    registration_subdivision,
    registration_number,
    founded_at
) VALUES (
    @profit_eid,
    (SELECT id FROM ref_key_values WHERE `group` = 'social_join_policy' AND `key` = 'open' LIMIT 1),
    'Example Studio',
    'Sample for-profit entity seeded for demos.',
    NULL,
    'Example Studio Pty Ltd',
    NULL,
    'AU',
    'NSW',
    NULL,
    NULL
);

-- @256app — 256.app community (global)
INSERT INTO social_entity (
    handle,
    type_id,
    status_id,
    location_id,
    created_by,
    modified_by
) VALUES (
    '256app',
    (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'community' LIMIT 1),
    (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1),
    @global_lid,
    @seed_uid,
    @seed_uid
);

SET @community_eid = LAST_INSERT_ID();

INSERT INTO social_community (
    social_entity_id,
    label,
    description,
    intention,
    rules,
    founded_at,
    join_policy_id
) VALUES (
    @community_eid,
    '256.app',
    'Community around the 256.app network — builders, hosts, and curious humans.',
    'Discuss features, share feedback, and coordinate experiments on an open social backbone.',
    NULL,
    NULL,
    (SELECT id FROM ref_key_values WHERE `group` = 'social_join_policy' AND `key` = 'open' LIMIT 1)
);

-- @256 moderates the profit entity and the community (graph edges; enforced in app later).
INSERT INTO social_entity_relationship (
    from_social_entity_id,
    to_social_entity_id,
    type_id,
    status_id,
    started_at,
    ended_at,
    created_by,
    modified_by
) VALUES (
    @seed_eid,
    @profit_eid,
    (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_relationship_type' AND `key` = 'moderator' LIMIT 1),
    (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1),
    NULL,
    NULL,
    @seed_uid,
    @seed_uid
);

INSERT INTO social_entity_relationship (
    from_social_entity_id,
    to_social_entity_id,
    type_id,
    status_id,
    started_at,
    ended_at,
    created_by,
    modified_by
) VALUES (
    @seed_eid,
    @community_eid,
    (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_relationship_type' AND `key` = 'moderator' LIMIT 1),
    (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1),
    NULL,
    NULL,
    @seed_uid,
    @seed_uid
);
