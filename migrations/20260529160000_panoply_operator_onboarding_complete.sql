-- operator@example.com (@sphinx): fully verified structural operator — no onboarding funnel.

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @verified_status := (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1);
SET @verified_user_status := (SELECT id FROM ref_key_values WHERE `group` = 'user_status' AND `key` = 'verified' LIMIT 1);
SET @member_of_type := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'member_of'
    LIMIT 1
);
SET @godmother_type := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'godmother'
    LIMIT 1
);
SET @moderator_type := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'moderator'
    LIMIT 1
);

SET @sphinx_uid := (SELECT id FROM users WHERE email = 'operator@example.com' LIMIT 1);

UPDATE users
SET terms_consent_version = 'v2-2026-05-29',
    status_id = @verified_status,
    user_status_id = @verified_user_status,
    email_verified_at = COALESCE(email_verified_at, UTC_TIMESTAMP(6)),
    modified_by = @system_uid
WHERE id = @sphinx_uid;

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

SET @sphinx_human_eid := COALESCE(
    @sphinx_human_eid,
    (SELECT id FROM social_entity WHERE handle = 'sphinx' LIMIT 1)
);

UPDATE social_entity
SET created_by = @sphinx_uid,
    modified_by = @system_uid
WHERE id = @sphinx_human_eid
  AND @sphinx_uid IS NOT NULL
  AND created_by <> @sphinx_uid;

INSERT INTO user_faces (
    user_id, embedding, embedding_dim, embedding_model_id,
    ai_appearance_description, generated_artwork_path, ai_model_id,
    captured_at, created_by, modified_by
)
SELECT
    @sphinx_uid,
    X'00000000',
    1,
    'panoply-operator-seed-v1',
    'Panoply operator structural profile.',
    NULL,
    'panoply-operator-seed-v1',
    UTC_TIMESTAMP(),
    @sphinx_uid,
    @sphinx_uid
FROM DUAL
WHERE @sphinx_uid IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM user_faces WHERE user_id = @sphinx_uid);

SET @milo_eid := (SELECT id FROM social_entity WHERE handle = 'milo' LIMIT 1);
SET @milo_community_eid := (
    SELECT modr.to_social_entity_id
    FROM social_entity_relationship modr
    WHERE modr.from_social_entity_id = @milo_eid
      AND modr.type_id = @moderator_type
      AND modr.ended_at IS NULL
    ORDER BY modr.id ASC
    LIMIT 1
);

INSERT INTO social_entity_relationship (
    from_social_entity_id, to_social_entity_id, type_id, status_id, created_by, modified_by
)
SELECT @sphinx_human_eid, @milo_eid, @godmother_type, @verified_status, @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_human_eid IS NOT NULL
  AND @milo_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM social_entity_relationship ser
      WHERE ser.from_social_entity_id = @sphinx_human_eid
        AND ser.to_social_entity_id = @milo_eid
        AND ser.type_id = @godmother_type
        AND ser.ended_at IS NULL
  );

INSERT INTO social_entity_relationship (
    from_social_entity_id, to_social_entity_id, type_id, status_id, created_by, modified_by
)
SELECT @sphinx_human_eid, @milo_community_eid, @member_of_type, @verified_status, @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_human_eid IS NOT NULL
  AND @milo_community_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM social_entity_relationship ser
      WHERE ser.from_social_entity_id = @sphinx_human_eid
        AND ser.to_social_entity_id = @milo_community_eid
        AND ser.type_id = @member_of_type
        AND ser.ended_at IS NULL
  );
