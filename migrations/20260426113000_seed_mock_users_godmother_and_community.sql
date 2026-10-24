-- Backfill seeded mock users with onboarding-like social connections:
-- - random AI godmother relationship
-- - initial community membership (aligned to chosen godmother)
--
-- This migration is idempotent and only targets seeded users (`seed.%@256.app`).

SET @human_type_id := (
    SELECT id
    FROM ref_key_values
    WHERE `group` = 'social_entity_type' AND `key` = 'human'
    LIMIT 1
);
SET @rel_type_moderator := (
    SELECT id
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'moderator'
    LIMIT 1
);
SET @rel_type_godmother := (
    SELECT id
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'godmother'
    LIMIT 1
);
SET @rel_type_member_of := (
    SELECT id
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'member_of'
    LIMIT 1
);
SET @status_verified := (
    SELECT id
    FROM ref_key_values
    WHERE `group` = 'status' AND `key` = 'verified'
    LIMIT 1
);
SET @relationship_status_verified := (
    SELECT scoped.id
    FROM ref_key_values old_status
    INNER JOIN ref_key_values scoped
      ON scoped.`group` = 'social_relationship_status'
     AND scoped.`key` = old_status.`key`
    WHERE old_status.id = @status_verified
    LIMIT 1
);

CREATE TEMPORARY TABLE tmp_seed_mock_humans (
    user_id BIGINT UNSIGNED NOT NULL,
    human_entity_id BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (user_id),
    UNIQUE KEY uq_tmp_seed_human_entity (human_entity_id)
) ENGINE=InnoDB;

INSERT INTO tmp_seed_mock_humans (user_id, human_entity_id)
SELECT
    u.id,
    se.id
FROM users u
INNER JOIN social_entity se
    ON se.created_by = u.id
   AND se.type_id = @human_type_id
INNER JOIN social_human sh
    ON sh.social_entity_id = se.id
WHERE u.email LIKE 'seed.%@256.app';

CREATE TEMPORARY TABLE tmp_seed_godmother_pairs (
    rn INT UNSIGNED NOT NULL,
    godmother_entity_id BIGINT UNSIGNED NOT NULL,
    community_entity_id BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (rn),
    UNIQUE KEY uq_tmp_seed_godmother_pair (godmother_entity_id, community_entity_id)
) ENGINE=InnoDB;

SET @pair_rn := 0;
INSERT INTO tmp_seed_godmother_pairs (rn, godmother_entity_id, community_entity_id)
SELECT
    (@pair_rn := @pair_rn + 1) AS rn,
    p.godmother_entity_id,
    p.community_entity_id
FROM (
    SELECT
        ai_se.id AS godmother_entity_id,
        cse.id AS community_entity_id
    FROM ai_actor aa
    INNER JOIN social_entity ai_se ON ai_se.id = aa.social_entity_id
    INNER JOIN social_entity_relationship modr
        ON modr.from_social_entity_id = ai_se.id
       AND modr.ended_at IS NULL
       AND modr.type_id = @rel_type_moderator
    INNER JOIN social_entity cse ON cse.id = modr.to_social_entity_id
    ORDER BY ai_se.id ASC, cse.id ASC
) AS p;

SET @pair_count := (SELECT COUNT(*) FROM tmp_seed_godmother_pairs);

-- Seed godmother edge if missing for a mock user.
INSERT INTO social_entity_relationship (
    from_social_entity_id,
    to_social_entity_id,
    type_id,
    status_id,
    relationship_status_id,
    started_at,
    ended_at,
    created_by,
    modified_by
)
SELECT
    h.human_entity_id,
    gp.godmother_entity_id,
    @rel_type_godmother,
    @status_verified,
    @relationship_status_verified,
    UTC_DATE(),
    NULL,
    h.user_id,
    h.user_id
FROM tmp_seed_mock_humans h
INNER JOIN tmp_seed_godmother_pairs gp
    ON gp.rn = ((h.user_id - 1) % @pair_count) + 1
LEFT JOIN social_entity_relationship ser_existing
    ON ser_existing.from_social_entity_id = h.human_entity_id
   AND ser_existing.type_id = @rel_type_godmother
   AND ser_existing.ended_at IS NULL
WHERE @pair_count > 0
  AND ser_existing.id IS NULL;

-- Seed initial member_of edge aligned to selected godmother's community, if missing.
INSERT INTO social_entity_relationship (
    from_social_entity_id,
    to_social_entity_id,
    type_id,
    status_id,
    relationship_status_id,
    rating,
    started_at,
    ended_at,
    created_by,
    modified_by
)
SELECT
    h.human_entity_id,
    gp.community_entity_id,
    @rel_type_member_of,
    @status_verified,
    @relationship_status_verified,
    2.50,
    UTC_DATE(),
    NULL,
    h.user_id,
    h.user_id
FROM tmp_seed_mock_humans h
INNER JOIN tmp_seed_godmother_pairs gp
    ON gp.rn = ((h.user_id - 1) % @pair_count) + 1
LEFT JOIN social_entity_relationship ser_existing
    ON ser_existing.from_social_entity_id = h.human_entity_id
   AND ser_existing.type_id = @rel_type_member_of
   AND ser_existing.ended_at IS NULL
WHERE @pair_count > 0
  AND ser_existing.id IS NULL;

DROP TEMPORARY TABLE IF EXISTS tmp_seed_godmother_pairs;
DROP TEMPORARY TABLE IF EXISTS tmp_seed_mock_humans;
