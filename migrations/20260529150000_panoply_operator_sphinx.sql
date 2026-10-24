-- Panoply structural operator: @sphinx (operator@example.com), founder and sole initial member.

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @sphinx_uid := (SELECT id FROM users WHERE email = 'operator@example.com' LIMIT 1);
SET @member_of_type := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'member_of'
    LIMIT 1
);
SET @owner_role := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'user_social_entity_role' AND `key` = 'owner'
    LIMIT 1
);
SET @verified_status := (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1);
SET @panoply_eid := (SELECT id FROM social_entity WHERE handle = 'panoply' LIMIT 1);

SET @sphinx_human_eid := (
    SELECT ur.social_entity_id
    FROM user_social_entity_role ur
    INNER JOIN social_entity se ON se.id = ur.social_entity_id
    INNER JOIN ref_key_values rkt ON rkt.id = se.type_id
        AND rkt.`group` = 'social_entity_type'
        AND rkt.`key` = 'human'
    WHERE ur.user_id = @sphinx_uid
      AND ur.ends_at IS NULL
    ORDER BY ur.is_primary DESC, ur.id ASC
    LIMIT 1
);

-- Early seed used @operator — rename the structural operator human when unambiguous.
UPDATE social_entity se
SET se.handle = 'sphinx',
    se.modified_by = @system_uid
WHERE se.id = @sphinx_human_eid
  AND se.handle = 'operator'
  AND NOT EXISTS (SELECT 1 FROM (SELECT id FROM social_entity WHERE handle = 'sphinx') AS taken);

SET @sphinx_human_eid := COALESCE(
    @sphinx_human_eid,
    (SELECT id FROM social_entity WHERE handle = 'sphinx' LIMIT 1)
);

UPDATE social_human sh
SET sh.given_name = COALESCE(sh.given_name, 'Founder'),
    sh.family_name = NULL
WHERE sh.social_entity_id = @sphinx_human_eid
  AND sh.family_name = 'Panoply';

UPDATE social_entity se
SET se.created_by = @sphinx_uid,
    se.modified_by = @system_uid
WHERE se.id = @panoply_eid
  AND @sphinx_uid IS NOT NULL;

UPDATE social_entity_relationship ser
INNER JOIN ref_key_values rkt ON rkt.id = ser.type_id
    AND rkt.`group` = 'social_entity_relationship_type'
    AND rkt.`key` = 'member_of'
SET ser.ended_at = UTC_DATE(),
    ser.modified_by = @system_uid
WHERE ser.to_social_entity_id = @panoply_eid
  AND ser.from_social_entity_id <> COALESCE(@sphinx_human_eid, 0)
  AND ser.ended_at IS NULL;

UPDATE user_social_entity_role ur
SET ur.ends_at = UTC_DATE(),
    ur.modified_by = @system_uid
WHERE ur.social_entity_id = @panoply_eid
  AND ur.user_id <> COALESCE(@sphinx_uid, 0)
  AND ur.ends_at IS NULL;

INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by)
SELECT @sphinx_uid, @panoply_eid, @owner_role, 0, UTC_DATE(), @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_uid IS NOT NULL
  AND @panoply_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM user_social_entity_role
      WHERE user_id = @sphinx_uid
        AND social_entity_id = @panoply_eid
        AND role_id = @owner_role
        AND ends_at IS NULL
  );

INSERT INTO social_entity_relationship (
    from_social_entity_id, to_social_entity_id, type_id, status_id, created_by, modified_by
)
SELECT @sphinx_human_eid, @panoply_eid, @member_of_type, @verified_status, @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_human_eid IS NOT NULL
  AND @panoply_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM social_entity_relationship ser
      WHERE ser.from_social_entity_id = @sphinx_human_eid
        AND ser.to_social_entity_id = @panoply_eid
        AND ser.type_id = @member_of_type
        AND ser.ended_at IS NULL
  );
